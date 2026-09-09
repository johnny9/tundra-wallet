//! Bounded, public-key-only validation of external PSBT responses.
//! The approved transaction and wallet-derived policies are authorities; response metadata is not.
use crate::{Error, Result};
use base64::{Engine as _, prelude::BASE64_STANDARD};
use bdk_wallet::{
    bitcoin::{
        PublicKey, Transaction, TxOut,
        consensus::{Decodable, encode::VarInt},
        ecdsa,
        psbt::{self, Psbt},
        secp256k1::{Message, Secp256k1},
        sighash::{EcdsaSighashType, SighashCache},
    },
    miniscript::{Descriptor, descriptor::WshInner},
};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_PSBT_BYTES: usize = 1_048_576;
pub const MAX_PSBT_FILE_BYTES: usize = MAX_PSBT_BYTES.div_ceil(3) * 4 + 2;
const MAX_INPUTS: usize = 200;
const MAX_OUTPUTS: usize = 400;

fn invalid() -> Error {
    Error::InvalidInput("invalid or unsupported signing response")
}
fn mismatch() -> Error {
    Error::InvalidInput("signing response does not match the approved draft")
}

fn take<'a>(bytes: &mut &'a [u8], count: usize) -> Result<&'a [u8]> {
    let value = bytes.get(..count).ok_or_else(invalid)?;
    *bytes = &bytes[count..];
    Ok(value)
}
fn count(bytes: &mut &[u8]) -> Result<usize> {
    usize::try_from(VarInt::consensus_decode(bytes).map_err(|_| invalid())?.0)
        .map_err(|_| invalid())
}

// Bound transaction vectors before the dependency parser can allocate them. PSBT v0's
// unsigned transaction uses legacy serialization, with empty scriptSigs and no witnesses.
fn transaction_shape(mut bytes: &[u8]) -> Result<(usize, usize)> {
    take(&mut bytes, 4)?;
    let inputs = count(&mut bytes)?;
    if !(1..=MAX_INPUTS).contains(&inputs) {
        return Err(invalid());
    }
    for _ in 0..inputs {
        take(&mut bytes, 36)?;
        if count(&mut bytes)? != 0 {
            return Err(invalid());
        }
        take(&mut bytes, 4)?;
    }
    let outputs = count(&mut bytes)?;
    if !(1..=MAX_OUTPUTS).contains(&outputs) {
        return Err(invalid());
    }
    for _ in 0..outputs {
        take(&mut bytes, 8)?;
        let size = count(&mut bytes)?;
        if size > 10_000 {
            return Err(invalid());
        }
        take(&mut bytes, size)?;
    }
    take(&mut bytes, 4)?;
    if !bytes.is_empty() {
        return Err(invalid());
    }
    Ok((inputs, outputs))
}

/// Accept binary PSBT or a single base64 value (optional surrounding whitespace).
/// Bounds are checked before decoding/allocation. Never accepts concatenated PSBTs.
pub fn parse_response(payload: &[u8]) -> Result<Psbt> {
    if payload.len() > MAX_PSBT_FILE_BYTES {
        return Err(invalid());
    }
    let decoded;
    let bytes = if payload.starts_with(b"psbt\xff") {
        payload
    } else {
        let text = std::str::from_utf8(payload).map_err(|_| invalid())?.trim();
        decoded = BASE64_STANDARD.decode(text).map_err(|_| invalid())?;
        &decoded
    };
    if bytes.len() > MAX_PSBT_BYTES || !bytes.starts_with(b"psbt\xff") {
        return Err(invalid());
    }
    let mut rest = &bytes[5..];
    let mut shape = None;
    let mut maps = 0;
    while !rest.is_empty() {
        let mut fields = 0;
        loop {
            let key_len = count(&mut rest)?;
            if key_len == 0 {
                break;
            }
            fields += 1;
            if fields > 64 || key_len > 256 {
                return Err(invalid());
            }
            let key = take(&mut rest, key_len)?;
            let value_len = count(&mut rest)?;
            let value = take(&mut rest, value_len)?;
            if maps == 0 && key == [0] {
                if shape.is_some() {
                    return Err(invalid());
                }
                shape = Some(transaction_shape(value)?);
            }
        }
        maps += 1;
        let (inputs, outputs) = shape.ok_or_else(invalid)?;
        if maps > 1 + inputs + outputs {
            return Err(invalid());
        }
    }
    let (inputs, outputs) = shape.ok_or_else(invalid)?;
    if maps != 1 + inputs + outputs {
        return Err(invalid());
    }
    let mut cursor = bytes;
    let psbt = Psbt::deserialize_from_reader(&mut cursor).map_err(|_| invalid())?;
    if !cursor.is_empty() || psbt.version != 0 {
        return Err(invalid());
    }
    Ok(psbt)
}

