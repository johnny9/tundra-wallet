use crate::{
    descriptor::{canonical_descriptor, label_origin, normalize_label_origin, preview_import},
    labels::{self, LabelPreview, LabelRecord},
    *,
};
use bdk_wallet::{
    ChangeSet, KeychainKind, Wallet,
    bitcoin::{
        Address, Amount, FeeRate, OutPoint, Txid,
        hashes::{Hash, sha256},
    },
    chain::{ChainPosition, Merge},
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    str::FromStr,
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// One shared service per app. All wallet mutations are committed atomically with metadata.
/// No in-memory BDK instance survives a failed DB transaction.
#[derive(Clone)]
pub struct Core {
    pub(crate) db: Arc<Mutex<Connection>>,
    pub(crate) syncs: Arc<crate::sync::Operations>,
}
pub(crate) struct Loaded {
    pub(crate) wallet: Wallet,
    pub(crate) state: ChangeSet,
    pub(crate) summary: WalletSummary,
}
pub(crate) fn now() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|t| t.as_secs())
        .map_err(|_| Error::Storage)
}
pub(crate) fn json<T: serde::Serialize>(v: &T) -> Result<String> {
    serde_json::to_string(v).map_err(|_| Error::CorruptState)
}
pub(crate) fn from_json<T: serde::de::DeserializeOwned>(v: &str) -> Result<T> {
    serde_json::from_str(v).map_err(|_| Error::CorruptState)
}
fn text(s: &str, max: usize) -> Result<()> {
    if s.trim().is_empty() || s.chars().count() > max || s.chars().any(char::is_control) {
        Err(Error::InvalidInput("name"))
    } else {
        Ok(())
    }
}
fn outpoint(s: &str) -> Result<OutPoint> {
    OutPoint::from_str(s).map_err(|_| Error::InvalidInput("outpoint"))
}

