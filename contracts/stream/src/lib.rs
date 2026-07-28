#![no_std]
#![allow(clippy::too_many_arguments)]
//! # SoroStream Contract

#[cfg(test)]
extern crate std;

mod errors;
mod events;
mod interface;
pub mod oracle;
mod storage;
mod types;
pub mod vesting_math;

pub use interface::SoroStreamInterface;
pub use errors::StreamError;
pub use types::{AuditEntry, HealthStatus, Stream, StreamHealth, Stats, StreamStatus, VestingCurve};
pub use oracle::IPriceOracle;

#[cfg(test)] mod test;
#[cfg(test)] mod cost_bench;
#[cfg(test)] mod storage_bench;
#[cfg(test)] mod integration_tests;
#[cfg(test)] mod testnet_integration_tests;
#[cfg(test)] mod proptest_tests;
#[cfg(test)] mod differential_fuzz;
#[cfg(test)] mod tranche_oracle_tests;
#[cfg(test)] mod decay_vesting_tests;
#[cfg(test)] mod feature_tests;

use soroban_sdk::{
    contract, contractimpl, token, Address, Bytes, BytesN, Env, String, Vec, Symbol, IntoVal,
};
use types::{VestingCurve, VestingTranche};
use storage::{
    accumulate_fees, add_fee_exempt, add_rate_limit_exempt, add_to_blocklist,
    add_to_whitelist, add_token_to_whitelist, append_audit_entry, check_admin,
    clear_pending_fee_proposal, clear_reentrancy_lock, cleanup_dual_stream_storage,
    decrement_active_stream_count, decrement_token_stream_count,
    derive_stream_id, drain_fees_collected, effective_sender_limit, extend_instance_ttl,
    get_active_stream_count, get_batch_nonce, get_creation_fee_xlm, get_delegate,
    get_dual_stream_deposit2, get_dual_stream_token2, get_dual_stream_withdrawn2,
    get_expiry_warning_window, get_federation_address, get_fees_collected,
    get_global_stream_at, get_global_stream_count, get_grace_period_ledgers,
    get_holdback, get_ids_by_recipient, get_ids_by_sender, get_max_streams_per_token,
    get_new_sender_stream_cap, get_pause_expiry, get_protocol_fee,
    get_rate_limit_max_creations, get_rate_limit_state, get_rate_limit_window,
    get_sender_last_creation_time, get_sender_lifetime_count,
    get_sender_promotion_threshold, get_sender_stream_count, get_slippage_params,
    get_stream_creation_cooldown, get_token_stream_count, get_treasury,
    get_withdrawal_cooldown, get_xlm_token, increment_active_stream_count,
    increment_batch_nonce, increment_dual_stream_withdrawn2,
    increment_sender_lifetime_count, increment_token_stream_count,
    index_by_recipient, index_by_sender, index_global_stream,
    is_blocked, is_fee_exempt, is_paused_or_auto_unpause, is_rate_limit_exempt,
    is_reentrancy_locked, is_sender_promoted, is_token_whitelist_enabled,
    is_token_whitelisted, is_whitelist_enabled, is_whitelisted, load_stream,
    load_tranches, mark_nonce_used, nonce_used, read_admin,
    read_applied_migrations, read_audit_log, read_governance, read_guardian,
    read_max_duration, read_min_duration, read_pending_fee_proposal, read_version,
    record_migration, register_federation_address, remove_delegate, remove_fee_exempt,
    remove_from_blocklist, remove_from_whitelist, remove_holdback, remove_rate_limit_exempt,
    remove_stream, remove_token_from_whitelist, remove_tranches, save_stream, save_tranches,
    sender_count_key, sender_slot_key, set_active_stream_count, set_creation_fee_xlm,
    set_delegate, set_dual_stream_deposit2, set_dual_stream_token2,
    set_dual_stream_withdrawn2, set_expiry_warning_window, set_grace_period_ledgers,
    set_holdback, set_max_streams_per_sender, set_max_streams_per_token,
    set_new_sender_stream_cap, set_pause_expiry, set_paused, set_protocol_fee,
    set_rate_limit_max_creations, set_rate_limit_state, set_rate_limit_window,
    set_reentrancy_lock, set_sender_last_creation_time, set_sender_limit,
    set_sender_promotion_threshold, set_slippage_params, set_stream_creation_cooldown,
    set_token_whitelist_enabled, set_treasury, set_whitelist_enabled, set_withdrawal_cooldown,
    set_xlm_token, stream_exists, unindex_by_recipient, unindex_by_sender,
    unregister_federation_address, write_admin, write_governance, write_guardian,
    write_max_duration, write_min_duration, write_pending_fee_proposal, write_version,
    MAX_PAUSE_DURATION, read_max_future_start_offset, write_max_future_start_offset,
    DEFAULT_MAX_FUTURE_START_OFFSET, get_fees_collected, increment_fees_collected,
    set_fees_collected,
};

// ── Helper: checked multiply ──────────────────────────────────────────────────
fn checked_flow_amount(flow_rate: i128, elapsed: u64) -> Result<i128, StreamError> {
    flow_rate.checked_mul(elapsed as i128).ok_or(StreamError::Overflow)
}

const MAX_STREAM_DURATION_SECONDS: u64 = 100 * 365 * 24 * 60 * 60;

// ── Helper: validate metadata URI ────────────────────────────────────────────
/// Minimum claimable amount before a withdrawal is considered meaningful.
///
/// Amounts at or below this threshold are treated as rounding dust and
/// suppressed in `get_claimable` and `withdraw` to prevent failed
/// micro-withdrawals and noisy UI displays. 1 stroop is the smallest
/// indivisible unit of any Stellar token.
const DUST_THRESHOLD: i128 = 1;

/// Validates a metadata URI format and length.
fn validate_metadata_uri(uri: &Option<String>) -> Result<(), StreamError> {
    if let Some(ref u) = uri {
        if u.len() > 128 { return Err(StreamError::InvalidMetadataUri); }
        let b = u.as_bytes();
        let ok = (b.len() >= 7
            && b[0]==b'i' && b[1]==b'p' && b[2]==b'f' && b[3]==b's'
            && b[4]==b':' && b[5]==b'/' && b[6]==b'/')
            || (b.len() >= 8
            && b[0]==b'h' && b[1]==b't' && b[2]==b't' && b[3]==b'p'
            && b[4]==b's' && b[5]==b':' && b[6]==b'/' && b[7]==b'/');
        if !ok { return Err(StreamError::InvalidMetadataUri); }
    }
    Ok(())
}

// ── Helper: rate limiting ────────────────────────────────────────────────────
fn check_rate_limit(env: &Env, sender: &Address, now: u64) -> Result<(), StreamError> {
    if is_rate_limit_exempt(env, sender) { return Ok(()); }
    let window = get_rate_limit_window(env);
    let max = get_rate_limit_max_creations(env);
    let (ws, count) = get_rate_limit_state(env, sender);
    let (new_ws, new_count) = if now >= ws + window {
        (now, 1u32)
    } else {
        if count >= max { events::rate_limit_exceeded(env, sender); return Err(StreamError::RateLimitExceeded); }
        (ws, count + 1)
    };
    set_rate_limit_state(env, sender, new_ws, new_count);
    Ok(())
}

// ── Helper: token whitelist ───────────────────────────────────────────────────
fn check_token_whitelist(env: &Env, token: &Address) -> Result<(), StreamError> {
    if is_token_whitelist_enabled(env) && !is_token_whitelisted(env, token) {
        return Err(StreamError::TokenNotWhitelisted);
    }
    Ok(())
}

// ── Helper: validate SAC address ─────────────────────────────────────────────
fn validate_token_address(env: &Env, token: &Address) -> Result<(), StreamError> {
    let zero = Address::from_contract_id(env, &BytesN::<32>::from_array(env, &[0u8; 32]));
    if token == &zero { return Err(StreamError::InvalidTokenAddress); }
    match token::Client::new(env, token).symbol() {
        Ok(_) => Ok(()),
        Err(_) => Err(StreamError::InvalidTokenAddress),
    }
}

// ── Feature (a): maybe emit StreamExpiryWarning ───────────────────────────────
fn maybe_emit_expiry_warning(env: &Env, stream: &mut Stream) {
    if stream.expiry_warning_emitted { return; }
    let now = env.ledger().timestamp();
    if now >= stream.end_time { return; }
    let remaining_seconds = stream.end_time - now;
    let remaining_ledgers = (remaining_seconds / 5) as u32;
    let window = get_expiry_warning_window(env);
    if remaining_ledgers <= window {
        let remaining_balance = stream.deposit.saturating_sub(stream.total_withdrawn);
        events::stream_expiry_warning(env, stream.id, &stream.sender, &stream.recipient,
            remaining_balance, remaining_ledgers);
        stream.expiry_warning_emitted = true;
    }
}