/// Constructed only from the saved wallet's independently known prevout and derived policy.
pub(crate) struct TrustedInput {
    pub(crate) prevout: TxOut,
    pub(crate) metadata: psbt::Input,
    pub(crate) descriptor: Descriptor<PublicKey>,
}

impl TrustedInput {
    fn keys(&self) -> Result<(Vec<PublicKey>, usize)> {
        let (mut keys, required) = match &self.descriptor {
            Descriptor::Wpkh(d) => (vec![*d.as_inner()], 1),
            Descriptor::Wsh(d) => match d.as_inner() {
                WshInner::SortedMulti(m) if m.k() == 2 && m.n() == 3 => (m.pks().to_vec(), 2),
                _ => return Err(Error::UnsupportedPolicy),
            },
            _ => return Err(Error::UnsupportedPolicy),
        };
        keys.sort_by_key(|key| key.to_bytes());
        if keys.iter().any(|key| !key.compressed)
            || keys.iter().collect::<BTreeSet<_>>().len() != keys.len()
            || self.descriptor.script_pubkey() != self.prevout.script_pubkey
        {
            return Err(Error::CorruptState);
        }
        Ok((keys, required))
    }
}

fn subset<K: Ord, V: PartialEq>(candidate: &BTreeMap<K, V>, expected: &BTreeMap<K, V>) -> bool {
    candidate.iter().all(|(k, v)| expected.get(k) == Some(v))
}
fn optional<T: PartialEq>(candidate: &Option<T>, expected: &Option<T>) -> bool {
    candidate.is_none() || candidate == expected
}

fn check_metadata(
    input: &psbt::Input,
    trusted: &TrustedInput,
    tx: &Transaction,
    index: usize,
) -> Result<()> {
    if !optional(&input.witness_utxo, &Some(trusted.prevout.clone()))
        || !optional(&input.witness_script, &trusted.metadata.witness_script)
        || !subset(&input.bip32_derivation, &trusted.metadata.bip32_derivation)
        || input
            .sighash_type
            .is_some_and(|s| s != EcdsaSighashType::All.into())
    {
        return Err(mismatch());
    }
    if let Some(prev) = &input.non_witness_utxo {
        let outpoint = tx.input[index].previous_output;
        if prev.compute_txid() != outpoint.txid
            || prev.output.get(outpoint.vout as usize) != Some(&trusted.prevout)
        {
            return Err(mismatch());
        }
    }
    // Explicit allowlist: reject unsupported scripts, preimages, taproot, unknown and
    // proprietary fields. They are never silently carried into a subsequent export.
    let allowed = psbt::Input {
        non_witness_utxo: input.non_witness_utxo.clone(),
        witness_utxo: input.witness_utxo.clone(),
        witness_script: input.witness_script.clone(),
        bip32_derivation: input.bip32_derivation.clone(),
        sighash_type: input.sighash_type,
        partial_sigs: input.partial_sigs.clone(),
        final_script_sig: input.final_script_sig.clone(),
        final_script_witness: input.final_script_witness.clone(),
        ..Default::default()
    };
    if input != &allowed
        || input
            .final_script_sig
            .as_ref()
            .is_some_and(|s| !s.is_empty())
    {
        return Err(invalid());
    }
    Ok(())
}

