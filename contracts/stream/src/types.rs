use soroban_sdk::{contracttype, Address, Bytes, BytesN, String, Vec};

/// Vesting release curve applied to a payment stream.
///
/// Choosing `Linear` reproduces the original constant-rate behaviour.
/// Choosing `TimeDecay` produces a front-weighted (convex) release schedule
/// where more tokens are claimable early in the stream lifetime.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VestingCurve {
    /// Constant rate: `claimable = flow_rate × elapsed`.
    Linear,
    /// Discretised exponential decay.
    ///
    /// `decay_factor` is expressed in **basis points per 1 000 seconds**
    /// (i.e. the per-mille decay rate per 1 ks window):
    ///
    /// ```text
    /// weight(t) = deposit × (1 − decay_factor/10_000)^(t / 1_000)
    /// cumulative_claimable(t) = deposit − weight(t)   (clamped to [0, deposit])
    /// ```
    ///
    /// A `decay_factor` of `0` degenerates to linear behaviour.
    /// Practical values: 50–500 bps (0.5 %–5 % per 1 ks window).
    TimeDecay {
        /// Decay rate in basis points per 1 000-second window (0–9 999).
        decay_factor: u32,
    },
}

/// A single step-vesting tranche: tokens that unlock atomically at `unlock_time`.
#[contracttype]
#[derive(Clone, Debug)]
pub struct VestingTranche {
    /// Ledger timestamp at which this tranche becomes claimable.
    pub unlock_time: u64,
    /// Amount of tokens (in stroops) that unlock at this timestamp.
    pub amount: i128,
}

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
    /// Stream is temporarily paused.
    Paused,
    /// Stream has passed its end_time and been explicitly marked as expired.
    Expired,
}

/// Status of a milestone.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MilestoneStatus {
    /// Milestone is pending (not yet released by sender).
    Pending,
    /// Milestone has been released and is claimable.
    Released,
    /// Milestone was forfeited (cancelled before release).
    Forfeited,
}

/// Represents a single milestone in a gated stream.
#[contracttype]
#[derive(Clone, Debug)]
pub struct Milestone {
    /// Amount of tokens for this milestone (in stroops).
    pub amount: i128,
    /// Hash of the milestone description (for reference).
    pub description_hash: BytesN<32>,
    /// Current status of the milestone.
    pub status: MilestoneStatus,
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
    /// Ledger timestamp before which no tokens are claimable (>= start_time, <= end_time).
    pub cliff_time: u64,
    /// Ledger timestamp before which no withdrawals are permitted (>= start_time, <= end_time).
    pub lock_until: u64,
    /// Ledger timestamp when the stream ends.
    pub end_time: u64,
    /// Ledger timestamp of the last withdrawal.
    pub last_withdraw_time: u64,
    /// Current status of the stream.
    pub status: StreamStatus,
    /// Whether the stream auto-renews on completion.
    pub auto_renew: bool,
    /// Whether the recipient is allowed to terminate the stream early.
    pub allow_recipient_termination: bool,
    /// Ledger timestamp of when the stream was last paused (0 if never paused).
    pub last_pause_time: u64,
    /// Total amount withdrawn from this stream so far.
    pub total_withdrawn: i128,
    /// Optional metadata blob associated with the stream.
    pub metadata: Bytes,
    /// Optional URI pointing to off-chain metadata (IPFS or HTTPS, max 128 bytes).
    pub metadata_uri: Option<String>,
    /// Optional milestones for gated release (empty if not milestone-gated).
    pub milestones: Vec<Milestone>,
    /// Reentrancy guard: true if currently processing a withdrawal to prevent re-entrance.
    pub locked: bool,
    /// Optional holdback amount kept in escrow until explicitly released (in stroops).
    /// Deducted from the streaming portion at creation time.
    pub holdback_amount: i128,
    /// Whether the holdback has been settled (released to recipient or clawed back to sender).
    pub holdback_claimed: bool,

    // ── Step-vesting (tranche) fields ────────────────────────────────────────

    /// Whether this stream uses step-vesting (tranche-based release).
    /// When `true`, token release is governed by `tranches` rather than the
    /// continuous flow rate.
    pub is_step_vesting: bool,
    /// Index of the next unclaimed tranche (cursor). Starts at 0.
    pub tranches_claimed: u32,

    // ── Oracle price-check fields ────────────────────────────────────────────

    /// Optional oracle contract address for on-chain price validation.
    /// When set, price is checked on stream creation and withdrawal.
    pub oracle: Option<Address>,
    /// Maximum allowed price deviation from the creation price, in basis points
    /// (e.g. 500 = 5 %).  Ignored when `oracle` is `None`.
    pub max_price_deviation_bps: u32,
    /// Token price (raw oracle value) recorded at stream-creation time.
    /// Used as the baseline for deviation calculations on subsequent calls.
    pub creation_price: i128,

    // ── Vesting curve ────────────────────────────────────────────────────────

    /// Release curve governing how tokens become claimable over time.
    /// Defaults to `VestingCurve::Linear` for all existing streams.
    pub curve: VestingCurve,

    // ── Withdrawal steps ─────────────────────────────────────────────────────

    /// Optional number of evenly-spaced withdrawal steps.
    ///
    /// When `Some(n)`, the stream duration is divided into `n` equal intervals
    /// of `(end_time - start_time) / n` seconds each.  Recipients may only call
    /// `withdraw` at or after the boundary of the next unclaimed step.
    /// `None` means free-form withdrawal (default behaviour).
    pub withdrawal_steps: Option<u32>,

