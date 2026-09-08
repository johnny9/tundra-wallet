//! The mobile apps depend on these dependency-independent, Tundra-owned types only.
use std::sync::Arc;
use tundra_core as core;
uniffi::setup_scaffolding!();

#[derive(Debug,Clone,Copy,uniffi::Enum)]
pub enum Chain { Mainnet,Signet,Testnet,Regtest }
impl From<Chain> for core::Network {fn from(n:Chain)->Self{match n{Chain::Mainnet=>Self::Mainnet,Chain::Signet=>Self::Signet,Chain::Testnet=>Self::Testnet,Chain::Regtest=>Self::Regtest}}}
impl From<core::Network> for Chain {fn from(n:core::Network)->Self{match n{core::Network::Mainnet=>Self::Mainnet,core::Network::Signet=>Self::Signet,core::Network::Testnet=>Self::Testnet,core::Network::Regtest=>Self::Regtest}}}
#[derive(Debug,Clone,Copy,uniffi::Enum)]
pub enum WalletPolicy { SingleSig,TwoOfThree }
impl From<core::Policy> for WalletPolicy{fn from(p:core::Policy)->Self{match p{core::Policy::SingleSig=>Self::SingleSig,core::Policy::TwoOfThree=>Self::TwoOfThree}}}
#[derive(Debug,Clone,Copy,uniffi::Enum)]
pub enum CoinState { Available,Frozen,Reserved,Pending,CoinbaseUnsupported }
impl From<core::CoinStatus> for CoinState{fn from(p:core::CoinStatus)->Self{match p{core::CoinStatus::Available=>Self::Available,core::CoinStatus::Frozen=>Self::Frozen,core::CoinStatus::Reserved=>Self::Reserved,core::CoinStatus::Pending=>Self::Pending,core::CoinStatus::CoinbaseUnsupported=>Self::CoinbaseUnsupported}}}
#[derive(Debug,Clone,uniffi::Record)]
pub struct WalletInfo{pub id:String,pub name:String,pub network:Chain,pub policy:WalletPolicy,pub synced:bool,pub total_sats:Option<u64>,pub available_sats:Option<u64>}
impl From<core::WalletSummary> for WalletInfo{fn from(w:core::WalletSummary)->Self{Self{id:w.id,name:w.name,network:w.network.into(),policy:w.policy.into(),synced:w.synced_at.is_some(),total_sats:w.total_sats,available_sats:w.available_sats}}}
#[derive(Debug,Clone,uniffi::Record)]
pub struct WalletPreview{pub policy:WalletPolicy,pub network:Chain,pub first_address:String}
#[derive(Debug,Clone,uniffi::Record)]
pub struct CoinInfo{pub outpoint:String,pub sats:u64,pub label:String,pub address:String,pub state:CoinState}
impl From<core::Coin> for CoinInfo{fn from(c:core::Coin)->Self{Self{outpoint:c.outpoint,sats:c.sats,label:c.label,address:c.address,state:c.status.into()}}}
#[derive(Debug,Clone,uniffi::Record)]
pub struct ActivityInfo{pub txid:String,pub label:String,pub delta_sats:i64,pub confirmed:bool}
#[derive(Debug,Clone,uniffi::Record)]
pub struct AddressInfo{pub address:String,pub index:u32,pub hardware_verified:bool}
#[derive(Debug,Clone,uniffi::Record)]
pub struct LabelImportPreview{pub matched:u32,pub skipped:u32,pub changed:u32}
#[derive(Debug,Clone,uniffi::Enum)]
pub enum PaymentIntent{Send{address:String,sats:u64},SendMax{address:String},Consolidate{privacy_acknowledged:bool}}
#[derive(Debug,Clone,uniffi::Record)]
pub struct PaymentRequest{pub wallet_id:String,pub intent:PaymentIntent,pub selected_outpoints:Option<Vec<String>>,pub fee_sat_per_vb:u64,pub label:String}
#[derive(Debug,Clone,uniffi::Record)]
pub struct InputInfo{pub outpoint:String,pub sats:u64,pub label:String}
#[derive(Debug,Clone,uniffi::Record)]
pub struct OutputInfo{pub address:String,pub sats:u64,pub is_change:bool,pub is_mine:bool}
#[derive(Debug,Clone,uniffi::Record)]
pub struct PaymentReview{pub id:String,pub wallet_id:String,pub inputs:Vec<InputInfo>,pub outputs:Vec<OutputInfo>,pub fee_sats:u64,pub label:String,pub is_consolidation:bool,pub state:String}
impl From<core::DraftReview> for PaymentReview{fn from(r:core::DraftReview)->Self{Self{id:r.id,wallet_id:r.wallet_id,
    inputs:r.inputs.into_iter().map(|i|InputInfo{outpoint:i.outpoint,sats:i.sats,label:i.label}).collect(),
    outputs:r.outputs.into_iter().map(|o|OutputInfo{address:o.address,sats:o.sats,is_change:o.is_change,is_mine:o.is_mine}).collect(),
    fee_sats:r.fee_sats,label:r.label,is_consolidation:r.is_consolidation,state:r.state}}}
