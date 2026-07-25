use soroban_sdk::{contracttype, Address, BytesN};

/// Status of a payment stream.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamStatus {
    /// Stream is currently active and tokens are flowing.
    Active,
    /// Stream was cancelled before its natural end time.
    Cancelled,
    /// Stream reached its end time naturally.
    Completed,
}

/// Represents a single payment stream.
#[contracttype]
#[derive(Clone, Debug)]
pub struct Stream {
    /// Unique stream identifier.
    pub id: u64,
    /// Address of the stream creator / payer.
    pub sender: Address,
    /// Address of the stream beneficiary.
    pub recipient: Address,
    /// SAC-compatible token contract address (e.g. USDC).
    pub token: Address,
    /// Total token deposit locked in the contract (in stroops).
    pub deposit: i128,
    /// Tokens released per second (stroops/second).
    pub flow_rate: i128,
    /// Ledger timestamp when the stream started.
    pub start_time: u64,
    /// Ledger timestamp when the stream ends.
    pub end_time: u64,
    /// Ledger timestamp of the last withdrawal.
    pub last_withdraw_time: u64,
    /// Current status of the stream.
    pub status: StreamStatus,
    /// Whether the stream auto-renews on completion.
    pub auto_renew: bool,
    /// Optimistic concurrency version counter. Starts at 1, increments on every write.
    /// Used to prevent lost-update races (issue #236).
    pub version: u32,
}

/// SEP-0010 authentication payload for classic Stellar account streaming (issue #235).
///
/// Classic Stellar keypair accounts cannot call Soroban contracts directly. Instead
/// they sign a challenge payload off-chain (per SEP-0010) and submit this struct
/// as proof of account ownership. The contract verifies the Ed25519 signature,
/// checks the nonce for replay protection, and validates the expiration timestamp.
#[contracttype]
#[derive(Clone, Debug)]
pub struct StellarAuth {
    /// The classic Stellar account public key (G… address on Stellar network).
    pub account: Address,
    /// Unique nonce to prevent replay attacks. Must not have been used before.
    pub nonce: BytesN<32>,
    /// Unix timestamp (seconds) after which this auth payload is considered expired.
    pub expires_at: u64,
    /// Ed25519 signature over `sha256(account || nonce || expires_at)`.
    pub signature: BytesN<64>,
}
