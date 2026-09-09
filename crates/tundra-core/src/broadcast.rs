//! Explicit test-network submission of immutable, independently verified final bytes.
//! An Esplora acknowledgement is not a mempool observation or confirmation.
use crate::{engine::load, *};
use bdk_wallet::{
    bitcoin::{BlockHash, Transaction, consensus::deserialize, hex::DisplayHex},
    chain::ChainPosition,
};
use reqwest::Client;
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BroadcastObservation {
    NotSeen,
    Mempool,
    Confirmed,
}
#[derive(Debug, Clone)]
pub struct BroadcastRequest {
    pub wallet_id: String,
    pub draft_id: String,
    pub endpoint: String,
    pub expected_txid: String,
    pub previous_attempt: Option<u64>,
    pub privacy_consent: bool,
    pub retry_acknowledged: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BroadcastInfo {
    pub attempt_id: u64,
    pub wallet_id: String,
    pub draft_id: String,
    pub endpoint: String,
    pub txid: String,
    pub wtxid: String,
    pub requested_at: u64,
    /// False means uncertain, including interruption before a request reached the server.
    pub acknowledged: bool,
    /// Derived only from the wallet's independently applied sync snapshot.
    pub observation: BroadcastObservation,
}

fn latest(
    db: &rusqlite::Connection,
    wallet_id: &str,
    draft_id: &str,
) -> Result<Option<BroadcastInfo>> {
    Ok(db.query_row(
        "SELECT id,endpoint,txid,wtxid,requested_at,acknowledged FROM broadcast_attempts WHERE wallet_id=?1 AND draft_id=?2 ORDER BY id DESC LIMIT 1",
        params![wallet_id, draft_id],
        |r| Ok(BroadcastInfo {
            attempt_id: r.get(0)?, wallet_id: wallet_id.into(), draft_id: draft_id.into(),
            endpoint: r.get(1)?, txid: r.get(2)?, wtxid: r.get(3)?, requested_at: r.get(4)?,
            acknowledged: r.get(5)?, observation: BroadcastObservation::NotSeen,
        }),
    ).optional()?)
}
fn check_previous(
    db: &rusqlite::Connection,
    wallet_id: &str,
    draft_id: &str,
    expected: Option<u64>,
    retry_acknowledged: bool,
) -> Result<()> {
    let previous = latest(db, wallet_id, draft_id)?;
    if previous.as_ref().map(|p| p.attempt_id) != expected {
        return Err(Error::InvalidInput(
            "broadcast state changed; review the latest attempt",
        ));
    }
    if previous.is_some() && !retry_acknowledged {
        return Err(Error::InvalidInput(
            "explicit acknowledgement required before resubmission",
        ));
    }
    let count: u64 = db.query_row(
        "SELECT count(*) FROM broadcast_attempts WHERE wallet_id=?1 AND draft_id=?2",
        params![wallet_id, draft_id],
        |r| r.get(0),
    )?;
    if count >= 64 {
        return Err(Error::InvalidInput("broadcast attempt limit reached"));
    }
    Ok(())
}

fn observation(
    db: &rusqlite::Connection,
    info: &BroadcastInfo,
    wallet: &bdk_wallet::Wallet,
) -> Result<BroadcastObservation> {
    let bytes: Vec<u8> = db.query_row(
        "SELECT transaction_bytes FROM finalized_drafts WHERE wallet_id=?1 AND draft_id=?2",
        params![info.wallet_id, info.draft_id],
        |r| r.get(0),
    )?;
    if bytes.len() > crate::signing::MAX_PSBT_BYTES {
        return Err(Error::CorruptState);
    }
    let transaction: Transaction = deserialize(&bytes).map_err(|_| Error::CorruptState)?;
    if transaction.compute_txid().to_string() != info.txid
        || transaction.compute_wtxid().to_string() != info.wtxid
    {
        return Err(Error::CorruptState);
    }
    Ok(match wallet.get_tx(transaction.compute_txid()) {
        Some(found) if *found.tx_node.tx == transaction => match found.chain_position {
            ChainPosition::Confirmed { .. } => BroadcastObservation::Confirmed,
            ChainPosition::Unconfirmed {
                last_seen: Some(_), ..
            } => BroadcastObservation::Mempool,
            ChainPosition::Unconfirmed { .. } => BroadcastObservation::NotSeen,
        },
        _ => BroadcastObservation::NotSeen,
    })
}

/// Called inside the sync transaction. Own observed spends release reservations without
/// being mistaken for a conflicting transaction. An evicted/reorged approval is never revived.
pub(crate) fn observe_draft(
    db: &rusqlite::Connection,
    wallet_id: &str,
    review: &mut DraftReview,
    wallet: &bdk_wallet::Wallet,
) -> Result<bool> {
    let Some(info) = latest(db, wallet_id, &review.id)? else {
        return Ok(false);
    };
    if observation(db, &info, wallet)? == BroadcastObservation::NotSeen {
        return Ok(false);
    }
    review.state = "observed".into();
    db.execute(
        "UPDATE drafts SET review_json=?1 WHERE wallet_id=?2 AND id=?3",
        params![crate::engine::json(review)?, wallet_id, review.id],
    )?;
    db.execute(
        "DELETE FROM reservations WHERE wallet_id=?1 AND draft_id=?2",
        params![wallet_id, review.id],
    )?;
    Ok(true)
}

impl Core {
    /// Read-only. Opening/resuming a wallet never retries a recorded attempt.
    pub fn broadcast_status(
        &self,
        wallet_id: &str,
        draft_id: &str,
    ) -> Result<Option<BroadcastInfo>> {
        let db = self.lock()?;
        let loaded = load(&db, wallet_id)?;
        let Some(mut info) = latest(&db, wallet_id, draft_id)? else {
            return Ok(None);
        };
        info.observation = observation(&db, &info, &loaded.wallet)?;
        Ok(Some(info))
    }

