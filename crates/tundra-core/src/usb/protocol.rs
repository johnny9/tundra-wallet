//! Restricted Ledger interpreter driver. The caller owns I/O and current-wallet checks.
//! No setup, restore, PIN, passphrase, backup, wipe or arbitrary command is exposed.
use super::framing::{ResponseReader, command_reports};
use crate::{Error, Result};
use bdk_wallet::bitcoin::{
    bip32::{DerivationPath, Fingerprint, Xpub},
    psbt::Psbt,
};
use bhwi::{
    Interpreter,
    common::{
        Command, DeviceContext, DisplayAddress, LedgerInterpreter, Recipient, Response, Transmit,
    },
    ledger::{LedgerWalletPolicy, Version},
};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

const MAX_EXCHANGES: u32 = 4096;
const MAX_RECEIVED_BYTES: usize = 4 * 1_048_576;
const LIFETIME: Duration = Duration::from_secs(180);

#[derive(Clone)]
pub(crate) struct Account {
    pub fingerprint: Fingerprint,
    pub path: DerivationPath,
    pub xpub: Xpub,
}
#[derive(Clone)]
pub(crate) struct Context {
    pub policy: LedgerWalletPolicy,
    pub accounts: Vec<Account>,
    pub registrations: BTreeMap<Fingerprint, [u8; 32]>,
}
impl Context {
    pub(crate) fn from_preview(preview: &crate::ImportPreview) -> Result<Self> {
        use bhwi::miniscript::{
            Descriptor,
            descriptor::{DescriptorPublicKey, WalletPolicy},
        };
        use std::str::FromStr;
        let checked = crate::descriptor::preview_import(
            &format!(
                "{}\n{}",
                preview.receive_descriptor, preview.change_descriptor
            ),
            preview.network,
        )?;
        if checked.network == crate::Network::Mainnet {
            return Err(Error::Unavailable("mainnet hardware operations"));
        }
        let descriptor = Descriptor::<DescriptorPublicKey>::from_str(&checked.receive_descriptor)
            .map_err(|_| invalid())?;
        let mut accounts = Vec::new();
        let mut keys = Vec::new();
        for key in descriptor.iter_pk() {
            let DescriptorPublicKey::XPub(key) = key else {
                return Err(invalid());
            };
            let (fingerprint, path) = key.origin.ok_or_else(invalid)?;
            keys.push(
                // This miniscript revision materializes placeholders from full key
                // expressions. bhwi strips the suffix only for BIP-388 wire key info.
                DescriptorPublicKey::from_str(&format!(
                    "[{fingerprint}/{path}]{}/<0;1>/*",
                    key.xkey
                ))
                .map_err(|_| invalid())?,
            );
            accounts.push(Account {
                fingerprint,
                path,
                xpub: key.xkey,
            });
        }
        let template = match checked.policy {
            crate::Policy::SingleSig => "wpkh(@0/**)",
            crate::Policy::TwoOfThree => "wsh(sortedmulti(2,@0/**,@1/**,@2/**))",
        };
        let mut policy = WalletPolicy::from_str(template).map_err(|_| invalid())?;
        policy.set_key_info(&keys).map_err(|_| invalid())?;
        Ok(Self {
            policy: LedgerWalletPolicy::new("Tundra".into(), Version::V2, policy),
            accounts,
            registrations: BTreeMap::new(),
        })
    }
}

pub(crate) enum Intent {
    Inspect,
    Register,
    Address { index: u32, expected: String },
    Sign { psbt: Box<Psbt> },
}
pub(crate) enum Outcome {
    Inspected,
    Registered { hmac: [u8; 32] },
    AddressMatches,
    Signed(Box<Psbt>),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Waiting,
    Complete,
    Cancelled,
    Failed,
}
#[derive(Debug, Clone)]
pub struct Update {
    pub state: State,
    /// A native caller must send these packets once, then echo this step on every read.
    pub step: u32,
    pub packets: Vec<Vec<u8>>,
    pub app_version: Option<String>,
    pub fingerprint: Option<String>,
}
#[derive(Clone, Copy)]
enum Phase {
    Info,
    Fingerprint,
    Account,
    Action,
}

pub(crate) struct Session {
    context: Context,
    intent: Option<Intent>,
    interpreter: Option<LedgerInterpreter>,
    reader: ResponseReader,
    phase: Phase,
    account: Option<Account>,
    started: Instant,
    bytes: usize,
    update: Update,
    outcome: Option<Outcome>,
}
fn invalid() -> Error {
    Error::InvalidInput("invalid or unexpected Ledger response")
}
fn protocol_error(error: bhwi::common::Error) -> Error {
    match error {
        bhwi::common::Error::UserCancelled => {
            Error::InvalidInput("hardware operation cancelled on the device")
        }
        _ => invalid(),
    }
}

