//! Unit tests for the four new features:
//! (a) StreamExpiryWarning events
//! (b) New-sender stream cap + SenderPromoted
//! (c) Stream redirect chaining
//! (d) Dual-token streams (create_dual_stream)

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, IntoVal, Symbol, Val,
};

// ── Shared test helpers ───────────────────────────────────────────────────────

struct FTestEnv {
    env: Env,
    contract_id: Address,
    token_id: Address,
    token2_id: Address,
    sender: Address,
    recipient: Address,
    admin: Address,
}

fn fsetup() -> FTestEnv {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SoroStreamContract, ());
    let token_admin = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(token_admin.clone()).address();
    let token2_id = env.register_stellar_asset_contract_v2(token_admin.clone()).address();

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);
    let admin = Address::generate(&env);

    StellarAssetClient::new(&env, &token_id).mint(&sender, &10_000_000);
    StellarAssetClient::new(&env, &token2_id).mint(&sender, &10_000_000);

    let c = SoroStreamContractClient::new(&env, &contract_id);
    c.initialize(&admin, &soroban_sdk::String::from_str(&env, "1.0.0"));
    c.set_min_duration(&admin, &0u64);

    FTestEnv { env, contract_id, token_id, token2_id, sender, recipient, admin }
}

fn fclient(t: &FTestEnv) -> SoroStreamContractClient<'_> {
    SoroStreamContractClient::new(&t.env, &t.contract_id)
}

fn make_stream(t: &FTestEnv, nonce: u64, duration: u64) -> u64 {
    fclient(t).create_stream(
        &t.sender, &t.recipient, &t.token_id,
        &100_000, &duration, &0, &nonce, &false, &0u64, &false, &0i128,
    )
}