    /// Blocking operation for a native IO worker. The caller must supply the txid shown
    /// in final review, current attempt identity and fresh endpoint/retry consent.
    /// HTTP errors after the durable attempt record return an uncertain result, never success.
    pub fn broadcast_draft(&self, request: BroadcastRequest) -> Result<BroadcastInfo> {
        let wallet_id = request.wallet_id.as_str();
        let draft_id = request.draft_id.as_str();
        let source = request.endpoint.as_str();
        let expected_txid = request.expected_txid.as_str();
        let previous_attempt = request.previous_attempt;
        let privacy_consent = request.privacy_consent;
        let retry_acknowledged = request.retry_acknowledged;
        let (endpoint, genesis, approved) = {
            let db = self.lock()?;
            let loaded = load(&db, wallet_id)?;
            if loaded.summary.network == Network::Mainnet {
                return Err(Error::Unavailable("mainnet broadcast"));
            }
            let endpoint = crate::sync::endpoint(source, loaded.summary.network, privacy_consent)?;
            let approved = crate::signing::validated_finalized(&db, wallet_id, draft_id)?.ok_or(
                Error::InvalidInput("finalize and review the transaction before broadcast"),
            )?;
            if expected_txid != approved.txid {
                return Err(Error::InvalidInput("final transaction ID changed"));
            }
            check_previous(
                &db,
                wallet_id,
                draft_id,
                previous_attempt,
                retry_acknowledged,
            )?;
            let genesis = bdk_wallet::bitcoin::blockdata::constants::genesis_block(
                loaded.summary.network.bitcoin(),
            )
            .block_hash();
            (endpoint, genesis, approved)
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| Error::BroadcastPreflight)?;
        runtime.block_on(async {
            let client = Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none())
                .retry(reqwest::retry::never()).connect_timeout(Duration::from_secs(10))
                .timeout(request_timeout()).build().map_err(|_| Error::BroadcastPreflight)?;
            let response = client.get(format!("{endpoint}/block-height/0")).send().await.map_err(|_| Error::BroadcastPreflight)?;
            let body = response_text(response).await.ok_or(Error::BroadcastPreflight)?;
            let found: BlockHash = body.trim().parse().map_err(|_| Error::BroadcastPreflight)?;
            if found != genesis { return Err(Error::NetworkMismatch); }
            // Recheck everything after network preflight. The compare-and-insert prevents
            // stale screens and competing connections from creating the same initial attempt.
            let attempt_id = {
                let mut db = self.lock()?;
                let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let current = crate::signing::validated_finalized(&tx, wallet_id, draft_id)?.ok_or(Error::CorruptState)?;
                if current != approved { return Err(Error::CorruptState); }
                check_previous(&tx, wallet_id, draft_id, previous_attempt, retry_acknowledged)?;
                tx.execute("INSERT INTO broadcast_attempts(wallet_id,draft_id,endpoint,txid,wtxid,requested_at,acknowledged) VALUES(?1,?2,?3,?4,?5,?6,0)",
                    params![wallet_id, draft_id, endpoint, current.txid, current.wtxid, crate::engine::now()?])?;
                let attempt_id = tx.last_insert_rowid();
                tx.commit()?; // A crash from this point must retain an uncertain outcome.
                attempt_id
            };
            let response = client.post(format!("{endpoint}/tx")).header("Content-Type", "text/plain")
                .body(approved.transaction_bytes.to_lower_hex_string()).send().await;
            let acknowledged = match response {
                Ok(response) => response_text(response).await.is_some_and(|text| text.trim() == approved.txid),
                Err(_) => false,
            };
            if acknowledged {
                let db = self.lock()?;
                // An old request may finish after a deliberate retry. Update only its own
                // durable row; the latest attempt remains authoritative for presentation.
                db.execute("UPDATE broadcast_attempts SET acknowledged=1 WHERE id=?1 AND wallet_id=?2 AND draft_id=?3",
                    params![attempt_id, wallet_id, draft_id])?;
            }
            self.broadcast_status(wallet_id, draft_id)?.ok_or(Error::CorruptState)
        })
    }
}

#[cfg(not(test))]
fn request_timeout() -> Duration {
    Duration::from_secs(15)
}
#[cfg(test)]
fn request_timeout() -> Duration {
    Duration::from_secs(1)
}
async fn response_text(mut response: reqwest::Response) -> Option<String> {
    if !response.status().is_success() || response.content_length().is_some_and(|n| n > 128) {
        return None;
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        if body.len().saturating_add(chunk.len()) > 128 {
            return None;
        }
        body.extend_from_slice(&chunk);
    }
    String::from_utf8(body).ok()
}

#[cfg(test)]
#[path = "broadcast_tests.rs"]
mod tests;