#[derive(Debug,Clone,Copy,uniffi::Enum)]
pub enum ErrorCode{InvalidInput,Unsupported,NotFound,Conflict,Storage,Unavailable,Internal}
#[derive(Debug,thiserror::Error,uniffi::Error)]
pub enum AppError{#[error("{message}")] Operation{code:ErrorCode,message:String}}
impl From<core::Error> for AppError{fn from(e:core::Error)->Self{
    let code=match &e{
        core::Error::UnsupportedKeys|core::Error::UnsupportedPolicy=>ErrorCode::Unsupported,
        core::Error::NotFound=>ErrorCode::NotFound,
        core::Error::AlreadyExists|core::Error::UnavailableCoin=>ErrorCode::Conflict,
        core::Error::Storage=>ErrorCode::Storage,core::Error::Unavailable(_)=>ErrorCode::Unavailable,
        core::Error::CorruptState|core::Error::Poisoned=>ErrorCode::Internal,_=>ErrorCode::InvalidInput};
    Self::Operation{code,message:e.to_string()}
}}
type Result<T>=std::result::Result<T,AppError>;

#[derive(uniffi::Object)]
pub struct Tundra { core:core::Core }
#[uniffi::export]
impl Tundra{
    #[uniffi::constructor]
    pub fn open(path:String)->Result<Arc<Self>>{Ok(Arc::new(Self{core:core::Core::open(path)?}))}
    pub fn wallets(&self)->Result<Vec<WalletInfo>>{Ok(self.core.wallets()?.into_iter().map(Into::into).collect())}
    pub fn preview_import(&self,payload:String,network:Chain)->Result<WalletPreview>{let p=self.core.preview_import(&payload,network.into())?;Ok(WalletPreview{policy:p.policy.into(),network:p.network.into(),first_address:p.first_address})}
    pub fn import_wallet(&self,name:String,payload:String,network:Chain)->Result<WalletInfo>{Ok(self.core.import_wallet(&name,&payload,network.into())?.into())}
    pub fn receive_address(&self,wallet_id:String)->Result<AddressInfo>{let a=self.core.receive_address(&wallet_id)?;Ok(AddressInfo{address:a.address,index:a.index,hardware_verified:a.hardware_verified})}
    pub fn coins(&self,wallet_id:String)->Result<Vec<CoinInfo>>{Ok(self.core.coins(&wallet_id)?.into_iter().map(Into::into).collect())}
    pub fn activity(&self,wallet_id:String)->Result<Vec<ActivityInfo>>{Ok(self.core.activity(&wallet_id)?.into_iter().map(|a|ActivityInfo{txid:a.txid,label:a.label,delta_sats:a.delta_sats,confirmed:a.confirmed}).collect())}
    pub fn set_label(&self,wallet_id:String,kind:String,reference:String,label:String)->Result<()>{Ok(self.core.set_label(&wallet_id,&kind,&reference,&label)?)}
    pub fn set_frozen(&self,wallet_id:String,outpoint:String,frozen:bool)->Result<()>{Ok(self.core.set_frozen(&wallet_id,&outpoint,frozen)?)}
    pub fn import_labels(&self,wallet_id:String,payload:String,apply:bool)->Result<LabelImportPreview>{let p=self.core.import_labels(&wallet_id,&payload,apply)?;Ok(LabelImportPreview{matched:p.matched,skipped:p.skipped,changed:p.changed})}
    pub fn export_labels(&self,wallet_id:String)->Result<String>{Ok(self.core.export_labels(&wallet_id)?)}
    pub fn create_draft(&self,request:PaymentRequest)->Result<PaymentReview>{
        let payment=match request.intent{PaymentIntent::Send{address,sats}=>core::Payment::Send{address,sats},PaymentIntent::SendMax{address}=>core::Payment::SendMax{address},PaymentIntent::Consolidate{privacy_acknowledged}=>core::Payment::Consolidate{privacy_acknowledged}};
        Ok(self.core.create_draft(core::DraftRequest{wallet_id:request.wallet_id,payment,selected_outpoints:request.selected_outpoints,fee_sat_per_vb:request.fee_sat_per_vb,label:request.label})?.into())
    }
    pub fn drafts(&self,wallet_id:String)->Result<Vec<PaymentReview>>{Ok(self.core.drafts(&wallet_id)?.into_iter().map(Into::into).collect())}
    pub fn export_unsigned_psbt(&self,wallet_id:String,draft_id:String)->Result<String>{Ok(self.core.export_unsigned_psbt(&wallet_id,&draft_id)?)}
    pub fn discard_draft(&self,wallet_id:String,draft_id:String)->Result<()>{Ok(self.core.discard_draft(&wallet_id,&draft_id)?)}
}
#[uniffi::export]
pub fn format_balance(sats:u64)->String{core::amount::format_btc(sats)}
#[uniffi::export]
pub fn parse_btc_amount(value:String)->Result<u64>{Ok(core::amount::parse_btc(&value)?)}