fn input_signatures(
    tx: &Transaction,
    index: usize,
    input: &psbt::Input,
    trusted: &TrustedInput,
) -> Result<BTreeMap<PublicKey, ecdsa::Signature>> {
    let (keys, required) = trusted.keys()?;
    let secp = Secp256k1::verification_only();
    let mut cache = SighashCache::new(tx);
    let hash = if required == 1 {
        cache
            .p2wpkh_signature_hash(
                index,
                &trusted.prevout.script_pubkey,
                trusted.prevout.value,
                EcdsaSighashType::All,
            )
            .map_err(|_| invalid())?
    } else {
        cache
            .p2wsh_signature_hash(
                index,
                &trusted
                    .descriptor
                    .explicit_script()
                    .map_err(|_| invalid())?,
                trusted.prevout.value,
                EcdsaSighashType::All,
            )
            .map_err(|_| invalid())?
    };
    let message = Message::from(hash);
    // libsecp256k1 rejects high-S signatures. Only SIGHASH_ALL can authorize this draft.
    let valid = |key: &PublicKey, sig: &ecdsa::Signature| {
        sig.sighash_type == EcdsaSighashType::All
            && secp
                .verify_ecdsa(&message, &sig.signature, &key.inner)
                .is_ok()
    };
    let mut signatures = input.partial_sigs.clone();
    for (key, sig) in &signatures {
        if !keys.contains(key) || !valid(key, sig) {
            return Err(invalid());
        }
    }
    if let Some(witness) = &input.final_script_witness {
        if !signatures.is_empty() {
            return Err(invalid());
        }
        let stack: Vec<_> = witness.iter().collect();
        if required == 1 {
            if stack.len() != 2 || stack[1] != keys[0].to_bytes() {
                return Err(invalid());
            }
            let sig = ecdsa::Signature::from_slice(stack[0]).map_err(|_| invalid())?;
            if !valid(&keys[0], &sig) {
                return Err(invalid());
            }
            signatures.insert(keys[0], sig);
        } else {
            let script = trusted
                .descriptor
                .explicit_script()
                .map_err(|_| invalid())?;
            if stack.len() != required + 2
                || !stack[0].is_empty()
                || stack[required + 1] != script.as_bytes()
            {
                return Err(invalid());
            }
            // CHECKMULTISIG consumes keys in script order, with a distinct key per signature.
            let mut remaining = keys.iter();
            for bytes in &stack[1..=required] {
                let sig = ecdsa::Signature::from_slice(bytes).map_err(|_| invalid())?;
                let key = remaining.find(|key| valid(key, &sig)).ok_or_else(invalid)?;
                signatures.insert(*key, sig);
            }
        }
    }
    Ok(signatures)
}

/// Validate and merge signatures only. All transaction and metadata bytes in the result
/// come from the immutable approved PSBT. No untrusted PSBT combine/finalize call is used.
pub(crate) fn merge_response(
    approved: &Psbt,
    previous: Option<&Psbt>,
    response: &Psbt,
    trusted: &[TrustedInput],
) -> Result<Psbt> {
    if approved.inputs.len() != trusted.len()
        || approved.inputs.len() != approved.unsigned_tx.input.len()
        || approved.outputs.len() != approved.unsigned_tx.output.len()
        || approved.version != 0
    {
        return Err(Error::CorruptState);
    }
    let mut merged = approved.clone();
    for (index, input) in approved.inputs.iter().enumerate() {
        check_metadata(input, &trusted[index], &approved.unsigned_tx, index)?;
        if !input.partial_sigs.is_empty()
            || input.final_script_witness.is_some()
            || input.final_script_sig.is_some()
        {
            return Err(Error::CorruptState);
        }
    }
    for candidate in previous.into_iter().chain(std::iter::once(response)) {
        if candidate.version != 0
            || candidate.unsigned_tx != approved.unsigned_tx
            || candidate.inputs.len() != trusted.len()
            || candidate.outputs.len() != approved.outputs.len()
            || !subset(&candidate.xpub, &approved.xpub)
            || !candidate.unknown.is_empty()
            || !candidate.proprietary.is_empty()
        {
            return Err(mismatch());
        }
        for (output, expected) in candidate.outputs.iter().zip(&approved.outputs) {
            let allowed = psbt::Output {
                redeem_script: output.redeem_script.clone(),
                witness_script: output.witness_script.clone(),
                bip32_derivation: output.bip32_derivation.clone(),
                ..Default::default()
            };
            if output != &allowed
                || !optional(&output.redeem_script, &expected.redeem_script)
                || !optional(&output.witness_script, &expected.witness_script)
                || !subset(&output.bip32_derivation, &expected.bip32_derivation)
            {
                return Err(mismatch());
            }
        }
        for (index, input) in candidate.inputs.iter().enumerate() {
            check_metadata(input, &trusted[index], &approved.unsigned_tx, index)?;
            for (key, sig) in
                input_signatures(&approved.unsigned_tx, index, input, &trusted[index])?
            {
                if merged.inputs[index]
                    .partial_sigs
                    .get(&key)
                    .is_some_and(|old| old != &sig)
                {
                    return Err(mismatch());
                }
                merged.inputs[index].partial_sigs.insert(key, sig);
            }
        }
    }
    Ok(merged)
}

