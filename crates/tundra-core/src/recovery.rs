//! Restored approvals require a fresh chain view and an explicit new review.
use crate::{
    Core, DraftReview, Error, FinalizedReview, Result,
    engine::{from_json, json},
};
use bdk_wallet::bitcoin::{
    Address, Transaction, Witness,
    consensus::deserialize,
    hashes::{Hash, sha256},
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

/// Called only in the new restore database's transaction, after schema verification.
pub(crate) fn suspend(db: &Connection) -> Result<()> {
    let mut labels = db.prepare("SELECT label FROM labels")?;
    for label in labels.query_map([], |r| r.get::<_, String>(0))? {
        crate::labels::validate_label(&label?)?;
    }
    let mut freezes = db.prepare("SELECT outpoint FROM freezes")?;
    for reference in freezes.query_map([], |r| r.get::<_, String>(0))? {
        reference?
            .parse::<bdk_wallet::bitcoin::OutPoint>()
            .map_err(|_| Error::CorruptState)?;
    }
    let oversized: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM drafts WHERE length(CAST(psbt AS BLOB))>1398106 OR length(CAST(review_json AS BLOB))>2097152) OR EXISTS(SELECT 1 FROM draft_signatures WHERE length(CAST(psbt AS BLOB))>1398106) OR EXISTS(SELECT 1 FROM finalized_drafts WHERE length(transaction_bytes)>1048576)", [], |r| r.get(0))?;
    if oversized {
        return Err(Error::CorruptState);
    }
    let ids = db
        .prepare("SELECT wallet_id,id FROM drafts ORDER BY wallet_id,id")?
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    db.execute_batch("DELETE FROM recovered_submissions; DELETE FROM reservations;")?;
    let mut cached: Option<(String, crate::engine::Loaded)> = None;
    for (wallet_id, draft_id) in ids {
        let (psbt, value): (String, String) = db.query_row(
            "SELECT psbt,review_json FROM drafts WHERE wallet_id=?1 AND id=?2",
            params![wallet_id, draft_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let approved = crate::signing::parse_response(psbt.as_bytes())?;
        let mut review: DraftReview = from_json(&value)?;
        if !matches!(
            review.state.as_str(),
            "unsigned" | "partially_signed" | "signed" | "finalized" | "observed" | "invalidated"
        ) {
            return Err(Error::CorruptState);
        }
        let identity = sha256::Hash::hash(
            format!("{wallet_id}:{}", approved.unsigned_tx.compute_txid()).as_bytes(),
        )
        .to_string();
        if review.wallet_id != wallet_id
            || review.id != draft_id
            || draft_id != identity
            || review.inputs.len() != approved.inputs.len()
            || review.inputs.is_empty()
            || review.inputs.len() > 200
            || review.outputs.len() != approved.outputs.len()
            || review.outputs.len() > 400
        {
            return Err(Error::CorruptState);
        }
        crate::labels::validate_label(&review.label)?;
        let mut input_sats = 0u64;
        for (input, expected) in review.inputs.iter().zip(&approved.unsigned_tx.input) {
            if input.outpoint != expected.previous_output.to_string() {
                return Err(Error::CorruptState);
            }
            crate::labels::validate_label(&input.label)?;
            input_sats = input_sats
                .checked_add(input.sats)
                .ok_or(Error::CorruptState)?;
        }
        if cached.as_ref().is_none_or(|(id, _)| id != &wallet_id) {
            cached = Some((wallet_id.clone(), crate::engine::load(db, &wallet_id)?));
        }
        let loaded = &cached.as_ref().ok_or(Error::CorruptState)?.1;
        let mut output_sats = 0u64;
        for (output, expected) in review.outputs.iter().zip(&approved.unsigned_tx.output) {
            let address =
                Address::from_script(&expected.script_pubkey, loaded.summary.network.bitcoin())
                    .map_err(|_| Error::CorruptState)?;
            if output.sats != expected.value.to_sat()
                || output.address != address.to_string()
                || output.is_mine != loaded.wallet.is_mine(expected.script_pubkey.clone())
                || (output.is_change && !output.is_mine)
            {
                return Err(Error::CorruptState);
            }
            output_sats = output_sats
                .checked_add(output.sats)
                .ok_or(Error::CorruptState)?;
        }
        if input_sats.checked_sub(output_sats) != Some(review.fee_sats) {
            return Err(Error::CorruptState);
        }
        let final_bytes: Option<Vec<u8>> = db
            .query_row(
                "SELECT transaction_bytes FROM finalized_drafts WHERE wallet_id=?1 AND draft_id=?2",
                params![wallet_id, draft_id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(bytes) = final_bytes {
            if bytes.len() > crate::signing::MAX_PSBT_BYTES {
                return Err(Error::CorruptState);
            }
            let transaction: Transaction = deserialize(&bytes).map_err(|_| Error::CorruptState)?;
            let mut unsigned = transaction.clone();
            for input in &mut unsigned.input {
                input.witness = Witness::new();
            }
            if unsigned != approved.unsigned_tx {
                return Err(Error::CorruptState);
            }
            let mismatch: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM broadcast_attempts WHERE wallet_id=?1 AND draft_id=?2 AND (txid!=?3 OR wtxid!=?4))", params![wallet_id,draft_id,transaction.compute_txid().to_string(),transaction.compute_wtxid().to_string()], |r| r.get(0))?;
            if mismatch {
                return Err(Error::CorruptState);
            }
        }
        // Archive signatures as data. Full cryptographic and eligibility validation is
        // mandatory again before a recovered submission can resume.
        let stored: Option<String> = db
            .query_row(
                "SELECT psbt FROM draft_signatures WHERE wallet_id=?1 AND draft_id=?2",
                params![wallet_id, draft_id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(stored) = stored
            && crate::signing::parse_response(stored.as_bytes())?.unsigned_tx
                != approved.unsigned_tx
        {
            return Err(Error::CorruptState);
        }
        let submitted: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM broadcast_attempts WHERE wallet_id=?1 AND draft_id=?2)",
            params![wallet_id, draft_id],
            |r| r.get(0),
        )?;
        if submitted {
            db.execute(
                "INSERT INTO recovered_submissions VALUES(?1,?2)",
                params![wallet_id, draft_id],
            )?;
            for input in &review.inputs {
                db.execute(
                    "INSERT INTO recovery_holds VALUES(?1,?2,?3)",
                    params![wallet_id, input.outpoint, draft_id],
                )?;
            }
        }
        review.state = "invalidated".into();
        db.execute(
            "UPDATE drafts SET review_json=?1 WHERE wallet_id=?2 AND id=?3",
            params![json(&review)?, wallet_id, draft_id],
        )?;
    }
    db.execute("UPDATE wallets SET synced_at=NULL", [])?;
    Ok(())
}

impl Core {
    pub fn recovery_required(&self, wallet_id: &str, draft_id: &str) -> Result<bool> {
        Ok(self.lock()?.query_row(
            "SELECT EXISTS(SELECT 1 FROM recovered_submissions WHERE wallet_id=?1 AND draft_id=?2)",
            params![wallet_id, draft_id],
            |r| r.get(0),
        )?)
    }

    /// Explicitly reapprove the exact recovered submitted transaction after fresh sync.
    /// This changes no signatures/final bytes and performs no network operation.
    pub fn resume_recovered_submission(
        &self,
        wallet_id: &str,
        draft_id: &str,
        expected_txid: &str,
        expected_attempt: u64,
        review_acknowledged: bool,
    ) -> Result<FinalizedReview> {
        if !review_acknowledged {
            return Err(Error::InvalidInput(
                "review the recovered payment before resuming it",
            ));
        }
        let mut db = self.lock()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let required: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM recovered_submissions WHERE wallet_id=?1 AND draft_id=?2)",
            params![wallet_id, draft_id],
            |r| r.get(0),
        )?;
        if !required {
            return Err(Error::NotFound);
        }
        let (attempt, txid): (u64,String) = tx.query_row("SELECT id,txid FROM broadcast_attempts WHERE wallet_id=?1 AND draft_id=?2 ORDER BY id DESC LIMIT 1", params![wallet_id,draft_id], |r| Ok((r.get(0)?,r.get(1)?)))?;
        if attempt != expected_attempt || txid != expected_txid {
            return Err(Error::InvalidInput(
                "recovered submission changed; review it again",
            ));
        }
        let value: String = tx.query_row(
            "SELECT review_json FROM drafts WHERE wallet_id=?1 AND id=?2",
            params![wallet_id, draft_id],
            |r| r.get(0),
        )?;
        let mut review: DraftReview = from_json(&value)?;
        if review.state != "invalidated" {
            return Err(Error::CorruptState);
        }
        review.state = "finalized".into();
        tx.execute(
            "UPDATE drafts SET review_json=?1 WHERE wallet_id=?2 AND id=?3",
            params![json(&review)?, wallet_id, draft_id],
        )?;
        for input in &review.inputs {
            tx.execute(
                "INSERT OR IGNORE INTO reservations VALUES(?1,?2,?3)",
                params![wallet_id, input.outpoint, draft_id],
            )?;
        }
        // This validator rechecks fresh sync, confirmed/mature prevouts, freezes,
        // reservation ownership, exact approval, every signature and final witness bytes.
        let finalized = crate::signing::validated_finalized(&tx, wallet_id, draft_id)?
            .ok_or(Error::CorruptState)?;
        if finalized.txid != expected_txid {
            return Err(Error::CorruptState);
        }
        tx.execute(
            "DELETE FROM recovered_submissions WHERE wallet_id=?1 AND draft_id=?2",
            params![wallet_id, draft_id],
        )?;
        tx.commit()?;
        Ok(finalized)
    }
}
