//! # SoroStream Contract Interface
//!
//! Defines the formal trait interface for the SoroStream payment streaming contract.
//! The `#[contractclient]` attribute generates a type-safe SDK client struct.

use soroban_sdk::{contractclient, Address, Bytes, BytesN, Env, String, Symbol, Vec};

use crate::errors::StreamError;
use crate::types::{AuditEntry, Stats, Stream, StreamHealth, VestingCurve, VestingTranche};

#[contractclient(name = "SoroStreamClient")]
pub trait SoroStreamInterface {
    fn initialize(env: Env, admin: Address, version: String) -> Result<(), StreamError>;
    fn get_admin(env: Env) -> Result<Address, StreamError>;
    fn get_version(env: Env) -> Result<String, StreamError>;
    fn set_admin(env: Env, new_admin: Address) -> Result<(), StreamError>;
    fn emergency_pause(env: Env) -> Result<(), StreamError>;
    fn emergency_resume(env: Env) -> Result<(), StreamError>;
    fn is_paused(env: Env) -> bool;
    fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), StreamError>;
    fn set_max_streams(env: Env, max_streams: u32) -> Result<(), StreamError>;
    fn set_sender_stream_limit(env: Env, sender: Address, limit: u32) -> Result<(), StreamError>;

    fn create_stream(
        env: Env,
        sender: Address,
        recipient: Address,
        token: Address,
        amount: i128,
        duration_seconds: u64,
        cliff_seconds: u64,
        nonce: u64,
        auto_renew: bool,
        lock_until: u64,
        allow_recipient_termination: bool,
        holdback_amount: i128,
        withdrawal_steps: Option<u32>,
        min_withdrawal_amount: Option<i128>,
    ) -> Result<u64, StreamError>;

    fn create_stream_with_federation(
        env: Env,
        sender: Address,
        federation_name: String,
        token: Address,
        amount: i128,
        duration_seconds: u64,
        cliff_seconds: u64,
        nonce: u64,
        auto_renew: bool,
        lock_until: u64,
        allow_recipient_termination: bool,
    ) -> Result<u64, StreamError>;

    fn create_stream_with_schedule(
        env: Env,
        sender: Address,
        recipient: Address,
        token: Address,
        deposit: i128,
        tranches: Vec<VestingTranche>,
        nonce: u64,
        lock_until: u64,
        allow_recipient_termination: bool,
        oracle: Option<Address>,
        max_price_deviation_bps: u32,
    ) -> Result<u64, StreamError>;

    fn create_stream_with_curve(
        env: Env,
        sender: Address,
        recipient: Address,
        token: Address,
        amount: i128,
        duration_seconds: u64,
        cliff_seconds: u64,
        nonce: u64,
        auto_renew: bool,
        lock_until: u64,
        allow_recipient_termination: bool,
        curve: VestingCurve,
        on_complete_contract: Option<Address>,
        on_complete_function: Option<Symbol>,
        escrow_hold: bool,
    ) -> Result<u64, StreamError>;

    fn register_federation(env: Env, admin: Address, federation_name: String, stellar_address: Address) -> Result<(), StreamError>;
    fn unregister_federation(env: Env, admin: Address, federation_name: String) -> Result<(), StreamError>;
    fn resolve_federation(env: Env, federation_name: String) -> Result<Address, StreamError>;

    fn release_holdback(env: Env, stream_id: u64, caller: Address) -> Result<(), StreamError>;
    fn claw_back_holdback(env: Env, stream_id: u64, caller: Address) -> Result<(), StreamError>;

    fn set_withdrawal_cooldown(env: Env, admin: Address, cooldown_seconds: u64) -> Result<(), StreamError>;
    fn set_whitelist_enabled(env: Env, admin: Address, enabled: bool) -> Result<(), StreamError>;
    fn add_to_whitelist(env: Env, admin: Address, recipient: Address) -> Result<(), StreamError>;
    fn remove_from_whitelist(env: Env, admin: Address, recipient: Address) -> Result<(), StreamError>;
    fn update_metadata(env: Env, sender: Address, stream_id: u64, metadata: Bytes) -> Result<(), StreamError>;
    fn cancel_auto_renew(env: Env, sender: Address, stream_id: u64) -> Result<(), StreamError>;

    /// Activates a stream that was created with escrow_hold = true.
    ///
    /// Transitions the stream from EscrowHold to Active state, enabling token flow.
    /// Only the sender may call this. No-op if the stream is not in EscrowHold state.
    fn activate_stream(env: Env, stream_id: u64, sender: Address) -> Result<(), StreamError>;

    fn withdraw(env: Env, stream_id: u64, recipient: Address) -> Result<(), StreamError>;
    fn cancel_stream(env: Env, stream_id: u64, sender: Address) -> Result<(), StreamError>;
    fn transfer_recipient(env: Env, stream_id: u64, current_recipient: Address, new_recipient: Address) -> Result<(), StreamError>;
    fn partial_cancel_stream(env: Env, stream_id: u64, sender: Address, cancel_amount: i128) -> Result<u64, StreamError>;
    fn top_up(env: Env, stream_id: u64, sender: Address, token: Address, amount: i128) -> Result<(), StreamError>;
    fn recipient_terminate(env: Env, stream_id: u64, recipient: Address) -> Result<(), StreamError>;

    fn get_stream(env: Env, stream_id: u64) -> Result<Stream, StreamError>;
    fn get_all_stream_ids(env: Env, start: u32, limit: u32) -> Vec<u64>;
    fn get_claimable(env: Env, stream_id: u64) -> Result<i128, StreamError>;
    fn is_participant(env: Env, stream_id: u64, address: Address) -> Result<bool, StreamError>;
    fn get_streams_by_sender(env: Env, sender: Address, start: u32, limit: u32) -> Vec<Stream>;
    fn get_streams_by_recipient(env: Env, recipient: Address, start: u32, limit: u32) -> Vec<Stream>;
    fn get_active_streams_by_sender(env: Env, sender: Address) -> Vec<Stream>;
    fn get_active_streams_by_recipient(env: Env, recipient: Address) -> Vec<Stream>;
    fn simulate_claimable(env: Env, stream_id: u64, query_time: u64) -> Result<i128, StreamError>;

    fn pause_stream(env: Env, stream_id: u64, sender: Address) -> Result<(), StreamError>;
    fn resume_stream(env: Env, stream_id: u64, sender: Address) -> Result<(), StreamError>;

    fn batch_create_stream(
        env: Env,
        sender: Address,
        recipients: Vec<Address>,
        amounts: Vec<i128>,
        tokens: Vec<Address>,
        duration_seconds: u64,
        auto_renew: bool,
        lock_untils: Vec<u64>,
        nonce: u64,
    ) -> Result<Vec<u64>, StreamError>;
    fn get_nonce(env: Env, sender: Address) -> u64;
    fn batch_withdraw(env: Env, stream_ids: Vec<u64>, recipient: Address) -> Result<Vec<i128>, StreamError>;
    fn batch_cancel_stream(env: Env, stream_ids: Vec<u64>, sender: Address) -> Result<Vec<Result<(), StreamError>>, StreamError>;

    fn set_protocol_fee(env: Env, fee_bps: u32) -> Result<(), StreamError>;
    fn propose_fee_change(env: Env, admin: Address, new_fee_bps: u32) -> Result<(), StreamError>;
    fn execute_fee_change(env: Env) -> Result<(), StreamError>;
    fn set_treasury_address(env: Env, treasury: Address) -> Result<(), StreamError>;
    fn get_protocol_fee_info(env: Env) -> (u32, Option<Address>);
    fn get_stats(env: Env) -> Stats;
    fn recalibrate_stats(env: Env, admin: Address) -> Result<(), StreamError>;

    fn min_duration(env: Env) -> u64;
    fn set_min_duration(env: Env, admin: Address, seconds: u64);
    fn max_duration(env: Env) -> u64;
    fn set_max_duration(env: Env, admin: Address, seconds: u64);

    /// Returns the maximum allowed future start-time offset in seconds (default: 365 days).
    fn max_future_start_offset(env: Env) -> u64;

    /// Sets the maximum allowed future start-time offset in seconds.
    /// Only the admin may call this.
    fn set_max_future_start_offset(env: Env, admin: Address, offset_seconds: u64);

    /// Creates a payment stream with a caller-supplied `start_time`.
    ///
    /// `start_time` must satisfy `now <= start_time <= now + max_future_start_offset`.
    /// Returns [`StreamError::InvalidStartTime`] for past timestamps and
    /// [`StreamError::StartTimeTooFar`] when the offset limit is exceeded.
    #[allow(clippy::too_many_arguments)]
    fn create_stream_scheduled(
        env: Env,
        sender: Address,
        recipient: Address,
        token: Address,
        amount: i128,
        duration_seconds: u64,
        start_time: u64,
        cliff_seconds: u64,
        nonce: u64,
        auto_renew: bool,
        lock_until: u64,
        allow_recipient_termination: bool,
        holdback_amount: i128,
        on_complete_contract: Option<Address>,
        on_complete_function: Option<Symbol>,
        escrow_hold: bool,
    ) -> Result<u64, StreamError>;

    /// Runs a one-time migration step after a WASM upgrade. Admin-gated and idempotent.
    fn migrate(env: Env, from_version: String, to_version: String) -> Result<(), StreamError>;
    fn get_admin_log(env: Env) -> Vec<AuditEntry>;
    fn archive_stream(env: Env, stream_id: u64, caller: Address) -> Result<(), StreamError>;
    fn mark_expired(env: Env, stream_id: u64) -> Result<(), StreamError>;
    fn bump_stream_ttl(env: Env, stream_id: u64) -> Result<(), StreamError>;

    // ── Issue #300: Instance TTL ────────────────────────────────────────────

    fn set_max_streams_per_token(env: Env, max: u32) -> Result<(), StreamError>;
    fn get_max_streams_per_token(env: Env) -> u32;

    // ── Issue #284: Address blocklist ───────────────────────────────────────

    fn add_to_blocklist(env: Env, addr: Address) -> Result<(), StreamError>;
    fn remove_from_blocklist(env: Env, addr: Address) -> Result<(), StreamError>;
    fn is_blocked(env: Env, addr: Address) -> bool;

    // ── Issue #282: Grace period & recovery ─────────────────────────────────

    fn set_grace_period_ledgers(env: Env, ledgers: u32) -> Result<(), StreamError>;
    fn get_grace_period_ledgers(env: Env) -> u32;
    fn recover_expired(env: Env, stream_id: u64, sender: Address) -> Result<(), StreamError>;
    fn sweep_expired(env: Env, stream_ids: Vec<u64>) -> Result<(), StreamError>;

    fn add_fee_exempt(env: Env, addr: Address) -> Result<(), StreamError>;
    fn remove_fee_exempt(env: Env, addr: Address) -> Result<(), StreamError>;
    fn is_fee_exempt(env: Env, addr: Address) -> bool;
    fn get_fees_collected(env: Env, token: Address) -> i128;
    fn sweep_fees(env: Env, token: Address, destination: Address) -> Result<(), StreamError>;

    fn set_guardian(env: Env, guardian: Address) -> Result<(), StreamError>;
    fn get_guardian(env: Env) -> Option<Address>;
    fn set_governance(env: Env, governance: Address) -> Result<(), StreamError>;
    fn get_governance(env: Env) -> Option<Address>;
    fn pause(env: Env, guardian: Address) -> Result<(), StreamError>;
    fn unpause(env: Env, governance: Address) -> Result<(), StreamError>;
    fn get_pause_expiry(env: Env) -> u64;

    fn set_creation_fee(env: Env, fee: i128, xlm_token: Address) -> Result<(), StreamError>;
    fn get_creation_fee(env: Env) -> i128;

    fn set_token_fee_tier(env: Env, admin: Address, token: Address, fee_bps: u32) -> Result<(), StreamError>;
    fn remove_token_fee_tier(env: Env, admin: Address, token: Address) -> Result<(), StreamError>;
    fn get_token_fee_tier(env: Env, token: Address) -> u32;

    fn get_metadata_uri(env: Env, stream_id: u64) -> Option<String>;
    fn update_metadata_uri(env: Env, stream_id: u64, sender: Address, new_uri: Option<String>) -> Result<(), StreamError>;
    fn release_milestone(env: Env, stream_id: u64, milestone_index: u32, sender: Address) -> Result<(), StreamError>;

    fn set_delegate(env: Env, sender: Address, stream_id: u64, delegate: Address) -> Result<(), StreamError>;
    fn revoke_delegate(env: Env, sender: Address, stream_id: u64) -> Result<(), StreamError>;
    fn get_delegate(env: Env, stream_id: u64) -> Option<Address>;

    fn set_rate_limit_window(env: Env, admin: Address, window_seconds: u64) -> Result<(), StreamError>;
    fn set_rate_limit_max(env: Env, admin: Address, max_creations: u32) -> Result<(), StreamError>;
    fn add_rate_limit_exempt(env: Env, admin: Address, address: Address) -> Result<(), StreamError>;
    fn remove_rate_limit_exempt(env: Env, admin: Address, address: Address) -> Result<(), StreamError>;
    fn remaining_quota(env: Env, address: Address) -> u32;

    fn set_token_whitelist_enabled(env: Env, admin: Address, enabled: bool) -> Result<(), StreamError>;
    fn add_token_to_whitelist(env: Env, admin: Address, token: Address) -> Result<(), StreamError>;
    fn remove_token_from_whitelist(env: Env, admin: Address, token: Address) -> Result<(), StreamError>;

    fn set_slippage_params(env: Env, sender: Address, stream_id: u64, reference_price: i128, max_slippage_bps: u32) -> Result<(), StreamError>;

    // ── Feature (a): StreamExpiryWarning ─────────────────────────────────────

    /// Sets the expiry warning window in ledgers. Only admin may call this.
    ///
    /// A `StreamExpiryWarning` event is emitted once per stream during the first
    /// interaction (withdraw, cancel, top_up) that occurs within `window_ledgers`
    /// ledgers of the stream's `end_time`. Default: 17280 (~24 h at 5 s/ledger).
    ///
    /// # Errors
    /// - `InvalidExpiryWindow` if `window_ledgers == 0`.
    fn set_expiry_warning_window(env: Env, window_ledgers: u32) -> Result<(), StreamError>;

    /// Returns the current expiry warning window in ledgers.
    fn get_expiry_warning_window(env: Env) -> u32;

    // ── Feature (b): Sender reputation cap ───────────────────────────────────

    /// Sets the stream cap applied to new (un-promoted) senders. Only admin may call this.
    ///
    /// Senders whose lifetime stream count is below `get_sender_promotion_threshold()`
    /// are limited to at most `cap` concurrent active streams.
    fn set_new_sender_stream_cap(env: Env, cap: u32) -> Result<(), StreamError>;

    /// Returns the current new-sender stream cap.
    fn get_new_sender_stream_cap(env: Env) -> u32;

    /// Sets the promotion threshold. Once a sender's lifetime count reaches this
    /// value the new-sender cap is permanently lifted. Only admin may call this.
    fn set_sender_promotion_threshold(env: Env, threshold: u32) -> Result<(), StreamError>;

    /// Returns the current sender promotion threshold.
    fn get_sender_promotion_threshold(env: Env) -> u32;

    /// Returns the total number of streams ever created by `sender` (lifetime count).
    fn get_sender_lifetime_count(env: Env, sender: Address) -> u32;

    /// Returns `true` if `sender` has crossed the promotion threshold.
    fn is_sender_promoted(env: Env, sender: Address) -> bool;

    // ── Feature (c): Stream redirect ─────────────────────────────────────────

    /// Sets a redirect target for a stream. Only the recipient may call this.
    ///
    /// When a redirect is active, `withdraw` calls will top-up `target_stream_id`
    /// instead of transferring tokens directly to the recipient.
    ///
    /// # Errors
    /// - `NotRecipient` — caller is not this stream's recipient.
    /// - `InvalidRedirectTarget` — target stream does not exist.
    /// - `RedirectRecipientMismatch` — target stream's recipient differs from this stream's.
    /// - `CircularRedirect` — setting this redirect would create a cycle.
    fn set_redirect(env: Env, stream_id: u64, target_stream_id: u64, recipient: Address) -> Result<(), StreamError>;

    /// Clears the redirect target for a stream. Only the recipient may call this.
    fn clear_redirect(env: Env, stream_id: u64, recipient: Address) -> Result<(), StreamError>;

    /// Returns the redirect target stream ID for a stream, if set.
    fn get_redirect(env: Env, stream_id: u64) -> Option<u64>;

    // ── Feature (d): Dual-token streams ──────────────────────────────────────

    /// Creates a dual-token payment stream under a single stream ID.
    ///
    /// Both `token1`/`amount1` and `token2`/`amount2` are locked and vested in lockstep
    /// with the same `start_time`, `end_time`, and cliff. A single `withdraw` distributes
    /// both tokens proportionally. A single `cancel_stream` refunds both.
    ///
    /// # Errors
    /// - `DuplicateTokenInDualStream` if `token1 == token2`.
    /// - `ZeroAmount` if either amount <= 0.
    /// - `ZeroFlowRate` if either `amount / duration_seconds` rounds to 0.
    /// - Standard errors from `create_stream` also apply.
    fn create_dual_stream(
        env: Env,
        sender: Address,
        recipient: Address,
        token1: Address,
        amount1: i128,
        token2: Address,
        amount2: i128,
        duration_seconds: u64,
        cliff_seconds: u64,
        nonce: u64,
        lock_until: u64,
        allow_recipient_termination: bool,
    ) -> Result<u64, StreamError>;

    /// Returns a health snapshot for the given stream's on-chain storage entry.
    ///
    /// This is a read-only instruction — no auth required, callable by anyone.
    /// It reports the current ledger, the stream's `end_time`, the number of
    /// ledgers remaining before the persistent storage entry is evicted, and a
    /// derived health classification.
    ///
    /// ## Health thresholds
    /// | Remaining ledgers | Status       |
    /// |-------------------|--------------|
    /// | >= 10,000         | `Healthy`    |
    /// | 1,000 – 9,999     | `TTLWarning` |
    /// | < 1,000           | `AtRisk`     |
    ///
    /// # Parameters
    /// * `stream_id` - The ID of the stream to inspect.
    ///
    /// # Returns
    /// A [`StreamHealth`] struct containing the health snapshot.
    ///
    /// # Errors
    /// Returns `StreamError::StreamNotFound` if no stream with this ID exists.
    fn get_stream_health(env: Env, stream_id: u64) -> Result<StreamHealth, StreamError>;
}