pub(crate) fn progress(
    psbt: &Psbt,
    trusted: &[TrustedInput],
) -> Result<Vec<crate::InputSigningProgress>> {
    psbt.inputs
        .iter()
        .zip(trusted)
        .enumerate()
        .map(|(index, (input, policy))| {
            Ok(crate::InputSigningProgress {
                outpoint: psbt.unsigned_tx.input[index].previous_output.to_string(),
                valid_signatures: input.partial_sigs.len() as u32,
                required_signatures: policy.keys()?.1 as u32,
            })
        })
        .collect()
}

struct SigningDraft {
    approved: Psbt,
    stored: Option<Psbt>,
    trusted: Vec<TrustedInput>,
    review: crate::DraftReview,
}

fn load_draft(db: &rusqlite::Connection, wallet_id: &str, draft_id: &str) -> Result<SigningDraft> {
    use crate::engine::{from_json, load};
    use bdk_wallet::{
        bitcoin::{
            Address,
            hashes::{Hash, sha256},
        },
        miniscript::{
            ForEachKey,
            descriptor::DescriptorPublicKey,
            psbt::{PsbtInputExt, PsbtOutputExt},
        },
    };
    use rusqlite::{OptionalExtension, params};
    let loaded = load(db, wallet_id)?;
    if loaded.summary.network == crate::Network::Mainnet {
        return Err(Error::Unavailable("mainnet signing"));
    }
    if loaded.summary.synced_at.is_none() {
        return Err(Error::Unavailable("blockchain synchronization"));
    }
    let (approved, review): (String, String) = db
        .query_row(
            "SELECT psbt,review_json FROM drafts WHERE wallet_id=?1 AND id=?2",
            params![wallet_id, draft_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or(Error::NotFound)?;
    let approved = parse_response(approved.as_bytes()).map_err(|_| Error::CorruptState)?;
    let review: crate::DraftReview = from_json(&review)?;
    if !matches!(
        review.state.as_str(),
        "unsigned" | "partially_signed" | "signed" | "finalized"
    ) {
        return Err(Error::UnavailableCoin);
    }
    let identity = sha256::Hash::hash(
        format!("{wallet_id}:{}", approved.unsigned_tx.compute_txid()).as_bytes(),
    )
    .to_string();
    if identity != draft_id
        || review.id != draft_id
        || review.wallet_id != wallet_id
        || review.inputs.len() != approved.inputs.len()
        || review.outputs.len() != approved.outputs.len()
    {
        return Err(Error::CorruptState);
    }
    let mut expected_xpubs = BTreeMap::new();
    for keychain in [
        bdk_wallet::KeychainKind::External,
        bdk_wallet::KeychainKind::Internal,
    ] {
        loaded
            .wallet
            .public_descriptor(keychain)
            .for_each_key(|key| {
                if let DescriptorPublicKey::XPub(key) = key
                    && let Some(origin) = &key.origin
                {
                    expected_xpubs.insert(key.xkey, origin.clone());
                }
                true
            });
    }
    if !subset(&approved.xpub, &expected_xpubs)
        || !approved.unknown.is_empty()
        || !approved.proprietary.is_empty()
    {
        return Err(Error::CorruptState);
    }
    let unspent: BTreeMap<_, _> = loaded
        .wallet
        .list_unspent()
        .map(|c| (c.outpoint, c))
        .collect();
    let mut seen = BTreeSet::new();
    let mut trusted = Vec::new();
    let mut input_sum = 0u64;
    for (index, input) in approved.unsigned_tx.input.iter().enumerate() {
        let outpoint = input.previous_output;
        let coin = unspent.get(&outpoint).ok_or(Error::UnavailableCoin)?;
        let funding = loaded
            .wallet
            .get_tx(outpoint.txid)
            .ok_or(Error::CorruptState)?;
        let reserved: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM reservations WHERE wallet_id=?1 AND draft_id=?2 AND outpoint=?3)",
            params![wallet_id, draft_id, outpoint.to_string()], |r| r.get(0),
        )?;
        let frozen: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM freezes WHERE wallet_id=?1 AND outpoint=?2)",
            params![wallet_id, outpoint.to_string()],
            |r| r.get(0),
        )?;
        if !reserved
            || frozen
            || !seen.insert(outpoint)
            || !crate::sync::spendable(
                coin.chain_position,
                loaded.wallet.local_chain().tip().height(),
                funding.tx_node.tx.is_coinbase(),
            )
        {
            return Err(Error::UnavailableCoin);
        }
        let reviewed = &review.inputs[index];
        if reviewed.outpoint != outpoint.to_string() || reviewed.sats != coin.txout.value.to_sat() {
            return Err(Error::CorruptState);
        }
        input_sum = input_sum
            .checked_add(reviewed.sats)
            .ok_or(Error::CorruptState)?;
        let (keychain, index) = loaded
            .wallet
            .derivation_of_spk(coin.txout.script_pubkey.clone())
            .ok_or(Error::CorruptState)?;
        let descriptor = loaded
            .wallet
            .public_descriptor(keychain)
            .at_derivation_index(index)
            .map_err(|_| Error::CorruptState)?;
        let mut metadata = psbt::Input::default();
        let descriptor = metadata
            .update_with_descriptor_unchecked(&descriptor)
            .map_err(|_| Error::CorruptState)?;
        trusted.push(TrustedInput {
            prevout: coin.txout.clone(),
            metadata,
            descriptor,
        });
    }
    let mut output_sum = 0u64;
    for (index, (output, reviewed)) in approved
        .unsigned_tx
        .output
        .iter()
        .zip(&review.outputs)
        .enumerate()
    {
        let address = Address::from_script(&output.script_pubkey, loaded.summary.network.bitcoin())
            .map_err(|_| Error::CorruptState)?;
        let mine = loaded.wallet.is_mine(output.script_pubkey.clone());
        if address.to_string() != reviewed.address
            || output.value.to_sat() != reviewed.sats
            || mine != reviewed.is_mine
            || (reviewed.is_change && !mine)
        {
            return Err(Error::CorruptState);
        }
        let mut expected = psbt::Output::default();
        if let Some((keychain, derivation)) = loaded
            .wallet
            .derivation_of_spk(output.script_pubkey.clone())
        {
            let descriptor = loaded
                .wallet
                .public_descriptor(keychain)
                .at_derivation_index(derivation)
                .map_err(|_| Error::CorruptState)?;
            let derived = expected
                .update_with_descriptor_unchecked(&descriptor)
                .map_err(|_| Error::CorruptState)?;
            if derived.script_pubkey() != output.script_pubkey {
                return Err(Error::CorruptState);
            }
        }
        let metadata = &approved.outputs[index];
        let allowed = psbt::Output {
            redeem_script: metadata.redeem_script.clone(),
            witness_script: metadata.witness_script.clone(),
            bip32_derivation: metadata.bip32_derivation.clone(),
            ..Default::default()
        };
        if metadata != &allowed
            || !optional(&metadata.redeem_script, &expected.redeem_script)
            || !optional(&metadata.witness_script, &expected.witness_script)
            || !subset(&metadata.bip32_derivation, &expected.bip32_derivation)
        {
            return Err(Error::CorruptState);
        }
        output_sum = output_sum
            .checked_add(reviewed.sats)
            .ok_or(Error::CorruptState)?;
    }
    if input_sum > crate::amount::MAX_SATS
        || input_sum.checked_sub(output_sum) != Some(review.fee_sats)
        || review.fee_sats > 1_000_000
    {
        return Err(Error::CorruptState);
    }
    let stored: Option<String> = db
        .query_row(
            "SELECT psbt FROM draft_signatures WHERE wallet_id=?1 AND draft_id=?2",
            params![wallet_id, draft_id],
            |r| r.get(0),
        )
        .optional()?;
    let stored = stored
        .map(|s| parse_response(s.as_bytes()).map_err(|_| Error::CorruptState))
        .transpose()?;
    Ok(SigningDraft {
        approved,
        stored,
        trusted,
        review,
    })
}