// ── Feature (b): new-sender cap check ────────────────────────────────────────
fn check_new_sender_cap(env: &Env, sender: &Address) -> Result<(), StreamError> {
    if is_sender_promoted(env, sender) { return Ok(()); }
    let cap = get_new_sender_stream_cap(env);
    if get_sender_stream_count(env, sender) >= cap {
        return Err(StreamError::NewSenderStreamCapExceeded);
    }
    Ok(())
}

fn post_create_sender_accounting(env: &Env, sender: &Address) {
    let was_promoted = is_sender_promoted(env, sender);
    increment_sender_lifetime_count(env, sender);
    if !was_promoted && is_sender_promoted(env, sender) {
        let lifetime = get_sender_lifetime_count(env, sender);
        let threshold = get_sender_promotion_threshold(env);
        events::sender_promoted(env, sender, lifetime, threshold);
    }
}

// ── Feature (c): circular redirect detection ─────────────────────────────────
const MAX_REDIRECT_DEPTH: u32 = 8;

fn check_no_circular_redirect(env: &Env, source_id: u64, target_id: u64) -> Result<(), StreamError> {
    let mut cur = target_id;
    for _ in 0..MAX_REDIRECT_DEPTH {
        if cur == source_id { return Err(StreamError::CircularRedirect); }
        match load_stream(env, cur) {
            None => return Ok(()),
            Some(s) => match s.redirect_to_stream_id {
                None => return Ok(()),
                Some(next) => {
                    if next == source_id { return Err(StreamError::CircularRedirect); }
                    cur = next;
                }
            },
        }
    }
    Err(StreamError::CircularRedirect)
}

#[contract]
pub struct SoroStreamContract;

#[contractimpl]
impl SoroStreamContract {

    // ─────────────────────────────────────────────────────────────────────────
    // Admin / lifecycle
    // ─────────────────────────────────────────────────────────────────────────

    pub fn initialize(env: Env, admin: Address, version: String) -> Result<(), StreamError> {
        if read_admin(&env).is_some() { return Err(StreamError::AlreadyInitialized); }
        write_admin(&env, &admin);
        write_version(&env, &version);
        events::contract_deployed(&env, &version, &admin);
        Ok(())
    }

    pub fn get_admin(env: Env) -> Result<Address, StreamError> {
        read_admin(&env).ok_or(StreamError::NotInitialized)
    }

    pub fn get_version(env: Env) -> Result<String, StreamError> {
        read_version(&env).ok_or(StreamError::NotInitialized)
    }

    pub fn set_admin(env: Env, new_admin: Address) -> Result<(), StreamError> {
        check_admin(&env);
        write_admin(&env, &new_admin);
        Ok(())
    }

    pub fn emergency_pause(env: Env) -> Result<(), StreamError> {
        check_admin(&env);
        set_paused(&env, true);
        let ts = env.ledger().timestamp();
        set_pause_expiry(&env, ts.saturating_add(MAX_PAUSE_DURATION));
        let admin = read_admin(&env).unwrap();
        events::contract_paused(&env, &admin, ts);
        let entry = AuditEntry { instruction: String::from_str(&env, "emergency_pause"),
            admin: admin.clone(), timestamp: ts, params: String::from_str(&env, "") };
        append_audit_entry(&env, &entry);
        events::admin_action(&env, &entry.instruction, &admin, ts);
        Ok(())
    }

    pub fn emergency_resume(env: Env) -> Result<(), StreamError> {
        check_admin(&env);
        set_paused(&env, false);
        set_pause_expiry(&env, 0);
        let admin = read_admin(&env).unwrap();
        let ts = env.ledger().timestamp();
        events::contract_resumed(&env, &admin, ts);
        let entry = AuditEntry { instruction: String::from_str(&env, "emergency_resume"),
            admin: admin.clone(), timestamp: ts, params: String::from_str(&env, "") };
        append_audit_entry(&env, &entry);
        events::admin_action(&env, &entry.instruction, &admin, ts);
        Ok(())
    }

    pub fn is_paused(env: Env) -> bool { is_paused_or_auto_unpause(&env) }

    pub fn set_guardian(env: Env, guardian: Address) -> Result<(), StreamError> {
        check_admin(&env); write_guardian(&env, &guardian); Ok(())
    }
    pub fn get_guardian(env: Env) -> Option<Address> { read_guardian(&env) }

    pub fn set_governance(env: Env, governance: Address) -> Result<(), StreamError> {
        check_admin(&env); write_governance(&env, &governance); Ok(())
    }
    pub fn get_governance(env: Env) -> Option<Address> { read_governance(&env) }

    pub fn pause(env: Env, guardian: Address) -> Result<(), StreamError> {
        guardian.require_auth();
        let stored = read_guardian(&env).ok_or(StreamError::NotAuthorized)?;
        if guardian != stored { return Err(StreamError::NotAuthorized); }
        set_paused(&env, true);
        let ts = env.ledger().timestamp();
        set_pause_expiry(&env, ts.saturating_add(MAX_PAUSE_DURATION));
        env.events().publish((Symbol::new(&env, "Paused"), guardian.clone()), ts);
        Ok(())
    }

    pub fn unpause(env: Env, governance: Address) -> Result<(), StreamError> {
        governance.require_auth();
        let stored = read_governance(&env).ok_or(StreamError::NotAuthorized)?;
        if governance != stored { return Err(StreamError::NotAuthorized); }
        set_paused(&env, false);
        set_pause_expiry(&env, 0);
        env.events().publish((Symbol::new(&env, "Unpaused"), governance.clone()), env.ledger().timestamp());
        Ok(())
    }

    pub fn get_pause_expiry(env: Env) -> u64 { get_pause_expiry(&env) }

    pub fn add_fee_exempt(env: Env, addr: Address) -> Result<(), StreamError> {
        check_admin(&env); add_fee_exempt(&env, &addr); Ok(())
    }
    pub fn remove_fee_exempt(env: Env, addr: Address) -> Result<(), StreamError> {
        check_admin(&env); remove_fee_exempt(&env, &addr); Ok(())
    }
    pub fn is_fee_exempt(env: Env, addr: Address) -> bool { is_fee_exempt(&env, &addr) }

    pub fn get_fees_collected(env: Env, token: Address) -> i128 { get_fees_collected(&env, &token) }

