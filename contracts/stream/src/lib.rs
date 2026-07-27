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
pub use types::{AuditEntry, Stream, Stats, StreamStatus, VestingCurve};
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
    accumulate_fees, add_fee_exempt, add_rate_limit_exempt, add_to_whitelist,
    add_token_to_whitelist, append_audit_entry, check_admin, clear_pending_fee_proposal,
    clear_reentrancy_lock, cleanup_dual_stream_storage, decrement_active_stream_count,
    derive_stream_id, drain_fees_collected, effective_sender_limit, get_active_stream_count,
    get_batch_nonce, get_creation_fee_xlm, get_delegate, get_dual_stream_deposit2,
    get_dual_stream_token2, get_dual_stream_withdrawn2, get_expiry_warning_window,
    get_federation_address, get_fees_collected, get_global_stream_at, get_global_stream_count,
    get_holdback, get_ids_by_recipient, get_ids_by_sender, get_new_sender_stream_cap,
    get_pause_expiry, get_protocol_fee, get_rate_limit_max_creations, get_rate_limit_state,
    get_rate_limit_window, get_sender_last_creation_time, get_sender_lifetime_count,
    get_sender_promotion_threshold, get_sender_stream_count, get_slippage_params,
    get_stream_creation_cooldown, get_treasury, get_withdrawal_cooldown, get_xlm_token,
    increment_active_stream_count, increment_batch_nonce, increment_dual_stream_withdrawn2,
    increment_sender_lifetime_count, index_by_recipient, index_by_sender, index_global_stream,
    is_fee_exempt, is_paused_or_auto_unpause, is_rate_limit_exempt, is_reentrancy_locked,
    is_sender_promoted, is_token_whitelist_enabled, is_token_whitelisted, is_whitelist_enabled,
    is_whitelisted, load_stream, load_tranches, mark_nonce_used, nonce_used, read_admin,
    read_applied_migrations, read_audit_log, read_governance, read_guardian, read_max_duration,
    read_min_duration, read_pending_fee_proposal, read_version, record_migration,
    register_federation_address, remove_delegate, remove_fee_exempt, remove_from_whitelist,
    remove_holdback, remove_rate_limit_exempt, remove_stream, remove_token_from_whitelist,
    remove_tranches, save_stream, save_tranches, sender_count_key, sender_slot_key,
    set_active_stream_count, set_creation_fee_xlm, set_delegate, set_dual_stream_deposit2,
    set_dual_stream_token2, set_dual_stream_withdrawn2, set_expiry_warning_window,
    set_holdback, set_max_streams_per_sender, set_new_sender_stream_cap, set_pause_expiry,
    set_paused, set_protocol_fee, set_rate_limit_max_creations, set_rate_limit_state,
    set_rate_limit_window, set_reentrancy_lock, set_sender_last_creation_time, set_sender_limit,
    set_sender_promotion_threshold, set_slippage_params, set_stream_creation_cooldown,
    set_token_whitelist_enabled, set_treasury, set_whitelist_enabled, set_withdrawal_cooldown,
    set_xlm_token, stream_exists, unindex_by_recipient, unindex_by_sender,
    unregister_federation_address, write_admin, write_governance, write_guardian,
    write_max_duration, write_min_duration, write_pending_fee_proposal, write_version,
    MAX_PAUSE_DURATION,
};

// ── Helper: checked multiply ──────────────────────────────────────────────────
fn checked_flow_amount(flow_rate: i128, elapsed: u64) -> Result<i128, StreamError> {
    flow_rate.checked_mul(elapsed as i128).ok_or(StreamError::Overflow)
}

const MAX_STREAM_DURATION_SECONDS: u64 = 100 * 365 * 24 * 60 * 60;

// ── Helper: validate metadata URI ────────────────────────────────────────────
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
