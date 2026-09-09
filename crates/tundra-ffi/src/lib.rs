//! The mobile apps depend on these dependency-independent, Tundra-owned types only.
use std::sync::{Arc, Mutex};
use tundra_core as core;
uniffi::setup_scaffolding!();

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum StorageFile {
    Missing,
    LegacyPlaintext,
    ProtectedOrUnknown,
}

/// Read-only hint for retaining native storage keys; not a database integrity verdict.
#[uniffi::export]
pub fn inspect_storage(path: String) -> Result<StorageFile> {
    Ok(match core::storage_format(path)? {
        core::StorageFormat::Missing => StorageFile::Missing,
        core::StorageFormat::Plaintext => StorageFile::LegacyPlaintext,
        core::StorageFormat::ProtectedOrUnknown => StorageFile::ProtectedOrUnknown,
    })
}

/// The caller must durably retain this storage key before requesting the upgrade.
#[uniffi::export]
pub fn upgrade_storage(path: String, storage_key: Vec<u8>) -> Result<()> {
    Ok(core::migrate_plaintext_storage(path, storage_key)?)
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BackupInfo {
    pub created_at: u64,
    pub wallets: Vec<WalletInfo>,
    pub drafts: u32,
    pub submissions: u32,
}
impl From<core::BackupSummary> for BackupInfo {
    fn from(value: core::BackupSummary) -> Self {
        Self {
            created_at: value.created_at,
            wallets: value.wallets.into_iter().map(Into::into).collect(),
            drafts: value.drafts,
            submissions: value.submissions,
        }
    }
}
/// Read-only inspection; this does not restore or approve any stored transaction.
#[uniffi::export]
pub fn inspect_backup(path: String, password: String) -> Result<BackupInfo> {
    Ok(core::inspect_backup(path, password)?.into())
}

/// The destination must be new, with its storage key durably retained by the caller.
#[uniffi::export]
pub fn restore_backup(
    source: String,
    destination: String,
    password: String,
    storage_key: Vec<u8>,
) -> Result<BackupInfo> {
    Ok(core::restore_backup(source, destination, password, storage_key)?.into())
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RecoveryReviewRequest {
    pub wallet_id: String,
    pub draft_id: String,
    pub expected_txid: String,
    pub expected_attempt: u64,
    pub review_acknowledged: bool,
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum Chain {
    Mainnet,
    Signet,
    Testnet,
    Regtest,
}
impl From<Chain> for core::Network {
    fn from(n: Chain) -> Self {
        match n {
            Chain::Mainnet => Self::Mainnet,
            Chain::Signet => Self::Signet,
            Chain::Testnet => Self::Testnet,
            Chain::Regtest => Self::Regtest,
        }
    }
}
impl From<core::Network> for Chain {
    fn from(n: core::Network) -> Self {
        match n {
            core::Network::Mainnet => Self::Mainnet,
            core::Network::Signet => Self::Signet,
            core::Network::Testnet => Self::Testnet,
            core::Network::Regtest => Self::Regtest,
        }
    }
}
#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum WalletPolicy {
    SingleSig,
    TwoOfThree,
}
impl From<core::Policy> for WalletPolicy {
    fn from(p: core::Policy) -> Self {
        match p {
            core::Policy::SingleSig => Self::SingleSig,
            core::Policy::TwoOfThree => Self::TwoOfThree,
        }
    }
}
#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum CoinState {
    Available,
    Frozen,
    Reserved,
    Pending,
    Immature,
}
impl From<core::CoinStatus> for CoinState {
    fn from(p: core::CoinStatus) -> Self {
        match p {
            core::CoinStatus::Available => Self::Available,
            core::CoinStatus::Frozen => Self::Frozen,
            core::CoinStatus::Reserved => Self::Reserved,
            core::CoinStatus::Pending => Self::Pending,
            core::CoinStatus::Immature => Self::Immature,
        }
    }
}
#[derive(Debug, Clone, uniffi::Record)]
pub struct WalletInfo {
    pub id: String,
    pub name: String,
    pub network: Chain,
    pub policy: WalletPolicy,
    pub synced: bool,
    pub synced_at: Option<u64>,
    pub total_sats: Option<u64>,
    pub available_sats: Option<u64>,
}
impl From<core::WalletSummary> for WalletInfo {
    fn from(w: core::WalletSummary) -> Self {
        Self {
            id: w.id,
            name: w.name,
            network: w.network.into(),
            policy: w.policy.into(),
            synced: w.synced_at.is_some(),
            synced_at: w.synced_at,
            total_sats: w.total_sats,
            available_sats: w.available_sats,
        }
    }
}
#[derive(Debug, Clone, uniffi::Record)]
pub struct WalletPreview {
    pub policy: WalletPolicy,
    pub network: Chain,
    pub first_address: String,
}
#[derive(Debug, Clone, uniffi::Record)]
pub struct CoinInfo {
    pub outpoint: String,
    pub sats: u64,
    pub label: String,
    pub address: String,
    pub state: CoinState,
}
impl From<core::Coin> for CoinInfo {
    fn from(c: core::Coin) -> Self {
        Self {
            outpoint: c.outpoint,
            sats: c.sats,
            label: c.label,
            address: c.address,
            state: c.status.into(),
        }
    }
}
#[derive(Debug, Clone, uniffi::Record)]
pub struct ActivityInfo {
    pub txid: String,
    pub label: String,
    pub delta_sats: i64,
    pub confirmed: bool,
}
#[derive(Debug, Clone, uniffi::Record)]
pub struct AddressInfo {
    pub address: String,
    pub index: u32,
    pub hardware_verified: bool,
}
#[derive(Debug, Clone, uniffi::Record)]
pub struct LabelImportPreview {
    pub matched: u32,
    pub skipped: u32,
    pub changed: u32,
}
#[derive(Debug, Clone, uniffi::Enum)]
pub enum PaymentIntent {
    Send { address: String, sats: u64 },
    SendMax { address: String },
    Consolidate { privacy_acknowledged: bool },
}
#[derive(Debug, Clone, uniffi::Record)]
pub struct PaymentRequest {
    pub wallet_id: String,
    pub intent: PaymentIntent,
    pub selected_outpoints: Option<Vec<String>>,
    pub fee_sat_per_kwu: u64,
    pub label: String,
}
#[derive(Debug, Clone, uniffi::Record)]
pub struct InputInfo {
    pub outpoint: String,
    pub sats: u64,
    pub label: String,
}
#[derive(Debug, Clone, uniffi::Record)]
pub struct OutputInfo {
    pub address: String,
    pub sats: u64,
    pub is_change: bool,
    pub is_mine: bool,
}
#[derive(Debug, Clone, uniffi::Record)]
pub struct PaymentReview {
    pub id: String,
    pub wallet_id: String,
    pub inputs: Vec<InputInfo>,
    pub outputs: Vec<OutputInfo>,
    pub fee_sats: u64,
    pub fee_sat_per_kwu: Option<u64>,
    pub label: String,
    pub is_consolidation: bool,
    pub state: String,
}
#[derive(Debug, Clone, uniffi::Record)]
pub struct InputSignatureInfo {
    pub outpoint: String,
    pub valid_signatures: u32,
    pub required_signatures: u32,
}
#[derive(Debug, Clone, uniffi::Record)]
pub struct SigningInfo {
    pub draft_id: String,
    pub inputs: Vec<InputSignatureInfo>,
    pub complete: bool,
}
#[derive(Debug, Clone, uniffi::Record)]
pub struct FinalTransactionInfo {
    pub wallet_id: String,
    pub draft_id: String,
    pub txid: String,
    pub wtxid: String,
    pub fee_sats: u64,
    pub weight_wu: u64,
    pub vsize: u64,
    pub transaction_bytes: Vec<u8>,
}
impl From<core::FinalizedReview> for FinalTransactionInfo {
    fn from(value: core::FinalizedReview) -> Self {
        Self {
            wallet_id: value.wallet_id,
            draft_id: value.draft_id,
            txid: value.txid,
            wtxid: value.wtxid,
            fee_sats: value.fee_sats,
            weight_wu: value.weight_wu,
            vsize: value.vsize,
            transaction_bytes: value.transaction_bytes,
        }
    }
}
#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum ChainObservation {
    NotSeen,
    Mempool,
    Confirmed,
}
#[derive(Debug, Clone, uniffi::Record)]
pub struct BroadcastRequest {
    pub wallet_id: String,
    pub draft_id: String,
    pub endpoint: String,
    pub expected_txid: String,
    pub previous_attempt: Option<u64>,
    pub privacy_consent: bool,
    pub retry_acknowledged: bool,
}
impl From<BroadcastRequest> for core::BroadcastRequest {
    fn from(value: BroadcastRequest) -> Self {
        Self {
            wallet_id: value.wallet_id,
            draft_id: value.draft_id,
            endpoint: value.endpoint,
            expected_txid: value.expected_txid,
            previous_attempt: value.previous_attempt,
            privacy_consent: value.privacy_consent,
            retry_acknowledged: value.retry_acknowledged,
        }
    }
}
#[derive(Debug, Clone, uniffi::Record)]
pub struct BroadcastInfo {
    pub attempt_id: u64,
    pub wallet_id: String,
    pub draft_id: String,
    pub endpoint: String,
    pub txid: String,
    pub wtxid: String,
    pub requested_at: u64,
    pub acknowledged: bool,
    pub observation: ChainObservation,
}
impl From<core::BroadcastInfo> for BroadcastInfo {
    fn from(value: core::BroadcastInfo) -> Self {
        Self {
            attempt_id: value.attempt_id,
            wallet_id: value.wallet_id,
            draft_id: value.draft_id,
            endpoint: value.endpoint,
            txid: value.txid,
            wtxid: value.wtxid,
            requested_at: value.requested_at,
            acknowledged: value.acknowledged,
            observation: match value.observation {
                core::BroadcastObservation::NotSeen => ChainObservation::NotSeen,
                core::BroadcastObservation::Mempool => ChainObservation::Mempool,
                core::BroadcastObservation::Confirmed => ChainObservation::Confirmed,
            },
        }
    }
}
#[derive(Debug, Clone, uniffi::Enum)]
pub enum UsbOperation {
    Inspect,
    RegisterPolicy,
    VerifyReceive { index: u32 },
    SignDraft { draft_id: String },
}
#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum UsbState {
    Waiting,
    Complete,
    Cancelled,
    Failed,
}
#[derive(Debug, Clone, uniffi::Record)]
pub struct UsbInfo {
    pub state: UsbState,
    pub step: u32,
    pub packets: Vec<Vec<u8>>,
    pub app_version: Option<String>,
    pub fingerprint: Option<String>,
}
impl From<core::usb::Update> for UsbInfo {
    fn from(value: core::usb::Update) -> Self {
        Self {
            state: match value.state {
                core::usb::State::Waiting => UsbState::Waiting,
                core::usb::State::Complete => UsbState::Complete,
                core::usb::State::Cancelled => UsbState::Cancelled,
                core::usb::State::Failed => UsbState::Failed,
            },
            step: value.step,
            packets: value.packets,
            app_version: value.app_version,
            fingerprint: value.fingerprint,
        }
    }
}
#[derive(uniffi::Object)]
pub struct UsbConnection {
    session: Mutex<core::usb::UsbSession>,
}
#[uniffi::export]
impl UsbConnection {
    pub fn progress(&self) -> Result<UsbInfo> {
        Ok(self
            .session
            .lock()
            .map_err(|_| core::Error::Poisoned)?
            .progress()
            .into())
    }
    pub fn receive(&self, step: u32, report: Vec<u8>) -> Result<UsbInfo> {
        Ok(self
            .session
            .lock()
            .map_err(|_| core::Error::Poisoned)?
            .receive(step, &report)?
            .into())
    }
    pub fn cancel(&self) -> Result<UsbInfo> {
        Ok(self
            .session
            .lock()
            .map_err(|_| core::Error::Poisoned)?
            .cancel()
            .into())
    }
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum QrEncoding {
    Ur,
    Bbqr,
}
impl From<QrEncoding> for core::qr::QrFormat {
    fn from(value: QrEncoding) -> Self {
        match value {
            QrEncoding::Ur => Self::Ur,
            QrEncoding::Bbqr => Self::Bbqr,
        }
    }
}
#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum QrPurpose {
    SignedPsbt,
    Descriptor { network: Chain },
}
#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum QrState {
    Scanning,
    Complete,
    Cancelled,
    Failed,
}
#[derive(Debug, Clone, uniffi::Record)]
pub struct QrInfo {
    pub state: QrState,
    pub resolved_fragments: u32,
    pub total_fragments: Option<u32>,
}
impl From<core::qr::QrProgress> for QrInfo {
    fn from(value: core::qr::QrProgress) -> Self {
        Self {
            state: match value.phase {
                core::qr::QrPhase::Scanning => QrState::Scanning,
                core::qr::QrPhase::Complete => QrState::Complete,
                core::qr::QrPhase::Cancelled => QrState::Cancelled,
                core::qr::QrPhase::Failed => QrState::Failed,
            },
            resolved_fragments: value.resolved_fragments,
            total_fragments: value.total_fragments,
        }
    }
}
#[derive(Debug, Clone, uniffi::Record)]
pub struct QrImage {
    pub side: u32,
    pub modules: Vec<u8>,
}

