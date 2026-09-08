//! Development Ledger USB adapter. Protocol work does not qualify a physical device.
pub mod framing;
pub(crate) mod protocol;
use crate::{Core, Error, Result};
use protocol::{Context, Intent, Outcome, Session};
pub use protocol::{State, Update};
use rusqlite::params;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub enum Operation {
    Inspect,
    RegisterPolicy,
    VerifyReceive { index: u32 },
    SignDraft { draft_id: String },
}
pub struct UsbSession {
    core: Core,
    wallet_id: String,
    policy_id: [u8; 32],
    operation: Operation,
    protocol: Session,
    pub signing: Option<crate::SigningProgress>,
}
fn context(db: &rusqlite::Connection, wallet_id: &str) -> Result<Context> {
    use bdk_wallet::KeychainKind;
    let loaded = crate::engine::load(db, wallet_id)?;
    let payload = format!(
        "{}\n{}",
        loaded.wallet.public_descriptor(KeychainKind::External),
        loaded.wallet.public_descriptor(KeychainKind::Internal)
    );
    let preview = crate::descriptor::preview_import(&payload, loaded.summary.network)?;
    let mut result = Context::from_preview(&preview)?;
    let policy_id = result.policy.id().map_err(|_| Error::CorruptState)?;
    let mut statement = db.prepare(
        "SELECT fingerprint,hmac FROM hardware_registrations WHERE wallet_id=?1 AND policy_id=?2",
    )?;
    let rows = statement.query_map(params![wallet_id, policy_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
    })?;
    for row in rows {
        let (fingerprint, hmac) = row?;
        let fingerprint = bhwi::bitcoin::bip32::Fingerprint::from_str(&fingerprint)
            .map_err(|_| Error::CorruptState)?;
        if !result
            .accounts
            .iter()
            .any(|account| account.fingerprint == fingerprint)
        {
            return Err(Error::CorruptState);
        }
        result.registrations.insert(
            fingerprint,
            hmac.try_into().map_err(|_| Error::CorruptState)?,
        );
    }
    Ok(result)
}
impl Core {
    pub fn prepare_usb(&self, wallet_id: &str, operation: Operation) -> Result<UsbSession> {
        // Device signing stays behind the explicit physical qualification gate. The
        // interpreter path is implemented so its software checks do not need a real device.
        if matches!(operation, Operation::SignDraft { .. })
            && !crate::hardware::capabilities(crate::hardware::Transport::Usb).available
        {
            return Err(Error::Unavailable("hardware signing"));
        }
        let db = self.lock()?;
        let context = context(&db, wallet_id)?;
        let policy_id = context.policy.id().map_err(|_| Error::CorruptState)?;
        let intent = match &operation {
            Operation::Inspect => Intent::Inspect,
            Operation::RegisterPolicy => Intent::Register,
            Operation::VerifyReceive { index } => {
                use bdk_wallet::KeychainKind;
                let loaded = crate::engine::load(&db, wallet_id)?;
                if loaded
                    .wallet
                    .derivation_index(KeychainKind::External)
                    .is_none_or(|last| *index > last)
                {
                    return Err(Error::InvalidInput(
                        "receive address has not been issued by this wallet",
                    ));
                }
                Intent::Address {
                    index: *index,
                    expected: loaded
                        .wallet
                        .peek_address(KeychainKind::External, *index)
                        .address
                        .to_string(),
                }
            }
            Operation::SignDraft { draft_id } => {
                // Avoid holding a database mutex across the public export method.
                drop(db);
                let psbt = crate::signing::parse_response(
                    self.export_signing_psbt(wallet_id, draft_id)?.as_bytes(),
                )?;
                return Ok(UsbSession {
                    core: self.clone(),
                    wallet_id: wallet_id.into(),
                    policy_id,
                    protocol: Session::start(
                        context,
                        Intent::Sign {
                            psbt: Box::new(psbt),
                        },
                    )?,
                    operation,
                    signing: None,
                });
            }
        };
        Ok(UsbSession {
            core: self.clone(),
            wallet_id: wallet_id.into(),
            policy_id,
            protocol: Session::start(context, intent)?,
            operation,
            signing: None,
        })
    }
}
impl UsbSession {
    pub fn progress(&mut self) -> Update {
        self.protocol.progress()
    }
    pub fn cancel(&mut self) -> Update {
        self.signing = None;
        self.protocol.cancel()
    }
    pub fn receive(&mut self, step: u32, report: &[u8]) -> Result<Update> {
        let result = self.receive_inner(step, report);
        if result.is_err() {
            self.protocol.fail();
            self.signing = None;
        }
        result
    }
    fn receive_inner(&mut self, step: u32, report: &[u8]) -> Result<Update> {
        let update = self.protocol.receive(step, report)?;
        if update.state != State::Complete {
            return Ok(update);
        }
        let outcome = self.protocol.take_outcome()?;
        // Completion is published only after wallet revalidation and any persistence.
        let mut db = self.core.lock()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let current = context(&tx, &self.wallet_id)?;
        if current.policy.id().map_err(|_| Error::CorruptState)? != self.policy_id {
            return Err(Error::CorruptState);
        }
        match (&self.operation, outcome) {
            (Operation::Inspect, Outcome::Inspected) => {}
            (Operation::RegisterPolicy, Outcome::Registered { hmac }) => {
                let fingerprint = update.fingerprint.as_ref().ok_or(Error::CorruptState)?;
                tx.execute("INSERT INTO hardware_registrations(wallet_id,fingerprint,policy_id,hmac) VALUES(?1,?2,?3,?4) ON CONFLICT(wallet_id,fingerprint,policy_id) DO UPDATE SET hmac=excluded.hmac",
                    params![self.wallet_id, fingerprint, self.policy_id, hmac])?;
            }
            (Operation::VerifyReceive { index }, Outcome::AddressMatches) => {
                if crate::engine::load(&tx, &self.wallet_id)?
                    .wallet
                    .derivation_index(bdk_wallet::KeychainKind::External)
                    .is_none_or(|last| *index > last)
                {
                    return Err(Error::CorruptState);
                }
            }
            (Operation::SignDraft { draft_id }, Outcome::Signed(psbt)) => {
                tx.commit()?;
                drop(db);
                self.signing = Some(self.core.accept_signed_psbt(
                    &self.wallet_id,
                    draft_id,
                    &psbt.serialize(),
                )?);
                return Ok(update);
            }
            _ => return Err(Error::CorruptState),
        }
        tx.commit()?;
        Ok(update)
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