fn signing_progress(
    draft_id: &str,
    psbt: &Psbt,
    trusted: &[TrustedInput],
) -> Result<crate::SigningProgress> {
    let inputs = progress(psbt, trusted)?;
    let complete = !inputs.is_empty()
        && inputs
            .iter()
            .all(|i| i.valid_signatures >= i.required_signatures);
    Ok(crate::SigningProgress {
        draft_id: draft_id.into(),
        inputs,
        complete,
    })
}

/// Assemble only the narrow policy already verified by merge_response. Re-validate the
/// resulting final witness before returning bytes; no generic best-effort finalizer is used.
fn finalize_transaction(
    approved: &Psbt,
    stored: Option<&Psbt>,
    trusted: &[TrustedInput],
) -> Result<Transaction> {
    use bdk_wallet::bitcoin::Witness;
    let merged = merge_response(approved, stored, approved, trusted)?;
    if !signing_progress("", &merged, trusted)?.complete {
        return Err(Error::Unavailable("all required signatures"));
    }
    let mut transaction = approved.unsigned_tx.clone();
    let mut finalized = approved.clone();
    for (index, policy) in trusted.iter().enumerate() {
        let (keys, required) = policy.keys()?;
        let signed: Vec<_> = keys
            .iter()
            .filter_map(|key| {
                merged.inputs[index]
                    .partial_sigs
                    .get(key)
                    .map(|signature| (key, signature))
            })
            .take(required)
            .collect();
        if signed.len() != required {
            return Err(Error::CorruptState);
        }
        let stack = match &policy.descriptor {
            Descriptor::Wpkh(_) => vec![signed[0].1.to_vec(), signed[0].0.to_bytes()],
            Descriptor::Wsh(_) => {
                let mut stack = vec![Vec::new()];
                stack.extend(signed.iter().map(|(_, signature)| signature.to_vec()));
                stack.push(
                    policy
                        .descriptor
                        .explicit_script()
                        .map_err(|_| Error::UnsupportedPolicy)?
                        .into_bytes(),
                );
                stack
            }
            _ => return Err(Error::UnsupportedPolicy),
        };
        let witness = Witness::from_slice(&stack);
        transaction.input[index].witness = witness.clone();
        finalized.inputs[index].final_script_witness = Some(witness);
    }
    merge_response(approved, None, &finalized, trusted)?;
    if transaction.compute_txid() != approved.unsigned_tx.compute_txid() {
        return Err(Error::CorruptState);
    }
    Ok(transaction)
}