impl Core {
    /// Development storage is SQLite in the app sandbox, NOT encrypted. Use test data only.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;",
        )?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > 2 {
            return Err(Error::CorruptState);
        }
        conn.execute_batch(include_str!("schema.sql"))?;
        Ok(Self {
            db: Arc::new(Mutex::new(conn)),
            syncs: Arc::new(crate::sync::Operations::default()),
        })
    }
    pub(crate) fn lock(&self) -> Result<MutexGuard<'_, Connection>> {
        self.db.lock().map_err(|_| Error::Poisoned)
    }
    pub fn preview_import(&self, payload: &str, network: Network) -> Result<ImportPreview> {
        preview_import(payload, network)
    }
    pub fn import_wallet(
        &self,
        name: &str,
        payload: &str,
        network: Network,
    ) -> Result<WalletSummary> {
        text(name, 80)?;
        let p = preview_import(payload, network)?;
        let mut db = self.lock()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM wallets WHERE id=?1)",
            [&p.id],
            |r| r.get(0),
        )?;
        if exists {
            return Err(Error::AlreadyExists);
        }
        // Pre-normalization development databases may have an order-dependent ID.
        // Keep that ID and its metadata intact, while refusing an equivalent import.
        if p.policy == Policy::TwoOfThree {
            let mut stmt = tx.prepare("SELECT id FROM wallets WHERE network=?1 AND policy=?2")?;
            let ids = stmt
                .query_map(params![network.key(), p.policy.key()], |r| {
                    r.get::<_, String>(0)
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            for id in ids {
                let existing = load(&tx, &id)?;
                if canonical_descriptor(existing.wallet.public_descriptor(KeychainKind::External))?
                    .to_string()
                    == p.receive_descriptor
                    && canonical_descriptor(
                        existing.wallet.public_descriptor(KeychainKind::Internal),
                    )?
                    .to_string()
                        == p.change_descriptor
                {
                    return Err(Error::AlreadyExists);
                }
            }
        }
        let mut wallet = Wallet::create(p.receive_descriptor, p.change_descriptor)
            .network(network.bitcoin())
            .create_wallet_no_persist()
            .map_err(|_| Error::Descriptor)?;
        let state = wallet.take_staged().ok_or(Error::CorruptState)?;
        tx.execute("INSERT INTO wallets(id,name,network,policy,state_json,created_at) VALUES(?1,?2,?3,?4,?5,?6)",
            params![p.id,name.trim(),network.key(),p.policy.key(),json(&state)?,now()?])?;
        tx.commit()?;
        Ok(WalletSummary {
            id: p.id,
            name: name.trim().into(),
            network,
            policy: p.policy,
            synced_at: None,
            total_sats: None,
            available_sats: None,
        })
    }
    pub fn wallets(&self) -> Result<Vec<WalletSummary>> {
        let db = self.lock()?;
        let mut stmt = db.prepare("SELECT id FROM wallets ORDER BY created_at,id")?;
        let ids = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.iter().map(|id| summary(&db, id)).collect()
    }
    pub fn receive_address(&self, id: &str) -> Result<ReceiveAddress> {
        let mut db = self.lock()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut loaded = load(&tx, id)?;
        let a = loaded.wallet.reveal_next_address(KeychainKind::External);
        let result = ReceiveAddress {
            address: a.address.to_string(),
            index: a.index,
            hardware_verified: false,
        };
        save(&tx, id, &mut loaded)?;
        tx.commit()?; // Never expose a new address before its index is durable.
        Ok(result)
    }
    pub fn coins(&self, id: &str) -> Result<Vec<Coin>> {
        let db = self.lock()?;
        let l = load(&db, id)?;
        coins(&db, id, &l.wallet)
    }
    pub fn activity(&self, id: &str) -> Result<Vec<Activity>> {
        let db = self.lock()?;
        let l = load(&db, id)?;
        l.wallet
            .transactions()
            .map(|t| {
                let txid = t.tx_node.tx.compute_txid();
                let (sent, received) = l.wallet.sent_and_received(t.tx_node.tx.as_ref());
                Ok(Activity {
                    txid: txid.to_string(),
                    label: get_label(&db, id, "tx", &txid.to_string())?,
                    delta_sats: received.to_sat() as i64 - sent.to_sat() as i64,
                    confirmed: matches!(t.chain_position, ChainPosition::Confirmed { .. }),
                })
            })
            .collect()
    }
    pub fn set_label(&self, id: &str, kind: &str, reference: &str, label: &str) -> Result<()> {
        labels::validate_label(label)?;
        let mut db = self.lock()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let l = load(&tx, id)?;
        if !known_reference(&l.wallet, kind, reference) {
            return Err(Error::NotFound);
        }
        put_label(&tx, id, kind, reference, label)?;
        tx.commit()?;
        Ok(())
    }
    pub fn set_frozen(&self, id: &str, reference: &str, frozen: bool) -> Result<()> {
        let op = outpoint(reference)?;
        let mut db = self.lock()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let l = load(&tx, id)?;
        if !l.wallet.list_unspent().any(|c| c.outpoint == op) {
            return Err(Error::NotFound);
        }
        if frozen {
            tx.execute(
                "INSERT OR IGNORE INTO freezes(wallet_id,outpoint) VALUES(?1,?2)",
                params![id, op.to_string()],
            )?;
        } else {
            tx.execute(
                "DELETE FROM freezes WHERE wallet_id=?1 AND outpoint=?2",
                params![id, op.to_string()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    /// All-or-nothing bulk metadata changes, scoped to current wallet-owned outputs.
    pub fn edit_coins(
        &self,
        id: &str,
        references: Vec<String>,
        label: Option<String>,
        frozen: Option<bool>,
    ) -> Result<()> {
        if references.is_empty() || references.len() > 200 || (label.is_none() && frozen.is_none())
        {
            return Err(Error::InvalidInput(
                "select 1–200 coins and a metadata change",
            ));
        }
        if let Some(ref label) = label {
            labels::validate_label(label)?;
        }
        let mut db = self.lock()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let loaded = load(&tx, id)?;
        let known: BTreeSet<_> = loaded
            .wallet
            .list_unspent()
            .map(|c| c.outpoint.to_string())
            .collect();
        let unique: BTreeSet<_> = references.iter().collect();
        if unique.len() != references.len() {
            return Err(Error::InvalidInput("duplicate outpoint"));
        }
        for reference in &references {
            if !known.contains(reference) {
                return Err(Error::UnavailableCoin);
            }
            if let Some(ref label) = label {
                put_label(&tx, id, "output", reference, label)?;
            }
            match frozen {
                Some(true) => {
                    tx.execute(
                        "INSERT OR IGNORE INTO freezes(wallet_id,outpoint) VALUES(?1,?2)",
                        params![id, reference],
                    )?;
                }
                Some(false) => {
                    tx.execute(
                        "DELETE FROM freezes WHERE wallet_id=?1 AND outpoint=?2",
                        params![id, reference],
                    )?;
                }
                None => {}
            }
        }
        tx.commit()?;
        Ok(())
    }
    /// Preview and apply both rematch against this wallet. Unknown references/origins are not guessed.
    pub fn import_labels(&self, id: &str, payload: &str, apply: bool) -> Result<LabelPreview> {
        let (records, mut skipped) = labels::parse_labels(payload)?;
        let mut db = self.lock()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let l = load(&tx, id)?;
        let (mut matched, mut changed) = (0, 0);
        let origin = label_origin(l.wallet.public_descriptor(KeychainKind::External))?;
        let mut seen = BTreeSet::new();
        for r in records {
            if r.origin
                .as_ref()
                .is_some_and(|o| normalize_label_origin(o).as_deref() != Some(origin.as_str()))
                || !known_reference(&l.wallet, &r.kind, &r.reference)
            {
                skipped += 1;
                continue;
            }
            if !seen.insert((r.kind.clone(), r.reference.clone())) {
                return Err(Error::InvalidInput("duplicate matching label reference"));
            }
            matched += 1;
            let mut differs = false;
            if let Some(label) = r.label {
                differs |= get_label(&tx, id, &r.kind, &r.reference)? != label;
                if apply {
                    put_label(&tx, id, &r.kind, &r.reference, &label)?;
                }
            }
            if let Some(spendable) = r.spendable {
                let frozen: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM freezes WHERE wallet_id=?1 AND outpoint=?2)",
                    params![id, r.reference],
                    |r| r.get(0),
                )?;
                differs |= frozen == spendable;
                if apply {
                    if spendable {
                        tx.execute(
                            "DELETE FROM freezes WHERE wallet_id=?1 AND outpoint=?2",
                            params![id, r.reference],
                        )?;
                    } else {
                        tx.execute(
                            "INSERT OR IGNORE INTO freezes(wallet_id,outpoint) VALUES(?1,?2)",
                            params![id, r.reference],
                        )?;
                    }
                }
            }
            if differs {
                changed += 1;
            }
        }
        if apply {
            tx.commit()?;
        }
        Ok(LabelPreview {
            matched,
            skipped,
            changed,
        })
    }
    pub fn export_labels(&self, id: &str) -> Result<String> {
        let db = self.lock()?;
        let loaded = load(&db, id)?;
        let origin = label_origin(loaded.wallet.public_descriptor(KeychainKind::External))?;
        let mut records: BTreeMap<(String, String), LabelRecord> = BTreeMap::new();
        let mut stmt = db.prepare(
            "SELECT kind,reference,label FROM labels WHERE wallet_id=?1 ORDER BY kind,reference",
        )?;
        for row in stmt.query_map([id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })? {
            let (kind, reference, label) = row?;
            records.insert(
                (kind.clone(), reference.clone()),
                LabelRecord {
                    kind,
                    reference,
                    label: Some(label),
                    spendable: None,
                    origin: Some(origin.clone()),
                },
            );
        }
        let mut stmt =
            db.prepare("SELECT outpoint FROM freezes WHERE wallet_id=?1 ORDER BY outpoint")?;
        for row in stmt.query_map([id], |r| r.get::<_, String>(0))? {
            let reference = row?;
            records
                .entry(("output".into(), reference.clone()))
                .or_insert(LabelRecord {
                    kind: "output".into(),
                    reference,
                    label: None,
                    spendable: None,
                    origin: Some(origin.clone()),
                })
                .spendable = Some(false);
        }
        // This is a patch export. Absence of a freeze does not instruct another wallet to unfreeze.
        let lines = records.values().map(json).collect::<Result<Vec<_>>>()?;
        Ok(if lines.is_empty() {
            String::new()
        } else {
            lines.join("\n") + "\n"
        })
    }
    pub fn create_draft(&self, request: DraftRequest) -> Result<DraftReview> {
        labels::validate_label(&request.label)?;
        if !(1..=250_000).contains(&request.fee_sat_per_kwu) {
            return Err(Error::InvalidInput("fee rate must be 1–250000 sat/kwu"));
        }
        let mut db = self.lock()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut l = load(&tx, &request.wallet_id)?;
        if l.summary.network == Network::Mainnet {
            return Err(Error::Unavailable("mainnet spending"));
        }
        if l.summary.synced_at.is_none() {
            return Err(Error::Unavailable("blockchain synchronization"));
        }
        let cs = coins(&tx, &request.wallet_id, &l.wallet)?;
        let available: BTreeMap<_, _> = cs
            .iter()
            .filter(|c| c.status == CoinStatus::Available)
            .map(|c| (c.outpoint.clone(), c))
            .collect();
        let selected = match &request.selected_outpoints {
            Some(ids) => {
                if ids.is_empty() || ids.len() > 200 {
                    return Err(Error::InvalidInput("select 1–200 coins"));
                }
                let set: BTreeSet<_> = ids.iter().collect();
                if set.len() != ids.len() {
                    return Err(Error::InvalidInput("duplicate outpoint"));
                }
                let mut selected = Vec::new();
                for id in ids {
                    if !available.contains_key(id) {
                        return Err(Error::UnavailableCoin);
                    }
                    selected.push(outpoint(id)?);
                }
                Some(selected)
            }
            None => None,
        };
        let consolidate = matches!(&request.payment, Payment::Consolidate { .. });
        let target = match &request.payment {
            Payment::Consolidate {
                privacy_acknowledged,
            } => {
                if !privacy_acknowledged {
                    return Err(Error::InvalidInput(
                        "acknowledge that consolidation links these coins",
                    ));
                }
                if selected.as_ref().map_or(0, Vec::len) < 2 {
                    return Err(Error::InvalidInput(
                        "select at least two coins to consolidate",
                    ));
                }
                l.wallet.reveal_next_address(KeychainKind::Internal).address
            }
            Payment::Send { address, sats } => {
                if *sats == 0 || *sats > amount::MAX_SATS {
                    return Err(Error::InvalidInput("payment amount"));
                }
                Address::from_str(address)
                    .map_err(|_| Error::InvalidInput("recipient address"))?
                    .require_network(l.summary.network.bitcoin())
                    .map_err(|_| Error::NetworkMismatch)?
            }
            Payment::SendMax { address } => {
                if selected.is_none() {
                    return Err(Error::InvalidInput(
                        "maximum requires explicit coin selection",
                    ));
                }
                Address::from_str(address)
                    .map_err(|_| Error::InvalidInput("recipient address"))?
                    .require_network(l.summary.network.bitcoin())
                    .map_err(|_| Error::NetworkMismatch)?
            }
        };
        let target_script = target.script_pubkey();
        let excluded = cs
            .iter()
            .filter(|c| c.status != CoinStatus::Available)
            .map(|c| outpoint(&c.outpoint))
            .collect::<Result<Vec<_>>>()?;
        let fee_rate = FeeRate::from_sat_per_kwu(request.fee_sat_per_kwu);
        let mut builder = l.wallet.build_tx();
        builder
            .fee_rate(fee_rate)
            .unspendable(excluded)
            .add_global_xpubs();
        if let Some(ref inputs) = selected {
            builder
                .add_utxos(inputs)
                .map_err(|_| Error::UnavailableCoin)?
                .manually_selected_only();
        }
        match &request.payment {
            Payment::Send { sats, .. } => {
                builder.add_recipient(target_script.clone(), Amount::from_sat(*sats));
            }
            Payment::SendMax { .. } | Payment::Consolidate { .. } => {
                builder.drain_to(target_script.clone());
            }
        }
        let psbt = builder.finish().map_err(|_| Error::CannotBuild)?;
        let actual: BTreeSet<_> = psbt
            .unsigned_tx
            .input
            .iter()
            .map(|i| i.previous_output)
            .collect();
        if let Some(ref chosen) = selected
            && actual != chosen.iter().copied().collect()
        {
            return Err(Error::CorruptState);
        }
        let mut inputs = Vec::new();
        for input in &psbt.unsigned_tx.input {
            let op = input.previous_output.to_string();
            let coin = available.get(&op).ok_or(Error::UnavailableCoin)?;
            inputs.push(ReviewedInput {
                outpoint: op,
                sats: coin.sats,
                label: coin.label.clone(),
            });
        }
        let mut outputs = Vec::new();
        for output in &psbt.unsigned_tx.output {
            let is_mine = l.wallet.is_mine(output.script_pubkey.clone());
            outputs.push(ReviewedOutput {
                address: Address::from_script(&output.script_pubkey, l.summary.network.bitcoin())
                    .map_err(|_| Error::CorruptState)?
                    .to_string(),
                sats: output.value.to_sat(),
                is_change: !consolidate && output.script_pubkey != target_script && is_mine,
                is_mine,
            });
        }
        if consolidate && (outputs.len() != 1 || !outputs[0].is_mine) {
            return Err(Error::CorruptState);
        }
        let fee = l
            .wallet
            .calculate_fee(&psbt.unsigned_tx)
            .map_err(|_| Error::CorruptState)?
            .to_sat();
        if fee > 1_000_000 {
            return Err(Error::InvalidInput(
                "fee exceeds the development safety cap",
            ));
        }
        let id = sha256::Hash::hash(
            format!("{}:{}", request.wallet_id, psbt.unsigned_tx.compute_txid()).as_bytes(),
        )
        .to_string();
        let review = DraftReview {
            id: id.clone(),
            wallet_id: request.wallet_id.clone(),
            inputs,
            outputs,
            fee_sats: fee,
            fee_sat_per_kwu: Some(request.fee_sat_per_kwu),
            label: request.label.clone(),
            is_consolidation: consolidate,
            state: "unsigned".into(),
        };
        tx.execute("INSERT INTO drafts(id,wallet_id,psbt,review_json,label,created_at) VALUES(?1,?2,?3,?4,?5,?6)",
            params![id,request.wallet_id,psbt.to_string(),json(&review)?,request.label,now()?])?;
        for input in &review.inputs {
            tx.execute(
                "INSERT INTO reservations(wallet_id,outpoint,draft_id) VALUES(?1,?2,?3)",
                params![request.wallet_id, input.outpoint, id],
            )?;
        }
        save(&tx, &request.wallet_id, &mut l)?;
        tx.commit()?;
        Ok(review)
    }
    pub fn drafts(&self, id: &str) -> Result<Vec<DraftReview>> {
        let db = self.lock()?;
        load(&db, id)?;
        let mut stmt =
            db.prepare("SELECT review_json FROM drafts WHERE wallet_id=?1 ORDER BY created_at,id")?;
        let rows = stmt
            .query_map([id], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.iter().map(|r| from_json(r)).collect()
    }
    pub fn export_unsigned_psbt(&self, id: &str, draft_id: &str) -> Result<String> {
        let db = self.lock()?;
        let (psbt, review): (String, String) = db
            .query_row(
                "SELECT psbt,review_json FROM drafts WHERE wallet_id=?1 AND id=?2",
                params![id, draft_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .ok_or(Error::NotFound)?;
        if from_json::<DraftReview>(&review)?.state != "unsigned" {
            return Err(Error::UnavailableCoin);
        }
        Ok(psbt)
    }
    pub fn discard_draft(&self, id: &str, draft_id: &str) -> Result<()> {
        let mut db = self.lock()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let n = tx.execute(
            "DELETE FROM drafts WHERE wallet_id=?1 AND id=?2",
            params![id, draft_id],
        )?;
        if n != 1 {
            return Err(Error::NotFound);
        }
        tx.commit()?;
        Ok(())
    }
}

pub(crate) fn load(db: &Connection, id: &str) -> Result<Loaded> {
    let row = db
        .query_row(
            "SELECT name,network,policy,state_json,synced_at FROM wallets WHERE id=?1",
            [id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<u64>>(4)?,
                ))
            },
        )
        .optional()?
        .ok_or(Error::NotFound)?;
    let network = Network::parse(&row.1)?;
    let policy = match row.2.as_str() {
        "single_sig" => Policy::SingleSig,
        "two_of_three" => Policy::TwoOfThree,
        _ => return Err(Error::CorruptState),
    };
    let state: ChangeSet = from_json(&row.3)?;
    let wallet = Wallet::load()
        .check_network(network.bitcoin())
        .load_wallet_no_persist(state.clone())
        .map_err(|_| Error::CorruptState)?
        .ok_or(Error::CorruptState)?;
    Ok(Loaded {
        wallet,
        state,
        summary: WalletSummary {
            id: id.into(),
            name: row.0,
            network,
            policy,
            synced_at: row.4,
            total_sats: None,
            available_sats: None,
        },
    })
}
pub(crate) fn save(db: &Connection, id: &str, l: &mut Loaded) -> Result<()> {
    if let Some(changes) = l.wallet.take_staged() {
        l.state.merge(changes);
    }
    db.execute(
        "UPDATE wallets SET state_json=?1 WHERE id=?2",
        params![json(&l.state)?, id],
    )?;
    Ok(())
}
fn get_label(db: &Connection, id: &str, kind: &str, reference: &str) -> Result<String> {
    Ok(db
        .query_row(
            "SELECT label FROM labels WHERE wallet_id=?1 AND kind=?2 AND reference=?3",
            params![id, kind, reference],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or_default())
}
fn put_label(db: &Connection, id: &str, kind: &str, reference: &str, label: &str) -> Result<()> {
    db.execute("INSERT INTO labels(wallet_id,kind,reference,label) VALUES(?1,?2,?3,?4) ON CONFLICT(wallet_id,kind,reference) DO UPDATE SET label=excluded.label",params![id,kind,reference,label])?;
    Ok(())
}
fn known_reference(w: &Wallet, kind: &str, reference: &str) -> bool {
    match kind {
        "output" => outpoint(reference)
            .ok()
            .is_some_and(|op| w.list_output().any(|c| c.outpoint == op)),
        "tx" => Txid::from_str(reference)
            .ok()
            .is_some_and(|txid| w.get_tx(txid).is_some()),
        "addr" => Address::from_str(reference)
            .ok()
            .and_then(|a| a.require_network(w.network()).ok())
            .is_some_and(|a| w.is_mine(a.script_pubkey())),
        _ => false,
    }
}
fn coins(db: &Connection, id: &str, w: &Wallet) -> Result<Vec<Coin>> {
    let mut out = Vec::new();
    for c in w.list_unspent() {
        let op = c.outpoint.to_string();
        let frozen: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM freezes WHERE wallet_id=?1 AND outpoint=?2)",
            params![id, op],
            |r| r.get(0),
        )?;
        let reserved: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM reservations WHERE wallet_id=?1 AND outpoint=?2)",
            params![id, op],
            |r| r.get(0),
        )?;
        let confirmed = matches!(c.chain_position, ChainPosition::Confirmed { .. });
        let coinbase = w
            .get_tx(c.outpoint.txid)
            .is_some_and(|tx| tx.tx_node.tx.is_coinbase());
        let status = if frozen {
            CoinStatus::Frozen
        } else if reserved {
            CoinStatus::Reserved
        } else if !confirmed {
            CoinStatus::Pending
        } else if coinbase
            && !crate::sync::spendable(c.chain_position, w.local_chain().tip().height(), true)
        {
            CoinStatus::Immature
        } else {
            CoinStatus::Available
        };
        out.push(Coin {
            outpoint: op.clone(),
            sats: c.txout.value.to_sat(),
            label: get_label(db, id, "output", &op)?,
            address: Address::from_script(&c.txout.script_pubkey, w.network())
                .map_err(|_| Error::CorruptState)?
                .to_string(),
            status,
        });
    }
    out.sort_by(|a, b| a.outpoint.cmp(&b.outpoint));
    Ok(out)
}
fn summary(db: &Connection, id: &str) -> Result<WalletSummary> {
    let mut l = load(db, id)?;
    if l.summary.synced_at.is_some() {
        let cs = coins(db, id, &l.wallet)?;
        l.summary.total_sats = Some(cs.iter().map(|c| c.sats).sum());
        l.summary.available_sats = Some(
            cs.iter()
                .filter(|c| c.status == CoinStatus::Available)
                .map(|c| c.sats)
                .sum(),
        );
    }
    Ok(l.summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bdk_wallet::bitcoin::{
        self, Block, CompactTarget, ScriptBuf, Sequence, Transaction, TxIn, TxMerkleNode, TxOut,
        Witness, absolute, block, transaction,
    };
    const SINGLE: &str = include_str!("../../../tests/fixtures/single-sig.txt");
    fn setup() -> (Core, String) {
        let c = Core::open(":memory:").unwrap();
        let w = c.import_wallet("Savings", SINGLE, Network::Signet).unwrap();
        (c, w.id)
    }
    // Synthetic block data only. There is no production FFI method to inject chain state.
    fn funded() -> (Core, String, Vec<Coin>) {
        let (c, id) = setup();
        let coins = fund(&c, &id);
        (c, id, coins)
    }
    fn fund(c: &Core, id: &str) -> Vec<Coin> {
        {
            let mut db = c.lock().unwrap();
            let tx = db
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .unwrap();
            let mut l = load(&tx, id).unwrap();
            let address = l.wallet.reveal_next_address(KeychainKind::External).address;
            let funding = Transaction {
                version: transaction::Version::TWO,
                lock_time: absolute::LockTime::ZERO,
                input: vec![TxIn {
                    previous_output: OutPoint {
                        txid: Txid::all_zeros(),
                        vout: 0,
                    },
                    script_sig: ScriptBuf::new(),
                    sequence: Sequence::MAX,
                    witness: Witness::new(),
                }],
                output: vec![
                    TxOut {
                        value: Amount::from_sat(100_000),
                        script_pubkey: address.script_pubkey(),
                    },
                    TxOut {
                        value: Amount::from_sat(200_000),
                        script_pubkey: address.script_pubkey(),
                    },
                ],
            };
            let genesis =
                bitcoin::blockdata::constants::genesis_block(bitcoin::Network::Signet).block_hash();
            let mut block = Block {
                header: block::Header {
                    version: block::Version::ONE,
                    prev_blockhash: genesis,
                    merkle_root: TxMerkleNode::all_zeros(),
                    time: 1,
                    bits: CompactTarget::from_consensus(0x207fffff),
                    nonce: 0,
                },
                txdata: vec![funding],
            };
            block.header.merkle_root = block.compute_merkle_root().unwrap();
            l.wallet.apply_block(&block, 1).unwrap();
            save(&tx, id, &mut l).unwrap();
            tx.execute("UPDATE wallets SET synced_at=1 WHERE id=?1", [id])
                .unwrap();
            tx.commit().unwrap();
        }
        c.coins(id).unwrap()
    }
    fn req(id: &str, coins: Vec<String>, sats: u64) -> DraftRequest {
        // An external test-network address derived from the other descriptor's first account.
        let p = preview_import(
            include_str!("../../../tests/fixtures/two-of-three.txt"),
            Network::Signet,
        )
        .unwrap();
        DraftRequest {
            wallet_id: id.into(),
            payment: Payment::Send {
                address: p.first_address,
                sats,
            },
            selected_outpoints: Some(coins),
            fee_sat_per_kwu: 500,
            label: "Test payment".into(),
        }
    }
    #[test]
    fn import_is_not_a_sync() {
        let (c, id) = setup();
        assert!(c.wallets().unwrap()[0].total_sats.is_none());
        assert!(c.coins(&id).unwrap().is_empty());
    }
    #[test]
    fn bip329_origins_disambiguate_without_losing_matching_records() {
        let (c, id, coins) = funded();
        c.set_label(&id, "output", &coins[0].outpoint, "Original")
            .unwrap();
        let exported = c.export_labels(&id).unwrap();
        let record: serde_json::Value = serde_json::from_str(exported.trim()).unwrap();
        let origin = record["origin"].as_str().unwrap();
        assert!(!origin.contains("tpub"));
        let wrong = serde_json::json!({"type":"output", "ref":coins[0].outpoint, "label":"Other account", "origin":"wpkh([00000000/84h/1h/9h])"});
        let right = serde_json::json!({"type":"output", "ref":coins[0].outpoint, "label":"Matched", "origin":origin.replace('\'', "h")});
        let payload = format!("{wrong}\n{right}");
        let preview = c.import_labels(&id, &payload, false).unwrap();
        assert_eq!(
            (preview.matched, preview.skipped, preview.changed),
            (1, 1, 1)
        );
        c.import_labels(&id, &payload, true).unwrap();
        assert_eq!(c.coins(&id).unwrap()[0].label, "Matched");
        // The same reference with two equivalent origin spellings is still a duplicate.
        let duplicate = format!(
            "{right}\n{}",
            serde_json::json!({"type":"output", "ref":coins[0].outpoint, "label":"Ambiguous", "origin":origin})
        );
        assert!(c.import_labels(&id, &duplicate, true).is_err());
        assert_eq!(c.coins(&id).unwrap()[0].label, "Matched");
    }
    #[test]
    fn bulk_metadata_rolls_back_if_any_coin_is_unavailable() {
        let (c, id, coins) = funded();
        let all = coins.iter().map(|c| c.outpoint.clone()).collect::<Vec<_>>();
        c.edit_coins(&id, all.clone(), Some("Batch 🧊".into()), Some(true))
            .unwrap();
        assert!(
            c.coins(&id)
                .unwrap()
                .iter()
                .all(|coin| coin.label == "Batch 🧊" && coin.status == CoinStatus::Frozen)
        );
        assert!(
            c.edit_coins(
                &id,
                vec![all[0].clone(), "unknown:0".into()],
                Some("Must roll back".into()),
                Some(false)
            )
            .is_err()
        );
        assert!(
            c.coins(&id)
                .unwrap()
                .iter()
                .all(|coin| coin.label == "Batch 🧊" && coin.status == CoinStatus::Frozen)
        );
        assert!(
            c.edit_coins(
                &id,
                vec![all[0].clone(), all[0].clone()],
                Some("Duplicate".into()),
                None
            )
            .is_err()
        );
        c.edit_coins(&id, all, None, Some(false)).unwrap();
        assert!(
            c.coins(&id)
                .unwrap()
                .iter()
                .all(|coin| coin.label == "Batch 🧊" && coin.status == CoinStatus::Available)
        );
    }
    #[test]
    fn fractional_fee_rate_is_recorded_in_immutable_review() {
        let (c, id, coins) = funded();
        let mut request = req(&id, vec![coins[0].outpoint.clone()], 50_000);
        request.fee_sat_per_kwu = amount::parse_fee_rate("1.001").unwrap();
        let review = c.create_draft(request).unwrap();
        assert_eq!(review.fee_sat_per_kwu, Some(251));
        assert_eq!(c.drafts(&id).unwrap()[0].fee_sat_per_kwu, Some(251));
        assert_eq!(
            review.inputs.iter().map(|i| i.sats).sum::<u64>(),
            review.outputs.iter().map(|o| o.sats).sum::<u64>() + review.fee_sats
        );
    }
    #[test]
    fn observed_spend_invalidates_draft_and_releases_other_inputs_only() {
        let (c, id, coins) = funded();
        let draft = c
            .create_draft(req(
                &id,
                coins.iter().map(|c| c.outpoint.clone()).collect(),
                50_000,
            ))
            .unwrap();
        c.set_frozen(&id, &coins[1].outpoint, true).unwrap();
        {
            let mut db = c.lock().unwrap();
            let tx = db
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .unwrap();
            let mut l = load(&tx, &id).unwrap();
            // Graph fixture only: not a signed or broadcast transaction.
            let spend = Transaction {
                version: transaction::Version::TWO,
                lock_time: absolute::LockTime::ZERO,
                input: vec![TxIn {
                    previous_output: outpoint(&coins[0].outpoint).unwrap(),
                    script_sig: ScriptBuf::new(),
                    sequence: Sequence::MAX,
                    witness: Witness::new(),
                }],
                output: vec![TxOut {
                    value: Amount::from_sat(coins[0].sats - 1000),
                    script_pubkey: ScriptBuf::new(),
                }],
            };
            l.wallet.apply_unconfirmed_txs([(spend, now().unwrap())]);
            crate::sync::invalidate_drafts(&tx, &id, &l.wallet).unwrap();
            save(&tx, &id, &mut l).unwrap();
            tx.commit().unwrap();
        }
        assert_eq!(c.drafts(&id).unwrap()[0].state, "invalidated");
        assert!(c.export_unsigned_psbt(&id, &draft.id).is_err());
        assert!(
            !c.coins(&id)
                .unwrap()
                .iter()
                .any(|c| c.outpoint == coins[0].outpoint)
        );
        c.set_frozen(&id, &coins[1].outpoint, false).unwrap();
        assert_eq!(c.coins(&id).unwrap()[0].status, CoinStatus::Available);
    }
    #[test]
    fn version_one_migration_keeps_indices_and_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wallet.sqlite");
        let c = Core::open(&path).unwrap();
        let w = c
            .import_wallet("Migration", SINGLE, Network::Signet)
            .unwrap();
        let address = c.receive_address(&w.id).unwrap();
        c.set_label(&w.id, "addr", &address.address, "Keep 🧊")
            .unwrap();
        c.lock()
            .unwrap()
            .execute_batch("DROP TABLE sync_state; PRAGMA user_version=1;")
            .unwrap();
        drop(c);
        let c = Core::open(&path).unwrap();
        assert_eq!(c.receive_address(&w.id).unwrap().index, 1);
        assert!(c.export_labels(&w.id).unwrap().contains("Keep 🧊"));
        assert!(c.wallets().unwrap()[0].total_sats.is_none());
        assert!(c.sync_endpoint(&w.id).unwrap().is_none());
        assert_eq!(
            c.lock()
                .unwrap()
                .query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            2
        );
    }
    #[test]
    fn duplicate_wallet_rejected() {
        let (c, _) = setup();
        assert!(matches!(
            c.import_wallet("Again", SINGLE, Network::Signet),
            Err(Error::AlreadyExists)
        ));
    }
    #[test]
    fn equivalent_multisig_import_keeps_legacy_id_and_metadata() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("wallet.sqlite");
        let c = Core::open(&path).unwrap();
        let pair: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/core-style-pair.json"))
                .unwrap();
        let receive = pair["descriptors"][0]["desc"].as_str().unwrap();
        let change = pair["descriptors"][1]["desc"].as_str().unwrap();
        let legacy_id = "pre-normalization-wallet-id";
        let mut wallet = Wallet::create(receive.to_owned(), change.to_owned())
            .network(Network::Signet.bitcoin())
            .create_wallet_no_persist()
            .unwrap();
        c.lock().unwrap().execute(
            "INSERT INTO wallets(id,name,network,policy,state_json,created_at) VALUES(?1,'Legacy','signet','two_of_three',?2,1)",
            params![legacy_id, json(&wallet.take_staged().unwrap()).unwrap()],
        ).unwrap();
        let address = c.receive_address(legacy_id).unwrap();
        c.set_label(legacy_id, "addr", &address.address, "Épargne 🧊")
            .unwrap();
        drop(c);
        let c = Core::open(&path).unwrap();
        assert!(matches!(
            c.import_wallet(
                "Duplicate",
                include_str!("../../../tests/fixtures/two-of-three.txt"),
                Network::Signet
            ),
            Err(Error::AlreadyExists)
        ));
        let wallets = c.wallets().unwrap();
        assert_eq!(wallets.len(), 1);
        assert_eq!(wallets[0].id, legacy_id);
        assert!(c.export_labels(legacy_id).unwrap().contains("Épargne 🧊"));
        assert_eq!(
            c.receive_address(legacy_id).unwrap().index,
            address.index + 1
        );
    }

    #[test]
    fn reopen_preserves_unknown_balance_address_index_and_unicode_labels() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("wallet.sqlite");
        let c = Core::open(&path).unwrap();
        let id = c
            .import_wallet("Épargne 🧊", SINGLE, Network::Signet)
            .unwrap()
            .id;
        let first = c.receive_address(&id).unwrap();
        c.set_label(&id, "addr", &first.address, "家族の貯蓄 🧊")
            .unwrap();
        let labels = c.export_labels(&id).unwrap();
        drop(c);
        let reopened = Core::open(&path).unwrap();
        let wallet = reopened.wallets().unwrap().remove(0);
        assert_eq!(wallet.name, "Épargne 🧊");
        assert!(wallet.synced_at.is_none());
        assert!(wallet.total_sats.is_none());
        assert!(wallet.available_sats.is_none());
        assert_eq!(reopened.export_labels(&id).unwrap(), labels);
        let next = reopened.receive_address(&id).unwrap();
        assert_eq!(next.index, first.index + 1);
        assert_ne!(next.address, first.address);
    }

    #[test]
    fn reopened_draft_reserves_inputs_and_discard_keeps_user_freezes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("wallet.sqlite");
        let c = Core::open(&path).unwrap();
        let id = c
            .import_wallet("Savings", SINGLE, Network::Signet)
            .unwrap()
            .id;
        let coins = fund(&c, &id);
        c.set_label(&id, "output", &coins[0].outpoint, "Épargne 🧊")
            .unwrap();
        let draft = c
            .create_draft(req(&id, vec![coins[0].outpoint.clone()], 10_000))
            .unwrap();
        let psbt = c.export_unsigned_psbt(&id, &draft.id).unwrap();
        c.set_frozen(&id, &coins[0].outpoint, true).unwrap();
        drop(c);
        let reopened = Core::open(&path).unwrap();
        assert_eq!(
            json(&reopened.drafts(&id).unwrap()[0]).unwrap(),
            json(&draft).unwrap()
        );
        assert_eq!(reopened.export_unsigned_psbt(&id, &draft.id).unwrap(), psbt);
        reopened.set_frozen(&id, &coins[0].outpoint, false).unwrap();
        assert!(matches!(
            reopened.create_draft(req(&id, vec![coins[0].outpoint.clone()], 20_000)),
            Err(Error::UnavailableCoin)
        ));
        reopened.set_frozen(&id, &coins[0].outpoint, true).unwrap();
        reopened.discard_draft(&id, &draft.id).unwrap();
        drop(reopened);
        let reopened = Core::open(&path).unwrap();
        assert!(reopened.drafts(&id).unwrap().is_empty());
        let coin = reopened
            .coins(&id)
            .unwrap()
            .into_iter()
            .find(|c| c.outpoint == coins[0].outpoint)
            .unwrap();
        assert_eq!(coin.status, CoinStatus::Frozen);
        assert_eq!(coin.label, "Épargne 🧊");
        reopened.set_frozen(&id, &coins[0].outpoint, false).unwrap();
        assert!(
            reopened
                .create_draft(req(&id, vec![coins[0].outpoint.clone()], 20_000))
                .is_ok()
        );
    }

    #[test]
    fn failed_snapshot_save_rolls_back_draft_reservations_and_change_index() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("wallet.sqlite");
        let c = Core::open(&path).unwrap();
        let id = c
            .import_wallet("Savings", SINGLE, Network::Signet)
            .unwrap()
            .id;
        let coins = fund(&c, &id);
        let before = json(&load(&c.lock().unwrap(), &id).unwrap().state).unwrap();
        c.lock()
            .unwrap()
            .execute_batch(
                "CREATE TEMP TRIGGER fail_save BEFORE UPDATE OF state_json ON wallets
             BEGIN SELECT RAISE(ABORT, 'injected failure'); END;",
            )
            .unwrap();
        assert!(matches!(
            c.create_draft(req(&id, vec![coins[0].outpoint.clone()], 10_000)),
            Err(Error::Storage)
        ));
        assert!(matches!(c.receive_address(&id), Err(Error::Storage)));
        drop(c); // Reopen discards the temporary fault injector as well as the failed operation.
        let reopened = Core::open(&path).unwrap();
        assert_eq!(
            json(&load(&reopened.lock().unwrap(), &id).unwrap().state).unwrap(),
            before
        );
        assert!(reopened.drafts(&id).unwrap().is_empty());
        assert!(
            reopened
                .coins(&id)
                .unwrap()
                .iter()
                .all(|c| c.status == CoinStatus::Available)
        );
        assert!(
            reopened
                .create_draft(req(&id, vec![coins[0].outpoint.clone()], 10_000))
                .is_ok()
        );
    }

    #[test]
    fn separate_connections_cannot_reserve_the_same_coin_concurrently() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("wallet.sqlite");
        let c = Core::open(&path).unwrap();
        let id = c
            .import_wallet("Savings", SINGLE, Network::Signet)
            .unwrap()
            .id;
        let coins = fund(&c, &id);
        let first = Core::open(&path).unwrap();
        let second = Core::open(&path).unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let requests = [
            req(&id, vec![coins[0].outpoint.clone()], 10_000),
            req(&id, vec![coins[0].outpoint.clone()], 20_000),
        ];
        let handles = [first, second]
            .into_iter()
            .zip(requests)
            .map(|(core, request)| {
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    core.create_draft(request)
                })
            })
            .collect::<Vec<_>>();
        let results = handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|r| matches!(r, Err(Error::UnavailableCoin)))
                .count(),
            1
        );
        assert_eq!(c.drafts(&id).unwrap().len(), 1);
    }
    #[test]
    fn addresses_advance_durably() {
        let (c, id) = setup();
        let a = c.receive_address(&id).unwrap();
        let b = c.receive_address(&id).unwrap();
        assert_ne!(a.address, b.address);
        assert_eq!(b.index, a.index + 1);
        assert!(!a.hardware_verified);
    }
    #[test]
    fn unknown_output_cannot_be_frozen() {
        let (c, id) = setup();
        assert!(
            c.set_frozen(&id, &format!("{}:0", Txid::all_zeros()), true)
                .is_err()
        );
    }
    #[test]
    fn address_label_roundtrip() {
        let (c, id) = setup();
        let a = c.receive_address(&id).unwrap();
        c.set_label(&id, "addr", &a.address, "Savings").unwrap();
        assert!(c.export_labels(&id).unwrap().contains("Savings"));
    }
    #[test]
    fn labels_preview_does_not_write() {
        let (c, id) = setup();
        let a = c.receive_address(&id).unwrap();
        let payload = format!(r#"{{"type":"addr","ref":"{}","label":"Test"}}"#, a.address);
        assert_eq!(c.import_labels(&id, &payload, false).unwrap().changed, 1);
        assert!(c.export_labels(&id).unwrap().is_empty());
    }
    #[test]
    fn freezes_do_not_reduce_total() {
        let (c, id, cs) = funded();
        c.set_frozen(&id, &cs[0].outpoint, true).unwrap();
        let w = &c.wallets().unwrap()[0];
        assert_eq!(w.total_sats, Some(300_000));
        assert_eq!(w.available_sats, Some(300_000 - cs[0].sats));
    }
    #[test]
    fn manually_frozen_input_rejected() {
        let (c, id, cs) = funded();
        c.set_frozen(&id, &cs[0].outpoint, true).unwrap();
        assert!(matches!(
            c.create_draft(req(&id, vec![cs[0].outpoint.clone()], 10_000)),
            Err(Error::UnavailableCoin)
        ));
    }
    #[test]
    fn insufficient_manual_never_adds_coins() {
        let (c, id, cs) = funded();
        let small = cs.iter().min_by_key(|c| c.sats).unwrap();
        assert!(
            c.create_draft(req(&id, vec![small.outpoint.clone()], 150_000))
                .is_err()
        );
        assert!(c.drafts(&id).unwrap().is_empty());
    }
    #[test]
    fn exact_selection_and_reservation() {
        let (c, id, cs) = funded();
        let d = c
            .create_draft(req(&id, vec![cs[0].outpoint.clone()], 50_000))
            .unwrap();
        assert_eq!(d.inputs.len(), 1);
        assert_eq!(d.inputs[0].outpoint, cs[0].outpoint);
        assert!(
            c.coins(&id)
                .unwrap()
                .iter()
                .any(|c| c.status == CoinStatus::Reserved)
        );
    }
    #[test]
    fn discard_preserves_freeze() {
        let (c, id, cs) = funded();
        let d = c
            .create_draft(req(&id, vec![cs[0].outpoint.clone()], 50_000))
            .unwrap();
        c.set_frozen(&id, &cs[0].outpoint, true).unwrap();
        c.discard_draft(&id, &d.id).unwrap();
        assert!(
            c.coins(&id)
                .unwrap()
                .iter()
                .any(|c| c.outpoint == cs[0].outpoint && c.status == CoinStatus::Frozen)
        );
    }
    #[test]
    fn consolidation_is_one_self_output() {
        let (c, id, cs) = funded();
        let mut r = req(&id, cs.iter().map(|c| c.outpoint.clone()).collect(), 1);
        r.payment = Payment::Consolidate {
            privacy_acknowledged: true,
        };
        let d = c.create_draft(r).unwrap();
        assert_eq!(d.outputs.len(), 1);
        assert!(d.outputs[0].is_mine);
        assert_eq!(d.outputs[0].sats + d.fee_sats, 300_000);
    }
    #[test]
    fn consolidation_requires_acknowledgment() {
        let (c, id, cs) = funded();
        let mut r = req(&id, cs.iter().map(|c| c.outpoint.clone()).collect(), 1);
        r.payment = Payment::Consolidate {
            privacy_acknowledged: false,
        };
        assert!(c.create_draft(r).is_err());
    }
    #[test]
    fn malformed_import_is_atomic() {
        let (c, id) = setup();
        let a = c.receive_address(&id).unwrap();
        let p = format!(
            r#"{{"type":"addr","ref":"{}","label":"Test"}}
not json"#,
            a.address
        );
        assert!(c.import_labels(&id, &p, true).is_err());
        assert!(c.export_labels(&id).unwrap().is_empty());
    }
    #[test]
    fn selected_max_spends_exactly_selected_and_subtracts_fee() {
        let (c, id, cs) = funded();
        let mut request = req(&id, vec![cs[0].outpoint.clone()], 1);
        let Payment::Send { address, .. } = request.payment else {
            unreachable!()
        };
        request.payment = Payment::SendMax { address };
        let draft = c.create_draft(request).unwrap();
        assert_eq!(draft.inputs.len(), 1);
        assert_eq!(draft.inputs[0].outpoint, cs[0].outpoint);
        assert_eq!(draft.outputs.len(), 1);
        assert_eq!(draft.outputs[0].sats + draft.fee_sats, cs[0].sats);
    }
    #[test]
    fn empty_explicit_selection_does_not_enable_automatic_selection() {
        let (c, id, _) = funded();
        assert!(c.create_draft(req(&id, vec![], 10_000)).is_err());
    }
    #[test]
    fn automatic_selection_excludes_user_freezes() {
        let (c, id, cs) = funded();
        c.set_frozen(&id, &cs[0].outpoint, true).unwrap();
        let mut request = req(&id, vec![], 10_000);
        request.selected_outpoints = None;
        let draft = c.create_draft(request).unwrap();
        assert!(
            draft
                .inputs
                .iter()
                .all(|input| input.outpoint != cs[0].outpoint)
        );
    }
    #[test]
    fn second_draft_cannot_reserve_the_same_outpoint() {
        let (c, id, cs) = funded();
        c.create_draft(req(&id, vec![cs[0].outpoint.clone()], 10_000))
            .unwrap();
        assert!(matches!(
            c.create_draft(req(&id, vec![cs[0].outpoint.clone()], 20_000)),
            Err(Error::UnavailableCoin)
        ));
    }
    #[test]
    fn omitted_label_patch_does_not_erase_label_or_freeze() {
        let (c, id, cs) = funded();
        c.set_label(&id, "output", &cs[0].outpoint, "Savings")
            .unwrap();
        c.set_frozen(&id, &cs[0].outpoint, true).unwrap();
        let payload = format!(r#"{{"type":"output","ref":"{}"}}"#, cs[0].outpoint);
        assert_eq!(c.import_labels(&id, &payload, true).unwrap().changed, 0);
        let coins = c.coins(&id).unwrap();
        let coin = coins
            .iter()
            .find(|coin| coin.outpoint == cs[0].outpoint)
            .unwrap();
        assert_eq!(coin.label, "Savings");
        assert_eq!(coin.status, CoinStatus::Frozen);
    }
    #[test]
    fn unfreeze_patch_does_not_release_a_draft_reservation() {
        let (c, id, cs) = funded();
        c.create_draft(req(&id, vec![cs[0].outpoint.clone()], 10_000))
            .unwrap();
        c.set_frozen(&id, &cs[0].outpoint, true).unwrap();
        let payload = format!(
            r#"{{"type":"output","ref":"{}","spendable":true}}"#,
            cs[0].outpoint
        );
        c.import_labels(&id, &payload, true).unwrap();
        assert!(
            c.coins(&id)
                .unwrap()
                .iter()
                .any(|coin| coin.outpoint == cs[0].outpoint && coin.status == CoinStatus::Reserved)
        );
    }
}