impl Session {
    pub(crate) fn start(context: Context, intent: Intent) -> Result<Self> {
        if context.accounts.is_empty()
            || context.accounts.len() > 3
            || context.policy.version != Version::V2
        {
            return Err(invalid());
        }
        let mut result = Self {
            context,
            intent: Some(intent),
            interpreter: None,
            reader: ResponseReader::default(),
            phase: Phase::Info,
            account: None,
            started: Instant::now(),
            bytes: 0,
            update: Update {
                state: State::Waiting,
                step: 0,
                packets: vec![],
                app_version: None,
                fingerprint: None,
            },
            outcome: None,
        };
        result.command(Command::GetVersion)?;
        Ok(result)
    }
    pub(crate) fn progress(&mut self) -> Update {
        if self.update.state == State::Waiting && self.started.elapsed() > LIFETIME {
            self.close(State::Failed);
        }
        self.update.clone()
    }
    pub(crate) fn cancel(&mut self) -> Update {
        self.close(State::Cancelled);
        self.update.clone()
    }
    fn close(&mut self, state: State) {
        self.update.state = state;
        self.update.packets.clear();
        self.interpreter = None;
        self.intent = None;
        self.outcome = None;
        self.reader = ResponseReader::default();
    }
    fn transmit(&mut self, transmit: Transmit) -> Result<()> {
        if !matches!(transmit.recipient, Recipient::Device)
            || transmit.encrypted
            || self.update.step >= MAX_EXCHANGES
        {
            return Err(invalid());
        }
        self.update.packets = command_reports(&transmit.payload)?;
        self.update.step += 1;
        self.reader = ResponseReader::default();
        Ok(())
    }
    fn command(&mut self, command: Command) -> Result<()> {
        let mut interpreter = LedgerInterpreter::default();
        let transmit = interpreter.start(command).map_err(protocol_error)?;
        self.transmit(transmit)?;
        self.interpreter = Some(interpreter);
        Ok(())
    }
    pub(crate) fn receive(&mut self, step: u32, report: &[u8]) -> Result<Update> {
        let result = self.receive_inner(step, report);
        if result.is_err() {
            self.close(State::Failed);
        }
        result.map(|()| self.update.clone())
    }
    fn receive_inner(&mut self, step: u32, report: &[u8]) -> Result<()> {
        if self.progress().state != State::Waiting || step != self.update.step {
            return Err(invalid());
        }
        self.bytes = self.bytes.checked_add(report.len()).ok_or_else(invalid)?;
        if self.bytes > MAX_RECEIVED_BYTES {
            return Err(invalid());
        }
        self.update.packets.clear();
        let Some(response) = self.reader.receive(report)? else {
            return Ok(());
        };
        self.preflight(&response)?;
        let interpreter = self.interpreter.as_mut().ok_or_else(invalid)?;
        if let Some(transmit) = interpreter.exchange(response).map_err(protocol_error)? {
            return self.transmit(transmit);
        }
        let response = self
            .interpreter
            .take()
            .ok_or_else(invalid)?
            .end()
            .map_err(protocol_error)?;
        match (self.phase, response) {
            (Phase::Info, Response::Info(info)) => {
                if info.firmware.as_deref() != Some("Bitcoin Test") {
                    return Err(Error::NetworkMismatch);
                }
                self.update.app_version = Some(info.version);
                self.phase = Phase::Fingerprint;
                self.command(Command::GetMasterFingerprint)
            }
            (Phase::Fingerprint, Response::MasterFingerprint(fingerprint)) => {
                let account = self
                    .context
                    .accounts
                    .iter()
                    .find(|a| a.fingerprint == fingerprint)
                    .ok_or(Error::InvalidInput(
                        "connected hardware does not match this wallet",
                    ))?
                    .clone();
                self.update.fingerprint = Some(fingerprint.to_string());
                self.phase = Phase::Account;
                let path = account.path.clone();
                self.account = Some(account);
                self.command(Command::GetXpub {
                    path,
                    display: false,
                })
            }
            (Phase::Account, Response::Xpub(xpub)) => {
                if self.account.as_ref().is_none_or(|a| a.xpub != xpub) {
                    return Err(Error::InvalidInput(
                        "hardware public account does not match this wallet",
                    ));
                }
                self.phase = Phase::Action;
                match self.intent.as_ref().ok_or_else(invalid)? {
                    Intent::Inspect => self.finish(Outcome::Inspected),
                    Intent::Register => self.command(Command::RegisterWallet {
                        name: self.context.policy.name.clone(),
                        policy: self.context.policy.policy.clone(),
                    }),
                    Intent::Address { index, .. } => self.command(Command::DisplayAddress(
                        DisplayAddress::ByDescriptor {
                            index: *index,
                            change: false,
                            display: true,
                            descriptor_name: self.context.policy.name.clone(),
                        },
                        Some(DeviceContext::Ledger {
                            wallet_policy: self.context.policy.clone(),
                            wallet_hmac: Some(self.registration()?),
                        }),
                    )),
                    Intent::Sign { psbt } => self.command(Command::SignTx(
                        (**psbt).clone(),
                        Some(DeviceContext::Ledger {
                            wallet_policy: self.context.policy.clone(),
                            wallet_hmac: Some(self.registration()?),
                        }),
                    )),
                }
            }
            (Phase::Action, Response::WalletRegistration(registration))
                if matches!(self.intent, Some(Intent::Register)) =>
            {
                let bhwi::common::WalletRegistration::Complete { hmac: Some(hmac) } = registration
                else {
                    return Err(invalid());
                };
                self.finish(Outcome::Registered { hmac })
            }
            (Phase::Action, Response::Address(address)) => {
                if !matches!(&self.intent, Some(Intent::Address { expected, .. }) if *expected == address)
                {
                    return Err(Error::InvalidInput(
                        "hardware address does not match the issued wallet address",
                    ));
                }
                self.finish(Outcome::AddressMatches)
            }
            (Phase::Action, Response::SignedPsbt(psbt))
                if matches!(self.intent, Some(Intent::Sign { .. })) =>
            {
                self.finish(Outcome::Signed(Box::new(psbt)))
            }
            _ => Err(invalid()),
        }
    }
    fn registration(&self) -> Result<[u8; 32]> {
        let fingerprint = self.account.as_ref().ok_or_else(invalid)?.fingerprint;
        self.context
            .registrations
            .get(&fingerprint)
            .copied()
            .ok_or(Error::InvalidInput(
                "register this wallet policy on the connected hardware first",
            ))
    }
    pub(crate) fn fail(&mut self) {
        self.close(State::Failed);
    }
    fn finish(&mut self, outcome: Outcome) -> Result<()> {
        self.update.state = State::Complete;
        self.update.packets.clear();
        self.intent = None;
        self.outcome = Some(outcome);
        Ok(())
    }
    pub(crate) fn take_outcome(&mut self) -> Result<Outcome> {
        if self.update.state != State::Complete {
            return Err(invalid());
        }
        self.outcome.take().ok_or_else(invalid)
    }
    fn preflight(&self, response: &[u8]) -> Result<()> {
        if response.len() < 2 {
            return Err(invalid());
        }
        let (data, status) = response.split_at(response.len() - 2);
        if status != [0x90, 0x00] {
            return match (self.phase, status) {
                (_, [0x69, 0x85] | [0x69, 0x82]) => Err(Error::InvalidInput(
                    "hardware operation cancelled on the device",
                )),
                (Phase::Action, [0xe0, 0x00]) | (Phase::Account, [0x6a, 0x82]) => Ok(()),
                _ => Err(invalid()),
            };
        }
        match self.phase {
            Phase::Info => {
                let mut rest = data;
                if rest.first() != Some(&1) {
                    return Err(invalid());
                }
                rest = &rest[1..];
                let mut fields = Vec::new();
                for _ in 0..3 {
                    let size = usize::from(*rest.first().ok_or_else(invalid)?);
                    rest = &rest[1..];
                    if size > 32 || rest.len() < size {
                        return Err(invalid());
                    }
                    fields.push(&rest[..size]);
                    rest = &rest[size..];
                }
                if !rest.is_empty()
                    || fields[0] != b"Bitcoin Test"
                    || fields[1].is_empty()
                    || !fields[1]
                        .iter()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))
                {
                    return Err(invalid());
                }
            }
            Phase::Fingerprint if data.len() != 4 => return Err(invalid()),
            Phase::Account if data.len() > 112 => return Err(invalid()),
            Phase::Action if matches!(self.intent, Some(Intent::Register)) => {
                if data.len() != 64
                    || data[..32] != self.context.policy.id().map_err(|_| invalid())?
                {
                    return Err(invalid());
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;