fn finalized_review(draft: &SigningDraft, transaction: &Transaction) -> crate::FinalizedReview {
    crate::FinalizedReview {
        wallet_id: draft.review.wallet_id.clone(),
        draft_id: draft.review.id.clone(),
        txid: transaction.compute_txid().to_string(),
        wtxid: transaction.compute_wtxid().to_string(),
        fee_sats: draft.review.fee_sats,
        weight_wu: transaction.weight().to_wu(),
        vsize: transaction.vsize() as u64,
        transaction_bytes: bdk_wallet::bitcoin::consensus::serialize(transaction),
    }
}
fn stored_finalized(
    db: &rusqlite::Connection,
    wallet_id: &str,
    draft_id: &str,
) -> Result<Option<Vec<u8>>> {
    use rusqlite::{OptionalExtension, params};
    Ok(db
        .query_row(
            "SELECT transaction_bytes FROM finalized_drafts WHERE wallet_id=?1 AND draft_id=?2",
            params![wallet_id, draft_id],
            |r| r.get(0),
        )
        .optional()?)
}

pub(crate) fn validated_finalized(
    db: &rusqlite::Connection,
    wallet_id: &str,
    draft_id: &str,
) -> Result<Option<crate::FinalizedReview>> {
    let draft = load_draft(db, wallet_id, draft_id)?;
    let Some(bytes) = stored_finalized(db, wallet_id, draft_id)? else {
        return if draft.review.state == "finalized" {
            Err(Error::CorruptState)
        } else {
            Ok(None)
        };
    };
    if draft.review.state != "finalized" {
        return Err(Error::CorruptState);
    }
    let transaction = finalize_transaction(&draft.approved, draft.stored.as_ref(), &draft.trusted)?;
    let result = finalized_review(&draft, &transaction);
    if bytes != result.transaction_bytes {
        return Err(Error::CorruptState);
    }
    Ok(Some(result))
}