    pub fn sweep_fees(env: Env, token: Address, destination: Address) -> Result<(), StreamError> {
        check_admin(&env);
        let amount = drain_fees_collected(&env, &token);
        if amount > 0 {
            token::Client::new(&env, &token).transfer(&env.current_contract_address(), &destination, &amount);
            events::fee_swept(&env, &token, amount, &destination);
        }
        Ok(())
    }

    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), StreamError> {
        let admin = read_admin(&env).ok_or(StreamError::NotInitialized)?;
        admin.require_auth();
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        Ok(())
    }

    pub fn set_max_streams(env: Env, max_streams: u32) -> Result<(), StreamError> {
        check_admin(&env); set_max_streams_per_sender(&env, max_streams); Ok(())
    }
    pub fn set_sender_stream_limit(env: Env, sender: Address, limit: u32) -> Result<(), StreamError> {
        check_admin(&env); set_sender_limit(&env, &sender, limit); Ok(())
    }

    pub fn migrate(env: Env, from_version: String, to_version: String) -> Result<(), StreamError> {
        check_admin(&env);
        let applied = read_applied_migrations(&env);
        if applied.contains(&to_version) { return Err(StreamError::MigrationAlreadyApplied); }
        write_version(&env, &to_version);
        record_migration(&env, &to_version);
        let admin = read_admin(&env).unwrap();
        events::contract_migrated(&env, &from_version, &to_version, &admin);
        let ts = env.ledger().timestamp();
        let entry = AuditEntry { instruction: String::from_str(&env, "migrate"),
            admin: admin.clone(), timestamp: ts, params: to_version.clone() };
        append_audit_entry(&env, &entry);
        events::admin_action(&env, &entry.instruction, &admin, ts);
        Ok(())
    }

    pub fn get_admin_log(env: Env) -> Vec<AuditEntry> { read_audit_log(&env) }

    pub fn archive_stream(env: Env, stream_id: u64, caller: Address) -> Result<(), StreamError> {
        caller.require_auth();
        let stream = load_stream(&env, stream_id).ok_or(StreamError::StreamNotFound)?;
        if stream.sender != caller && stream.recipient != caller { return Err(StreamError::NotAuthorized); }
        let duration = stream.end_time.saturating_sub(stream.start_time);
        let dust = stream.deposit.saturating_sub(stream.flow_rate.saturating_mul(duration as i128));
        if stream.total_withdrawn.saturating_add(dust) < stream.deposit { return Err(StreamError::StreamNotSettled); }
        remove_stream(&env, stream_id);
        unindex_by_sender(&env, &stream.sender, stream_id);
        unindex_by_recipient(&env, &stream.recipient, stream_id);
        if get_delegate(&env, stream_id).is_some() { remove_delegate(&env, stream_id); }
        if stream.is_dual_stream { cleanup_dual_stream_storage(&env, stream_id); }
        events::stream_archived(&env, stream_id, &stream.sender, &stream.recipient, stream.deposit);
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Feature (a): Expiry warning window config
    // ─────────────────────────────────────────────────────────────────────────
    /// Creates a new payment stream.
    #[allow(clippy::too_many_arguments)]
    pub fn create_stream(
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
    ) -> Result<u64, StreamError> {
        sender.require_auth();

        if is_paused_or_auto_unpause(&env) {
            return Err(StreamError::ContractPaused);
        }

        // Get current time early for validations
        let now = env.ledger().timestamp();

        if nonce_used(&env, &sender, nonce) {
            return Err(StreamError::DuplicateStream);
        }
        if amount <= 0 {
            return Err(StreamError::ZeroAmount);
        }
        // holdback must be non-negative and strictly less than total amount (0 = no holdback)
        if holdback_amount < 0 || holdback_amount >= amount {
            return Err(StreamError::ZeroAmount);
        }
        if cliff_seconds > duration_seconds {
            return Err(StreamError::InvalidCliff);
        }
        if is_whitelist_enabled(&env) && !is_whitelisted(&env, &recipient) {
            return Err(StreamError::RecipientNotWhitelisted);
        }

        let min_dur = read_min_duration(&env);
        if duration_seconds < min_dur {
            return Err(StreamError::StreamDurationTooShort);
        }

        let max_dur = read_max_duration(&env);
        if max_dur > 0 && duration_seconds > max_dur {
            return Err(StreamError::DurationExceedsMax);
        }

        // The streaming portion is the total minus the holdback escrow.
        let streaming_amount = amount
            .checked_sub(holdback_amount)
            .ok_or(StreamError::Overflow)?;
        let flow_rate = streaming_amount / duration_seconds as i128;
        if flow_rate == 0 {
            return Err(StreamError::ZeroFlowRate);
        }

        // ── Validate withdrawal_steps ────────────────────────────────────────
        // Steps must be >= 1.  A value of 0 is nonsensical; callers should pass
        // None instead of Some(0).
        if let Some(steps) = withdrawal_steps {
            if steps == 0 {
                return Err(StreamError::InvalidDuration);
            }
        }

        // ── Validate min_withdrawal_amount ───────────────────────────────────
        // The floor must be positive; 0 is indistinguishable from "no floor".
        if let Some(floor) = min_withdrawal_amount {
            if floor <= 0 {
                return Err(StreamError::ZeroAmount);
            }
        }

        let sender_count = get_sender_stream_count(&env, &sender);
        let limit = effective_sender_limit(&env, &sender);
        if sender_count >= limit {
            return Err(StreamError::SenderStreamLimitExceeded);
        }

        // Check blocklist (Issue #284)
        if is_blocked(&env, &sender) || is_blocked(&env, &recipient) {
            return Err(StreamError::AddressBlocked);
        }

        // Check per-token stream cap (Issue #286)
        let max_per_token = get_max_streams_per_token(&env);
        if max_per_token > 0 && get_token_stream_count(&env, &token) >= max_per_token {
            return Err(StreamError::TokenStreamCapExceeded);
        }

        mark_nonce_used(&env, &sender, nonce);

        let end_time = now
            .checked_add(duration_seconds)
            .ok_or(StreamError::Overflow)?;
        if end_time <= now {
            return Err(StreamError::InvalidEndTime);
        }
        let cliff_time = now
            .checked_add(cliff_seconds)
            .ok_or(StreamError::Overflow)?;

        // ── Defensive stream ID collision check ─────────────────────────────
        // derive_stream_id produces the first 8 bytes of a SHA-256 hash.
        // Collisions are astronomically unlikely, but we add an explicit retry
        // loop as a defence-in-depth measure: if a collision is detected, retry
        // up to MAX_ID_RETRIES times by XOR-ing a retry counter into the nonce
        // input.  All retries colliding returns IDCollision — a clear signal
        // that something is structurally wrong.
        const MAX_ID_RETRIES: u64 = 3;
        let mut stream_id = derive_stream_id(&env, &sender, &recipient, now, nonce);
        if stream_exists(&env, stream_id) {
            let mut found = false;
            for retry in 1u64..=MAX_ID_RETRIES {
                let candidate = derive_stream_id(
                    &env, &sender, &recipient, now, nonce ^ (retry << 32),
                );
                if !stream_exists(&env, candidate) {
                    stream_id = candidate;
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(StreamError::IDCollision);
            }
        }

        let creation_fee = get_creation_fee_xlm(&env);
        if creation_fee > 0 {
            let treasury = get_treasury(&env).ok_or(StreamError::NotInitialized)?;
            let xlm_token = get_xlm_token(&env).ok_or(StreamError::NotInitialized)?;
            token::Client::new(&env, &xlm_token).transfer(
                &sender,
                &treasury,
                &creation_fee,
            );
            events::creation_fee_collected(&env, creation_fee, &treasury);
        }

        // Transfer total amount (streaming + holdback) from sender into contract escrow.
        token::Client::new(&env, &token).transfer(
            &sender,
            &env.current_contract_address(),
            &amount,
        );

        let stream = Stream {
            id: stream_id,
            sender: sender.clone(),
            recipient: recipient.clone(),
            token,
            deposit: streaming_amount,
            flow_rate,
            start_time: now,
            cliff_time,
            lock_until,
            end_time,
            last_withdraw_time: now,
            status: StreamStatus::Active,
            auto_renew,
            allow_recipient_termination,
            last_pause_time: 0,
            total_withdrawn: 0,
            metadata: Bytes::new(&env),
            locked: false,
            metadata_uri: None,
            milestones: Vec::new(&env),
            holdback_amount,
            holdback_claimed: false,
            is_dual_stream: false,
            is_step_vesting: false,
            tranches_claimed: 0,
            oracle: None,
            max_price_deviation_bps: 0,
            creation_price: 0,
            curve: VestingCurve::Linear,
            withdrawal_steps,
            current_step: 0,
            min_withdrawal_amount,
        };

        save_stream(&env, &stream);
        extend_instance_ttl(&env);
        index_by_sender(&env, &sender, stream_id);
        index_by_recipient(&env, &recipient, stream_id);
        index_global_stream(&env, stream_id);
        increment_active_stream_count(&env);
        increment_token_stream_count(&env, &token);

        // Update sender's last stream creation time (Issue #239)
        set_sender_last_creation_time(&env, &sender, now);

        events::stream_created(
            &env, stream_id, &sender, &recipient, amount, flow_rate, end_time,
        );

        // Emit supplemental config event when non-default options are set so
        // indexers can surface step/floor configuration without parsing the
        // full stream struct.
        if withdrawal_steps.is_some() || min_withdrawal_amount.is_some() {
            events::stream_config(&env, stream_id, withdrawal_steps, min_withdrawal_amount);
        }

        Ok(stream_id)
    }

    /// Creates a new payment stream using a federation name (Issue #238).
    pub fn create_stream_with_federation(
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
    ) -> Result<u64, StreamError> {
        let recipient = get_federation_address(&env, &federation_name)
            .ok_or(StreamError::StreamNotFound)?;

        Self::create_stream(
            env,
            sender,
            recipient,
            token,
            amount,
            duration_seconds,
            cliff_seconds,
            nonce,
            auto_renew,
            lock_until,
            allow_recipient_termination,
            0i128, // holdback_amount
            None,  // withdrawal_steps
            None,  // min_withdrawal_amount
        )
    }

    /// Returns the minimum allowed stream duration in seconds.
    pub fn min_duration(env: Env) -> u64 {
        read_min_duration(&env)
    }

    /// Sets the minimum allowed stream duration in seconds. Only the admin may call this.
    pub fn set_min_duration(env: Env, admin: Address, seconds: u64) {
        admin.require_auth();
        write_min_duration(&env, seconds);
    }

    /// Returns the maximum allowed stream duration in seconds (0 = unlimited).
    pub fn max_duration(env: Env) -> u64 {
        read_max_duration(&env)
    }

    /// Sets the maximum allowed stream duration in seconds. Setting to 0 disables the cap. Only the admin may call this.
    pub fn set_max_duration(env: Env, admin: Address, seconds: u64) {
        admin.require_auth();
        write_max_duration(&env, seconds);
    }

    /// Returns the maximum allowed future start-time offset in seconds.
    ///
    /// Scheduled streams must have `start_time <= now + max_future_start_offset`.
    /// Defaults to 365 days (31_536_000 seconds) when not explicitly configured.
    pub fn max_future_start_offset(env: Env) -> u64 {
        read_max_future_start_offset(&env)
    }

    /// Sets the maximum allowed future start-time offset in seconds.
    ///
    /// A value of `0` disables future-dated streams entirely (start_time must equal now).
    /// Only the admin may call this.
    pub fn set_max_future_start_offset(env: Env, admin: Address, offset_seconds: u64) {
        admin.require_auth();
        write_max_future_start_offset(&env, offset_seconds);
    }

    // ── Step-vesting: create_stream_with_schedule ────────────────────────────

    /// Creates a step-vesting stream whose tokens release in discrete tranches.
    ///
    /// Each tranche unlocks its full `amount` atomically once `unlock_time` is
    /// reached.  Tranches must be sorted by `unlock_time` (ascending), non-empty,
    /// each have a positive amount, and their amounts must sum exactly to `deposit`.
    ///
    /// Optionally attaches an oracle for on-chain price validation.  When
    /// `oracle` is `Some(addr)`, `get_price(token)` is called immediately to
    /// record the baseline price; subsequent withdrawals will fail if the current
    /// price deviates by more than `max_price_deviation_bps`.
    #[allow(clippy::too_many_arguments)]
    pub fn create_stream_with_schedule(
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
    ) -> Result<u64, StreamError> {
        sender.require_auth();

        if is_paused_or_auto_unpause(&env) {
            return Err(StreamError::ContractPaused);
        }
        if nonce_used(&env, &sender, nonce) {
            return Err(StreamError::DuplicateStream);
        }
        if deposit <= 0 {
            return Err(StreamError::ZeroAmount);
        }
        if tranches.is_empty() {
            return Err(StreamError::InvalidTranches);
        }
        if is_whitelist_enabled(&env) && !is_whitelisted(&env, &recipient) {
            return Err(StreamError::RecipientNotWhitelisted);
        }

        // Validate tranches: sorted unlock times, positive amounts, sum == deposit.
        let mut tranche_sum: i128 = 0;
        let mut prev_unlock: u64 = 0;
        for i in 0..tranches.len() {
            let t = tranches.get(i).unwrap();
            if t.amount <= 0 {
                return Err(StreamError::InvalidTranches);
            }
            if i > 0 && t.unlock_time <= prev_unlock {
                return Err(StreamError::InvalidTranches);
            }
            prev_unlock = t.unlock_time;
            tranche_sum = tranche_sum
                .checked_add(t.amount)
                .ok_or(StreamError::Overflow)?;
        }
        if tranche_sum != deposit {
            return Err(StreamError::InvalidTranches);
        }

        let sender_count = get_sender_stream_count(&env, &sender);
        let limit = effective_sender_limit(&env, &sender);
        if sender_count >= limit {
            return Err(StreamError::SenderStreamLimitExceeded);
        }

        mark_nonce_used(&env, &sender, nonce);

        let now = env.ledger().timestamp();
        // end_time is the unlock_time of the last tranche.
        let last_tranche = tranches.get(tranches.len() - 1).unwrap();
        let end_time = last_tranche.unlock_time;
        if end_time <= now {
            return Err(StreamError::InvalidEndTime);
        }

        // ── Defensive stream ID collision check (schedule path) ─────────────
        const MAX_ID_RETRIES_SCHED: u64 = 3;
        let mut stream_id = derive_stream_id(&env, &sender, &recipient, now, nonce);
        if stream_exists(&env, stream_id) {
            let mut found = false;
            for retry in 1u64..=MAX_ID_RETRIES_SCHED {
                let candidate = derive_stream_id(
                    &env, &sender, &recipient, now, nonce ^ (retry << 32),
                );
                if !stream_exists(&env, candidate) {
                    stream_id = candidate;
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(StreamError::IDCollision);
            }
        }

        let creation_price = if let Some(ref oracle_addr) = oracle {
            oracle::fetch_price(&env, oracle_addr, &token)?
        } else {
            0
        };

        // Collect XLM creation fee if configured.
        let creation_fee = get_creation_fee_xlm(&env);
        if creation_fee > 0 {
            let treasury = get_treasury(&env).ok_or(StreamError::NotInitialized)?;
            let xlm_token = get_xlm_token(&env).ok_or(StreamError::NotInitialized)?;
            token::Client::new(&env, &xlm_token).transfer(
                &sender,
                &treasury,
                &creation_fee,
            );
            events::creation_fee_collected(&env, creation_fee, &treasury);
        }

        // Transfer deposit into the contract.
        token::Client::new(&env, &token).transfer(
            &sender,
            &env.current_contract_address(),
            &deposit,
        );

        let tranche_count = tranches.len();

        let stream = Stream {
            id: stream_id,
            sender: sender.clone(),
            recipient: recipient.clone(),
            token: token.clone(),
            deposit,
            // flow_rate is unused for step-vesting; 0 sentinel.
            flow_rate: 0,
            start_time: now,
            cliff_time: now,
            lock_until,
            end_time,
            last_withdraw_time: now,
            status: StreamStatus::Active,
            auto_renew: false,
            allow_recipient_termination,
            last_pause_time: 0,
            total_withdrawn: 0,
            metadata: Bytes::new(&env),
            metadata_uri: None,
            milestones: soroban_sdk::Vec::new(&env),
            is_step_vesting: true,
            tranches_claimed: 0,
            oracle: oracle.clone(),
            max_price_deviation_bps,
            creation_price,
            curve: VestingCurve::Linear,
            withdrawal_steps: None,
            current_step: 0,
            min_withdrawal_amount: None,
        };

        save_stream(&env, &stream);
        extend_instance_ttl(&env);
        save_tranches(&env, stream_id, &tranches);
        index_by_sender(&env, &sender, stream_id);
        index_by_recipient(&env, &recipient, stream_id);
        index_global_stream(&env, stream_id);
        increment_active_stream_count(&env);
        increment_token_stream_count(&env, &token);

        events::tranche_stream_created(&env, stream_id, &sender, tranche_count, deposit);
        events::stream_created(&env, stream_id, &sender, &recipient, deposit, 0, end_time);

        Ok(stream_id)
    }

    // ── Time-decay vesting: create_stream_with_curve ─────────────────────────

    /// Creates a stream with an explicit vesting curve.
    ///
    /// Pass `curve: VestingCurve::Linear` to reproduce the standard constant-rate
    /// behaviour.  Pass `curve: VestingCurve::TimeDecay { decay_factor }` to get a
    /// front-weighted release where more tokens unlock early in the stream lifetime.
    ///
    /// The `decay_factor` is expressed in **basis points per 1 000 seconds**
    /// (e.g. `100` = 1 % per 1 ks window).  A value of `0` is identical to
    /// `VestingCurve::Linear`.  Values ≥ 10 000 are clamped to 9 999 internally.
    ///
    /// All other fields behave identically to `create_stream`.
    #[allow(clippy::too_many_arguments)]
    pub fn create_stream_with_curve(
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
    ) -> Result<u64, StreamError> {
        sender.require_auth();

        if is_paused_or_auto_unpause(&env) {
            return Err(StreamError::ContractPaused);
        }
        if nonce_used(&env, &sender, nonce) {
            return Err(StreamError::DuplicateStream);
        }
        if amount <= 0 {
            return Err(StreamError::ZeroAmount);
        }
        if cliff_seconds > duration_seconds {
            return Err(StreamError::InvalidCliff);
        }
        if is_whitelist_enabled(&env) && !is_whitelisted(&env, &recipient) {
            return Err(StreamError::RecipientNotWhitelisted);
        }

        let min_dur = read_min_duration(&env);
        if duration_seconds < min_dur {
            return Err(StreamError::StreamDurationTooShort);
        }

        // For linear streams the flow_rate is used; for TimeDecay it is stored
        // for reference but actual claimable is driven by the decay formula.
        let flow_rate = amount / duration_seconds as i128;
        if flow_rate == 0 {
            return Err(StreamError::ZeroFlowRate);
        }

        let sender_count = get_sender_stream_count(&env, &sender);
        let limit = effective_sender_limit(&env, &sender);
        if sender_count >= limit {
            return Err(StreamError::SenderStreamLimitExceeded);
        }

        mark_nonce_used(&env, &sender, nonce);

        let now = env.ledger().timestamp();
        let end_time = now
            .checked_add(duration_seconds)
            .ok_or(StreamError::Overflow)?;
        if end_time <= now {
            return Err(StreamError::InvalidEndTime);
        }
        let cliff_time = now
            .checked_add(cliff_seconds)
            .ok_or(StreamError::Overflow)?;

        // ── Defensive stream ID collision check (curve path) ─────────────────
        const MAX_ID_RETRIES_CURVE: u64 = 3;
        let mut stream_id = derive_stream_id(&env, &sender, &recipient, now, nonce);
        if stream_exists(&env, stream_id) {
            let mut found = false;
            for retry in 1u64..=MAX_ID_RETRIES_CURVE {
                let candidate = derive_stream_id(
                    &env, &sender, &recipient, now, nonce ^ (retry << 32),
                );
                if !stream_exists(&env, candidate) {
                    stream_id = candidate;
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(StreamError::IDCollision);
            }
        }

        let creation_fee = get_creation_fee_xlm(&env);
        if creation_fee > 0 {
            let treasury = get_treasury(&env).ok_or(StreamError::NotInitialized)?;
            let xlm_token = get_xlm_token(&env).ok_or(StreamError::NotInitialized)?;
            token::Client::new(&env, &xlm_token).transfer(
                &sender,
                &treasury,
                &creation_fee,
            );
            events::creation_fee_collected(&env, creation_fee, &treasury);
        }

        token::Client::new(&env, &token).transfer(
            &sender,
            &env.current_contract_address(),
            &amount,
        );

        let stream = Stream {
            id: stream_id,
            sender: sender.clone(),
            recipient: recipient.clone(),
            token,
            deposit: amount,
            flow_rate,
            start_time: now,
            cliff_time,
            lock_until,
            end_time,
            last_withdraw_time: now,
            status: StreamStatus::Active,
            auto_renew,
            allow_recipient_termination,
            last_pause_time: 0,
            total_withdrawn: 0,
            metadata: Bytes::new(&env),
            metadata_uri: None,
            milestones: soroban_sdk::Vec::new(&env),
            holdback_amount: 0,
            holdback_claimed: false,
            locked: false,
            is_step_vesting: false,
            tranches_claimed: 0,
            oracle: None,
            max_price_deviation_bps: 0,
            creation_price: 0,
            curve,
            withdrawal_steps: None,
            current_step: 0,
            min_withdrawal_amount: None,
        };

        save_stream(&env, &stream);
        extend_instance_ttl(&env);
        index_by_sender(&env, &sender, stream_id);
        index_by_recipient(&env, &recipient, stream_id);
        index_global_stream(&env, stream_id);
        increment_active_stream_count(&env);
        increment_token_stream_count(&env, &token);

        events::stream_created(
            &env, stream_id, &sender, &recipient, amount, flow_rate, end_time,
        );

        Ok(stream_id)
    }

    // ── Off-chain preview utility ─────────────────────────────────────────────

    /// Returns the cumulative amount that **would** be claimable at `query_time`
    /// if the given stream were evaluated at that moment — regardless of how much
    /// has already been withdrawn.
    ///
    /// This is a **read-only** preview function for off-chain UIs and analytics.
    /// It does not check stream status, reentrancy, or auth.
    ///
    /// For `VestingCurve::Linear` this is simply `flow_rate × min(elapsed, duration)`.
    /// For `VestingCurve::TimeDecay` it returns the cumulative decay-weighted amount.
    /// For step-vesting streams (`is_step_vesting = true`) it returns the sum of
    /// tranches whose `unlock_time ≤ query_time`.
    pub fn simulate_claimable(
        env: Env,
        stream_id: u64,
        query_time: u64,
    ) -> Result<i128, StreamError> {
        let stream = load_stream(&env, stream_id).ok_or(StreamError::StreamNotFound)?;

        // Step-vesting: sum tranches whose unlock_time ≤ query_time.
        if stream.is_step_vesting {
            let tranches = load_tranches(&env, stream_id);
            let mut total: i128 = 0;
            for i in 0..tranches.len() {
                let t = tranches.get(i).unwrap();
                if query_time >= t.unlock_time {
                    total = total.checked_add(t.amount).ok_or(StreamError::Overflow)?;
                } else {
                    break;
                }
            }
            return Ok(total);
        }

        // Continuous vesting: use simulate_claimable from vesting_math (cumulative from start).
        let decay_factor = match &stream.curve {
            VestingCurve::Linear => 0u32,
            VestingCurve::TimeDecay { decay_factor } => *decay_factor,
        };

        vesting_math::simulate_claimable(
            stream.deposit,
            stream.start_time,
            stream.end_time,
            query_time,
            stream.cliff_time,
            decay_factor,
        )
        .ok_or(StreamError::Overflow)
    }

    /// Sets the global withdrawal cooldown in seconds.
    pub fn set_withdrawal_cooldown(env: Env, admin: Address, cooldown_seconds: u64) -> Result<(), StreamError> {
        check_admin(&env);
        admin.require_auth();
        set_withdrawal_cooldown(&env, cooldown_seconds);
        Ok(())
    }

    /// Sets the global stream creation cooldown in seconds (Issue #239).
    /// Cooldown of 0 disables the mechanism (default).
    pub fn set_stream_creation_cooldown(env: Env, admin: Address, cooldown_seconds: u64) -> Result<(), StreamError> {
        check_admin(&env);
        admin.require_auth();
        set_stream_creation_cooldown(&env, cooldown_seconds);
        Ok(())
    }

    /// Registers a federation name to a Stellar address (Issue #238).
    /// Only the admin may call this function.
    pub fn register_federation(
        env: Env,
        admin: Address,
        federation_name: String,
        stellar_address: Address,
    ) -> Result<(), StreamError> {
        check_admin(&env);
        admin.require_auth();
        register_federation_address(&env, &federation_name, &stellar_address);
        events::federation_registered(&env, &federation_name, &stellar_address);
        Ok(())
    }

    /// Unregisters a federation name from the registry (Issue #238).
    /// Only the admin may call this function.
    pub fn unregister_federation(
        env: Env,
        admin: Address,
        federation_name: String,
    ) -> Result<(), StreamError> {
        check_admin(&env);
        admin.require_auth();
        unregister_federation_address(&env, &federation_name);
        events::federation_unregistered(&env, &federation_name);
        Ok(())
    }

    /// Resolves a federation name to its registered Stellar address.
    pub fn resolve_federation(env: Env, federation_name: String) -> Result<Address, StreamError> {
        get_federation_address(&env, &federation_name).ok_or(StreamError::StreamNotFound)
    }

    /// Enables or disables recipient whitelisting.
    pub fn set_whitelist_enabled(env: Env, admin: Address, enabled: bool) -> Result<(), StreamError> {
        check_admin(&env);
        admin.require_auth();
        set_whitelist_enabled(&env, enabled);
        Ok(())
    }

    /// Sets the expiry warning window in ledgers. Admin only.
    /// Default: 17280 (~24 h at 5 s/ledger). Must be > 0.
    pub fn set_expiry_warning_window(env: Env, window_ledgers: u32) -> Result<(), StreamError> {
        check_admin(&env);
        if window_ledgers == 0 { return Err(StreamError::InvalidExpiryWindow); }
        set_expiry_warning_window(&env, window_ledgers);
        Ok(())
    }
    pub fn get_expiry_warning_window(env: Env) -> u32 { get_expiry_warning_window(&env) }

    // ─────────────────────────────────────────────────────────────────────────
    // Feature (b): Sender reputation cap config
    // ─────────────────────────────────────────────────────────────────────────

    /// Sets the new-sender stream cap (max concurrent streams before promotion). Admin only.
    pub fn set_new_sender_stream_cap(env: Env, cap: u32) -> Result<(), StreamError> {
        check_admin(&env); set_new_sender_stream_cap(&env, cap); Ok(())
    }
    pub fn get_new_sender_stream_cap(env: Env) -> u32 { get_new_sender_stream_cap(&env) }

    /// Sets the promotion threshold (lifetime stream count). Admin only.
    pub fn set_sender_promotion_threshold(env: Env, threshold: u32) -> Result<(), StreamError> {
        check_admin(&env); set_sender_promotion_threshold(&env, threshold); Ok(())
    }
    pub fn get_sender_promotion_threshold(env: Env) -> u32 { get_sender_promotion_threshold(&env) }
    pub fn get_sender_lifetime_count(env: Env, sender: Address) -> u32 { get_sender_lifetime_count(&env, &sender) }
    pub fn is_sender_promoted(env: Env, sender: Address) -> bool { is_sender_promoted(&env, &sender) }

    // ─────────────────────────────────────────────────────────────────────────
    // Feature (c): Stream redirect management
    // ─────────────────────────────────────────────────────────────────────────

    /// Sets a redirect target on a stream. Only the recipient may call this.
    /// On withdraw, claimable tokens will be topped up into the target stream
    /// instead of sent directly to the recipient.
    pub fn set_redirect(env: Env, stream_id: u64, target_stream_id: u64, recipient: Address) -> Result<(), StreamError> {
        recipient.require_auth();
        let mut stream = load_stream(&env, stream_id).ok_or(StreamError::StreamNotFound)?;
        if stream.recipient != recipient { return Err(StreamError::NotRecipient); }
        let target = load_stream(&env, target_stream_id).ok_or(StreamError::InvalidRedirectTarget)?;
        if target.recipient != recipient { return Err(StreamError::RedirectRecipientMismatch); }
        check_no_circular_redirect(&env, stream_id, target_stream_id)?;
        stream.redirect_to_stream_id = Some(target_stream_id);
        save_stream(&env, &stream);
        events::stream_redirect_set(&env, stream_id, target_stream_id, &recipient);
        Ok(())
    }

    /// Clears the redirect target on a stream. Only the recipient may call this.
    pub fn clear_redirect(env: Env, stream_id: u64, recipient: Address) -> Result<(), StreamError> {
        recipient.require_auth();
        let mut stream = load_stream(&env, stream_id).ok_or(StreamError::StreamNotFound)?;
        if stream.recipient != recipient { return Err(StreamError::NotRecipient); }
        stream.redirect_to_stream_id = None;
        save_stream(&env, &stream);
        events::stream_redirect_cleared(&env, stream_id, &recipient);
        Ok(())
    }

    pub fn get_redirect(env: Env, stream_id: u64) -> Option<u64> {
        load_stream(&env, stream_id).and_then(|s| s.redirect_to_stream_id)
    }

    /// Removes a token from the whitelist (Issue #221).
    pub fn remove_token_from_whitelist(env: Env, admin: Address, token: Address) -> Result<(), StreamError> {
        check_admin(&env);
        admin.require_auth();
        remove_token_from_whitelist(&env, &token);
        events::token_dwhitelisted(&env, &token);
        Ok(())
    }

    // ── Issue #286: Per-token stream count cap ──────────────────────────────

    /// Sets the per-token stream cap. Setting to 0 disables the cap. Admin only.
    pub fn set_max_streams_per_token(env: Env, max: u32) -> Result<(), StreamError> {
        check_admin(&env);
        set_max_streams_per_token(&env, max);
        Ok(())
    }

    /// Returns the current per-token stream cap (0 = unlimited).
    pub fn get_max_streams_per_token(env: Env) -> u32 {
        get_max_streams_per_token(&env)
    }

    // ── Issue #284: Address blocklist ───────────────────────────────────────

    /// Adds an address to the blocklist. Admin only.
    pub fn add_to_blocklist(env: Env, addr: Address) -> Result<(), StreamError> {
        check_admin(&env);
        add_to_blocklist(&env, &addr);
        events::address_blocked(&env, &read_admin(&env).unwrap(), &addr);
        Ok(())
    }

    /// Removes an address from the blocklist. Admin only.
    pub fn remove_from_blocklist(env: Env, addr: Address) -> Result<(), StreamError> {
        check_admin(&env);
        remove_from_blocklist(&env, &addr);
        events::address_unblocked(&env, &read_admin(&env).unwrap(), &addr);
        Ok(())
    }

    /// Returns true if the address is on the blocklist.
    pub fn is_blocked(env: Env, addr: Address) -> bool {
        is_blocked(&env, &addr)
    }

    // ── Issue #282: Grace period & recovery ─────────────────────────────────

    /// Sets the grace period in ledgers. Zero means no grace period. Admin only.
    pub fn set_grace_period_ledgers(env: Env, ledgers: u32) -> Result<(), StreamError> {
        check_admin(&env);
        set_grace_period_ledgers(&env, ledgers);
        Ok(())
    }

    /// Returns the current grace period in ledgers (0 = no grace period).
    pub fn get_grace_period_ledgers(env: Env) -> u32 {
        get_grace_period_ledgers(&env)
    }

    /// Allows the sender to recover unclaimed funds from an expired stream after
    /// the grace period has elapsed.
    ///
    /// The stream must be past its `end_time` and the grace period (in ledgers)
    /// must have passed since `end_time`. After recovery the stream is removed.
    pub fn recover_expired(env: Env, stream_id: u64, sender: Address) -> Result<(), StreamError> {
        sender.require_auth();

        let stream = load_stream(&env, stream_id).ok_or(StreamError::StreamNotFound)?;
        if stream.sender != sender {
            return Err(StreamError::NotSender);
        }

        let now = env.ledger().timestamp();
        if now < stream.end_time {
            return Err(StreamError::StreamNotComplete);
        }

        let grace = get_grace_period_ledgers(&env);
        if grace > 0 {
            let grace_seconds = (grace as u64).saturating_mul(5);
            let grace_end = stream.end_time.saturating_add(grace_seconds);
            if now < grace_end {
                return Err(StreamError::GracePeriodActive);
            }
        }

        let available = stream.deposit.saturating_sub(stream.total_withdrawn);
        if available > 0 {
            token::Client::new(&env, &stream.token).transfer(
                &env.current_contract_address(),
                &sender,
                &available,
            );
        }

        remove_stream(&env, stream_id);
        unindex_by_sender(&env, &stream.sender, stream_id);
        unindex_by_recipient(&env, &stream.recipient, stream_id);
        decrement_token_stream_count(&env, &stream.token);

        events::stream_recovered(&env, stream_id, &sender, available);

        Ok(())
    }

    /// Sweeps accumulated fees from the contract to a destination address (Issue #222).
    pub fn sweep_fees(env: Env, admin: Address, token: Address, destination: Address) -> Result<(), StreamError> {
        check_admin(&env);
        admin.require_auth();

        let token_client = token::Client::new(&env, &token);
        let contract_balance = token_client.balance(&env.current_contract_address());
        
        if contract_balance <= 0 {
            return Ok(());
        }

        token_client.transfer(
            &env.current_contract_address(),
            &destination,
            &contract_balance,
        );

        events::fee_swept(&env, &token, contract_balance, &destination);
        Ok(())
    }

    /// Updates slippage protection parameters for a stream (Issue #218).
    pub fn set_slippage_params(
        env: Env,
        sender: Address,
        stream_id: u64,
        reference_price: i128,
        max_slippage_bps: u32,
    ) -> Result<(), StreamError> {
        sender.require_auth();

        if max_slippage_bps > 10000 {
            return Err(StreamError::InvalidSlippage);
        }

        let stream = load_stream(&env, stream_id).ok_or(StreamError::StreamNotFound)?;
        if stream.sender != sender {
            return Err(StreamError::NotSender);
        }

        set_slippage_params(&env, stream_id, reference_price, max_slippage_bps);
        Ok(())
    }

    /// Updates metadata URI for a stream.
    pub fn update_metadata_uri(
        env: Env,
        sender: Address,
        stream_id: u64,
        metadata_uri: Option<String>,
    ) -> Result<(), StreamError> {
        sender.require_auth();
        validate_metadata_uri(&metadata_uri)?;

        let mut stream = load_stream(&env, stream_id).ok_or(StreamError::StreamNotFound)?;
        if stream.sender != sender {
            return Err(StreamError::NotSender);
        }

        stream.metadata_uri = new_uri.clone();
        save_stream(&env, &stream);
        events::metadata_uri_updated(&env, stream_id, &new_uri);

        Ok(())
    }

    /// Sweeps expired, fully-withdrawn streams from storage and refunds rent incentive.
    pub fn sweep_expired(env: Env, stream_ids: Vec<u64>) -> Result<(), StreamError> {
        let now = env.ledger().timestamp();

        for stream_id in stream_ids.iter() {
            let stream = load_stream(&env, stream_id).ok_or(StreamError::StreamNotFound)?;

            // Check if stream is expired and fully withdrawn (or cancelled)
            let is_expired = now >= stream.end_time;
            let is_fully_withdrawn = stream.total_withdrawn >= stream.deposit || stream.status == StreamStatus::Cancelled;

            if !is_expired || !is_fully_withdrawn {
                return Err(StreamError::StreamNotComplete);
            }

            // Delete storage entries
            remove_stream(&env, stream_id);
            unindex_by_sender(&env, &stream.sender, stream_id);
            unindex_by_recipient(&env, &stream.recipient, stream_id);
            decrement_token_stream_count(&env, &stream.token);

            events::stream_swept(&env, stream_id, &stream.sender);
        }

        Ok(())
    }

    /// Releases a milestone, making its funds claimable by the recipient.
    pub fn release_milestone(
        env: Env,
        stream_id: u64,
        milestone_index: u32,
        sender: Address,
    ) -> Result<(), StreamError> {
        sender.require_auth();

        let mut stream = load_stream(&env, stream_id).ok_or(StreamError::StreamNotFound)?;
        if stream.sender != sender {
            return Err(StreamError::NotSender);
        }

        if milestone_index >= stream.milestones.len() {
            return Err(StreamError::InvalidDuration);
        }

        // Get mutable reference to the milestone and change its status
        let mut milestone = stream.milestones.get(milestone_index).unwrap();
        milestone.status = crate::types::MilestoneStatus::Released;
        stream.milestones.set(milestone_index, milestone);

        save_stream(&env, &stream);
        events::milestone_released(&env, stream_id, milestone_index);

        Ok(())
    }

    /// Extends the Soroban persistent storage TTL for a stream and its indices.
    ///
    /// Callable by anyone — no auth required. Bumps the TTL to cover the remaining
    /// stream duration plus a 24-hour safety buffer (~17280 ledgers). No-op when
    /// the current TTL is already sufficient (extend_ttl is a no-op if new TTL <=
    /// current TTL internally).
    ///
    /// Emits `TtlBumped { stream_id, new_expiry_ledger }`.
    pub fn bump_stream_ttl(env: Env, stream_id: u64) -> Result<(), StreamError> {
        let stream = load_stream(&env, stream_id).ok_or(StreamError::StreamNotFound)?;

        if stream.status != StreamStatus::Active && stream.status != StreamStatus::Paused {
            return Err(StreamError::StreamNotActive);
        }

        let now = env.ledger().timestamp();
        let remaining = stream.end_time.saturating_sub(now);

        const SAFETY_BUFFER_LEDGERS: u32 = 17_280;
        let ledgers_for_remaining = (remaining / 5) as u32;
        let ledgers_needed = ledgers_for_remaining.saturating_add(SAFETY_BUFFER_LEDGERS);

        env.storage()
            .persistent()
            .extend_ttl(&stream_id, ledgers_needed, ledgers_needed);

        let scnt: u32 = env
            .storage()
            .persistent()
            .get(&storage::sender_count_key(&env, &stream.sender))
            .unwrap_or(0u32);
        for i in 0..scnt {
            let slot = storage::sender_slot_key(&env, &stream.sender, i);
            if let Some(id) = env.storage().persistent().get::<_, u64>(&slot) {
                if id == stream_id {
                    env.storage()
                        .persistent()
                        .extend_ttl(&slot, ledgers_needed, ledgers_needed);
                    break;
                }
            }
        }

        let rcnt: u32 = env
            .storage()
            .persistent()
            .get(&storage::recipient_count_key(&env, &stream.recipient))
            .unwrap_or(0u32);
        for i in 0..rcnt {
            let slot = storage::recipient_slot_key(&env, &stream.recipient, i);
            if let Some(id) = env.storage().persistent().get::<_, u64>(&slot) {
                if id == stream_id {
                    env.storage()
                        .persistent()
                        .extend_ttl(&slot, ledgers_needed, ledgers_needed);
                    break;
                }
            }
        }

        let new_expiry_ledger = env.ledger().sequence().saturating_add(ledgers_needed);
        events::ttl_bumped(&env, stream_id, new_expiry_ledger);

        Ok(())
    }

    /// Sets the flat XLM creation fee (in stroops) and the XLM SAC token address.
    pub fn set_creation_fee(env: Env, fee: i128, xlm_token: Address) -> Result<(), StreamError> {
        check_admin(&env);
        if fee < 0 {
            return Err(StreamError::ZeroAmount);
        }
        set_creation_fee_xlm(&env, fee);
        set_xlm_token(&env, &xlm_token);
        Ok(())
    }

    /// Returns the current XLM creation fee in stroops (0 = disabled).
    pub fn get_creation_fee(env: Env) -> i128 {
        get_creation_fee_xlm(&env)
    }

    /// Returns protocol fee configuration.
    pub fn get_protocol_fee_info(env: Env) -> (u32, Option<Address>) {
        (get_protocol_fee(&env), get_treasury(&env))
    }

    /// Withdraws accumulated protocol fees from the treasury contract.
    pub fn withdraw_treasury(
        env: Env,
        token: Address,
        amount: i128,
        destination: Address,
    ) -> Result<(), StreamError> {
        check_admin(&env);
        let treasury = get_treasury(&env).ok_or(StreamError::NotInitialized)?;
        env.invoke_contract::<()>(
            &treasury,
            &Symbol::new(&env, "withdraw_treasury"),
            (token, amount, destination).into_val(&env),
        );
        Ok(())
    }

    /// Withdraws all accumulated protocol fees for a token from the treasury contract.
    pub fn withdraw_all_from_treasury(
        env: Env,
        token: Address,
        destination: Address,
    ) -> Result<i128, StreamError> {
        check_admin(&env);
        let treasury = get_treasury(&env).ok_or(StreamError::NotInitialized)?;
        let result = env.invoke_contract::<i128>(
            &treasury,
            &Symbol::new(&env, "withdraw_all"),
            (token, destination).into_val(&env),
        );
        Ok(result)
    }

    /// Returns aggregate contract statistics.
    pub fn get_stats(env: Env) -> Stats {
        let total_streams = get_global_stream_count(&env) as u64;
        let active_streams = get_active_stream_count(&env) as u64;

        let mut total_volume: i128 = 0;
        let count = get_global_stream_count(&env);

        for i in 0..count {
            if let Some(stream_id) = get_global_stream_at(&env, i) {
                if let Some(stream) = load_stream(&env, stream_id) {
                    total_volume = total_volume.saturating_add(stream.deposit);
                }
            }
        }

        Stats {
            total_streams,
            active_streams,
            total_volume,
        }
    }

    /// Recalibrates the active stream count by scanning all streams.
    /// Only callable by admin. Use when counter drift is suspected.
    pub fn recalibrate_stats(env: Env, admin: Address) -> Result<(), StreamError> {
        check_admin(&env);
        admin.require_auth();

        let mut correct_count = 0u32;
        let count = get_global_stream_count(&env);

        for i in 0..count {
            if let Some(stream_id) = get_global_stream_at(&env, i) {
                if let Some(stream) = load_stream(&env, stream_id) {
                    if stream.status == StreamStatus::Active {
                        correct_count += 1;
                    }
                }
            }
        }

        set_active_stream_count(&env, correct_count);
        Ok(())
    }

    /// Returns a health snapshot for the given stream's on-chain storage entry.
    ///
    /// Read-only, no auth required.  Reports the current ledger sequence, the
    /// stream's `end_time`, ledgers remaining before the persistent storage entry
    /// is evicted, and a derived health classification.
    ///
    /// ## Health thresholds
    /// | Remaining ledgers | Status       |
    /// |-------------------|--------------|
    /// | >= 10,000         | `Healthy`    |
    /// | 1,000 – 9,999     | `TTLWarning` |
    /// | < 1,000           | `AtRisk`     |
    ///
    /// # Errors
    /// Returns `StreamError::StreamNotFound` if no stream with this ID exists.
    pub fn get_stream_health(env: Env, stream_id: u64) -> Result<StreamHealth, StreamError> {
        use types::{HealthStatus, StreamHealth};

        // Confirm the stream exists — returns StreamNotFound for unknown IDs.
        let stream = load_stream(&env, stream_id).ok_or(StreamError::StreamNotFound)?;

        let current_ledger = env.ledger().sequence();

        // Query the live TTL of the persistent storage entry.
        // `get_ttl` returns the number of ledgers from *now* until the entry expires.
        let ttl_remaining: u32 = env
            .storage()
            .persistent()
            .get_ttl(&stream_id)
            .unwrap_or(0u32);

        const TTL_WARNING_THRESHOLD: u32 = 10_000;
        const TTL_AT_RISK_THRESHOLD: u32 = 1_000;

        let status = if ttl_remaining >= TTL_WARNING_THRESHOLD {
            HealthStatus::Healthy
        } else if ttl_remaining >= TTL_AT_RISK_THRESHOLD {
            HealthStatus::TTLWarning
        } else {
            HealthStatus::AtRisk
        };

        Ok(StreamHealth {
            current_ledger,
            end_time: stream.end_time,
            ttl_remaining_ledgers: ttl_remaining,
            status,
        })
    }
}

impl SoroStreamInterface for SoroStreamContract {
    fn initialize(env: Env, admin: Address, version: String) -> Result<(), StreamError> {
        Self::initialize(env, admin, version)
    }

    fn get_admin(env: Env) -> Result<Address, StreamError> {
        Self::get_admin(env)
    }

    fn get_version(env: Env) -> Result<String, StreamError> {
        Self::get_version(env)
    }

    fn set_admin(env: Env, new_admin: Address) -> Result<(), StreamError> {
        Self::set_admin(env, new_admin)
    }

    fn emergency_pause(env: Env) -> Result<(), StreamError> {
        Self::emergency_pause(env)
    }

    fn emergency_resume(env: Env) -> Result<(), StreamError> {
        Self::emergency_resume(env)
    }

    fn is_paused(env: Env) -> bool {
        Self::is_paused(env)
    }

    fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), StreamError> {
        Self::upgrade(env, new_wasm_hash)
    }

    fn set_max_streams(env: Env, max_streams: u32) -> Result<(), StreamError> {
        Self::set_max_streams(env, max_streams)
    }

    fn set_sender_stream_limit(env: Env, sender: Address, limit: u32) -> Result<(), StreamError> {
        Self::set_sender_stream_limit(env, sender, limit)
    }

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
    ) -> Result<u64, StreamError> {
        Self::create_stream(
            env,
            sender,
            recipient,
            token,
            amount,
            duration_seconds,
            cliff_seconds,
            nonce,
            auto_renew,
            lock_until,
            allow_recipient_termination,
            holdback_amount,
            withdrawal_steps,
            min_withdrawal_amount,
        )
    }

    fn set_withdrawal_cooldown(env: Env, admin: Address, cooldown_seconds: u64) -> Result<(), StreamError> {
        Self::set_withdrawal_cooldown(env, admin, cooldown_seconds)
    }

    fn set_whitelist_enabled(env: Env, admin: Address, enabled: bool) -> Result<(), StreamError> {
        Self::set_whitelist_enabled(env, admin, enabled)
    }

    fn add_to_whitelist(env: Env, admin: Address, recipient: Address) -> Result<(), StreamError> {
        Self::add_to_whitelist(env, admin, recipient)
    }

    fn remove_from_whitelist(env: Env, admin: Address, recipient: Address) -> Result<(), StreamError> {
        Self::remove_from_whitelist(env, admin, recipient)
    }

    fn update_metadata(env: Env, sender: Address, stream_id: u64, metadata: Bytes) -> Result<(), StreamError> {
        Self::update_metadata(env, sender, stream_id, metadata)
    }

    fn cancel_auto_renew(env: Env, sender: Address, stream_id: u64) -> Result<(), StreamError> {
        Self::cancel_auto_renew(env, sender, stream_id)
    }

    fn withdraw(env: Env, stream_id: u64, recipient: Address) -> Result<(), StreamError> {
        Self::withdraw(env, stream_id, recipient)
    }

    fn cancel_stream(env: Env, stream_id: u64, sender: Address) -> Result<(), StreamError> {
        Self::cancel_stream(env, stream_id, sender)
    }

    fn partial_cancel_stream(
        env: Env,
        stream_id: u64,
        sender: Address,
        cancel_amount: i128,
    ) -> Result<u64, StreamError> {
        Self::partial_cancel_stream(env, stream_id, sender, cancel_amount)
    }

    fn top_up(
        env: Env,
        stream_id: u64,
        sender: Address,
        token: Address,
        amount: i128,
    ) -> Result<(), StreamError> {
        Self::top_up(env, stream_id, sender, token, amount)
    }

    fn get_stream(env: Env, stream_id: u64) -> Result<Stream, StreamError> {
        Self::get_stream(env, stream_id)
    }

    fn get_all_stream_ids(env: Env, start: u32, limit: u32) -> Vec<u64> {
        Self::get_all_stream_ids(env, start, limit)
    }

    fn get_nonce(env: Env, sender: Address) -> u64 {
        Self::get_nonce(env, sender)
    }

    fn get_claimable(env: Env, stream_id: u64) -> Result<i128, StreamError> {
        Self::get_claimable(env, stream_id)
    }

    fn is_participant(env: Env, stream_id: u64, address: Address) -> Result<bool, StreamError> {
        Self::is_participant(env, stream_id, address)
    }

    fn get_streams_by_sender(env: Env, sender: Address, start: u32, limit: u32) -> Vec<Stream> {
        Self::get_streams_by_sender(env, sender, start, limit)
    }

    fn get_streams_by_recipient(
        env: Env,
        recipient: Address,
        start: u32,
        limit: u32,
    ) -> Vec<Stream> {
        Self::get_streams_by_recipient(env, recipient, start, limit)
    }

    fn get_active_streams_by_sender(env: Env, sender: Address) -> Vec<Stream> {
        Self::get_active_streams_by_sender(env, sender)
    }

    fn get_active_streams_by_recipient(env: Env, recipient: Address) -> Vec<Stream> {
        Self::get_active_streams_by_recipient(env, recipient)
    }

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
    ) -> Result<Vec<u64>, StreamError> {
        Self::batch_create_stream(
            env,
            sender,
            recipients,
            amounts,
            tokens,
            duration_seconds,
            auto_renew,
            lock_untils,
            nonce,
        )
    }

    fn batch_withdraw(
        env: Env,
        stream_ids: Vec<u64>,
        recipient: Address,
    ) -> Result<Vec<i128>, StreamError> {
        Self::batch_withdraw(env, stream_ids, recipient)
    }

    fn batch_cancel_stream(env: Env, stream_ids: Vec<u64>, sender: Address) -> Result<Vec<Result<(), StreamError>>, StreamError> {
        Self::batch_cancel_stream(env, stream_ids, sender)
    }

    fn set_protocol_fee(env: Env, fee_bps: u32) -> Result<(), StreamError> {
        Self::set_protocol_fee(env, fee_bps)
    }

    fn set_treasury_address(env: Env, treasury: Address) -> Result<(), StreamError> {
        Self::set_treasury_address(env, treasury)
    }

    fn get_protocol_fee_info(env: Env) -> (u32, Option<Address>) {
        Self::get_protocol_fee_info(env)
    }

    fn get_stats(env: Env) -> Stats {
        Self::get_stats(env)
    }

    fn min_duration(env: Env) -> u64 {
        Self::min_duration(env)
    }

    fn set_min_duration(env: Env, admin: Address, seconds: u64) {
        Self::set_min_duration(env, admin, seconds)
    }

    fn pause_stream(env: Env, stream_id: u64, sender: Address) -> Result<(), StreamError> {
        Self::pause_stream(env, stream_id, sender)
    }

    fn resume_stream(env: Env, stream_id: u64, sender: Address) -> Result<(), StreamError> {
        Self::resume_stream(env, stream_id, sender)
    }

    fn transfer_recipient(env: Env, stream_id: u64, current_recipient: Address, new_recipient: Address) -> Result<(), StreamError> {
        Self::transfer_recipient(env, stream_id, current_recipient, new_recipient)
    }

    fn propose_fee_change(env: Env, admin: Address, new_fee_bps: u32) -> Result<(), StreamError> {
        Self::propose_fee_change(env, admin, new_fee_bps)
    }

    fn execute_fee_change(env: Env) -> Result<(), StreamError> {
        Self::execute_fee_change(env)
    }

    fn recipient_terminate(env: Env, stream_id: u64, recipient: Address) -> Result<(), StreamError> {
        Self::recipient_terminate(env, stream_id, recipient)
    }

    fn add_fee_exempt(env: Env, addr: Address) -> Result<(), StreamError> {
        Self::add_fee_exempt(env, addr)
    }

    fn remove_fee_exempt(env: Env, addr: Address) -> Result<(), StreamError> {
        Self::remove_fee_exempt(env, addr)
    }

    fn is_fee_exempt(env: Env, addr: Address) -> bool {
        Self::is_fee_exempt(env, addr)
    }

    fn set_guardian(env: Env, guardian: Address) -> Result<(), StreamError> {
        Self::set_guardian(env, guardian)
    }

    fn get_guardian(env: Env) -> Option<Address> {
        Self::get_guardian(env)
    }

    fn set_governance(env: Env, governance: Address) -> Result<(), StreamError> {
        Self::set_governance(env, governance)
    }

    fn get_governance(env: Env) -> Option<Address> {
        Self::get_governance(env)
    }

    fn pause(env: Env, guardian: Address) -> Result<(), StreamError> {
        Self::pause(env, guardian)
    }

    fn unpause(env: Env, governance: Address) -> Result<(), StreamError> {
        Self::unpause(env, governance)
    }

    fn get_pause_expiry(env: Env) -> u64 {
        Self::get_pause_expiry(env)
    }

    fn set_creation_fee(env: Env, fee: i128, xlm_token: Address) -> Result<(), StreamError> {
        Self::set_creation_fee(env, fee, xlm_token)
    }

    fn get_creation_fee(env: Env) -> i128 {
        Self::get_creation_fee(env)
    }

    fn recalibrate_stats(env: Env, admin: Address) -> Result<(), StreamError> {
        Self::recalibrate_stats(env, admin)
    }

    fn get_stream_health(env: Env, stream_id: u64) -> Result<types::StreamHealth, StreamError> {
        Self::get_stream_health(env, stream_id)
    }

    fn mark_expired(env: Env, stream_id: u64) -> Result<(), StreamError> {
        Self::mark_expired(env, stream_id)
    }

    fn bump_stream_ttl(env: Env, stream_id: u64) -> Result<(), StreamError> {
        Self::bump_stream_ttl(env, stream_id)
    }

    fn set_delegate(env: Env, sender: Address, stream_id: u64, delegate: Address) -> Result<(), StreamError> {
        Self::set_delegate(env, sender, stream_id, delegate)
    }

    fn revoke_delegate(env: Env, sender: Address, stream_id: u64) -> Result<(), StreamError> {
        Self::revoke_delegate(env, sender, stream_id)
    }

    fn get_delegate(env: Env, stream_id: u64) -> Option<Address> {
        Self::get_delegate(env, stream_id)
    }

    fn migrate(env: Env, from_version: String, to_version: String) -> Result<(), StreamError> {
        Self::migrate(env, from_version, to_version)
    }

    fn get_admin_log(env: Env) -> Vec<AuditEntry> {
        Self::get_admin_log(env)
    }

    fn archive_stream(env: Env, stream_id: u64, caller: Address) -> Result<(), StreamError> {
        Self::archive_stream(env, stream_id, caller)
    }

    // ── Issue #286: Per-token stream cap ────────────────────────────────────

    fn set_max_streams_per_token(env: Env, max: u32) -> Result<(), StreamError> {
        Self::set_max_streams_per_token(env, max)
    }

    fn get_max_streams_per_token(env: Env) -> u32 {
        Self::get_max_streams_per_token(env)
    }

    // ── Issue #284: Address blocklist ───────────────────────────────────────

    fn add_to_blocklist(env: Env, addr: Address) -> Result<(), StreamError> {
        Self::add_to_blocklist(env, addr)
    }

    fn remove_from_blocklist(env: Env, addr: Address) -> Result<(), StreamError> {
        Self::remove_from_blocklist(env, addr)
    }

    fn is_blocked(env: Env, addr: Address) -> bool {
        Self::is_blocked(env, addr)
    }

    // ── Issue #282: Grace period & recovery ─────────────────────────────────

    fn set_grace_period_ledgers(env: Env, ledgers: u32) -> Result<(), StreamError> {
        Self::set_grace_period_ledgers(env, ledgers)
    }

    fn get_grace_period_ledgers(env: Env) -> u32 {
        Self::get_grace_period_ledgers(env)
    }

    fn recover_expired(env: Env, stream_id: u64, sender: Address) -> Result<(), StreamError> {
        Self::recover_expired(env, stream_id, sender)
    }
}