fn has_event(t: &FTestEnv, name: &str) -> bool {
    t.env.events().all().iter().any(|(_, topics, _)| {
        let v: soroban_sdk::Vec<Val> = topics.clone();
        if v.is_empty() { return false; }
        let sym: Symbol = v.get(0).unwrap().into_val(&t.env);
        sym == Symbol::new(&t.env, name)
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// Feature (a): StreamExpiryWarning
// ═══════════════════════════════════════════════════════════════════════════

/// Default window is 17280 ledgers (~24 h). Admin can change it.
#[test]
fn test_expiry_warning_window_default_and_set() {
    let t = fsetup();
    let c = fclient(&t);

    assert_eq!(c.get_expiry_warning_window(), 17_280u32);

    c.set_expiry_warning_window(&1000u32);
    assert_eq!(c.get_expiry_warning_window(), 1000u32);
}

/// Setting window to 0 is rejected.
#[test]
fn test_expiry_warning_window_zero_rejected() {
    let t = fsetup();
    let result = fclient(&t).try_set_expiry_warning_window(&0u32);
    assert_eq!(result, Err(Ok(StreamError::InvalidExpiryWindow)));
}

/// StreamExpiryWarning is emitted on withdraw when the stream is within the window.
/// A 1000-second stream at t=0; set window to cover the whole stream (200001 ledgers).
/// Withdraw at t=500 (halfway) should emit the warning.
#[test]
fn test_expiry_warning_emitted_on_withdraw_within_window() {
    let t = fsetup();
    let c = fclient(&t);
    t.env.ledger().set_timestamp(0);

    // Window = 200001 ledgers so remaining ~= (1000/5)=200 ledgers is always inside
    c.set_expiry_warning_window(&200_001u32);

    let stream_id = make_stream(&t, 0, 1000);

    t.env.ledger().set_timestamp(500);
    c.withdraw(&stream_id, &t.recipient);

    assert!(has_event(&t, "StreamExpiryWarning"), "expected StreamExpiryWarning event");
}

/// StreamExpiryWarning is NOT emitted when stream is outside the window.
#[test]
fn test_expiry_warning_not_emitted_outside_window() {
    let t = fsetup();
    let c = fclient(&t);
    t.env.ledger().set_timestamp(0);

    // Very small window: 1 ledger. At t=0 with 1000s remaining, 200 ledgers remain — outside window.
    c.set_expiry_warning_window(&1u32);

    let stream_id = make_stream(&t, 0, 1000);

    t.env.ledger().set_timestamp(0);
    c.withdraw(&stream_id, &t.recipient);

    assert!(!has_event(&t, "StreamExpiryWarning"), "unexpected StreamExpiryWarning event");
}

/// Idempotency: second interaction in window does NOT re-emit the warning.
#[test]
fn test_expiry_warning_idempotent() {
    let t = fsetup();
    let c = fclient(&t);
    t.env.ledger().set_timestamp(0);
    c.set_expiry_warning_window(&200_001u32);

    let stream_id = make_stream(&t, 0, 1000);

    // First withdraw at t=500 — should emit warning
    t.env.ledger().set_timestamp(500);
    c.withdraw(&stream_id, &t.recipient);

    let count_after_first = t.env.events().all().iter()
        .filter(|(_, topics, _)| {
            let v: soroban_sdk::Vec<Val> = topics.clone();
            if v.is_empty() { return false; }
            let sym: Symbol = v.get(0).unwrap().into_val(&t.env);
            sym == Symbol::new(&t.env, "StreamExpiryWarning")
        })
        .count();
    assert_eq!(count_after_first, 1, "exactly one StreamExpiryWarning after first withdraw");

    // Second withdraw at t=600 — should NOT re-emit
    t.env.ledger().set_timestamp(600);
    c.withdraw(&stream_id, &t.recipient);

    let count_after_second = t.env.events().all().iter()
        .filter(|(_, topics, _)| {
            let v: soroban_sdk::Vec<Val> = topics.clone();
            if v.is_empty() { return false; }
            let sym: Symbol = v.get(0).unwrap().into_val(&t.env);
            sym == Symbol::new(&t.env, "StreamExpiryWarning")
        })
        .count();
    assert_eq!(count_after_second, 1, "still exactly one StreamExpiryWarning after second withdraw");
}

/// StreamExpiryWarning event data fields match expected values.
#[test]
fn test_expiry_warning_event_fields() {
    let t = fsetup();
    let c = fclient(&t);
    t.env.ledger().set_timestamp(0);
    c.set_expiry_warning_window(&200_001u32);

    let stream_id = make_stream(&t, 0, 1000);

    t.env.ledger().set_timestamp(500);
    c.withdraw(&stream_id, &t.recipient);

    let events = t.env.events().all();
    let warning: std::vec::Vec<_> = events.iter().filter(|(_, topics, _)| {
        let v: soroban_sdk::Vec<Val> = topics.clone();
        if v.is_empty() { return false; }
        let sym: Symbol = v.get(0).unwrap().into_val(&t.env);
        sym == Symbol::new(&t.env, "StreamExpiryWarning")
    }).collect();

    assert_eq!(warning.len(), 1);
    let (_, topics, data) = &warning[0];

    // Topic[1] = stream_id
    let v: soroban_sdk::Vec<Val> = topics.clone();
    let sid: u64 = v.get(1).unwrap().into_val(&t.env);
    assert_eq!(sid, stream_id);

    // Data: (sender, recipient, remaining_balance, ledgers_until_expiry)
    let (s, r, _bal, _ledgers): (Address, Address, i128, u32) = data.clone().into_val(&t.env);
    assert_eq!(s, t.sender);
    assert_eq!(r, t.recipient);
}

// ═══════════════════════════════════════════════════════════════════════════
// Feature (b): New-sender stream cap + SenderPromoted
// ═══════════════════════════════════════════════════════════════════════════

/// A fresh sender is subject to the new-sender cap.
#[test]
fn test_new_sender_cap_enforced() {
    let t = fsetup();
    let c = fclient(&t);

    // Set cap = 2 streams, threshold = 100 (far away)
    c.set_new_sender_stream_cap(&2u32);
    c.set_sender_promotion_threshold(&100u32);

    // Create 2 streams — both succeed
    make_stream(&t, 0, 1000);
    make_stream(&t, 1, 1000);

    // 3rd stream should hit the cap
    let result = c.try_create_stream(
        &t.sender, &t.recipient, &t.token_id,
        &100_000, &1000, &0, &2u64, &false, &0u64, &false, &0i128,
    );
    assert_eq!(result, Err(Ok(StreamError::NewSenderStreamCapExceeded)));
}

/// After cancelling a stream the sender can create another (slot freed).
#[test]
fn test_new_sender_cap_lifted_after_cancel() {
    let t = fsetup();
    let c = fclient(&t);
    c.set_new_sender_stream_cap(&1u32);
    c.set_sender_promotion_threshold(&100u32);

    let id = make_stream(&t, 0, 1000);

    // Cancel frees the active slot
    c.cancel_stream(&id, &t.sender);

    // Now lifetime count = 1 (still below threshold=100), active = 0 — cap not hit
    make_stream(&t, 1, 1000);
}

/// Lifetime count is tracked persistently and increments on each creation.
#[test]
fn test_sender_lifetime_count_increments() {
    let t = fsetup();
    let c = fclient(&t);

    assert_eq!(c.get_sender_lifetime_count(&t.sender), 0);
    make_stream(&t, 0, 1000);
    assert_eq!(c.get_sender_lifetime_count(&t.sender), 1);
    make_stream(&t, 1, 1000);
    assert_eq!(c.get_sender_lifetime_count(&t.sender), 2);
}

/// SenderPromoted event is emitted exactly once when crossing the threshold.
#[test]
fn test_sender_promoted_event_emitted_at_threshold() {
    let t = fsetup();
    let c = fclient(&t);

    // Threshold = 2: after creating 2 streams the sender is promoted
    c.set_new_sender_stream_cap(&10u32);
    c.set_sender_promotion_threshold(&2u32);

    assert!(!c.is_sender_promoted(&t.sender));
    make_stream(&t, 0, 1000);
    assert!(!c.is_sender_promoted(&t.sender));

    make_stream(&t, 1, 1000);
    assert!(c.is_sender_promoted(&t.sender));
    assert!(has_event(&t, "SenderPromoted"));
}

/// After promotion, the new-sender cap no longer applies.
#[test]
fn test_promoted_sender_bypasses_cap() {
    let t = fsetup();
    let c = fclient(&t);

    // Cap = 1, threshold = 2 — after 2 creations, cap is lifted
    c.set_new_sender_stream_cap(&1u32);
    c.set_sender_promotion_threshold(&2u32);

    // First stream — succeeds (cap=1, active=0)
    make_stream(&t, 0, 1000);
    // Second stream would normally be blocked by cap=1, but this is the threshold-crossing one
    make_stream(&t, 1, 1000);

    // Now promoted — can create even though active streams > original cap
    make_stream(&t, 2, 1000);
}

/// SenderPromoted event fires only once even with many subsequent creations.
#[test]
fn test_sender_promoted_event_fires_once() {
    let t = fsetup();
    let c = fclient(&t);
    c.set_new_sender_stream_cap(&10u32);
    c.set_sender_promotion_threshold(&2u32);

    make_stream(&t, 0, 1000);
    make_stream(&t, 1, 1000); // crosses threshold
    make_stream(&t, 2, 1000);
    make_stream(&t, 3, 1000);

    let promoted_count = t.env.events().all().iter()
        .filter(|(_, topics, _)| {
            let v: soroban_sdk::Vec<Val> = topics.clone();
            if v.is_empty() { return false; }
            let sym: Symbol = v.get(0).unwrap().into_val(&t.env);
            sym == Symbol::new(&t.env, "SenderPromoted")
        })
        .count();
    assert_eq!(promoted_count, 1, "SenderPromoted should emit exactly once");
}

// ═══════════════════════════════════════════════════════════════════════════
// Feature (c): Stream redirect chaining
// ═══════════════════════════════════════════════════════════════════════════

/// Recipient can set and retrieve a redirect target.
#[test]
fn test_set_redirect_stores_target() {
    let t = fsetup();
    let c = fclient(&t);
    t.env.ledger().set_timestamp(0);

    let src = make_stream(&t, 0, 2000);
    let tgt = make_stream(&t, 1, 2000);

    assert_eq!(c.get_redirect(&src), None);
    c.set_redirect(&src, &tgt, &t.recipient);
    assert_eq!(c.get_redirect(&src), Some(tgt));
}

/// Redirect emits StreamRedirectSet event with correct fields.
#[test]
fn test_set_redirect_emits_event() {
    let t = fsetup();
    let c = fclient(&t);
    t.env.ledger().set_timestamp(0);

    let src = make_stream(&t, 0, 2000);
    let tgt = make_stream(&t, 1, 2000);
    c.set_redirect(&src, &tgt, &t.recipient);

    assert!(has_event(&t, "StreamRedirectSet"));
}

/// Recipient can clear a redirect; get_redirect returns None afterwards.
#[test]
fn test_clear_redirect() {
    let t = fsetup();
    let c = fclient(&t);
    t.env.ledger().set_timestamp(0);

    let src = make_stream(&t, 0, 2000);
    let tgt = make_stream(&t, 1, 2000);
    c.set_redirect(&src, &tgt, &t.recipient);
    c.clear_redirect(&src, &t.recipient);

    assert_eq!(c.get_redirect(&src), None);
    assert!(has_event(&t, "StreamRedirectCleared"));
}

/// Non-recipient cannot set a redirect.
#[test]
fn test_set_redirect_rejected_for_non_recipient() {
    let t = fsetup();
    let c = fclient(&t);
    t.env.ledger().set_timestamp(0);

    let src = make_stream(&t, 0, 2000);
    let tgt = make_stream(&t, 1, 2000);
    let other = Address::generate(&t.env);

    let result = c.try_set_redirect(&src, &tgt, &other);
    assert_eq!(result, Err(Ok(StreamError::NotRecipient)));
}

/// Redirect to a non-existent stream is rejected.
#[test]
fn test_set_redirect_invalid_target_rejected() {
    let t = fsetup();
    let c = fclient(&t);
    t.env.ledger().set_timestamp(0);

    let src = make_stream(&t, 0, 2000);
    let result = c.try_set_redirect(&src, &999999u64, &t.recipient);
    assert_eq!(result, Err(Ok(StreamError::InvalidRedirectTarget)));
}

/// Redirect to a stream with a different recipient is rejected.
#[test]
fn test_set_redirect_recipient_mismatch_rejected() {
    let t = fsetup();
    let c = fclient(&t);
    t.env.ledger().set_timestamp(0);

    let other_recipient = Address::generate(&t.env);
    StellarAssetClient::new(&t.env, &t.token_id).mint(&t.sender, &200_000);

    let src = make_stream(&t, 0, 2000);
    // Create a stream with a different recipient
    let tgt = c.create_stream(
        &t.sender, &other_recipient, &t.token_id,
        &100_000, &2000, &0, &1u64, &false, &0u64, &false, &0i128,
    );

    let result = c.try_set_redirect(&src, &tgt, &t.recipient);
    assert_eq!(result, Err(Ok(StreamError::RedirectRecipientMismatch)));
}

/// Direct circular redirect A→A is rejected.
#[test]
fn test_circular_redirect_self_rejected() {
    let t = fsetup();
    let c = fclient(&t);
    t.env.ledger().set_timestamp(0);

    let src = make_stream(&t, 0, 2000);
    let result = c.try_set_redirect(&src, &src, &t.recipient);
    assert_eq!(result, Err(Ok(StreamError::CircularRedirect)));
}

/// Indirect circular redirect A→B→A is rejected when setting B's redirect to A.
#[test]
fn test_circular_redirect_indirect_rejected() {
    let t = fsetup();
    let c = fclient(&t);
    t.env.ledger().set_timestamp(0);

    let a = make_stream(&t, 0, 3000);
    let b = make_stream(&t, 1, 3000);

    // Set A → B
    c.set_redirect(&a, &b, &t.recipient);

    // Setting B → A would create A→B→A cycle
    let result = c.try_set_redirect(&b, &a, &t.recipient);
    assert_eq!(result, Err(Ok(StreamError::CircularRedirect)));
}

/// Withdraw with redirect active: StreamRedirected event is emitted.
#[test]
fn test_redirect_withdraw_emits_redirected_event() {
    let t = fsetup();
    let c = fclient(&t);
    t.env.ledger().set_timestamp(0);

    let src = make_stream(&t, 0, 2000);
    let tgt = make_stream(&t, 1, 4000);

    c.set_redirect(&src, &tgt, &t.recipient);

    t.env.ledger().set_timestamp(500);
    c.withdraw(&src, &t.recipient);

    assert!(has_event(&t, "StreamRedirected"));
}

// ═══════════════════════════════════════════════════════════════════════════
// Feature (d): Dual-token streams
// ═══════════════════════════════════════════════════════════════════════════

fn make_dual_stream(t: &FTestEnv, nonce: u64, duration: u64) -> u64 {
    fclient(t).create_dual_stream(
        &t.sender, &t.recipient,
        &t.token_id, &100_000,
        &t.token2_id, &200_000,
        &duration, &0, &nonce, &0u64, &false,
    )
}

/// create_dual_stream creates a single on-chain record covering two tokens.
#[test]
fn test_dual_stream_created_single_record() {
    let t = fsetup();
    let c = fclient(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = make_dual_stream(&t, 0, 1000);
    let stream = c.get_stream(&stream_id);

    assert!(stream.is_dual_stream);
    assert_eq!(stream.deposit, 100_000i128);  // token1 deposit
    assert_eq!(stream.token, t.token_id);
    assert_eq!(stream.status, StreamStatus::Active);
}

/// DualStreamCreated event is emitted with both token amounts.
#[test]
fn test_dual_stream_created_event() {
    let t = fsetup();
    t.env.ledger().set_timestamp(0);
    make_dual_stream(&t, 0, 1000);
    assert!(has_event(&t, "DualStreamCreated"));
}

/// create_dual_stream rejects identical token addresses.
#[test]
fn test_dual_stream_same_token_rejected() {
    let t = fsetup();
    let result = fclient(&t).try_create_dual_stream(
        &t.sender, &t.recipient,
        &t.token_id, &100_000,
        &t.token_id, &200_000,   // same token
        &1000, &0, &0u64, &0u64, &false,
    );
    assert_eq!(result, Err(Ok(StreamError::DuplicateTokenInDualStream)));
}

/// create_dual_stream rejects zero amount for either token.
#[test]
fn test_dual_stream_zero_amount_rejected() {
    let t = fsetup();
    let r1 = fclient(&t).try_create_dual_stream(
        &t.sender, &t.recipient,
        &t.token_id, &0i128,
        &t.token2_id, &200_000,
        &1000, &0, &0u64, &0u64, &false,
    );
    assert_eq!(r1, Err(Ok(StreamError::ZeroAmount)));

    let r2 = fclient(&t).try_create_dual_stream(
        &t.sender, &t.recipient,
        &t.token_id, &100_000,
        &t.token2_id, &0i128,
        &1000, &0, &1u64, &0u64, &false,
    );
    assert_eq!(r2, Err(Ok(StreamError::ZeroAmount)));
}

/// withdraw distributes both tokens proportionally in a single transaction.
#[test]
fn test_dual_stream_withdraw_distributes_both_tokens() {
    let t = fsetup();
    let c = fclient(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = make_dual_stream(&t, 0, 1000);

    t.env.ledger().set_timestamp(500);
    c.withdraw(&stream_id, &t.recipient);

    // token1: flow_rate = 100_000/1000 = 100 stroops/s → 500s → 50_000
    let bal1 = TokenClient::new(&t.env, &t.token_id).balance(&t.recipient);
    assert_eq!(bal1, 50_000i128);

    // token2: flow_rate = 200_000/1000 = 200 stroops/s → 500s → 100_000
    let bal2 = TokenClient::new(&t.env, &t.token2_id).balance(&t.recipient);
    assert_eq!(bal2, 100_000i128);
}

/// DualStreamWithdrawn event is emitted on withdraw.
#[test]
fn test_dual_stream_withdrawn_event() {
    let t = fsetup();
    let c = fclient(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = make_dual_stream(&t, 0, 1000);
    t.env.ledger().set_timestamp(500);
    c.withdraw(&stream_id, &t.recipient);

    assert!(has_event(&t, "DualStreamWithdrawn"));
}

/// cancel_stream refunds both token amounts to sender proportionally.
#[test]
fn test_dual_stream_cancel_refunds_both_tokens() {
    let t = fsetup();
    let c = fclient(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = make_dual_stream(&t, 0, 1000);

    let sender_tok1_before = TokenClient::new(&t.env, &t.token_id).balance(&t.sender);
    let sender_tok2_before = TokenClient::new(&t.env, &t.token2_id).balance(&t.sender);

    // Cancel at t=200: elapsed=200, flow1=100, flow2=200
    t.env.ledger().set_timestamp(200);
    c.cancel_stream(&stream_id, &t.sender);

    let earned1 = 100i128 * 200;   // 20_000
    let earned2 = 200i128 * 200;   // 40_000
    let refund1 = 100_000 - earned1;  // 80_000
    let refund2 = 200_000 - earned2;  // 160_000

    let rec_tok1 = TokenClient::new(&t.env, &t.token_id).balance(&t.recipient);
    let rec_tok2 = TokenClient::new(&t.env, &t.token2_id).balance(&t.recipient);
    assert_eq!(rec_tok1, earned1);
    assert_eq!(rec_tok2, earned2);

    let snd_tok1 = TokenClient::new(&t.env, &t.token_id).balance(&t.sender);
    let snd_tok2 = TokenClient::new(&t.env, &t.token2_id).balance(&t.sender);
    assert_eq!(snd_tok1 - sender_tok1_before, refund1);
    assert_eq!(snd_tok2 - sender_tok2_before, refund2);
}

/// DualStreamCancelled event is emitted on cancel.
#[test]
fn test_dual_stream_cancelled_event() {
    let t = fsetup();
    let c = fclient(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = make_dual_stream(&t, 0, 1000);
    t.env.ledger().set_timestamp(200);
    c.cancel_stream(&stream_id, &t.sender);

    assert!(has_event(&t, "DualStreamCancelled"));
}

/// Both streams share start_time, end_time, and cliff configuration.
#[test]
fn test_dual_stream_shares_timing() {
    let t = fsetup();
    let c = fclient(&t);
    t.env.ledger().set_timestamp(100);

    let stream_id = fclient(&t).create_dual_stream(
        &t.sender, &t.recipient,
        &t.token_id, &100_000,
        &t.token2_id, &200_000,
        &1000, &500, &0u64, &0u64, &false,
    );
    let stream = c.get_stream(&stream_id);

    assert_eq!(stream.start_time, 100);
    assert_eq!(stream.end_time, 1100);
    assert_eq!(stream.cliff_time, 600);
}

/// top_up is rejected on dual-token streams (use token directly, not via top_up).
#[test]
fn test_dual_stream_top_up_rejected() {
    let t = fsetup();
    let c = fclient(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = make_dual_stream(&t, 0, 1000);
    let result = c.try_top_up(&stream_id, &t.sender, &t.token_id, &10_000);
    assert_eq!(result, Err(Ok(StreamError::IsDualStream)));
}