impl crate::Core {
    /// Freeze exact final bytes and review state atomically. Requires every input's
    /// signatures and current wallet eligibility. Does not contact a network or broadcast.
    pub fn finalize_draft(
        &self,
        wallet_id: &str,
        draft_id: &str,
    ) -> Result<crate::FinalizedReview> {
        use rusqlite::{TransactionBehavior, params};
        let mut db = self.lock()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut draft = load_draft(&tx, wallet_id, draft_id)?;
        let transaction =
            finalize_transaction(&draft.approved, draft.stored.as_ref(), &draft.trusted)?;
        let result = finalized_review(&draft, &transaction);
        match stored_finalized(&tx, wallet_id, draft_id)? {
            Some(bytes)
                if bytes == result.transaction_bytes && draft.review.state == "finalized" => {}
            Some(_) => return Err(Error::CorruptState),
            None if draft.review.state == "finalized" => return Err(Error::CorruptState),
            None => {
                tx.execute("INSERT INTO finalized_drafts(wallet_id,draft_id,transaction_bytes) VALUES(?1,?2,?3)",
                    params![wallet_id, draft_id, result.transaction_bytes])?;
                draft.review.state = "finalized".into();
                tx.execute(
                    "UPDATE drafts SET review_json=?1 WHERE wallet_id=?2 AND id=?3",
                    params![crate::engine::json(&draft.review)?, wallet_id, draft_id],
                )?;
            }
        }
        tx.commit()?;
        Ok(result)
    }
    /// Reopening a final review rechecks signatures, current coins and exact persisted bytes.
    pub fn finalized_draft(
        &self,
        wallet_id: &str,
        draft_id: &str,
    ) -> Result<Option<crate::FinalizedReview>> {
        let db = self.lock()?;
        validated_finalized(&db, wallet_id, draft_id)
    }

    /// Import a response atomically after verifying every supplied signature. This does not
    /// request device signing, finalize, or broadcast. Repeated responses are idempotent.
    pub fn accept_signed_psbt(
        &self,
        wallet_id: &str,
        draft_id: &str,
        payload: &[u8],
    ) -> Result<crate::SigningProgress> {
        use rusqlite::{TransactionBehavior, params};
        let response = parse_response(payload)?;
        let mut db = self.lock()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut draft = load_draft(&tx, wallet_id, draft_id)?;
        if draft.review.state == "finalized" {
            return Err(Error::Unavailable("signature changes after finalization"));
        }
        let merged = merge_response(
            &draft.approved,
            draft.stored.as_ref(),
            &response,
            &draft.trusted,
        )?;
        let progress = signing_progress(draft_id, &merged, &draft.trusted)?;
        if progress.inputs.iter().all(|i| i.valid_signatures == 0) {
            return Err(Error::InvalidInput("response contains no valid signatures"));
        }
        draft.review.state = if progress.complete {
            "signed"
        } else {
            "partially_signed"
        }
        .into();
        tx.execute("INSERT INTO draft_signatures(wallet_id,draft_id,psbt) VALUES(?1,?2,?3) ON CONFLICT(wallet_id,draft_id) DO UPDATE SET psbt=excluded.psbt",
            params![wallet_id, draft_id, merged.to_string()])?;
        tx.execute(
            "UPDATE drafts SET review_json=?1 WHERE wallet_id=?2 AND id=?3",
            params![crate::engine::json(&draft.review)?, wallet_id, draft_id],
        )?;
        tx.commit()?;
        Ok(progress)
    }

    pub fn signing_progress(
        &self,
        wallet_id: &str,
        draft_id: &str,
    ) -> Result<crate::SigningProgress> {
        let db = self.lock()?;
        let draft = load_draft(&db, wallet_id, draft_id)?;
        let merged = merge_response(
            &draft.approved,
            draft.stored.as_ref(),
            &draft.approved,
            &draft.trusted,
        )?;
        signing_progress(draft_id, &merged, &draft.trusted)
    }

    /// Revalidate the stored aggregate before returning the next hardware signing payload.
    pub fn export_signing_psbt(&self, wallet_id: &str, draft_id: &str) -> Result<String> {
        let db = self.lock()?;
        let draft = load_draft(&db, wallet_id, draft_id)?;
        Ok(merge_response(
            &draft.approved,
            draft.stored.as_ref(),
            &draft.approved,
            &draft.trusted,
        )?
        .to_string())
    }
}

#[cfg(test)]
#[path = "signing_tests.rs"]
pub(crate) mod tests;

#[cfg(any(test, fuzzing))]
#[path = "../../../tests/signing_fixtures.rs"]
mod public_fixtures;

// Fuzz only, never a production library or UniFFI fixture API.
#[cfg(fuzzing)]
#[path = "../../../fuzz/support/signatures.rs"]
#[doc(hidden)]
pub mod fuzz_support;