#[derive(uniffi::Object)]
pub struct QrScanner {
    decoder: Mutex<core::qr::QrDecoder>,
}
#[uniffi::export]
impl QrScanner {
    #[uniffi::constructor]
    pub fn new(purpose: QrPurpose) -> Arc<Self> {
        let purpose = match purpose {
            QrPurpose::SignedPsbt => core::qr::QrPurpose::SignedPsbt,
            QrPurpose::Descriptor { network } => core::qr::QrPurpose::Descriptor {
                network: network.into(),
            },
        };
        Arc::new(Self {
            decoder: Mutex::new(core::qr::QrDecoder::new(purpose)),
        })
    }
    pub fn receive(&self, frame: String) -> Result<QrInfo> {
        Ok(self
            .decoder
            .lock()
            .map_err(|_| core::Error::Poisoned)?
            .receive(&frame)?
            .into())
    }
    pub fn progress(&self) -> Result<QrInfo> {
        Ok(self
            .decoder
            .lock()
            .map_err(|_| core::Error::Poisoned)?
            .progress()
            .into())
    }
    pub fn cancel(&self) -> Result<QrInfo> {
        Ok(self
            .decoder
            .lock()
            .map_err(|_| core::Error::Poisoned)?
            .cancel()
            .into())
    }
    pub fn payload(&self) -> Result<Vec<u8>> {
        Ok(self
            .decoder
            .lock()
            .map_err(|_| core::Error::Poisoned)?
            .payload()?)
    }
}
impl From<core::SigningProgress> for SigningInfo {
    fn from(p: core::SigningProgress) -> Self {
        Self {
            draft_id: p.draft_id,
            inputs: p
                .inputs
                .into_iter()
                .map(|i| InputSignatureInfo {
                    outpoint: i.outpoint,
                    valid_signatures: i.valid_signatures,
                    required_signatures: i.required_signatures,
                })
                .collect(),
            complete: p.complete,
        }
    }
}
impl From<core::DraftReview> for PaymentReview {
    fn from(r: core::DraftReview) -> Self {
        Self {
            id: r.id,
            wallet_id: r.wallet_id,
            inputs: r
                .inputs
                .into_iter()
                .map(|i| InputInfo {
                    outpoint: i.outpoint,
                    sats: i.sats,
                    label: i.label,
                })
                .collect(),
            outputs: r
                .outputs
                .into_iter()
                .map(|o| OutputInfo {
                    address: o.address,
                    sats: o.sats,
                    is_change: o.is_change,
                    is_mine: o.is_mine,
                })
                .collect(),
            fee_sats: r.fee_sats,
            fee_sat_per_kwu: r.fee_sat_per_kwu,
            label: r.label,
            is_consolidation: r.is_consolidation,
            state: r.state,
        }
    }
}
#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum ErrorCode {
    InvalidInput,
    Unsupported,
    NotFound,
    Conflict,
    Storage,
    Unavailable,
    Internal,
}
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum AppError {
    // `message` conflicts with kotlin.Exception.message in generated Kotlin errors.
    #[error("{detail}")]
    Operation { code: ErrorCode, detail: String },
}
impl From<core::Error> for AppError {
    fn from(e: core::Error) -> Self {
        let code = match &e {
            core::Error::UnsupportedKeys | core::Error::UnsupportedPolicy => ErrorCode::Unsupported,
            core::Error::NotFound => ErrorCode::NotFound,
            core::Error::AlreadyExists | core::Error::UnavailableCoin => ErrorCode::Conflict,
            core::Error::Storage | core::Error::StorageLocked | core::Error::StorageBusy => {
                ErrorCode::Storage
            }
            core::Error::Unavailable(_) => ErrorCode::Unavailable,
            core::Error::CorruptState | core::Error::Poisoned => ErrorCode::Internal,
            _ => ErrorCode::InvalidInput,
        };
        Self::Operation {
            code,
            detail: e.to_string(),
        }
    }
}
type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum SyncState {
    Prepared,
    Scanning,
    Applying,
    Complete,
    Cancelled,
    Failed,
}
#[derive(Debug, Clone, uniffi::Record)]
pub struct SyncInfo {
    pub id: u64,
    pub wallet_id: String,
    pub state: SyncState,
    pub scanned_scripts: u32,
    pub error: Option<String>,
}
impl From<core::SyncProgress> for SyncInfo {
    fn from(p: core::SyncProgress) -> Self {
        Self {
            id: p.id,
            wallet_id: p.wallet_id,
            state: match p.phase {
                core::SyncPhase::Prepared => SyncState::Prepared,
                core::SyncPhase::Scanning => SyncState::Scanning,
                core::SyncPhase::Applying => SyncState::Applying,
                core::SyncPhase::Complete => SyncState::Complete,
                core::SyncPhase::Cancelled => SyncState::Cancelled,
                core::SyncPhase::Failed => SyncState::Failed,
            },
            scanned_scripts: p.scanned_scripts,
            error: p.error,
        }
    }
}