    /// Index of the last completed withdrawal step (0-based).
    /// Starts at 0; incremented each time a step boundary is crossed.
    /// Only meaningful when `withdrawal_steps` is `Some`.
    pub current_step: u32,

    // ── Minimum withdrawal amount ─────────────────────────────────────────────

    /// Optional minimum claimable amount required before a withdrawal is accepted.
    ///
    /// When `Some(floor)`, `withdraw` rejects any call where the claimable
    /// amount is below `floor` — unless it is the final claim (i.e. the full
    /// remaining deposit is being drained), in which case the floor is bypassed.
    /// `None` means no minimum (default behaviour).
    pub min_withdrawal_amount: Option<i128>,

    // ── Pause accounting ──────────────────────────────────────────────────────

    /// Total number of seconds this stream has spent in the Paused state.
    ///
    /// Accumulated on every `resume_stream` call:
    ///   `paused_duration_seconds += now - last_pause_time`
    ///
    /// Used to offset the `elapsed` window in claimable / refund calculations
    /// so that paused time is never counted as streamed time.  Starts at 0.
    pub paused_duration_seconds: u64,

    // ── Claim-frequency throttle ──────────────────────────────────────────────

    /// Optional minimum number of ledgers that must pass between two successful
    /// `withdraw` calls.
    ///
    /// When `Some(n)`, a `withdraw` call is rejected with
    /// `StreamError::ClaimTooFrequent` if fewer than `n` ledgers have elapsed
    /// since `last_claim_ledger`.  The final claim (draining the full remaining
    /// balance) always bypasses this restriction.
    /// `None` means no frequency limit (default behaviour).
    pub min_claim_interval_ledgers: Option<u32>,

    /// Ledger sequence number of the most recent successful `withdraw` call.
    ///
    /// Initialised to `0` at stream creation.  Updated after every withdrawal
    /// that moves tokens (including the final claim).
    /// Only meaningful when `min_claim_interval_ledgers` is `Some`.
    pub last_claim_ledger: u32,
}

/// Health status of a stream's on-chain storage entry, based on its TTL.
///
/// Thresholds:
/// - `Healthy`    — TTL remaining >= 10,000 ledgers
/// - `TTLWarning` — TTL remaining in [1,000 .. 10,000) ledgers
/// - `AtRisk`     — TTL remaining < 1,000 ledgers (eviction imminent)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HealthStatus {
    /// Stream storage TTL is comfortable (>= 10,000 ledgers remaining).
    Healthy,
    /// Stream storage TTL is getting low (< 10,000 ledgers remaining).
    /// Clients should consider calling `bump_stream_ttl` soon.
    TTLWarning,
    /// Stream storage TTL is critically low (< 1,000 ledgers remaining).
    /// The stream is at risk of being evicted from the ledger.
    AtRisk,
}

/// Snapshot of a stream's on-chain storage health.
///
/// Returned by `get_stream_health(stream_id)`.
#[contracttype]
#[derive(Clone, Debug)]
pub struct StreamHealth {
    /// Current ledger sequence number at the time of the query.
    pub current_ledger: u32,
    /// Stream end timestamp (Unix seconds).
    pub end_time: u64,
    /// Ledgers remaining before the stream's persistent storage entry expires.
    /// A value of 0 means the TTL information could not be determined
    /// (e.g. the entry has already expired or the query is not supported).
    pub ttl_remaining_ledgers: u32,
    /// Derived health classification based on `ttl_remaining_ledgers`.
    pub status: HealthStatus,
    // ── Feature (a): StreamExpiryWarning ─────────────────────────────────────

    /// Whether the `StreamExpiryWarning` event has already been emitted for this
    /// stream in the current expiry window.  Prevents duplicate warnings when
    /// multiple interactions occur before `end_time`.
    pub expiry_warning_emitted: bool,

    // ── Feature (c): Stream redirect ─────────────────────────────────────────

    /// Optional ID of the stream that claimed tokens should be forwarded into.
    /// When set, a `withdraw` call on this stream will top-up the target stream
    /// instead of transferring tokens directly to the recipient.
    /// The target stream's recipient must equal this stream's recipient.
    pub redirect_to_stream_id: Option<u64>,

    // ── Feature (d): Dual-token streams ──────────────────────────────────────

    /// Whether this stream is a dual-token stream.
    /// When `true`, a second token (`token2`) and second deposit (`deposit2`) are
    /// stored separately in persistent storage.  The `token` and `deposit` fields
    /// represent the primary (first) token allocation as usual.
    pub is_dual_stream: bool,
}

/// Aggregate contract statistics.
#[contracttype]
#[derive(Clone, Debug)]
pub struct Stats {
    /// Total number of streams ever created.
    pub total_streams: u64,
    /// Number of currently active streams.
    pub active_streams: u64,
    /// Sum of all deposits in stroops.
    pub total_volume: i128,
}

/// A single admin audit log entry.
#[contracttype]
#[derive(Clone, Debug)]
pub struct AuditEntry {
    /// Name of the admin instruction (e.g. "emergency_pause").
    pub instruction: String,
    /// Admin address that performed the action.
    pub admin: Address,
    /// Ledger timestamp of the action.
    pub timestamp: u64,
    /// Serialised parameters (JSON-style string for human readability).
    pub params: String,
}
