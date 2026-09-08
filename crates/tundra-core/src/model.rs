use crate::{Error, Result};
use bdk_wallet::bitcoin;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Network {
    Mainnet,
    Signet,
    Testnet,
    Regtest,
}
impl Network {
    pub fn bitcoin(self) -> bitcoin::Network {
        match self {
            Self::Mainnet => bitcoin::Network::Bitcoin,
            Self::Signet => bitcoin::Network::Signet,
            Self::Testnet => bitcoin::Network::Testnet,
            Self::Regtest => bitcoin::Network::Regtest,
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Mainnet => "mainnet",
            Self::Signet => "signet",
            Self::Testnet => "testnet",
            Self::Regtest => "regtest",
        }
    }
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "mainnet" => Ok(Self::Mainnet),
            "signet" => Ok(Self::Signet),
            "testnet" => Ok(Self::Testnet),
            "regtest" => Ok(Self::Regtest),
            _ => Err(Error::InvalidInput("network")),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Policy {
    SingleSig,
    TwoOfThree,
}
impl Policy {
    pub fn key(self) -> &'static str {
        match self {
            Self::SingleSig => "single_sig",
            Self::TwoOfThree => "two_of_three",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreview {
    pub id: String,
    pub network: Network,
    pub policy: Policy,
    pub receive_descriptor: String,
    pub change_descriptor: String,
    pub first_address: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletSummary {
    pub id: String,
    pub name: String,
    pub network: Network,
    pub policy: Policy,
    /// None means no successful scan. Do not present this as an empty wallet.
    pub synced_at: Option<u64>,
    pub total_sats: Option<u64>,
    pub available_sats: Option<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiveAddress {
    pub address: String,
    pub index: u32,
    pub hardware_verified: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoinStatus {
    Available,
    Frozen,
    Reserved,
    Pending,
    Immature,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coin {
    pub outpoint: String,
    pub sats: u64,
    pub label: String,
    pub address: String,
    pub status: CoinStatus,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity {
    pub txid: String,
    pub label: String,
    pub delta_sats: i64,
    pub confirmed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum Payment {
    Send { address: String, sats: u64 },
    SendMax { address: String },
    Consolidate { privacy_acknowledged: bool },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftRequest {
    pub wallet_id: String,
    pub payment: Payment,
    /// None means automatic selection. Some(vec![]) is an error, not automatic.
    pub selected_outpoints: Option<Vec<String>>,
    pub fee_sat_per_kwu: u64,
    pub label: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewedInput {
    pub outpoint: String,
    pub sats: u64,
    pub label: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewedOutput {
    pub address: String,
    pub sats: u64,
    pub is_change: bool,
    pub is_mine: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftReview {
    pub id: String,
    pub wallet_id: String,
    pub inputs: Vec<ReviewedInput>,
    pub outputs: Vec<ReviewedOutput>,
    pub fee_sats: u64,
    /// None for older stored drafts which predate normalized fee-rate review.
    #[serde(default)]
    pub fee_sat_per_kwu: Option<u64>,
    pub label: String,
    pub is_consolidation: bool,
    /// unsigned, partially_signed, signed, finalized, or invalidated. None means broadcast.
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputSigningProgress {
    pub outpoint: String,
    pub valid_signatures: u32,
    pub required_signatures: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningProgress {
    pub draft_id: String,
    pub inputs: Vec<InputSigningProgress>,
    pub complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalizedReview {
    pub wallet_id: String,
    pub draft_id: String,
    pub txid: String,
    pub wtxid: String,
    pub fee_sats: u64,
    pub weight_wu: u64,
    pub vsize: u64,
    /// Exact transaction bytes, including the verified witnesses; private wallet metadata.
    pub transaction_bytes: Vec<u8>,
}