#[derive(uniffi::Object)]
pub struct Tundra {
    core: core::Core,
}
#[uniffi::export]
impl Tundra {
    #[uniffi::constructor]
    pub fn open(path: String) -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            core: core::Core::open(path)?,
        }))
    }
    #[uniffi::constructor]
    pub fn open_protected(path: String, storage_key: Vec<u8>) -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            core: core::Core::open_protected(path, storage_key)?,
        }))
    }
    pub fn wallets(&self) -> Result<Vec<WalletInfo>> {
        Ok(self.core.wallets()?.into_iter().map(Into::into).collect())
    }
    pub fn edit_coins(
        &self,
        wallet_id: String,
        outpoints: Vec<String>,
        label: Option<String>,
        frozen: Option<bool>,
    ) -> Result<()> {
        Ok(self.core.edit_coins(&wallet_id, outpoints, label, frozen)?)
    }
    pub fn prepare_sync(
        &self,
        wallet_id: String,
        endpoint: String,
        privacy_consent: bool,
    ) -> Result<SyncInfo> {
        Ok(self
            .core
            .prepare_sync(&wallet_id, &endpoint, privacy_consent)?
            .into())
    }
    pub fn run_sync(&self, operation_id: u64) -> Result<()> {
        Ok(self.core.run_sync(operation_id)?)
    }
    pub fn sync_progress(&self, operation_id: u64) -> Result<SyncInfo> {
        Ok(self.core.sync_progress(operation_id)?.into())
    }
    pub fn cancel_sync(&self, operation_id: u64) -> Result<SyncInfo> {
        Ok(self.core.cancel_sync(operation_id)?.into())
    }
    pub fn sync_endpoint(&self, wallet_id: String) -> Result<Option<String>> {
        Ok(self.core.sync_endpoint(&wallet_id)?)
    }
    pub fn preview_import(&self, payload: String, network: Chain) -> Result<WalletPreview> {
        let p = self.core.preview_import(&payload, network.into())?;
        Ok(WalletPreview {
            policy: p.policy.into(),
            network: p.network.into(),
            first_address: p.first_address,
        })
    }
    pub fn import_wallet(
        &self,
        name: String,
        payload: String,
        network: Chain,
    ) -> Result<WalletInfo> {
        Ok(self
            .core
            .import_wallet(&name, &payload, network.into())?
            .into())
    }
    pub fn receive_address(&self, wallet_id: String) -> Result<AddressInfo> {
        let a = self.core.receive_address(&wallet_id)?;
        Ok(AddressInfo {
            address: a.address,
            index: a.index,
            hardware_verified: a.hardware_verified,
        })
    }
    pub fn coins(&self, wallet_id: String) -> Result<Vec<CoinInfo>> {
        Ok(self
            .core
            .coins(&wallet_id)?
            .into_iter()
            .map(Into::into)
            .collect())
    }
    pub fn output_source(
        &self,
        wallet_id: String,
        outpoint: String,
    ) -> Result<Option<PaymentReview>> {
        Ok(self
            .core
            .output_source(&wallet_id, &outpoint)?
            .map(Into::into))
    }
    pub fn export_backup(&self, path: String, password: String) -> Result<BackupInfo> {
        Ok(self.core.export_backup(path, password)?.into())
    }
    pub fn recovery_required(&self, wallet_id: String, draft_id: String) -> Result<bool> {
        Ok(self.core.recovery_required(&wallet_id, &draft_id)?)
    }
    pub fn resume_recovered_submission(
        &self,
        request: RecoveryReviewRequest,
    ) -> Result<FinalTransactionInfo> {
        Ok(self
            .core
            .resume_recovered_submission(
                &request.wallet_id,
                &request.draft_id,
                &request.expected_txid,
                request.expected_attempt,
                request.review_acknowledged,
            )?
            .into())
    }
    pub fn activity(&self, wallet_id: String) -> Result<Vec<ActivityInfo>> {
        Ok(self
            .core
            .activity(&wallet_id)?
            .into_iter()
            .map(|a| ActivityInfo {
                txid: a.txid,
                label: a.label,
                delta_sats: a.delta_sats,
                confirmed: a.confirmed,
            })
            .collect())
    }
    pub fn set_label(
        &self,
        wallet_id: String,
        kind: String,
        reference: String,
        label: String,
    ) -> Result<()> {
        Ok(self.core.set_label(&wallet_id, &kind, &reference, &label)?)
    }
    pub fn set_frozen(&self, wallet_id: String, outpoint: String, frozen: bool) -> Result<()> {
        Ok(self.core.set_frozen(&wallet_id, &outpoint, frozen)?)
    }
    pub fn import_labels(
        &self,
        wallet_id: String,
        payload: String,
        apply: bool,
    ) -> Result<LabelImportPreview> {
        let p = self.core.import_labels(&wallet_id, &payload, apply)?;
        Ok(LabelImportPreview {
            matched: p.matched,
            skipped: p.skipped,
            changed: p.changed,
        })
    }
    pub fn export_labels(&self, wallet_id: String) -> Result<String> {
        Ok(self.core.export_labels(&wallet_id)?)
    }
    pub fn create_draft(&self, request: PaymentRequest) -> Result<PaymentReview> {
        let payment = match request.intent {
            PaymentIntent::Send { address, sats } => core::Payment::Send { address, sats },
            PaymentIntent::SendMax { address } => core::Payment::SendMax { address },
            PaymentIntent::Consolidate {
                privacy_acknowledged,
            } => core::Payment::Consolidate {
                privacy_acknowledged,
            },
        };
        Ok(self
            .core
            .create_draft(core::DraftRequest {
                wallet_id: request.wallet_id,
                payment,
                selected_outpoints: request.selected_outpoints,
                fee_sat_per_kwu: request.fee_sat_per_kwu,
                label: request.label,
            })?
            .into())
    }
    pub fn drafts(&self, wallet_id: String) -> Result<Vec<PaymentReview>> {
        Ok(self
            .core
            .drafts(&wallet_id)?
            .into_iter()
            .map(Into::into)
            .collect())
    }
    pub fn export_unsigned_psbt(&self, wallet_id: String, draft_id: String) -> Result<String> {
        Ok(self.core.export_unsigned_psbt(&wallet_id, &draft_id)?)
    }
    pub fn accept_signed_psbt(
        &self,
        wallet_id: String,
        draft_id: String,
        payload: Vec<u8>,
    ) -> Result<SigningInfo> {
        Ok(self
            .core
            .accept_signed_psbt(&wallet_id, &draft_id, &payload)?
            .into())
    }
    pub fn signing_progress(&self, wallet_id: String, draft_id: String) -> Result<SigningInfo> {
        Ok(self.core.signing_progress(&wallet_id, &draft_id)?.into())
    }
    pub fn export_signing_psbt(&self, wallet_id: String, draft_id: String) -> Result<String> {
        Ok(self.core.export_signing_psbt(&wallet_id, &draft_id)?)
    }
    pub fn prepare_usb(
        &self,
        wallet_id: String,
        operation: UsbOperation,
    ) -> Result<Arc<UsbConnection>> {
        let operation = match operation {
            UsbOperation::Inspect => core::usb::Operation::Inspect,
            UsbOperation::RegisterPolicy => core::usb::Operation::RegisterPolicy,
            UsbOperation::VerifyReceive { index } => core::usb::Operation::VerifyReceive { index },
            UsbOperation::SignDraft { draft_id } => core::usb::Operation::SignDraft { draft_id },
        };
        Ok(Arc::new(UsbConnection {
            session: Mutex::new(self.core.prepare_usb(&wallet_id, operation)?),
        }))
    }
    pub fn finalize_draft(
        &self,
        wallet_id: String,
        draft_id: String,
    ) -> Result<FinalTransactionInfo> {
        Ok(self.core.finalize_draft(&wallet_id, &draft_id)?.into())
    }
    pub fn finalized_draft(
        &self,
        wallet_id: String,
        draft_id: String,
    ) -> Result<Option<FinalTransactionInfo>> {
        Ok(self
            .core
            .finalized_draft(&wallet_id, &draft_id)?
            .map(Into::into))
    }
    pub fn export_draft_qr(
        &self,
        wallet_id: String,
        draft_id: String,
        encoding: QrEncoding,
    ) -> Result<Vec<String>> {
        let payload = self.core.export_signing_psbt(&wallet_id, &draft_id)?;
        Ok(core::qr::encode_psbt(payload.as_bytes(), encoding.into())?)
    }
    pub fn broadcast_draft(&self, request: BroadcastRequest) -> Result<BroadcastInfo> {
        Ok(self.core.broadcast_draft(request.into())?.into())
    }
    pub fn broadcast_status(
        &self,
        wallet_id: String,
        draft_id: String,
    ) -> Result<Option<BroadcastInfo>> {
        Ok(self
            .core
            .broadcast_status(&wallet_id, &draft_id)?
            .map(Into::into))
    }
    pub fn discard_draft(&self, wallet_id: String, draft_id: String) -> Result<()> {
        Ok(self.core.discard_draft(&wallet_id, &draft_id)?)
    }
}
#[uniffi::export]
pub fn format_balance(sats: u64) -> String {
    core::amount::format_btc(sats)
}
#[uniffi::export]
pub fn parse_btc_amount(value: String) -> Result<u64> {
    Ok(core::amount::parse_btc(&value)?)
}
#[uniffi::export]
pub fn parse_fee_rate(value: String) -> Result<u64> {
    Ok(core::amount::parse_fee_rate(&value)?)
}
#[uniffi::export]
pub fn format_fee_rate(sat_per_kwu: u64) -> String {
    core::amount::format_fee_rate(sat_per_kwu)
}
#[uniffi::export]
pub fn render_qr_frame(frame: String) -> Result<QrImage> {
    let matrix = core::qr::render_frame(&frame)?;
    Ok(QrImage {
        side: matrix.side,
        modules: matrix.modules,
    })
}
