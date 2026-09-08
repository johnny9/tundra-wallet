//! Explicit, bounded Esplora scans. The endpoint is a trusted view of the test chain,
//! not an SPV peer. Only script hashes/transaction IDs are requested; never xpubs or labels.
use crate::{
    engine::{Loaded, from_json, json, load, now, save},
    *,
};
use bdk_wallet::{
    Update, Wallet,
    bitcoin::{
        BlockHash, Transaction, Txid,
        block::Header,
        consensus::deserialize,
        hashes::{Hash, sha256, sha256d},
        hex::FromHex,
    },
    chain::{BlockId, ChainPosition, ConfirmationBlockTime},
};
use reqwest::{Client, Url};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::Notify;

const GAP: u32 = 20;
const MAX_SCRIPTS: u32 = 2_000; // per keychain; reaching the cap is an error, never a partial success.
const MAX_TXS: usize = 5_000;
const MAX_BODY: usize = 4_000_000;
const MAX_TOTAL: usize = 64_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncPhase {
    Prepared,
    Scanning,
    Applying,
    Complete,
    Cancelled,
    Failed,
}
impl SyncPhase {
    fn terminal(self) -> bool {
        matches!(self, Self::Complete | Self::Cancelled | Self::Failed)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProgress {
    pub id: u64,
    pub wallet_id: String,
    pub phase: SyncPhase,
    pub scanned_scripts: u32,
    pub error: Option<String>,
}
struct Snapshot {
    loaded: Loaded,
    revision: u64,
    serialized: String,
    endpoint: String,
}
struct Operation {
    progress: SyncProgress,
    pending: Option<Snapshot>,
    cancel: Arc<Notify>,
}
#[derive(Default)]
pub(crate) struct Operations {
    next: AtomicU64,
    entries: Mutex<BTreeMap<u64, Operation>>,
}

/// HTTPS, or loopback HTTP for local test nodes/emulators. Credentials, queries,
/// fragments, redirects and implicit environment proxies are deliberately unsupported.
fn endpoint(value: &str, network: Network, consent: bool) -> Result<String> {
    if !consent {
        return Err(Error::InvalidInput(
            "explicit endpoint privacy consent required",
        ));
    }
    if network == Network::Mainnet {
        return Err(Error::Unavailable("mainnet synchronization"));
    }
    if value.len() > 2048 || value.chars().any(char::is_control) {
        return Err(Error::InvalidInput("endpoint"));
    }
    let url = Url::parse(value).map_err(|_| Error::InvalidInput("endpoint"))?;
    let local = matches!(
        url.host_str(),
        Some("127.0.0.1" | "[::1]" | "localhost" | "10.0.2.2")
    );
    if !(url.scheme() == "https" || (url.scheme() == "http" && local))
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(Error::InvalidInput(
            "use HTTPS or loopback HTTP without credentials, query or fragment",
        ));
    }
    Ok(url.as_str().trim_end_matches('/').to_owned())
}

impl Core {
    /// Allocate an operation before any network IO. Native callers can display/cancel
    /// this ID before calling run_sync. Opening the database never starts a scan.
    pub fn prepare_sync(
        &self,
        wallet_id: &str,
        source: &str,
        privacy_consent: bool,
    ) -> Result<SyncProgress> {
        let snapshot = {
            let db = self.lock()?;
            let loaded = load(&db, wallet_id)?;
            let endpoint = endpoint(source, loaded.summary.network, privacy_consent)?;
            let revision = revision(&db, wallet_id)?;
            let serialized = json(&loaded.state)?;
            Snapshot {
                loaded,
                revision,
                serialized,
                endpoint,
            }
        };
        let mut entries = self.syncs.entries.lock().map_err(|_| Error::Poisoned)?;
        if entries
            .values()
            .any(|o| o.progress.wallet_id == wallet_id && !o.progress.phase.terminal())
        {
            return Err(Error::SyncBusy);
        }
        if entries.len() >= 32 {
            entries.retain(|_, o| !o.progress.phase.terminal());
        }
        if entries.len() >= 32 {
            return Err(Error::SyncBusy);
        }
        let id = self
            .syncs
            .next
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| Error::SyncLimit)?
            + 1;
        let progress = SyncProgress {
            id,
            wallet_id: wallet_id.into(),
            phase: SyncPhase::Prepared,
            scanned_scripts: 0,
            error: None,
        };
        entries.insert(
            id,
            Operation {
                progress: progress.clone(),
                pending: Some(snapshot),
                cancel: Arc::new(Notify::new()),
            },
        );
        Ok(progress)
    }
    pub fn sync_progress(&self, id: u64) -> Result<SyncProgress> {
        Ok(self
            .syncs
            .entries
            .lock()
            .map_err(|_| Error::Poisoned)?
            .get(&id)
            .ok_or(Error::NotFound)?
            .progress
            .clone())
    }
    pub fn sync_endpoint(&self, wallet_id: &str) -> Result<Option<String>> {
        let db = self.lock()?;
        load(&db, wallet_id)?;
        Ok(db
            .query_row(
                "SELECT endpoint FROM sync_state WHERE wallet_id=?1",
                [wallet_id],
                |r| r.get(0),
            )
            .optional()?)
    }
    /// Cancellation and commit serialize through the operation lock. If Complete is
    /// returned the commit already won; otherwise a cancelled scan cannot commit.
    pub fn cancel_sync(&self, id: u64) -> Result<SyncProgress> {
        let mut entries = self.syncs.entries.lock().map_err(|_| Error::Poisoned)?;
        let op = entries.get_mut(&id).ok_or(Error::NotFound)?;
        if !op.progress.phase.terminal() {
            op.progress.phase = SyncPhase::Cancelled;
            op.pending = None;
            op.cancel.notify_one();
        }
        Ok(op.progress.clone())
    }
    pub fn run_sync(&self, id: u64) -> Result<()> {
        let (snapshot, cancel) = {
            let mut entries = self.syncs.entries.lock().map_err(|_| Error::Poisoned)?;
            let op = entries.get_mut(&id).ok_or(Error::NotFound)?;
            if op.progress.phase == SyncPhase::Cancelled {
                return Err(Error::Cancelled);
            }
            let snapshot = op.pending.take().ok_or(Error::SyncBusy)?;
            op.progress.phase = SyncPhase::Scanning;
            (snapshot, op.cancel.clone())
        };
        let core = self.clone();
        if std::thread::Builder::new()
            .name("tundra-sync".into())
            .spawn(move || {
                let result = core.scan_and_apply(id, snapshot, cancel);
                if let Err(error) = result
                    && let Ok(mut entries) = core.syncs.entries.lock()
                    && let Some(op) = entries.get_mut(&id)
                    && !op.progress.phase.terminal()
                {
                    op.progress.phase = if matches!(error, Error::Cancelled) {
                        SyncPhase::Cancelled
                    } else {
                        SyncPhase::Failed
                    };
                    op.progress.error = Some(error.to_string());
                }
            })
            .is_err()
        {
            self.cancel_sync(id)?;
            return Err(Error::SyncFailed);
        }
        Ok(())
    }
    fn scan_and_apply(&self, id: u64, snapshot: Snapshot, cancel: Arc<Notify>) -> Result<()> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| Error::SyncFailed)?;
        // BDK observation timestamps must strictly increase, even for two scans in one second.
        let observed_at = now()?.max(
            snapshot
                .loaded
                .summary
                .synced_at
                .unwrap_or(0)
                .saturating_add(1),
        );
        let result = runtime.block_on(async {
            tokio::select! {
                biased;
                _ = cancel.notified() => Err(Error::Cancelled),
                result = tokio::time::timeout(Duration::from_secs(180), scan(self, id, &snapshot, observed_at)) =>
                    result.map_err(|_| Error::SyncLimit)?,
            }
        })?;
        // No database mutex or SQLite transaction was held across the network work.
        let mut entries = self.syncs.entries.lock().map_err(|_| Error::Poisoned)?;
        let op = entries.get_mut(&id).ok_or(Error::NotFound)?;
        if op.progress.phase == SyncPhase::Cancelled {
            return Err(Error::Cancelled);
        }
        op.progress.phase = SyncPhase::Applying;
        let wallet_id = &op.progress.wallet_id;
        let mut db = self.lock()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut loaded = load(&tx, wallet_id)?;
        if revision(&tx, wallet_id)? != snapshot.revision
            || json(&loaded.state)? != snapshot.serialized
        {
            return Err(Error::StaleSync);
        }
        // A complete replacement of validated checkpoints also handles a shorter
        // remote tip (BDK's additive Update alone cannot express truncating a tip).
        loaded.state.local_chain.blocks = result
            .blocks
            .into_iter()
            .map(|(h, b)| (h, Some(b)))
            .collect();
        loaded.wallet = Wallet::load()
            .check_network(loaded.summary.network.bitcoin())
            .load_wallet_no_persist(loaded.state.clone())
            .map_err(|_| Error::CorruptState)?
            .ok_or(Error::CorruptState)?;
        loaded
            .wallet
            .apply_update(result.update)
            .map_err(|_| Error::SyncFailed)?;
        invalidate_drafts(&tx, wallet_id, &loaded.wallet)?;
        save(&tx, wallet_id, &mut loaded)?;
        tx.execute(
            "UPDATE wallets SET synced_at=?1 WHERE id=?2",
            params![observed_at, wallet_id],
        )?;
        tx.execute("INSERT INTO sync_state(wallet_id,revision,endpoint) VALUES(?1,1,?2) ON CONFLICT(wallet_id) DO UPDATE SET revision=revision+1,endpoint=excluded.endpoint", params![wallet_id, snapshot.endpoint])?;
        tx.commit()?;
        op.progress.phase = SyncPhase::Complete;
        Ok(())
    }
}
fn revision(db: &rusqlite::Connection, id: &str) -> Result<u64> {
    Ok(db
        .query_row(
            "SELECT revision FROM sync_state WHERE wallet_id=?1",
            [id],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0))
}
pub(crate) fn invalidate_drafts(
    db: &rusqlite::Connection,
    id: &str,
    wallet: &Wallet,
) -> Result<()> {
    let eligible: BTreeSet<_> = wallet
        .list_unspent()
        .filter(|c| {
            let coinbase = wallet
                .get_tx(c.outpoint.txid)
                .is_some_and(|t| t.tx_node.tx.is_coinbase());
            spendable(
                c.chain_position,
                wallet.local_chain().tip().height(),
                coinbase,
            )
        })
        .map(|c| c.outpoint.to_string())
        .collect();
    let mut stmt = db.prepare("SELECT id,review_json FROM drafts WHERE wallet_id=?1")?;
    let drafts = stmt
        .query_map([id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for (draft_id, value) in drafts {
        let mut review: DraftReview = from_json(&value)?;
        if review
            .inputs
            .iter()
            .any(|i| !eligible.contains(&i.outpoint))
        {
            review.state = "invalidated".into();
            db.execute(
                "UPDATE drafts SET review_json=?1 WHERE id=?2",
                params![json(&review)?, draft_id],
            )?;
            db.execute(
                "DELETE FROM reservations WHERE wallet_id=?1 AND draft_id=?2",
                params![id, draft_id],
            )?;
        }
    }
    Ok(())
}
pub(crate) fn spendable(
    position: ChainPosition<ConfirmationBlockTime>,
    tip: u32,
    coinbase: bool,
) -> bool {
    match position {
        ChainPosition::Confirmed { anchor, .. } => {
            !coinbase || tip.saturating_sub(anchor.block_id.height) + 1 >= 100
        }
        _ => false,
    }
}

struct Http {
    client: Client,
    endpoint: String,
    total: usize,
    requests: usize,
}
impl Http {
    fn new(endpoint: &str) -> Result<Self> {
        let client = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|_| Error::SyncFailed)?;
        Ok(Self {
            client,
            endpoint: endpoint.into(),
            total: 0,
            requests: 0,
        })
    }
    async fn bytes(&mut self, path: &str, limit: usize) -> Result<Vec<u8>> {
        self.requests += 1;
        if self.requests > 20_000 {
            return Err(Error::SyncLimit);
        }
        let mut response = self
            .client
            .get(format!("{}{path}", self.endpoint))
            .send()
            .await
            .map_err(|_| Error::SyncFailed)?;
        if !response.status().is_success() {
            return Err(Error::SyncFailed);
        }
        if response.content_length().is_some_and(|n| n > limit as u64) {
            return Err(Error::SyncLimit);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| Error::SyncFailed)? {
            if bytes.len().saturating_add(chunk.len()) > limit
                || self.total.saturating_add(chunk.len()) > MAX_TOTAL
            {
                return Err(Error::SyncLimit);
            }
            self.total += chunk.len();
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
    async fn text(&mut self, path: &str, limit: usize) -> Result<String> {
        String::from_utf8(self.bytes(path, limit).await?).map_err(|_| Error::SyncFailed)
    }
    async fn json<T: serde::de::DeserializeOwned>(&mut self, path: &str) -> Result<T> {
        serde_json::from_slice(&self.bytes(path, MAX_BODY).await?).map_err(|_| Error::SyncFailed)
    }
    async fn hash(&mut self, height: u32) -> Result<BlockHash> {
        self.text(&format!("/block-height/{height}"), 64)
            .await?
            .parse()
            .map_err(|_| Error::SyncFailed)
    }
}
#[derive(Clone, Deserialize, PartialEq, Eq)]
struct Status {
    confirmed: bool,
    block_height: Option<u32>,
    block_hash: Option<BlockHash>,
    block_time: Option<u64>,
}
#[derive(Deserialize)]
struct History {
    txid: Txid,
    status: Status,
}
#[derive(Deserialize)]
struct Proof {
    block_height: u32,
    merkle: Vec<Txid>,
    pos: u32,
}
struct Scan {
    blocks: BTreeMap<u32, BlockHash>,
    update: Update,
}

async fn scan(core: &Core, id: u64, snapshot: &Snapshot, observed_at: u64) -> Result<Scan> {
    let wallet = &snapshot.loaded.wallet;
    let mut http = Http::new(&snapshot.endpoint)?;
    let genesis =
        bdk_wallet::bitcoin::blockdata::constants::genesis_block(wallet.network()).block_hash();
    if http.hash(0).await? != genesis {
        return Err(Error::NetworkMismatch);
    }
    let height: u32 = http
        .text("/blocks/tip/height", 10)
        .await?
        .parse()
        .map_err(|_| Error::SyncFailed)?;
    let tip = http.hash(height).await?;
    let mut blocks = BTreeMap::from([(0, genesis), (height, tip)]);
    for checkpoint in wallet.local_chain().tip().iter() {
        if checkpoint.height() <= height && !blocks.contains_key(&checkpoint.height()) {
            blocks.insert(checkpoint.height(), http.hash(checkpoint.height()).await?);
        }
    }
    let mut update = Update::default();
    let mut history = BTreeMap::<Txid, Status>::new();
    for (keychain, scripts) in wallet.all_unbounded_spk_iters() {
        let mut gap = 0;
        let mut complete = false;
        for (index, script) in scripts.take(MAX_SCRIPTS as usize) {
            let hash = sha256::Hash::hash(script.as_bytes());
            let mut path = format!("/scripthash/{hash}/txs");
            let mut seen = BTreeSet::new();
            let mut active = false;
            loop {
                let page: Vec<History> = http.json(&path).await?;
                let confirmed = page.iter().filter(|t| t.status.confirmed).count();
                let pending = page.len() - confirmed;
                if pending >= 50 || confirmed > 25 {
                    return Err(Error::SyncLimit);
                }
                let last = page
                    .iter()
                    .rev()
                    .find(|t| t.status.confirmed)
                    .map(|t| t.txid);
                active |= !page.is_empty();
                for tx in page {
                    if !seen.insert(tx.txid) {
                        return Err(Error::SyncFailed);
                    }
                    if history.get(&tx.txid).is_some_and(|s| *s != tx.status) {
                        return Err(Error::StaleSync);
                    }
                    history.insert(tx.txid, tx.status);
                    if history.len() > MAX_TXS {
                        return Err(Error::SyncLimit);
                    }
                }
                if confirmed < 25 {
                    break;
                }
                path = format!(
                    "/scripthash/{hash}/txs/chain/{}",
                    last.ok_or(Error::SyncFailed)?
                );
            }
            if active {
                gap = 0;
                update.last_active_indices.insert(keychain, index);
            } else {
                gap += 1;
            }
            {
                let mut entries = core.syncs.entries.lock().map_err(|_| Error::Poisoned)?;
                let op = entries.get_mut(&id).ok_or(Error::NotFound)?;
                if op.progress.phase == SyncPhase::Cancelled {
                    return Err(Error::Cancelled);
                }
                op.progress.scanned_scripts += 1;
            }
            // Scan past every already revealed address, including unused receive/change.
            if gap >= GAP
                && index
                    >= wallet
                        .derivation_index(keychain)
                        .unwrap_or(0)
                        .saturating_add(GAP - 1)
            {
                complete = true;
                break;
            }
        }
        if !complete {
            return Err(Error::SyncLimit);
        }
    }
    let mut headers = BTreeMap::<BlockHash, Header>::new();
    for (txid, status) in &history {
        let raw = http.bytes(&format!("/tx/{txid}/raw"), MAX_BODY).await?;
        let transaction: Transaction = deserialize(&raw).map_err(|_| Error::SyncFailed)?;
        if transaction.compute_txid() != *txid {
            return Err(Error::SyncFailed);
        }
        if status.confirmed {
            let h = status.block_height.ok_or(Error::SyncFailed)?;
            let hash = status.block_hash.ok_or(Error::SyncFailed)?;
            if h > height {
                return Err(Error::StaleSync);
            }
            if let std::collections::btree_map::Entry::Vacant(e) = blocks.entry(h) {
                e.insert(http.hash(h).await?);
            }
            if blocks.get(&h) != Some(&hash) {
                return Err(Error::StaleSync);
            }
            if let std::collections::btree_map::Entry::Vacant(e) = headers.entry(hash) {
                let bytes =
                    Vec::<u8>::from_hex(&http.text(&format!("/block/{hash}/header"), 160).await?)
                        .map_err(|_| Error::SyncFailed)?;
                let header: Header = deserialize(&bytes).map_err(|_| Error::SyncFailed)?;
                if header.block_hash() != hash {
                    return Err(Error::SyncFailed);
                }
                e.insert(header);
            }
            let header = headers.get(&hash).ok_or(Error::SyncFailed)?;
            let proof: Proof = http.json(&format!("/tx/{txid}/merkle-proof")).await?;
            verify_proof(*txid, &proof, header, h)?;
            if status.block_time != Some(header.time as u64) {
                return Err(Error::SyncFailed);
            }
            update.tx_update.anchors.insert((
                ConfirmationBlockTime {
                    block_id: BlockId { height: h, hash },
                    confirmation_time: header.time as u64,
                },
                *txid,
            ));
        } else {
            if status.block_hash.is_some() || status.block_height.is_some() {
                return Err(Error::SyncFailed);
            }
            update.tx_update.seen_ats.insert((*txid, observed_at));
        }
        update.tx_update.txs.push(Arc::new(transaction));
    }
    for tx in wallet.transactions() {
        let txid = tx.tx_node.txid;
        if !history.contains_key(&txid) {
            update.tx_update.evicted_ats.insert((txid, observed_at));
        }
    }
    // Never publish a mixture of different chain tips or a partially completed scan.
    if http.hash(height).await? != tip
        || http
            .text("/blocks/tip/height", 10)
            .await?
            .parse::<u32>()
            .map_err(|_| Error::SyncFailed)?
            != height
    {
        return Err(Error::StaleSync);
    }
    Ok(Scan { blocks, update })
}
fn verify_proof(txid: Txid, proof: &Proof, header: &Header, height: u32) -> Result<()> {
    if proof.block_height != height || proof.merkle.len() > 32 {
        return Err(Error::SyncFailed);
    }
    let mut hash = txid.to_byte_array();
    let mut pos = proof.pos;
    for sibling in &proof.merkle {
        let sibling = sibling.to_byte_array();
        let mut pair = [0u8; 64];
        let (left, right) = if pos & 1 == 0 {
            (hash, sibling)
        } else {
            (sibling, hash)
        };
        pair[..32].copy_from_slice(&left);
        pair[32..].copy_from_slice(&right);
        hash = sha256d::Hash::hash(&pair).to_byte_array();
        pos >>= 1;
    }
    if pos != 0 || hash != header.merkle_root.to_byte_array() {
        return Err(Error::SyncFailed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const SINGLE: &str = include_str!("../../../tests/fixtures/single-sig.txt");
    #[test]
    fn endpoint_requires_explicit_consent_and_test_network() {
        for source in [
            "http://example.com/api",
            "https://user:pass@example.com",
            "https://example.com?token=x",
            "file:///tmp/x",
            "https://example.com/#fragment",
            "https://example.com/\n",
        ] {
            assert!(endpoint(source, Network::Signet, true).is_err());
        }
        assert!(endpoint("https://example.com/api", Network::Signet, false).is_err());
        assert!(endpoint("https://example.com/api", Network::Mainnet, true).is_err());
        assert!(endpoint("http://127.0.0.1:1234/api", Network::Regtest, true).is_ok());
    }
    #[test]
    fn preparation_and_cancellation_do_not_sync_or_contact_endpoint() {
        let core = Core::open(":memory:").unwrap();
        let wallet = core.import_wallet("Test", SINGLE, Network::Signet).unwrap();
        let operation = core
            .prepare_sync(&wallet.id, "https://unused.invalid", true)
            .unwrap();
        assert_eq!(operation.phase, SyncPhase::Prepared);
        assert!(matches!(
            core.prepare_sync(&wallet.id, "https://unused.invalid", true),
            Err(Error::SyncBusy)
        ));
        core.cancel_sync(operation.id).unwrap();
        assert!(matches!(core.run_sync(operation.id), Err(Error::Cancelled)));
        assert!(core.wallets().unwrap()[0].synced_at.is_none());
        assert!(core.sync_endpoint(&wallet.id).unwrap().is_none());
    }
}
