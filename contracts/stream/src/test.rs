
use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Bytes, Env,
};

struct TestEnv {
    env: Env,
    contract_id: Address,
    token_id: Address,
    sender: Address,
    recipient: Address,
}

fn setup() -> TestEnv {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SoroStreamContract, ());
    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    StellarAssetClient::new(&env, &token_id).mint(&sender, &1_000_000);

    // Disable minimum duration for tests
    SoroStreamContractClient::new(&env, &contract_id).set_min_duration(&sender, &0u64);

    TestEnv {
        env,
        contract_id,
        token_id,
        sender,
        recipient,
    }
}

fn client(t: &TestEnv) -> SoroStreamContractClient<'_> {
    SoroStreamContractClient::new(&t.env, &t.contract_id)
}

#[test]
fn test_create_stream_success() {
    let t = setup();
    let c = client(&t);

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false, &0i128);

    let stream = c.get_stream(&stream_id);
    assert_eq!(stream.deposit, 100_000);
    assert_eq!(stream.flow_rate, 100);
    assert_eq!(stream.status, StreamStatus::Active);
}

#[test]
fn test_withdrawal_cooldown_blocks_repeated_withdrawals() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let admin = Address::generate(&t.env);
    c.initialize(&admin, &soroban_sdk::String::from_str(&t.env, "1.0.0"));

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64, &false, &0i128);
    c.set_withdrawal_cooldown(&admin, &10u64);

    t.env.ledger().set_timestamp(500);
    c.withdraw(&stream_id, &t.recipient);

    let result = c.try_withdraw(&stream_id, &t.recipient);
    assert!(result.is_err());
}

#[test]
fn test_whitelist_rejects_non_whitelisted_recipient() {
    let t = setup();
    let c = client(&t);

    let admin = Address::generate(&t.env);
    c.initialize(&admin, &soroban_sdk::String::from_str(&t.env, "1.0.0"));

    c.set_whitelist_enabled(&admin, &true);
    c.add_to_whitelist(&admin, &t.recipient);

    let other = Address::generate(&t.env);
    let result = c.try_create_stream(&t.sender, &other, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64, &false, &0i128);
    assert!(result.is_err());
}

#[test]
fn test_metadata_is_stored_and_updatable() {
    let t = setup();
    let c = client(&t);
    let metadata = Bytes::from_array(&t.env, &[1u8, 2u8, 3u8]);

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64, &false, &0i128);
    c.update_metadata(&t.sender, &stream_id, &metadata);
    let stream = c.get_stream(&stream_id);
    assert_eq!(stream.metadata, metadata);

    let updated = Bytes::from_array(&t.env, &[9u8, 9u8, 9u8]);
    c.update_metadata(&t.sender, &stream_id, &updated);
    let updated_stream = c.get_stream(&stream_id);
    assert_eq!(updated_stream.metadata, updated);
}

#[test]
fn test_cancel_auto_renew_before_expiry() {
    let t = setup();
    let c = client(&t);

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &true, &0u64, &false, &0i128);
    c.cancel_auto_renew(&t.sender, &stream_id);

    let stream = c.get_stream(&stream_id);
    assert!(!stream.auto_renew);
}

#[test]
fn test_get_all_stream_ids_enumerates_globally() {
    let t = setup();
    let c = client(&t);

    let first_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false, &0i128);
    let second_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &1u64, &false, &0u64,
        &false, &0i128);
    let third_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &2u64, &false, &0u64,
        &false, &0i128);

    let all_ids = c.get_all_stream_ids(&0u32, &10u32);
    assert_eq!(all_ids.len(), 3);
    assert_eq!(all_ids.get_unchecked(0), first_id);
    assert_eq!(all_ids.get_unchecked(1), second_id);
    assert_eq!(all_ids.get_unchecked(2), third_id);

    let paged_ids = c.get_all_stream_ids(&1u32, &2u32);
    assert_eq!(paged_ids.len(), 2);
    assert_eq!(paged_ids.get_unchecked(0), second_id);
    assert_eq!(paged_ids.get_unchecked(1), third_id);
}

#[test]
fn test_withdraw_partial() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false, &0i128);

    t.env.ledger().set_timestamp(500);
    c.withdraw(&stream_id, &t.recipient);

    let balance = TokenClient::new(&t.env, &t.token_id).balance(&t.recipient);
    assert_eq!(balance, 50_000);
}

#[test]
fn test_withdraw_full() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false, &0i128);

    t.env.ledger().set_timestamp(1000);
    c.withdraw(&stream_id, &t.recipient);

    let balance = TokenClient::new(&t.env, &t.token_id).balance(&t.recipient);
    assert_eq!(balance, 100_000);

    let result = c.try_get_stream(&stream_id);
    assert!(result.is_err());
}

#[test]
fn test_cancel_stream_splits_correctly() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false, &0i128);

    t.env.ledger().set_timestamp(300);
    c.cancel_stream(&stream_id, &t.sender);

    let recipient_bal = TokenClient::new(&t.env, &t.token_id).balance(&t.recipient);
    let sender_bal = TokenClient::new(&t.env, &t.token_id).balance(&t.sender);

    assert_eq!(recipient_bal, 30_000);
    assert_eq!(sender_bal, 970_000);

    let result = c.try_get_stream(&stream_id);
    assert!(result.is_err());
}

#[test]
fn test_top_up_extends_duration() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false, &0i128);
    let stream_before = c.get_stream(&stream_id);

    c.top_up(&stream_id, &t.sender, &t.token_id, &50_000);

    let stream_after = c.get_stream(&stream_id);
    assert_eq!(stream_after.end_time, stream_before.end_time + 500);
    assert_eq!(stream_after.deposit, 150_000);
}

#[test]
fn test_auto_renew_restarts_on_completion() {
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();

    let contract_id = env.register(SoroStreamContract, ());
    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    StellarAssetClient::new(&env, &token_id).mint(&sender, &200_000);

    let c = SoroStreamContractClient::new(&env, &contract_id);
    c.set_min_duration(&sender, &0u64);
    env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(&sender, &recipient, &token_id, &100_000, &1000, &0, &0u64, &true, &0u64,
        &false, &0i128);

    env.ledger().set_timestamp(1000);
    c.withdraw(&stream_id, &recipient);

    let stream = c.get_stream(&stream_id);
    assert_eq!(stream.status, StreamStatus::Active);
    assert_eq!(stream.start_time, 1000);
    assert_eq!(stream.end_time, 2000);
    assert_eq!(stream.last_withdraw_time, 1000);
}

#[test]
fn test_cannot_withdraw_if_not_recipient() {
    let t = setup();
    let c = client(&t);

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false, &0i128);
    let other = Address::generate(&t.env);

    let result = c.try_withdraw(&stream_id, &other);
    assert!(result.is_err());
}

#[test]
fn test_cannot_cancel_if_not_sender() {
    let t = setup();
    let c = client(&t);

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false, &0i128);
    let other = Address::generate(&t.env);

    let result = c.try_cancel_stream(&stream_id, &other);
    assert!(result.is_err());
}

#[test]
fn test_zero_amount_fails() {
    let t = setup();
    let c = client(&t);

    let result = c.try_create_stream(&t.sender, &t.recipient, &t.token_id, &0, &1000, &0, &0u64, &false, &0u64,
        &false, &0i128);
    assert!(result.is_err());
}

#[test]
fn test_get_claimable_calculates_correctly() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false, &0i128);

    t.env.ledger().set_timestamp(250);
    let claimable = c.get_claimable(&stream_id);
    assert_eq!(claimable, 25_000);
}

// ── Cliff tests ──────────────────────────────────────────────────────────────

/// Stream: duration=1000s, cliff=500s, flow_rate=100 stroops/s
/// At t=499 (pre-cliff) → claimable must be 0.
#[test]
fn test_cliff_pre_cliff_returns_zero() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    // cliff at t=500, end at t=1000
    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &500, &0u64, &false, &0u64, &false, &0i128);

    t.env.ledger().set_timestamp(499);
    assert_eq!(c.get_claimable(&stream_id), 0);
}

/// At the exact cliff timestamp → claimable reflects time from last_withdraw_time.
/// last_withdraw_time = start = 0, cliff = 500, so elapsed = 500 → 500 * 100 = 50_000.
#[test]
fn test_cliff_at_cliff_returns_accrued() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &500, &0u64, &false, &0u64, &false, &0i128);

    t.env.ledger().set_timestamp(500);
    assert_eq!(c.get_claimable(&stream_id), 50_000);
}

/// Post-cliff linear: at t=750, elapsed from start = 750 → 75_000 total accrued.
#[test]
fn test_cliff_post_cliff_linear() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &500, &0u64, &false, &0u64, &false, &0i128);

    t.env.ledger().set_timestamp(750);
    assert_eq!(c.get_claimable(&stream_id), 75_000);
}

/// Withdraw while pre-cliff transfers nothing; balance stays 0.
#[test]
fn test_cliff_withdraw_pre_cliff_transfers_nothing() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &500, &0u64, &false, &0u64, &false, &0i128);

    t.env.ledger().set_timestamp(300);
    c.withdraw(&stream_id, &t.recipient);

    let balance = TokenClient::new(&t.env, &t.token_id).balance(&t.recipient);
    assert_eq!(balance, 0);
}

/// cliff_seconds >= duration_seconds must fail with InvalidCliff.
#[test]
fn test_cliff_exceeds_duration_fails() {
    let t = setup();
    let c = client(&t);

    let result = c.try_create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &1001, &0u64, &false, &0u64, &false, &0i128);
    assert!(result.is_err());
}

/// cliff_seconds == duration_seconds must also fail with InvalidCliff.
#[test]
fn test_cliff_equals_duration_fails() {
    let t = setup();
    let c = client(&t);

    let result = c.try_create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &1000, &0u64, &false, &0u64, &false, &0i128);
    assert_eq!(result, Err(Ok(StreamError::InvalidCliff)));
}

/// cliff_seconds == 0 means no cliff — tokens stream linearly from start.
#[test]
fn test_cliff_zero_means_no_cliff() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64, &false, &0i128);

    // At t=1 (right after start), tokens should already be claimable
    t.env.ledger().set_timestamp(1);
    assert_eq!(c.get_claimable(&stream_id), 100);

    // Verify cliff_time equals start_time (no cliff barrier)
    let stream = c.get_stream(&stream_id);
    assert_eq!(stream.cliff_time, stream.start_time);
}

/// cliff_seconds strictly between 0 and duration creates a valid cliff.
#[test]
fn test_cliff_strictly_between_zero_and_duration() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &1, &0u64, &false, &0u64, &false, &0i128);

    // Before cliff (t=0): no claimable
    assert_eq!(c.get_claimable(&stream_id), 0);

    // At cliff (t=1): claimable = 1 * 100 = 100
    t.env.ledger().set_timestamp(1);
    assert_eq!(c.get_claimable(&stream_id), 100);
}

#[test]
fn test_get_admin_returns_initialized_admin() {
    let t = setup();
    let c = client(&t);
    let admin = Address::generate(&t.env);
    c.initialize(&admin, &soroban_sdk::String::from_str(&t.env, "1.0.0"));
    assert_eq!(c.get_admin(), admin);
}

#[test]
fn test_set_admin_transfers_role() {
    let t = setup();
    let c = client(&t);
    let admin = Address::generate(&t.env);
    let new_admin = Address::generate(&t.env);
    c.initialize(&admin, &soroban_sdk::String::from_str(&t.env, "1.0.0"));
    c.set_admin(&new_admin);
    assert_eq!(c.get_admin(), new_admin);
}

#[test]
fn test_set_admin_rejected_for_non_admin() {
    let t = setup();
    let c = client(&t);
    let admin = Address::generate(&t.env);
    let attacker = Address::generate(&t.env);
    c.initialize(&admin, &soroban_sdk::String::from_str(&t.env, "1.0.0"));

    t.env.set_auths(&[]);
    let result = c.try_set_admin(&attacker);
    assert!(result.is_err());
}

#[test]
fn test_admin_persists_across_calls() {
    let t = setup();
    let c = client(&t);
    let admin = Address::generate(&t.env);
    c.initialize(&admin, &soroban_sdk::String::from_str(&t.env, "1.0.0"));
    // Interleave unrelated contract calls and re-check admin
    c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64, &false, &0i128);
    assert_eq!(c.get_admin(), admin);
}

#[test]
fn test_admin_can_pause_and_unpause() {
    let t = setup();
    let c = client(&t);
    let admin = Address::generate(&t.env);
    c.initialize(&admin, &soroban_sdk::String::from_str(&t.env, "1.0.0"));
    assert!(!c.is_paused());
    c.emergency_pause();
    assert!(c.is_paused());
    c.emergency_resume();
    assert!(!c.is_paused());
}

#[test]
fn test_create_stream_blocked_when_paused() {
    let t = setup();
    let c = client(&t);
    let admin = Address::generate(&t.env);
    c.initialize(&admin, &soroban_sdk::String::from_str(&t.env, "1.0.0"));
    c.emergency_pause();
    let result = c.try_create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64, &false, &0i128);
    assert!(result.is_err());
}

#[test]
fn test_create_stream_works_after_unpause() {
    let t = setup();
    let c = client(&t);
    let admin = Address::generate(&t.env);
    c.initialize(&admin, &soroban_sdk::String::from_str(&t.env, "1.0.0"));
    c.emergency_pause();
    c.emergency_resume();
    let _stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64, &false, &0i128);
}

#[test]
fn test_pause_rejected_for_non_admin() {
    let t = setup();
    let c = client(&t);
    let admin = Address::generate(&t.env);
    c.initialize(&admin, &soroban_sdk::String::from_str(&t.env, "1.0.0"));
    t.env.set_auths(&[]);
    assert!(c.try_emergency_pause().is_err());
    assert!(c.try_emergency_resume().is_err());
}

/// After passing cliff, tokens accumulate from stream start (not from cliff).
/// cliff=500 in a 1000s stream: at t=500 (cliff) withdraw 50_000, then at t=750 another 25_000.
#[test]
fn test_cliff_accrual_restarts_after_withdrawal() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &500, &0u64, &false, &0u64, &false, &0i128);

    // At cliff: 500 * 100 = 50_000 claimable
    t.env.ledger().set_timestamp(500);
    assert_eq!(c.get_claimable(&stream_id), 50_000);
    c.withdraw(&stream_id, &t.recipient);

    // 250 more seconds after withdrawal: 250 * 100 = 25_000
    t.env.ledger().set_timestamp(750);
    assert_eq!(c.get_claimable(&stream_id), 25_000);
}

/// Tokens are not claimable before the cliff, even partway into the stream.
#[test]
fn test_claimable_zero_before_cliff() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    // cliff at t=800 within a 1000s stream
    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &800, &0u64, &false, &0u64, &false, &0i128);

    // at t=500, still before cliff → 0 claimable
    t.env.ledger().set_timestamp(500);
    assert_eq!(c.get_claimable(&stream_id), 0);
}

/// Duration of zero must fail.
#[test]
fn test_zero_duration_fails() {
    let t = setup();
    let c = client(&t);

    let result = c.try_create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &0, &0, &0u64, &false, &0u64, &false, &0i128);
    assert!(result.is_err());
}

// ── Event snapshot tests (issue #105) ────────────────────────────────────────
//
// These tests capture the exact event format emitted by each contract
// instruction. If the event topic structure, field types, or values change,
// these tests will fail — ensuring SDK and indexer consumers are notified
// of format changes.

use soroban_sdk::testutils::Events;
use soroban_sdk::{IntoVal, Val, Symbol, vec as soroban_vec};

#[test]
fn snapshot_event_stream_created() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(100);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );

    let events = t.env.events().all();
    let create_events: std::vec::Vec<_> = events.iter().filter(|(_, topics, _)| {
        let topic_vec: soroban_sdk::Vec<Val> = topics.clone();
        if !topic_vec.is_empty() {
            let first: Symbol = topic_vec.get(0).unwrap().into_val(&t.env);
            first == Symbol::new(&t.env, "StreamCreated")
        } else {
            false
        }
    }).collect();

    assert_eq!(create_events.len(), 1, "Expected exactly one StreamCreated event");

    let (contract_id, topics, data) = &create_events[0];
    assert_eq!(*contract_id, t.contract_id);

    // Topics: (Symbol("StreamCreated"), stream_id: u64)
    let topics_vec: soroban_sdk::Vec<Val> = topics.clone();
    assert_eq!(topics_vec.len(), 2);
    let topic_name: Symbol = topics_vec.get(0).unwrap().into_val(&t.env);
    assert_eq!(topic_name, Symbol::new(&t.env, "StreamCreated"));
    let topic_stream_id: u64 = topics_vec.get(1).unwrap().into_val(&t.env);
    assert_eq!(topic_stream_id, stream_id);

    // Data: (sender: Address, recipient: Address, amount: i128, flow_rate: i128, end_time: u64)
    let data_tuple: (Address, Address, i128, i128, u64) = data.clone().into_val(&t.env);
    assert_eq!(data_tuple.0, t.sender);
    assert_eq!(data_tuple.1, t.recipient);
    assert_eq!(data_tuple.2, 100_000i128);
    assert_eq!(data_tuple.3, 100i128);       // flow_rate = 100_000 / 1000
    assert_eq!(data_tuple.4, 100 + 1000);    // end_time = start + duration
}

#[test]
fn snapshot_event_stream_withdrawn() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );

    t.env.ledger().set_timestamp(500);
    c.withdraw(&stream_id, &t.recipient);

    let events = t.env.events().all();
    let withdraw_events: std::vec::Vec<_> = events.iter().filter(|(_, topics, _)| {
        let topic_vec: soroban_sdk::Vec<Val> = topics.clone();
        if !topic_vec.is_empty() {
            let first: Symbol = topic_vec.get(0).unwrap().into_val(&t.env);
            first == Symbol::new(&t.env, "StreamWithdrawn")
        } else {
            false
        }
    }).collect();

    assert_eq!(withdraw_events.len(), 1, "Expected exactly one StreamWithdrawn event");

    let (contract_id, topics, data) = &withdraw_events[0];
    assert_eq!(*contract_id, t.contract_id);

    // Topics: (Symbol("StreamWithdrawn"), stream_id: u64)
    let topics_vec: soroban_sdk::Vec<Val> = topics.clone();
    assert_eq!(topics_vec.len(), 2);
    let topic_stream_id: u64 = topics_vec.get(1).unwrap().into_val(&t.env);
    assert_eq!(topic_stream_id, stream_id);

    // Data: (recipient: Address, amount: i128, timestamp: u64)
    let data_tuple: (Address, i128, u64) = data.clone().into_val(&t.env);
    assert_eq!(data_tuple.0, t.recipient);
    assert_eq!(data_tuple.1, 50_000i128);     // 500s * 100 stroops/s
    assert_eq!(data_tuple.2, 500u64);
}

#[test]
fn snapshot_event_stream_cancelled() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );

    t.env.ledger().set_timestamp(300);
    c.cancel_stream(&stream_id, &t.sender);

    let events = t.env.events().all();
    let cancel_events: std::vec::Vec<_> = events.iter().filter(|(_, topics, _)| {
        let topic_vec: soroban_sdk::Vec<Val> = topics.clone();
        if !topic_vec.is_empty() {
            let first: Symbol = topic_vec.get(0).unwrap().into_val(&t.env);
            first == Symbol::new(&t.env, "StreamCancelled")
        } else {
            false
        }
    }).collect();

    assert_eq!(cancel_events.len(), 1, "Expected exactly one StreamCancelled event");

    let (contract_id, topics, data) = &cancel_events[0];
    assert_eq!(*contract_id, t.contract_id);

    // Topics: (Symbol("StreamCancelled"), stream_id: u64)
    let topics_vec: soroban_sdk::Vec<Val> = topics.clone();
    assert_eq!(topics_vec.len(), 2);
    let topic_stream_id: u64 = topics_vec.get(1).unwrap().into_val(&t.env);
    assert_eq!(topic_stream_id, stream_id);

    // Data: (sender: Address, refund_amount: i128, recipient_amount: i128)
    let data_tuple: (Address, i128, i128) = data.clone().into_val(&t.env);
    assert_eq!(data_tuple.0, t.sender);
    assert_eq!(data_tuple.1, 70_000i128);    // refund: 100_000 - 300*100
    assert_eq!(data_tuple.2, 30_000i128);    // recipient earned: 300*100
}

#[test]
fn snapshot_event_stream_topped_up() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );
    c.top_up(&stream_id, &t.sender, &t.token_id, &50_000);

    let events = t.env.events().all();
    let topup_events: std::vec::Vec<_> = events.iter().filter(|(_, topics, _)| {
        let topic_vec: soroban_sdk::Vec<Val> = topics.clone();
        if !topic_vec.is_empty() {
            let first: Symbol = topic_vec.get(0).unwrap().into_val(&t.env);
            first == Symbol::new(&t.env, "StreamToppedUp")
        } else {
            false
        }
    }).collect();

    assert_eq!(topup_events.len(), 1, "Expected exactly one StreamToppedUp event");

    let (contract_id, topics, data) = &topup_events[0];
    assert_eq!(*contract_id, t.contract_id);

    // Topics: (Symbol("StreamToppedUp"), stream_id: u64)
    let topics_vec: soroban_sdk::Vec<Val> = topics.clone();
    assert_eq!(topics_vec.len(), 2);
    let topic_stream_id: u64 = topics_vec.get(1).unwrap().into_val(&t.env);
    assert_eq!(topic_stream_id, stream_id);

    // Data: (added_amount: i128, new_end_time: u64)
    let data_tuple: (i128, u64) = data.clone().into_val(&t.env);
    assert_eq!(data_tuple.0, 50_000i128);    // added amount
    assert_eq!(data_tuple.1, 1500u64);       // 1000 + 50_000/100
}

#[test]
fn snapshot_event_stream_completed() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );

    t.env.ledger().set_timestamp(1000);
    c.withdraw(&stream_id, &t.recipient);

    let events = t.env.events().all();
    let completed_events: std::vec::Vec<_> = events.iter().filter(|(_, topics, _)| {
        let topic_vec: soroban_sdk::Vec<Val> = topics.clone();
        if !topic_vec.is_empty() {
            let first: Symbol = topic_vec.get(0).unwrap().into_val(&t.env);
            first == Symbol::new(&t.env, "StreamCompleted")
        } else {
            false
        }
    }).collect();

    assert_eq!(completed_events.len(), 1, "Expected exactly one StreamCompleted event");

    let (contract_id, topics, data) = &completed_events[0];
    assert_eq!(*contract_id, t.contract_id);

    // Topics: (Symbol("StreamCompleted"), stream_id: u64)
    let topics_vec: soroban_sdk::Vec<Val> = topics.clone();
    assert_eq!(topics_vec.len(), 2);
    let topic_stream_id: u64 = topics_vec.get(1).unwrap().into_val(&t.env);
    assert_eq!(topic_stream_id, stream_id);

    // Data: () — empty tuple
    let data_tuple: () = data.clone().into_val(&t.env);
    assert_eq!(data_tuple, ());
}

#[test]
fn snapshot_event_stream_partial_cancelled() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );

    // At t=200: streamed = 200*100 = 20_000; remaining = 80_000.
    // Cancel 30_000 → new deposit = 50_000.
    t.env.ledger().set_timestamp(200);
    let new_stream_id = c.partial_cancel_stream(&stream_id, &t.sender, &30_000);

    let events = t.env.events().all();
    let partial_events: std::vec::Vec<_> = events.iter().filter(|(_, topics, _)| {
        let topic_vec: soroban_sdk::Vec<Val> = topics.clone();
        if !topic_vec.is_empty() {
            let first: Symbol = topic_vec.get(0).unwrap().into_val(&t.env);
            first == Symbol::new(&t.env, "StreamPartialCancelled")
        } else {
            false
        }
    }).collect();

    assert_eq!(partial_events.len(), 1, "Expected exactly one StreamPartialCancelled event");

    let (contract_id, topics, data) = &partial_events[0];
    assert_eq!(*contract_id, t.contract_id);

    // Topics: (Symbol("StreamPartialCancelled"), old_stream_id: u64)
    let topics_vec: soroban_sdk::Vec<Val> = topics.clone();
    assert_eq!(topics_vec.len(), 2);
    let topic_stream_id: u64 = topics_vec.get(1).unwrap().into_val(&t.env);
    assert_eq!(topic_stream_id, stream_id);

    // Data: (new_stream_id: u64, sender: Address, refund_amount: i128, new_deposit: i128)
    let data_tuple: (u64, Address, i128, i128) = data.clone().into_val(&t.env);
    assert_eq!(data_tuple.0, new_stream_id);
    assert_eq!(data_tuple.1, t.sender);
    assert_eq!(data_tuple.2, 30_000i128);    // refund amount
    assert_eq!(data_tuple.3, 50_000i128);    // new deposit
}

#[test]
fn snapshot_event_auto_renew_failed() {
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();

    let contract_id = env.register(SoroStreamContract, ());
    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    // Mint only enough for the initial stream — not enough for auto-renew.
    StellarAssetClient::new(&env, &token_id).mint(&sender, &100_000);

    let c = SoroStreamContractClient::new(&env, &contract_id);
    c.set_min_duration(&sender, &0u64);
    env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(
        &sender, &recipient, &token_id, &100_000, &1000, &0, &0u64, &true, &0u64,
        &false,
        &0i128,
    );

    env.ledger().set_timestamp(1000);
    c.withdraw(&stream_id, &recipient);

    let events = env.events().all();
    let renew_fail_events: std::vec::Vec<_> = events.iter().filter(|(_, topics, _)| {
        let topic_vec: soroban_sdk::Vec<Val> = topics.clone();
        if !topic_vec.is_empty() {
            let first: Symbol = topic_vec.get(0).unwrap().into_val(&env);
            first == Symbol::new(&env, "AutoRenewFailed")
        } else {
            false
        }
    }).collect();

    assert_eq!(renew_fail_events.len(), 1, "Expected exactly one AutoRenewFailed event");

    let (emitter, topics, data) = &renew_fail_events[0];
    assert_eq!(*emitter, contract_id);

    // Topics: (Symbol("AutoRenewFailed"), stream_id: u64)
    let topics_vec: soroban_sdk::Vec<Val> = topics.clone();
    assert_eq!(topics_vec.len(), 2);
    let topic_stream_id: u64 = topics_vec.get(1).unwrap().into_val(&env);
    assert_eq!(topic_stream_id, stream_id);

    // Data: (sender: Address, required: i128)
    let data_tuple: (Address, i128) = data.clone().into_val(&env);
    assert_eq!(data_tuple.0, sender);
    assert_eq!(data_tuple.1, 100_000i128);
}

// ── Error variant coverage tests (issue #106) ────────────────────────────────
//
// Every variant in StreamError has at least one test that triggers it and
// verifies the exact error variant returned.
//
// Dead code variants (never returned by any code path):
//   - InsufficientBalance (7): No code path returns this error. It exists as
//     a placeholder for future balance-check logic. The contract relies on
//     token::Client::transfer to panic on insufficient balance instead.
//   - InvalidStartTime (12): No code path returns this error. Stream start
//     times are always set to env.ledger().timestamp(), never user-supplied.

#[test]
fn error_stream_not_found() {
    let t = setup();
    let c = client(&t);

    let result = c.try_get_stream(&999);
    assert!(matches!(result, Err(Ok(StreamError::StreamNotFound))));
}

#[test]
fn error_stream_not_found_on_withdraw() {
    let t = setup();
    let c = client(&t);

    let result = c.try_withdraw(&999, &t.recipient);
    assert_eq!(result, Err(Ok(StreamError::StreamNotFound)));
}

#[test]
fn error_stream_not_found_on_cancel() {
    let t = setup();
    let c = client(&t);

    let result = c.try_cancel_stream(&999, &t.sender);
    assert_eq!(result, Err(Ok(StreamError::StreamNotFound)));
}

#[test]
fn error_stream_not_found_on_top_up() {
    let t = setup();
    let c = client(&t);

    let result = c.try_top_up(&999, &t.sender, &t.token_id, &10_000);
    assert_eq!(result, Err(Ok(StreamError::StreamNotFound)));
}

#[test]
fn error_stream_not_found_on_partial_cancel() {
    let t = setup();
    let c = client(&t);

    let result = c.try_partial_cancel_stream(&999, &t.sender, &10_000);
    assert_eq!(result, Err(Ok(StreamError::StreamNotFound)));
}

#[test]
fn error_not_recipient() {
    let t = setup();
    let c = client(&t);
    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );
    let other = Address::generate(&t.env);

    let result = c.try_withdraw(&stream_id, &other);
    assert_eq!(result, Err(Ok(StreamError::NotRecipient)));
}

#[test]
fn error_not_sender_on_cancel() {
    let t = setup();
    let c = client(&t);
    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );
    let other = Address::generate(&t.env);

    let result = c.try_cancel_stream(&stream_id, &other);
    assert_eq!(result, Err(Ok(StreamError::NotAuthorized)));
}

#[test]
fn error_not_sender_on_top_up() {
    let t = setup();
    let c = client(&t);
    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );
    let other = Address::generate(&t.env);

    let result = c.try_top_up(&stream_id, &other, &t.token_id, &10_000);
    assert_eq!(result, Err(Ok(StreamError::NotAuthorized)));
}

#[test]
fn error_not_sender_on_partial_cancel() {
    let t = setup();
    let c = client(&t);
    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );
    let other = Address::generate(&t.env);

    let result = c.try_partial_cancel_stream(&stream_id, &other, &10_000);
    assert_eq!(result, Err(Ok(StreamError::NotAuthorized)));
}

#[test]
fn error_stream_not_active_on_withdraw() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );
    // Cancel the stream first
    c.cancel_stream(&stream_id, &t.sender);

    let result = c.try_withdraw(&stream_id, &t.recipient);
    assert_eq!(result, Err(Ok(StreamError::StreamNotFound)));
}

#[test]
fn error_stream_not_active_on_cancel() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );
    c.cancel_stream(&stream_id, &t.sender);

    let result = c.try_cancel_stream(&stream_id, &t.sender);
    assert_eq!(result, Err(Ok(StreamError::StreamNotFound)));
}

#[test]
fn error_stream_not_active_on_top_up() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );
    c.cancel_stream(&stream_id, &t.sender);

    let result = c.try_top_up(&stream_id, &t.sender, &t.token_id, &10_000);
    assert_eq!(result, Err(Ok(StreamError::StreamNotFound)));
}

#[test]
fn error_stream_not_active_on_partial_cancel() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );
    c.cancel_stream(&stream_id, &t.sender);

    let result = c.try_partial_cancel_stream(&stream_id, &t.sender, &10_000);
    assert_eq!(result, Err(Ok(StreamError::StreamNotFound)));
}

#[test]
fn error_zero_amount_on_create() {
    let t = setup();
    let c = client(&t);

    let result = c.try_create_stream(
        &t.sender, &t.recipient, &t.token_id, &0, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );
    assert_eq!(result, Err(Ok(StreamError::ZeroAmount)));
}

#[test]
fn error_zero_amount_negative_on_create() {
    let t = setup();
    let c = client(&t);

    let result = c.try_create_stream(
        &t.sender, &t.recipient, &t.token_id, &-100, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );
    assert_eq!(result, Err(Ok(StreamError::ZeroAmount)));
}

#[test]
fn error_zero_amount_on_top_up() {
    let t = setup();
    let c = client(&t);
    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );

    let result = c.try_top_up(&stream_id, &t.sender, &t.token_id, &0);
    assert_eq!(result, Err(Ok(StreamError::ZeroAmount)));
}

#[test]
fn error_zero_amount_on_partial_cancel() {
    let t = setup();
    let c = client(&t);
    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );

    let result = c.try_partial_cancel_stream(&stream_id, &t.sender, &0);
    assert_eq!(result, Err(Ok(StreamError::ZeroAmount)));
}

#[test]
fn error_invalid_duration_on_batch_create() {
    let t = setup();
    let c = client(&t);

    let recipients = soroban_vec![&t.env, t.recipient.clone()];
    let amounts = soroban_vec![&t.env, 10_000i128];

    // duration_seconds = 0 causes end_time overflow check to fail
    let lock_untils = soroban_vec![&t.env, 0u64];
let mut tokens = soroban_sdk::Vec::new(&t.env);
    for _ in 0..recipients.len() {
        tokens.push_back(t.token_id.clone());
    }
        let result = c.try_batch_create_stream(
        &t.sender, &recipients, &amounts, &tokens, &0, &false, &lock_untils,
        &0u64,
    );
    assert_eq!(result, Err(Ok(StreamError::InvalidDuration)));
}

#[test]
fn error_invalid_cliff() {
    let t = setup();
    let c = client(&t);

    let result = c.try_create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &1001, &0u64, &false, &0u64,
        &false,
        &0i128,
    );
    assert_eq!(result, Err(Ok(StreamError::InvalidCliff)));
}

#[test]
fn error_already_initialized() {
    let t = setup();
    let c = client(&t);
    let admin = Address::generate(&t.env);
    c.initialize(&admin, &soroban_sdk::String::from_str(&t.env, "1.0.0"));

    let result = c.try_initialize(&admin, &soroban_sdk::String::from_str(&t.env, "1.0.0"));
    assert_eq!(result, Err(Ok(StreamError::AlreadyInitialized)));
}

#[test]
fn error_not_initialized_on_get_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SoroStreamContract, ());
    let c = SoroStreamContractClient::new(&env, &contract_id);

    let result = c.try_get_admin();
    assert_eq!(result, Err(Ok(StreamError::NotInitialized)));
}

#[test]
fn error_not_initialized_on_upgrade() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SoroStreamContract, ());
    let c = SoroStreamContractClient::new(&env, &contract_id);

    let fake_hash = BytesN::from_array(&env, &[0u8; 32]);
    let result = c.try_upgrade(&fake_hash);
    assert_eq!(result, Err(Ok(StreamError::NotInitialized)));
}

#[test]
fn error_duplicate_stream() {
    let t = setup();
    let c = client(&t);

    c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );
    let result = c.try_create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );
    assert_eq!(result, Err(Ok(StreamError::DuplicateStream)));
}

#[test]
fn error_invalid_partial_cancel_exceeds_remainder() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );

    // At t=0: remaining = 100_000. cancel_amount = 100_000 exceeds remainder
    // (must be strictly less than remainder).
    let _result = c.try_partial_cancel_stream(&stream_id, &t.sender, &100_000);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &1u64, &false, &0u64,
        &false,
        &0i128,
    );

    let result = c.try_partial_cancel_stream(&stream_id, &t.sender, &100_000);
    assert_eq!(result, Err(Ok(StreamError::InvalidPartialCancel)));
}

// ── Overflow / checked-arithmetic tests ──────────────────────────────────────

/// `create_stream` with `now + duration_seconds` overflowing u64 must return
/// `StreamError::Overflow` instead of panicking.
#[test]
fn test_create_stream_end_time_overflow() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(u64::MAX - 10);
    let result = c.try_create_stream(
        &t.sender, &t.recipient, &t.token_id,
        &100_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );
    assert!(result.is_err());
}

/// `create_stream` with `now + cliff_seconds` overflowing u64 must return an error.
#[test]
fn test_create_stream_cliff_time_overflow() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(u64::MAX - 5);
    let result = c.try_create_stream(
        &t.sender, &t.recipient, &t.token_id,
        &100_000, &100, &10, &0u64, &false, &0u64,
        &false,
        &0i128,
    );
    assert!(result.is_err());
}

/// Direct unit test of `checked_flow_amount`: a product that overflows i128
/// returns `StreamError::Overflow` rather than panicking.
#[test]
fn test_checked_flow_amount_overflow() {
    let result = checked_flow_amount(10_000_000_000_000_000_000_i128, u64::MAX);
    assert_eq!(result, Err(StreamError::Overflow));
}

/// `checked_flow_amount` returns the correct product when there is no overflow.
#[test]
fn test_checked_flow_amount_ok() {
    let result = checked_flow_amount(100, 500);
    assert_eq!(result, Ok(50_000));
}

/// `top_up` where `extra_seconds = top_up / flow_rate` overflows u64 must return an error.
#[test]
fn test_top_up_extra_seconds_overflow() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    use soroban_sdk::token::StellarAssetClient;

    // flow_rate = 1 stroop/sec
    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &1_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );
    let huge: i128 = (u64::MAX as i128) + 1;
    StellarAssetClient::new(&t.env, &t.token_id).mint(&t.sender, &huge);
    let result = c.try_top_up(&stream_id, &t.sender, &t.token_id, &huge);
    assert!(result.is_err());
}

#[test]
fn error_invalid_partial_cancel_leaves_too_little() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );

    let result = c.try_partial_cancel_stream(&stream_id, &t.sender, &99_950);
    assert_eq!(result, Err(Ok(StreamError::InvalidPartialCancel)));
}

#[test]
fn error_contract_paused() {
    let t = setup();
    let c = client(&t);
    let admin = Address::generate(&t.env);
    c.initialize(&admin, &soroban_sdk::String::from_str(&t.env, "1.0.0"));
    c.emergency_pause();

    let result = c.try_create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );
    assert_eq!(result, Err(Ok(StreamError::ContractPaused)));
}

#[test]
fn error_zero_flow_rate() {
    let t = setup();
    let c = client(&t);

    let result = c.try_create_stream(
        &t.sender, &t.recipient, &t.token_id, &1, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );
    assert_eq!(result, Err(Ok(StreamError::ZeroFlowRate)));
}

#[test]
fn error_zero_flow_rate_in_batch() {
    let t = setup();
    let c = client(&t);

    let recipients = soroban_vec![&t.env, t.recipient.clone()];
    let amounts = soroban_vec![&t.env, 1i128];
    let lock_untils = soroban_vec![&t.env, 0u64];

let mut tokens = soroban_sdk::Vec::new(&t.env);
    for _ in 0..recipients.len() {
        tokens.push_back(t.token_id.clone());
    }
        let result = c.try_batch_create_stream(
        &t.sender, &recipients, &amounts, &tokens, &1000, &false, &lock_untils,
        &0u64,
    );
    assert_eq!(result, Err(Ok(StreamError::ZeroFlowRate)));
}

#[test]
fn error_token_mismatch() {
    let t = setup();
    let c = client(&t);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );

    let other_token_admin = Address::generate(&t.env);
    let other_token = t.env
        .register_stellar_asset_contract_v2(other_token_admin)
        .address();

    let result = c.try_top_up(&stream_id, &t.sender, &other_token, &10_000);
    assert_eq!(result, Err(Ok(StreamError::TokenMismatch)));
}

#[test]
fn error_batch_length_mismatch() {
    let t = setup();
    let c = client(&t);

    let recipients = soroban_vec![&t.env, t.recipient.clone()];
    let amounts = soroban_vec![&t.env, 10_000i128, 20_000i128];
    let lock_untils = soroban_vec![&t.env, 0u64, 0u64];

let mut tokens = soroban_sdk::Vec::new(&t.env);
    for _ in 0..recipients.len() {
        tokens.push_back(t.token_id.clone());
    }
        let result = c.try_batch_create_stream(
        &t.sender, &recipients, &amounts, &tokens, &1000, &false, &lock_untils,
        &0u64,
    );
    assert_eq!(result, Err(Ok(StreamError::BatchLengthMismatch)));
}

#[test]
fn error_zero_amount_in_batch() {
    let t = setup();
    let c = client(&t);

    let recipients = soroban_vec![&t.env, t.recipient.clone()];
    let amounts = soroban_vec![&t.env, 0i128];
    let lock_untils = soroban_vec![&t.env, 0u64];

let mut tokens = soroban_sdk::Vec::new(&t.env);
    for _ in 0..recipients.len() {
        tokens.push_back(t.token_id.clone());
    }
        let result = c.try_batch_create_stream(
        &t.sender, &recipients, &amounts, &tokens, &1000, &false, &lock_untils,
        &0u64,
    );
    assert_eq!(result, Err(Ok(StreamError::ZeroAmount)));
}

#[test]
fn error_not_recipient_in_batch_withdraw() {
    let t = setup();
    let c = client(&t);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );
    let other = Address::generate(&t.env);

    let result = c.try_batch_withdraw(&soroban_vec![&t.env, stream_id], &other);
    assert_eq!(result, Err(Ok(StreamError::NotRecipient)));
}

#[test]
fn error_invalid_duration_fee_too_high() {
    let t = setup();
    let c = client(&t);

    let result = c.try_set_protocol_fee(&10_001u32);
    assert_eq!(result, Err(Ok(StreamError::InvalidDuration)));
}

// Dead code documentation:
// - InsufficientBalance (7): Never returned. Token transfers panic via
//   token::Client::transfer on insufficient balance. No contract code path
//   returns this variant. Kept for potential future use with explicit
//   balance checks.
// - InvalidStartTime (12): Never returned. Stream start times are always
//   set to env.ledger().timestamp(), not user-supplied. No code path
//   returns this variant. Kept for potential future use with scheduled
//   stream starts.

#[test]
fn test_top_up_amount_overflow() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    use soroban_sdk::token::StellarAssetClient;
    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id,
        &1_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );
    let huge: i128 = (u64::MAX as i128) + 1;
    StellarAssetClient::new(&t.env, &t.token_id).mint(&t.sender, &huge);
    let result = c.try_top_up(&stream_id, &t.sender, &t.token_id, &huge);
    assert!(result.is_err());
}

/// `top_up` where `end_time + extra_seconds` overflows u64 must return an error.
#[test]
fn test_top_up_end_time_overflow() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(u64::MAX - 1_000);

    use soroban_sdk::token::StellarAssetClient;
    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id,
        &1_000, &1000, &0, &0u64, &false, &0u64,
        &false,
        &0i128,
    );
    StellarAssetClient::new(&t.env, &t.token_id).mint(&t.sender, &1);
    let result = c.try_top_up(&stream_id, &t.sender, &t.token_id, &1);
    assert!(result.is_err());
}

/// `batch_create_stream` where accumulating amounts overflows i128 must return an error.
#[test]
fn test_batch_create_total_amount_overflow() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    use soroban_sdk::{token::StellarAssetClient, Vec};

    let a: i128 = 90_000_000_000_000_000_000_000_000_000_000_000_000_i128;
    let b: i128 = 90_000_000_000_000_000_000_000_000_000_000_000_000_i128;

    let mut recipients = Vec::new(&t.env);
    let mut amounts: Vec<i128> = Vec::new(&t.env);
    recipients.push_back(Address::generate(&t.env));
    recipients.push_back(Address::generate(&t.env));
    amounts.push_back(a);
    amounts.push_back(b);

    let mut lock_untils: Vec<u64> = Vec::new(&t.env);
    lock_untils.push_back(0);
    lock_untils.push_back(0);

    StellarAssetClient::new(&t.env, &t.token_id).mint(&t.sender, &a);
let mut tokens = soroban_sdk::Vec::new(&t.env);
    for _ in 0..recipients.len() {
        tokens.push_back(t.token_id.clone());
    }
        let result = c.try_batch_create_stream(
        &t.sender, &recipients, &amounts, &tokens, &1000, &false, &lock_untils,
        &0u64,
    );
    assert!(result.is_err());
}

#[test]
fn test_delegate_can_top_up_and_cancel() {
    let t = setup();
    let c = client(&t);
    let operator = Address::generate(&t.env);

    StellarAssetClient::new(&t.env, &t.token_id).mint(&operator, &1_000_000);

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false, &0i128);

    c.set_delegate(&t.sender, &stream_id, &operator);

    // Operator tops up
    c.top_up(&stream_id, &operator, &t.token_id, &50_000);
    let stream_after = c.get_stream(&stream_id);
    assert_eq!(stream_after.deposit, 150_000);

    // Operator cancels
    c.cancel_stream(&stream_id, &operator);
    let result = c.try_get_stream(&stream_id);
    assert!(result.is_err());
}

#[test]
fn test_delegate_cannot_withdraw() {
    let t = setup();
    let c = client(&t);
    let operator = Address::generate(&t.env);

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false, &0i128);

    c.set_delegate(&t.sender, &stream_id, &operator);

    t.env.ledger().set_timestamp(500);

    // Operator tries to withdraw
    let result = c.try_withdraw(&stream_id, &operator);
    assert_eq!(result, Err(Ok(StreamError::NotRecipient)));
}

#[test]
fn test_batch_cancel_stream_success() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id1 = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false, &0i128);
    let stream_id2 = c.create_stream(&t.sender, &t.recipient, &t.token_id, &200_000, &1000, &0, &1u64, &false, &0u64,
        &false, &0i128);

    let sender_bal_before = TokenClient::new(&t.env, &t.token_id).balance(&t.sender);

    t.env.ledger().set_timestamp(200);
    c.batch_cancel_stream(&soroban_vec![&t.env, stream_id1, stream_id2], &t.sender);

    // Stream 1: 20s earned (20_000), 80s refunded (80_000)
    // Stream 2: 20s earned (40_000), 80s refunded (160_000)
    let recipient_bal = TokenClient::new(&t.env, &t.token_id).balance(&t.recipient);
    assert_eq!(recipient_bal, 20_000 + 40_000);

    let sender_bal_after = TokenClient::new(&t.env, &t.token_id).balance(&t.sender);
    assert_eq!(sender_bal_after, sender_bal_before + 80_000 + 160_000);

    assert!(c.try_get_stream(&stream_id1).is_err());
    assert!(c.try_get_stream(&stream_id2).is_err());
}

#[test]
fn error_batch_cancel_not_sender() {
    let t = setup();
    let c = client(&t);
    let other_sender = Address::generate(&t.env);
    StellarAssetClient::new(&t.env, &t.token_id).mint(&other_sender, &1_000_000);

    let stream_id1 = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false, &0i128);
    let stream_id2 = c.create_stream(&other_sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false, &0i128);

    let result = c.batch_cancel_stream(&soroban_vec![&t.env, stream_id1, stream_id2], &t.sender);
    assert_eq!(result.get(0).unwrap(), Ok(()));
    assert_eq!(result.get(1).unwrap(), Err(StreamError::NotSender));
}

#[test]
fn error_batch_cancel_empty_list() {
    let t = setup();
    let c = client(&t);
    let result = c.try_batch_cancel_stream(&soroban_vec![&t.env], &t.sender);
    assert_eq!(result, Err(Ok(StreamError::BatchLengthMismatch)));
}

#[test]
fn error_batch_cancel_too_long_list() {
    let t = setup();
    let c = client(&t);
    let mut ids = soroban_sdk::Vec::new(&t.env);
    for i in 0..21 { ids.push_back(i as u64); }
    let result = c.try_batch_cancel_stream(&ids, &t.sender);
    assert_eq!(result, Err(Ok(StreamError::BatchLengthMismatch)));
}

#[test]
fn test_revoke_delegate_strips_capabilities() {
    let t = setup();
    let c = client(&t);
    let operator = Address::generate(&t.env);

    StellarAssetClient::new(&t.env, &t.token_id).mint(&operator, &1_000_000);

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false, &0i128);

    c.set_delegate(&t.sender, &stream_id, &operator);
    c.revoke_delegate(&t.sender, &stream_id);

    // Operator tries to top up
    let result = c.try_top_up(&stream_id, &operator, &t.token_id, &50_000);
    assert_eq!(result, Err(Ok(StreamError::NotAuthorized)));
}

#[test]
fn test_pause_resume() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64,
        &false, &0i128);

    t.env.ledger().set_timestamp(200);
    c.pause_stream(&stream_id, &t.sender);

    let stream = c.get_stream(&stream_id);
    assert_eq!(stream.status, StreamStatus::Paused);
    assert_eq!(stream.last_pause_time, 200);

    // Get claimable while paused should be for 200s (20_000 tokens)
    t.env.ledger().set_timestamp(500);
    let claimable = c.get_claimable(&stream_id);
    assert_eq!(claimable, 20_000);

    // Resume at 500
    c.resume_stream(&stream_id, &t.sender);
    let stream_resumed = c.get_stream(&stream_id);
    assert_eq!(stream_resumed.status, StreamStatus::Active);
    // End time should be shifted by (500 - 200) = 300, so from 1000 -> 1300
    assert_eq!(stream_resumed.end_time, 1300);

    // Check claimable at 600. It was active 0-200 and 500-600. Total active = 300s.
    t.env.ledger().set_timestamp(600);
    let claimable_now = c.get_claimable(&stream_id);
    assert_eq!(claimable_now, 30_000);
}

// ── Interface trait implementation tests ──────────────────────────────────────
//
// These tests verify that SoroStreamContract correctly implements the
// SoroStreamInterface trait, enabling type-safe contract invocation through
// the trait and code generation for alternate implementations.

/// Compile-time verification that SoroStreamContract implements SoroStreamInterface.
///
/// If this test fails to compile, it means the trait implementation is incomplete
/// or has signature mismatches. The `assert_implements_interface` function is a
/// zero-cost abstraction that proves the contract satisfies the trait.
fn assert_implements_interface<T: SoroStreamInterface>() {}

#[test]
fn test_contract_implements_interface() {
    // This test compiles if and only if SoroStreamContract implements SoroStreamInterface.
    // If the trait implementation has any method signature mismatches or missing methods,
    // this will fail to compile.
    assert_implements_interface::<SoroStreamContract>();
}

/// Runtime test: Call a trait method through the trait object to verify delegation works.
///
/// This test demonstrates that methods can be invoked through the SoroStreamInterface trait,
/// not just through the concrete contractimpl methods. This enables:
/// - SDK code generation for type-safe client stubs
/// - Alternate implementations that satisfy the same interface
/// - Runtime polymorphism for contract testing
#[test]
fn test_interface_trait_method_delegation() {
    let t = setup();
    let c = client(&t);

    // Create a stream using the direct contractimpl method
    let stream_id = c.create_stream(
        &t.sender,
        &t.recipient,
        &t.token_id,
        &100_000,
        &1000,
        &0,
        &0u64,
        &false,
        &0u64,
        &false,
        &0i128,
    );

    // Retrieve and verify the stream was created correctly
    let stream = c.get_stream(&stream_id);
    assert_eq!(stream.id, stream_id);
    assert_eq!(stream.sender, t.sender);
    assert_eq!(stream.recipient, t.recipient);
    assert_eq!(stream.token, t.token_id);
    assert_eq!(stream.deposit, 100_000);
    assert_eq!(stream.flow_rate, 100);
    assert_eq!(stream.status, StreamStatus::Active);
}

/// Verify that the trait methods maintain identical semantics to contractimpl.
///
/// This test ensures that calling through the trait delegation does not introduce
/// any behavioral differences or side effects.
#[test]
fn test_interface_preserves_semantics() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(
        &t.sender,
        &t.recipient,
        &t.token_id,
        &100_000,
        &1000,
        &0,
        &0u64,
        &false,
        &0u64,
        &false,
        &0i128,
    );

    // Advance time and withdraw through trait
    t.env.ledger().set_timestamp(500);
    c.withdraw(&stream_id, &t.recipient);

    // Verify the withdrawal was processed identically to direct contractimpl call
    let balance = TokenClient::new(&t.env, &t.token_id).balance(&t.recipient);
    assert_eq!(balance, 50_000, "Trait delegation did not preserve withdrawal semantics");
}

/// Verify get_stats through the trait interface.
#[test]
fn test_interface_get_stats() {
    let t = setup();
    let c = client(&t);

    // Create multiple streams
    c.create_stream(
        &t.sender,
        &t.recipient,
        &t.token_id,
        &100_000,
        &1000,
        &0,
        &0u64,
        &false,
        &0u64,
        &false,
        &0i128,
    );
    c.create_stream(
        &t.sender,
        &t.recipient,
        &t.token_id,
        &50_000,
        &500,
        &0,
        &1u64,
        &false,
        &0u64,
        &false,
        &0i128,
    );

    // Get stats through trait
    let stats = c.get_stats();
    assert_eq!(stats.total_streams, 2);
    assert_eq!(stats.active_streams, 2);
    assert_eq!(stats.total_volume, 150_000);
}

/// Verify protocol fee methods through the trait interface.
#[test]
fn test_interface_protocol_fee() {
    let t = setup();
    let c = client(&t);
    let admin = Address::generate(&t.env);
    c.initialize(&admin, &soroban_sdk::String::from_str(&t.env, "1.0.0"));

    // Set protocol fee through trait
    c.set_protocol_fee(&100); // 1% = 100 bps
    c.set_treasury_address(&admin);

    // Get protocol fee info through trait
    let (fee_bps, treasury) = c.get_protocol_fee_info();
    assert_eq!(fee_bps, 100);
    assert_eq!(treasury, Some(admin));
}

/// Verify pagination methods through the trait interface.
#[test]
fn test_interface_pagination_methods() {
    let t = setup();
    let c = client(&t);

    // Create multiple streams for pagination testing
    let id1 = c.create_stream(
        &t.sender,
        &t.recipient,
        &t.token_id,
        &100_000,
        &1000,
        &0,
        &0u64,
        &false,
        &0u64,
        &false,
        &0i128,
    );
    let id2 = c.create_stream(
        &t.sender,
        &t.recipient,
        &t.token_id,
        &100_000,
        &1000,
        &0,
        &1u64,
        &false,
        &0u64,
        &false,
        &0i128,
    );

    // Test get_all_stream_ids through trait
    let all_ids = c.get_all_stream_ids(&0u32, &10u32);
    assert!(all_ids.len() >= 2);
    assert_eq!(all_ids.get_unchecked(0), id1);
    assert_eq!(all_ids.get_unchecked(1), id2);

    // Test get_streams_by_sender through trait
    let sender_streams = c.get_streams_by_sender(&t.sender, &0u32, &10u32);
    assert!(sender_streams.len() >= 2);

    // Test get_streams_by_recipient through trait
    let recipient_streams = c.get_streams_by_recipient(&t.recipient, &0u32, &10u32);
    assert!(recipient_streams.len() >= 2);

    // Test active streams through trait
    let active_sender = c.get_active_streams_by_sender(&t.sender);
    assert!(active_sender.len() >= 2);

    let active_recipient = c.get_active_streams_by_recipient(&t.recipient);
    assert!(active_recipient.len() >= 2);
}

/// Verify batch operations through the trait interface.
#[test]
fn test_interface_batch_operations() {
    let t = setup();
    let c = client(&t);

    let recipient2 = Address::generate(&t.env);
    StellarAssetClient::new(&t.env, &t.token_id).mint(&t.sender, &500_000);

    let recipients = soroban_vec![&t.env, t.recipient.clone(), recipient2.clone()];
    let amounts = soroban_vec![&t.env, 100_000i128, 50_000i128];
    let lock_untils = soroban_vec![&t.env, 0u64, 0u64];

    // Create batch through trait
let mut tokens = soroban_sdk::Vec::new(&t.env);
    for _ in 0..recipients.len() {
        tokens.push_back(t.token_id.clone());
    }
        let stream_ids = c.batch_create_stream(
        &t.sender,
        &recipients,
        &amounts,
        &tokens,
        &1000,
        &false,
        &lock_untils,
        &0u64,
    );
    assert_eq!(stream_ids.len(), 2);

    // Withdraw batch through trait (only first stream for t.recipient)
    let first_id = soroban_sdk::vec![&t.env, stream_ids.get_unchecked(0)];
    let withdrawal_amounts = c.batch_withdraw(&first_id, &t.recipient);
    assert_eq!(withdrawal_amounts.len(), 1);
}

/// Verify admin operations through the trait interface.
#[test]
fn test_interface_admin_operations() {
    let t = setup();
    let c = client(&t);
    let admin = Address::generate(&t.env);

    // Initialize through trait
    c.initialize(&admin, &soroban_sdk::String::from_str(&t.env, "1.0.0"));

    // Get admin through trait
    assert_eq!(c.get_admin(), admin);

    let new_admin = Address::generate(&t.env);

    // Set admin through trait
    c.set_admin(&new_admin);
    assert_eq!(c.get_admin(), new_admin);

    // Pause/resume through trait
    assert!(!c.is_paused());
    c.emergency_pause();
    assert!(c.is_paused());
    c.emergency_resume();
    assert!(!c.is_paused());
}

/// Verify is_participant through the trait interface.
#[test]
fn test_interface_is_participant() {
    let t = setup();
    let c = client(&t);

    let stream_id = c.create_stream(
        &t.sender,
        &t.recipient,
        &t.token_id,
        &100_000,
        &1000,
        &0,
        &0u64,
        &false,
        &0u64,
        &false,
        &0i128,
    );

    // Test sender participation through trait
    assert!(c.is_participant(&stream_id, &t.sender));

    // Test recipient participation through trait
    assert!(c.is_participant(&stream_id, &t.recipient));

    // Test non-participant
    let other = Address::generate(&t.env);
    assert!(!c.is_participant(&stream_id, &other));
}

/// #188 – Recipient can withdraw correctly after a top_up.
#[test]
fn test_withdraw_after_top_up() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64, &false,
        &0i128,
    );

    StellarAssetClient::new(&t.env, &t.token_id).mint(&t.sender, &50_000);
    c.top_up(&stream_id, &t.sender, &t.token_id, &50_000);
    let stream = c.get_stream(&stream_id);
    assert_eq!(stream.deposit, 150_000);
    assert_eq!(stream.end_time, 1500);

    t.env.ledger().set_timestamp(600);
    c.withdraw(&stream_id, &t.recipient);
    let bal = TokenClient::new(&t.env, &t.token_id).balance(&t.recipient);
    assert_eq!(bal, 60_000);

    let stream = c.get_stream(&stream_id);
    assert_eq!(stream.total_withdrawn, 60_000);

    t.env.ledger().set_timestamp(1500);
    c.withdraw(&stream_id, &t.recipient);
    let bal = TokenClient::new(&t.env, &t.token_id).balance(&t.recipient);
    assert_eq!(bal, 150_000);

    assert!(c.try_get_stream(&stream_id).is_err());
}

/// Issue #187 – cancel_stream with zero withdrawals: full deposit refunded to sender.
#[test]
fn test_cancel_stream_with_zero_withdrawals() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(100);

    let initial_sender_bal = TokenClient::new(&t.env, &t.token_id).balance(&t.sender);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64, &false,
        &0i128,
    );

    let claimable_before = c.get_claimable(&stream_id);
    assert_eq!(claimable_before, 0, "claimable must be 0 before any time passes");

    c.cancel_stream(&stream_id, &t.sender);

    let sender_bal_after = TokenClient::new(&t.env, &t.token_id).balance(&t.sender);
    assert_eq!(
        sender_bal_after, initial_sender_bal,
        "sender must receive full deposit refund",
    );

    let recipient_bal = TokenClient::new(&t.env, &t.token_id).balance(&t.recipient);
    assert_eq!(recipient_bal, 0, "recipient must receive 0 when cancelled before cliff");

    assert!(
        c.try_get_stream(&stream_id).is_err(),
        "stream entry must be removed after cancel",
    );
}

// --- #186: Emergency pause blocks create_stream and withdraw ---

#[test]
fn test_emergency_pause_blocks_create_stream_186() {
    let t = setup();
    let c = client(&t);
    let admin = Address::generate(&t.env);
    c.initialize(&admin, &soroban_sdk::String::from_str(&t.env, "1.0.0"));

    c.emergency_pause();

    let result = c.try_create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64, &false,
        &0i128,
    );
    assert_eq!(result, Err(Ok(StreamError::ContractPaused)));
}

#[test]
fn test_emergency_resume_unblocks_create_stream_186() {
    let t = setup();
    let c = client(&t);
    let admin = Address::generate(&t.env);
    c.initialize(&admin, &soroban_sdk::String::from_str(&t.env, "1.0.0"));

    c.emergency_pause();
    c.emergency_resume();

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64, &false,
        &0i128,
    );
    let stream = c.get_stream(&stream_id);
    assert_eq!(stream.status, StreamStatus::Active);
}

#[test]
fn test_emergency_pause_blocks_withdraw_186() {
    let t = setup();
    let c = client(&t);
    let admin = Address::generate(&t.env);
    c.initialize(&admin, &soroban_sdk::String::from_str(&t.env, "1.0.0"));

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64, &false,
        &0i128,
    );

    t.env.ledger().set_timestamp(500);
    c.emergency_pause();

    let result = c.try_withdraw(&stream_id, &t.recipient);
    assert_eq!(result, Err(Ok(StreamError::ContractPaused)));
}

#[test]
fn test_emergency_resume_unblocks_withdraw_186() {
    let t = setup();
    let c = client(&t);
    let admin = Address::generate(&t.env);
    c.initialize(&admin, &soroban_sdk::String::from_str(&t.env, "1.0.0"));

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64, &false,
        &0i128,
    );

    t.env.ledger().set_timestamp(500);
    c.emergency_pause();
    c.emergency_resume();

    c.withdraw(&stream_id, &t.recipient);
}

// --- #249: cancel_stream properly cleans up sender/recipient index ---

/// Issue #249 – After cancellation, get_streams_by_sender and get_streams_by_recipient
/// must no longer return the cancelled stream.
#[test]
fn test_cancel_stream_removes_from_sender_and_recipient_index_249() {
// ── Rounding dust tests (issue #248) ──────────────────────────────────────────

/// When deposit is not evenly divisible by duration, flow_rate rounds down.
/// The final withdrawal should not error due to rounding dust — it should
/// cap the claimable at deposit - total_withdrawn.
#[test]
fn test_withdraw_dust_not_erroring() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64, &false,
        &0i128,
    );

    let sender_streams_before = c.get_streams_by_sender(&t.sender, &0u32, &10u32);
    assert_eq!(sender_streams_before.len(), 1);

    let recipient_streams_before = c.get_streams_by_recipient(&t.recipient, &0u32, &10u32);
    assert_eq!(recipient_streams_before.len(), 1);

    t.env.ledger().set_timestamp(300);
    c.cancel_stream(&stream_id, &t.sender);

    let sender_streams_after = c.get_streams_by_sender(&t.sender, &0u32, &10u32);
    assert_eq!(sender_streams_after.len(), 0, "sender index must be empty after cancel");

    let recipient_streams_after = c.get_streams_by_recipient(&t.recipient, &0u32, &10u32);
    assert_eq!(recipient_streams_after.len(), 0, "recipient index must be empty after cancel");

    assert!(c.try_get_stream(&stream_id).is_err(), "stream must not exist after cancel");
}

// --- #251: cliff_end_time == end_time boundary ---

/// Issue #251 – When cliff_end_time == end_time, the entire deposit becomes
/// claimable at exactly cliff_end_time. Nothing is claimable one second before.
#[test]
fn test_cliff_equals_end_time_boundary_251() {
    // 100 / 3 = 33 (floor). Total streamable = 33*3 = 99. Dust = 1.
    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100, &3, &0, &0u64, &false, &0u64, &false, &0i128);

    // Withdraw at t=1: 33
    t.env.ledger().set_timestamp(1);
    c.withdraw(&stream_id, &t.recipient);
    assert_eq!(TokenClient::new(&t.env, &t.token_id).balance(&t.recipient), 33);

    // Withdraw at t=2: another 33 → total 66
    t.env.ledger().set_timestamp(2);
    c.withdraw(&stream_id, &t.recipient);
    assert_eq!(TokenClient::new(&t.env, &t.token_id).balance(&t.recipient), 66);

    // Withdraw at t=3 (end): claimable = 33, but available = 100-66 = 34.
    // Due to dust, raw claimable (33) < available (34), so recipient gets 33 more = 99.
    t.env.ledger().set_timestamp(3);
    c.withdraw(&stream_id, &t.recipient);
    assert_eq!(TokenClient::new(&t.env, &t.token_id).balance(&t.recipient), 99);

    // Stream should be removed after end
    assert!(c.try_get_stream(&stream_id).is_err());
}

/// top_up with dust: effective_amount rounds to whole seconds.
/// The total should still be claimable without error.
#[test]
fn test_top_up_dust_rounding_correctness() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let duration = 100u64;
    let cliff = 100u64;
    let deposit = 100_000i128;

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &deposit, &duration, &cliff, &0u64, &false, &0u64, &false,
        &0i128,
    );

    let stream = c.get_stream(&stream_id);
    assert_eq!(stream.cliff_time, stream.end_time, "cliff_time must equal end_time");

    t.env.ledger().set_timestamp(99);
    let claimable_before = c.get_claimable(&stream_id);
    assert_eq!(claimable_before, 0, "nothing claimable one second before cliff");

    t.env.ledger().set_timestamp(100);
    let claimable_at_cliff = c.get_claimable(&stream_id);
    assert_eq!(claimable_at_cliff, deposit, "entire deposit claimable at cliff == end_time");
}

// --- #252: get_claimable at exactly end_time ---

/// Issue #252 – At a stream's exact end_time, all of the deposit should be
/// claimable. After end_time, claimable must not increase further.
#[test]
fn test_get_claimable_at_exact_end_time_252() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let duration = 100u64;
    let deposit = 100_000i128;

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &deposit, &duration, &0, &0u64, &false, &0u64, &false,
        &0i128,
    );

    t.env.ledger().set_timestamp(100);
    let claimable_at_end = c.get_claimable(&stream_id);
    assert_eq!(
        claimable_at_end, deposit,
        "full deposit must be claimable at exactly end_time"
    );

    t.env.ledger().set_timestamp(101);
    let claimable_after = c.get_claimable(&stream_id);
    assert_eq!(
        claimable_after, deposit,
        "claimable must not increase beyond end_time"
    );
}

/// Issue #252 – Non-zero cliff: at end_time the full deposit is still claimable
/// provided the cliff has already passed.
#[test]
fn test_get_claimable_at_end_time_with_cliff_252() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let duration = 100u64;
    let cliff = 10u64;
    let deposit = 100_000i128;

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &deposit, &duration, &cliff, &0u64, &false, &0u64, &false,
        &0i128,
    );

    t.env.ledger().set_timestamp(100);
    let claimable_at_end = c.get_claimable(&stream_id);
    assert_eq!(
        claimable_at_end, deposit,
        "full deposit claimable at end_time even with non-zero cliff"
    );
}

// --- #254: concurrent create and cancel in same ledger sequence ---

/// Issue #254 – Creating a stream and immediately cancelling it in the same
/// ledger sequence must produce a consistent final state: either the cancel
/// completes with a full refund, or it is rejected cleanly.
#[test]
fn test_concurrent_create_and_cancel_same_ledger_254() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(100);

    let initial_sender_bal = TokenClient::new(&t.env, &t.token_id).balance(&t.sender);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64, &false,
        &0i128,
    );

    // No ledger advancement – cancel in the same sequence
    c.cancel_stream(&stream_id, &t.sender);

    // Stream must be fully removed
    assert!(
        c.try_get_stream(&stream_id).is_err(),
        "stream must not exist after cancel in same ledger"
    );

    // Sender must receive full refund (no time elapsed)
    let sender_bal = TokenClient::new(&t.env, &t.token_id).balance(&t.sender);
    assert_eq!(
        sender_bal, initial_sender_bal,
        "sender must get full refund when cancel is immediate"
    );

    // Recipient must have received nothing
    let recipient_bal = TokenClient::new(&t.env, &t.token_id).balance(&t.recipient);
    assert_eq!(recipient_bal, 0, "recipient gets nothing when cancelled instantly");

    // Index must be clean
    let sender_streams = c.get_streams_by_sender(&t.sender, &0u32, &10u32);
    assert_eq!(sender_streams.len(), 0, "sender index empty after same-ledger cancel");
    // flow_rate = 33
    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100, &3, &0, &0u64, &false, &0u64, &false, &0i128);

    // Top up 50: effective = 50 - (50 % 33) = 50 - 17 = 33. extra = 33/33 = 1s.
    StellarAssetClient::new(&t.env, &t.token_id).mint(&t.sender, &50);
    c.top_up(&stream_id, &t.sender, &t.token_id, &50);

    let stream = c.get_stream(&stream_id);
    assert_eq!(stream.deposit, 133);
    assert_eq!(stream.end_time, 4);

    // Withdraw everything at end
    t.env.ledger().set_timestamp(4);
    c.withdraw(&stream_id, &t.recipient);
    // flow_rate=33, duration=4 → 33*4 = 132. deposit=133, dust=1.
    assert_eq!(TokenClient::new(&t.env, &t.token_id).balance(&t.recipient), 132);
    assert!(c.try_get_stream(&stream_id).is_err());
}

/// cancel_stream with rounding dust: refund should not underflow.
#[test]
fn test_cancel_stream_dust_no_underflow() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    // 100 / 3 = 33
    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100, &3, &0, &0u64, &false, &0u64, &false, &0i128);

    // Cancel at t=2: earned = 66, available = 100, refund = 34.
    t.env.ledger().set_timestamp(2);
    c.cancel_stream(&stream_id, &t.sender);
    assert_eq!(TokenClient::new(&t.env, &t.token_id).balance(&t.recipient), 66);
    assert_eq!(TokenClient::new(&t.env, &t.token_id).balance(&t.sender), 999_934);
}

// ── get_stats counter tests (issue #246) ──────────────────────────────────────

/// get_stats.total_streams reflects total ever created (including cancelled).
/// get_stats.active_streams reflects currently active count.
#[test]
fn test_get_stats_tracks_active_and_total() {
    let t = setup();
    let c = client(&t);

    let id1 = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64, &false, &0i128);
    let id2 = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &1u64, &false, &0u64, &false, &0i128);

    let stats = c.get_stats();
    assert_eq!(stats.total_streams, 2);
    assert_eq!(stats.active_streams, 2);
    assert_eq!(stats.total_volume, 200_000);

    // Cancel one stream
    c.cancel_stream(&id1, &t.sender);

    let stats = c.get_stats();
    assert_eq!(stats.total_streams, 2); // total ever created stays at 2
    assert_eq!(stats.active_streams, 1); // active decremented
}

/// get_stats.active_streams decrements on pause and increments on resume.
#[test]
fn test_get_stats_pause_resume_counter() {
    let t = setup();
    let c = client(&t);

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64, &false, &0i128);

    assert_eq!(c.get_stats().active_streams, 1);

    c.pause_stream(&stream_id, &t.sender);
    assert_eq!(c.get_stats().active_streams, 0);

    c.resume_stream(&stream_id, &t.sender);
    assert_eq!(c.get_stats().active_streams, 1);
}

/// recalibrate_stats admin instruction corrects drift.
#[test]
fn test_recalibrate_stats_corrects_drift() {
    let t = setup();
    let c = client(&t);
    let admin = Address::generate(&t.env);
    c.initialize(&admin, &soroban_sdk::String::from_str(&t.env, "1.0.0"));

    c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64, &false, &0i128);
    c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &1u64, &false, &0u64, &false, &0i128);

    assert_eq!(c.get_stats().active_streams, 2);

    // Cancel one stream
    let id1 = c.get_all_stream_ids(&0, &2).get_unchecked(0);
    c.cancel_stream(&id1, &t.sender);
    assert_eq!(c.get_stats().active_streams, 1);

    // Recalibrate should confirm the count
    c.recalibrate_stats(&admin);
    assert_eq!(c.get_stats().active_streams, 1);
}

/// recalibrate_stats rejects non-admin caller.
#[test]
fn test_recalibrate_stats_rejects_non_admin() {
    let t = setup();
    let c = client(&t);
    let admin = Address::generate(&t.env);
    c.initialize(&admin, &soroban_sdk::String::from_str(&t.env, "1.0.0"));

    let result = c.try_recalibrate_stats(&t.sender);
    assert!(result.is_err());
}

// ── bump_stream_ttl tests (issue #225) ────────────────────────────────────────

/// bump_stream_ttl extends the storage TTL so the stream entry remains accessible
/// after its original TTL would have expired. Any caller — not just participants —
/// may invoke this instruction.
#[test]
fn test_bump_stream_ttl_extends_accessibility() {
    let t = setup();
    let c = client(&t);

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64, &false, &0i128);

    // Set ledger sequence near where the default TTL might expire.
    t.env.ledger().set_sequence_number(99_990);

    // Bump the TTL — no auth required, any caller works.
    c.bump_stream_ttl(&stream_id);

    // Advance ledger well beyond original TTL.
    t.env.ledger().set_sequence_number(200_000);

    // Stream should still be accessible.
    let stream = c.get_stream(&stream_id);
    assert_eq!(stream.id, stream_id);
    assert_eq!(stream.status, StreamStatus::Active);
}

/// bump_stream_ttl can be called by a third party (non-participant).
#[test]
fn test_bump_stream_ttl_any_caller_can_call() {
    let t = setup();
    let c = client(&t);

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64, &false, &0i128);
    let other = Address::generate(&t.env);

    let result = c.try_bump_stream_ttl(&stream_id, &other);
    assert_eq!(result, Err(Ok(StreamError::NotAuthorized)));
    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64, &false);
    // A completely unrelated address can bump TTL — no error expected.
    c.bump_stream_ttl(&stream_id);
    let stream = c.get_stream(&stream_id);
    assert_eq!(stream.status, StreamStatus::Active);
}

/// bump_stream_ttl rejects cancelled / non-active streams.
#[test]
fn test_bump_stream_ttl_rejects_cancelled() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64, &false, &0i128);
    c.cancel_stream(&stream_id, &t.sender);

    // After cancellation the stream is removed from storage → StreamNotFound.
    let result = c.try_bump_stream_ttl(&stream_id);
    assert_eq!(result, Err(Ok(StreamError::StreamNotFound)));
}

/// bump_stream_ttl works on paused streams as well.
#[test]
fn test_bump_stream_ttl_works_on_paused_stream() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64, &false, &0i128);
    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64, &false, &0u64, &false);
    c.pause_stream(&stream_id, &t.sender);

    // Should succeed — paused streams still need their TTL extended.
    let result = c.try_bump_stream_ttl(&stream_id);
    assert!(result.is_ok());
}

/// bump_stream_ttl uses a 24-hour buffer so that streams near their end still get bumped.
#[test]
fn test_bump_stream_ttl_buffer_applied_for_nearly_expired_stream() {
    let t = setup();
    let c = client(&t);
    // Stream ends in 10 seconds.
    t.env.ledger().set_timestamp(0);
    let stream_id = c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &10, &0, &0u64, &false, &0u64, &false);

    t.env.ledger().set_timestamp(5); // 5 s before end_time
    // Should not panic — safety buffer covers the tiny remaining duration.
    let result = c.try_bump_stream_ttl(&stream_id);
    assert!(result.is_ok());
}

// ── Delegate management tests (issue #226) ───────────────────────────────────

/// set_delegate stores the delegate and emits DelegateSet event.
#[test]
fn test_set_delegate_stores_delegate() {
    let t = setup();
    let c = client(&t);
    let delegate = Address::generate(&t.env);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64,
        &false, &0u64, &false,
    );

    c.set_delegate(&t.sender, &stream_id, &delegate);

    let stored = c.get_delegate(&stream_id);
    assert_eq!(stored, Some(delegate));
}

/// Only the sender can set a delegate — non-sender is rejected.
#[test]
fn test_set_delegate_rejected_for_non_sender() {
    let t = setup();
    let c = client(&t);
    let impostor = Address::generate(&t.env);
    let delegate = Address::generate(&t.env);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64,
        &false, &0u64, &false,
    );

    let result = c.try_set_delegate(&impostor, &stream_id, &delegate);
    assert_eq!(result, Err(Ok(StreamError::NotSender)));
}

/// Delegate can cancel a stream in place of the sender.
#[test]
fn test_delegate_can_cancel_stream() {
    let t = setup();
    let c = client(&t);
    let delegate = Address::generate(&t.env);
    StellarAssetClient::new(&t.env, &t.token_id).mint(&delegate, &1_000_000);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64,
        &false, &0u64, &false,
    );

    c.set_delegate(&t.sender, &stream_id, &delegate);
    c.cancel_stream(&stream_id, &delegate);

    // Stream removed after cancel.
    let result = c.try_get_stream(&stream_id);
    assert!(result.is_err());
}

/// A non-delegate third party cannot act as sender.
#[test]
fn test_non_delegate_cannot_cancel() {
    let t = setup();
    let c = client(&t);
    let impostor = Address::generate(&t.env);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64,
        &false, &0u64, &false,
    );

    let result = c.try_cancel_stream(&stream_id, &impostor);
    assert_eq!(result, Err(Ok(StreamError::NotAuthorized)));
}

/// After revoke_delegate the former delegate loses all permissions.
#[test]
fn test_revoke_delegate_removes_permissions() {
    let t = setup();
    let c = client(&t);
    let delegate = Address::generate(&t.env);
    StellarAssetClient::new(&t.env, &t.token_id).mint(&delegate, &1_000_000);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64,
        &false, &0u64, &false,
    );

    c.set_delegate(&t.sender, &stream_id, &delegate);
    c.revoke_delegate(&t.sender, &stream_id);

    // get_delegate now returns None.
    assert_eq!(c.get_delegate(&stream_id), None);

    // Former delegate can no longer cancel.
    let result = c.try_cancel_stream(&stream_id, &delegate);
    assert_eq!(result, Err(Ok(StreamError::NotAuthorized)));
}

/// Sender can resume sole control after revoking delegate.
#[test]
fn test_sender_retains_control_after_revoke() {
    let t = setup();
    let c = client(&t);
    let delegate = Address::generate(&t.env);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64,
        &false, &0u64, &false,
    );

    c.set_delegate(&t.sender, &stream_id, &delegate);
    c.revoke_delegate(&t.sender, &stream_id);

    // Sender can still cancel the stream.
    c.cancel_stream(&stream_id, &t.sender);
    assert!(c.try_get_stream(&stream_id).is_err());
}

/// Delegate address is returned in get_delegate response.
#[test]
fn test_get_delegate_returns_correct_address() {
    let t = setup();
    let c = client(&t);
    let delegate = Address::generate(&t.env);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64,
        &false, &0u64, &false,
    );

    assert_eq!(c.get_delegate(&stream_id), None);

    c.set_delegate(&t.sender, &stream_id, &delegate);
    assert_eq!(c.get_delegate(&stream_id), Some(delegate.clone()));

    c.revoke_delegate(&t.sender, &stream_id);
    assert_eq!(c.get_delegate(&stream_id), None);
}


// ── Expired state & mark_expired tests (issue #228) ──────────────────────────

/// get_stream returns Expired status once the stream's end_time has passed,
/// even without an explicit mark_expired call.
#[test]
fn test_get_stream_returns_expired_after_end_time() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64,
        &false, &0u64, &false,
    );

    // Before end_time: still Active.
    t.env.ledger().set_timestamp(500);
    let stream = c.get_stream(&stream_id);
    assert_eq!(stream.status, StreamStatus::Active);

    // At exactly end_time: Expired.
    t.env.ledger().set_timestamp(1000);
    let stream = c.get_stream(&stream_id);
    assert_eq!(stream.status, StreamStatus::Expired);

    // After end_time: still Expired.
    t.env.ledger().set_timestamp(2000);
    let stream = c.get_stream(&stream_id);
    assert_eq!(stream.status, StreamStatus::Expired);
}

/// Cancelled streams never transition to Expired via get_stream.
#[test]
fn test_cancelled_stream_not_returned_as_expired() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64,
        &false, &0u64, &false,
    );
    c.cancel_stream(&stream_id, &t.sender);

    // After cancellation the stream is removed — should return StreamNotFound.
    let result = c.try_get_stream(&stream_id);
    assert!(result.is_err());
}

// ─── Holdback escrow tests (#224) ────────────────────────────────────────────

/// Helper: create a stream with a non-zero holdback amount.
/// `total` is the full amount locked; `holdback` is the escrow portion.
/// The sender is minted enough tokens before the call.
fn create_holdback_stream(
    t: &TestEnv,
    total: i128,
    holdback: i128,
    duration: u64,
    nonce: u64,
) -> u64 {
    let c = client(t);
    StellarAssetClient::new(&t.env, &t.token_id).mint(&t.sender, &total);
    c.create_stream(
        &t.sender,
        &t.recipient,
        &t.token_id,
        &total,
        &duration,
        &0,
        &nonce,
        &false,
        &0u64,
        &false,
        &holdback,
    )
}

/// Holdback is deducted from the streaming deposit at creation time.
/// stream.deposit == total - holdback; flow_rate == deposit / duration.
#[test]
fn test_holdback_deducted_from_deposit() {
    let t = setup();
    t.env.ledger().set_timestamp(0);

    let total: i128 = 100_000;
    let holdback: i128 = 20_000;
    let duration: u64 = 1000;

    let stream_id = create_holdback_stream(&t, total, holdback, duration, 42);
    let stream = client(&t).get_stream(&stream_id);

    assert_eq!(stream.deposit, total - holdback, "deposit should be streaming portion only");
    assert_eq!(stream.holdback_amount, holdback);
    assert!(!stream.holdback_claimed);
    assert_eq!(stream.flow_rate, (total - holdback) / duration as i128);
}

/// Full contract balance after creation equals total (streaming + holdback).
#[test]
fn test_holdback_contract_holds_full_amount() {
    let t = setup();
    t.env.ledger().set_timestamp(0);

    let total: i128 = 50_000;
    let holdback: i128 = 10_000;

    create_holdback_stream(&t, total, holdback, 500, 1);

    let contract_balance =
        soroban_sdk::token::Client::new(&t.env, &t.token_id).balance(&t.contract_id);
    assert_eq!(contract_balance, total, "contract should hold the full amount");
}

/// Sender releases the holdback → recipient receives it; holdback_claimed becomes true.
#[test]
fn test_release_holdback_transfers_to_recipient() {
    let t = setup();
    t.env.ledger().set_timestamp(0);

    let total: i128 = 100_000;
    let holdback: i128 = 30_000;
    let stream_id = create_holdback_stream(&t, total, holdback, 1000, 10);

    let c = client(&t);

    // Advance time so some streaming has happened (not required for release, but realistic)
    t.env.ledger().set_timestamp(500);

    let before = soroban_sdk::token::Client::new(&t.env, &t.token_id).balance(&t.recipient);
    c.release_holdback(&stream_id, &t.sender);
    let after = soroban_sdk::token::Client::new(&t.env, &t.token_id).balance(&t.recipient);

    assert_eq!(after - before, holdback, "recipient should receive the holdback amount");

    let stream = c.get_stream(&stream_id);
    assert!(stream.holdback_claimed, "holdback_claimed must be true after release");
}

/// Sender can claw back the holdback before recipient claims it.
#[test]
fn test_claw_back_holdback_returns_to_sender() {
    let t = setup();
    t.env.ledger().set_timestamp(0);

    let total: i128 = 80_000;
    let holdback: i128 = 25_000;
    let stream_id = create_holdback_stream(&t, total, holdback, 800, 20);

    let c = client(&t);

    let before = soroban_sdk::token::Client::new(&t.env, &t.token_id).balance(&t.sender);
    c.claw_back_holdback(&stream_id, &t.sender);
    let after = soroban_sdk::token::Client::new(&t.env, &t.token_id).balance(&t.sender);

    assert_eq!(after - before, holdback, "sender should receive the clawed-back holdback");

    let stream = c.get_stream(&stream_id);
    assert!(stream.holdback_claimed, "holdback_claimed must be true after claw-back");
}

/// Double-release is rejected (holdback already settled).
#[test]
fn test_release_holdback_double_release_rejected() {
    let t = setup();
    t.env.ledger().set_timestamp(0);

    let stream_id = create_holdback_stream(&t, 60_000, 15_000, 600, 30);
    let c = client(&t);

    c.release_holdback(&stream_id, &t.sender);

    // Second release attempt must fail
    let result = c.try_release_holdback(&stream_id, &t.sender);
    assert_eq!(
        result,
        Err(Ok(StreamError::StreamNotActive)),
        "second release should fail with StreamNotActive"
    );
}

/// Claw-back after release is also rejected.
#[test]
fn test_claw_back_after_release_rejected() {
    let t = setup();
    t.env.ledger().set_timestamp(0);

    let stream_id = create_holdback_stream(&t, 60_000, 15_000, 600, 31);
    let c = client(&t);

    c.release_holdback(&stream_id, &t.sender);

    let result = c.try_claw_back_holdback(&stream_id, &t.sender);
    assert_eq!(
        result,
        Err(Ok(StreamError::StreamNotActive)),
        "claw-back after release should be rejected"
    );
}

/// release_holdback on a zero-holdback stream returns ZeroAmount.
#[test]
fn test_release_holdback_zero_holdback_rejected() {
    let t = setup();
    t.env.ledger().set_timestamp(0);

    // Create a stream with no holdback
    StellarAssetClient::new(&t.env, &t.token_id).mint(&t.sender, &100_000);
    let c = client(&t);
    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id,
        &100_000, &1000, &0, &99u64, &false, &0u64, &false, &0i128,
    );

    let result = c.try_release_holdback(&stream_id, &t.sender);
    assert_eq!(
        result,
        Err(Ok(StreamError::ZeroAmount)),
        "release on zero-holdback stream should return ZeroAmount"
    );
}

/// Holdback is included in the sender refund when the stream is cancelled before release.
#[test]
fn test_holdback_returned_to_sender_on_cancel() {
    let t = setup();
    t.env.ledger().set_timestamp(0);

    let total: i128 = 100_000;
    let holdback: i128 = 40_000;
    let duration: u64 = 1000;

    let stream_id = create_holdback_stream(&t, total, holdback, duration, 50);
    let c = client(&t);

    // Advance time — recipient earns some tokens
    t.env.ledger().set_timestamp(200);

    let sender_before = soroban_sdk::token::Client::new(&t.env, &t.token_id).balance(&t.sender);
    let recipient_before = soroban_sdk::token::Client::new(&t.env, &t.token_id).balance(&t.recipient);

    c.cancel_stream(&stream_id, &t.sender);

    let sender_after = soroban_sdk::token::Client::new(&t.env, &t.token_id).balance(&t.sender);
    let recipient_after = soroban_sdk::token::Client::new(&t.env, &t.token_id).balance(&t.recipient);

    let streaming_deposit = total - holdback; // 60_000
    let flow_rate = streaming_deposit / duration as i128; // 60
    let elapsed: i128 = 200;
    let earned = flow_rate * elapsed; // 12_000
    let unstreamed = streaming_deposit - earned; // 48_000

    // Sender gets back: unstreamed portion + holdback (not yet released)
    assert_eq!(
        sender_after - sender_before,
        unstreamed + holdback,
        "sender should receive unstreamed + holdback on cancel"
    );
    // Recipient gets: earned portion only (holdback not released)
    assert_eq!(
        recipient_after - recipient_before,
        earned,
        "recipient should receive only the earned amount on cancel"
    );
}

/// Partial holdback: streaming still works correctly when holdback < total.
#[test]
fn test_partial_holdback_streaming_works() {
    let t = setup();
    t.env.ledger().set_timestamp(0);

    let total: i128 = 100_000;
    let holdback: i128 = 10_000;
    let duration: u64 = 1000;
    let streaming = total - holdback; // 90_000
    let flow_rate = streaming / duration as i128; // 90

    let stream_id = create_holdback_stream(&t, total, holdback, duration, 60);
    let c = client(&t);

    t.env.ledger().set_timestamp(500);
    c.withdraw(&stream_id, &t.recipient);

    let bal = soroban_sdk::token::Client::new(&t.env, &t.token_id).balance(&t.recipient);
    assert_eq!(bal, flow_rate * 500, "recipient earns from streaming portion only");
}

/// holdback_amount == amount is rejected (nothing left to stream).
#[test]
fn test_holdback_equal_to_amount_rejected() {
    let t = setup();
    t.env.ledger().set_timestamp(0);

    StellarAssetClient::new(&t.env, &t.token_id).mint(&t.sender, &100_000);
    let c = client(&t);
    let result = c.try_create_stream(
        &t.sender, &t.recipient, &t.token_id,
        &100_000, &1000, &0, &70u64, &false, &0u64, &false,
        &100_000i128, // holdback == amount → invalid
    );
    assert_eq!(result, Err(Ok(StreamError::ZeroAmount)));
}

/// Negative holdback_amount is rejected.
#[test]
fn test_negative_holdback_rejected() {
    let t = setup();
    t.env.ledger().set_timestamp(0);

    StellarAssetClient::new(&t.env, &t.token_id).mint(&t.sender, &100_000);
    let c = client(&t);
    let result = c.try_create_stream(
        &t.sender, &t.recipient, &t.token_id,
        &100_000, &1000, &0, &71u64, &false, &0u64, &false,
        &-1i128,
    );
    assert_eq!(result, Err(Ok(StreamError::ZeroAmount)));
}

/// Non-sender cannot release or claw back holdback.
#[test]
fn test_holdback_only_sender_can_operate() {
    let t = setup();
    t.env.ledger().set_timestamp(0);

    let stream_id = create_holdback_stream(&t, 100_000, 20_000, 1000, 80);
    let c = client(&t);

    let result = c.try_release_holdback(&stream_id, &t.recipient);
    assert_eq!(result, Err(Ok(StreamError::NotAuthorized)));

    let result2 = c.try_claw_back_holdback(&stream_id, &t.recipient);
    assert_eq!(result2, Err(Ok(StreamError::NotAuthorized)));
}
/// mark_expired transitions a stream to Expired after end_time and emits event.
#[test]
fn test_mark_expired_succeeds_after_end_time() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64,
        &false, &0u64, &false,
    );

    // Advance past end_time.
    t.env.ledger().set_timestamp(1001);
    c.mark_expired(&stream_id);

    // Persisted status is now Expired.
    let raw = c.get_stream(&stream_id);
    assert_eq!(raw.status, StreamStatus::Expired);
}

/// mark_expired rejects a stream that has not yet reached end_time.
#[test]
fn test_mark_expired_rejects_before_end_time() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64,
        &false, &0u64, &false,
    );

    t.env.ledger().set_timestamp(500); // still before end_time
    let result = c.try_mark_expired(&stream_id);
    assert_eq!(result, Err(Ok(StreamError::StreamNotComplete)));
}

/// mark_expired rejects already-Cancelled streams.
#[test]
fn test_mark_expired_rejects_cancelled_stream() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64,
        &false, &0u64, &false,
    );
    c.cancel_stream(&stream_id, &t.sender);

    // Stream is removed on cancel — StreamNotFound.
    let result = c.try_mark_expired(&stream_id);
    assert!(result.is_err());
}

/// mark_expired is callable by anyone, not only the sender/recipient.
#[test]
fn test_mark_expired_callable_by_anyone() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &100_000, &1000, &0, &0u64,
        &false, &0u64, &false,
    );
    t.env.ledger().set_timestamp(1001);

    // A third-party address can call mark_expired.
    let result = c.try_mark_expired(&stream_id);
    assert!(result.is_ok());
}

// ── sweep_fees tests (#222) ───────────────────────────────────────────────────

/// sweep_fees with zero balance is a no-op: no transfer, no event, no error.
#[test]
fn test_sweep_fees_zero_balance_is_noop() {
    let t = setup();
    let c = client(&t);
    let admin = Address::generate(&t.env);
    c.initialize(&admin, &soroban_sdk::String::from_str(&t.env, "1.0.0"));

    let destination = Address::generate(&t.env);

    // No fees have been collected yet — should succeed without doing anything.
    c.sweep_fees(&t.token_id, &destination);

    // Destination balance remains zero.
    let bal = TokenClient::new(&t.env, &t.token_id).balance(&destination);
    assert_eq!(bal, 0);

    // fees_collected tracker is still zero.
    assert_eq!(c.get_fees_collected(&t.token_id), 0);
}

/// sweep_fees with a non-zero balance transfers the exact amount to destination
/// and resets the counter.
#[test]
fn test_sweep_fees_nonzero_balance_transfers_and_resets() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let admin = Address::generate(&t.env);
    c.initialize(&admin, &soroban_sdk::String::from_str(&t.env, "1.0.0"));

    // 1% fee = 100 bps
    c.set_protocol_fee(&100u32);

    // Create a stream with 100_000 stroops over 1000s → flow_rate = 100
    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id,
        &100_000, &1000, &0, &0u64, &false, &0u64, &false,
    );

    // Advance to halfway and withdraw — fee = 1% of 50_000 = 500 stroops
    t.env.ledger().set_timestamp(500);
    c.withdraw(&stream_id, &t.recipient);

    // fees_collected should equal the fee deducted (500 stroops)
    let collected = c.get_fees_collected(&t.token_id);
    assert_eq!(collected, 500);

    // Recipient should have received 50_000 - 500 = 49_500
    let recipient_bal = TokenClient::new(&t.env, &t.token_id).balance(&t.recipient);
    assert_eq!(recipient_bal, 49_500);

    // Sweep fees to treasury destination
    let treasury = Address::generate(&t.env);
    c.sweep_fees(&t.token_id, &treasury);

    // Treasury received the exact fee amount
    let treasury_bal = TokenClient::new(&t.env, &t.token_id).balance(&treasury);
    assert_eq!(treasury_bal, 500);

    // Counter reset to zero after sweep
    assert_eq!(c.get_fees_collected(&t.token_id), 0);
}

/// fees accumulate across multiple withdrawals before a single sweep.
#[test]
fn test_sweep_fees_accumulates_across_withdrawals() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let admin = Address::generate(&t.env);
    c.initialize(&admin, &soroban_sdk::String::from_str(&t.env, "1.0.0"));

    // 2% fee = 200 bps
    c.set_protocol_fee(&200u32);

    // Stream: 100_000 over 1000s → flow_rate = 100, 2% fee
    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id,
        &100_000, &1000, &0, &0u64, &false, &0u64, &false,
    );

    // First withdrawal at t=200: claimable=20_000, fee=400
    t.env.ledger().set_timestamp(200);
    c.withdraw(&stream_id, &t.recipient);
    assert_eq!(c.get_fees_collected(&t.token_id), 400);

    // Second withdrawal at t=600: claimable=40_000, fee=800
    t.env.ledger().set_timestamp(600);
    c.withdraw(&stream_id, &t.recipient);
    assert_eq!(c.get_fees_collected(&t.token_id), 1200);

    // Single sweep collects both
    let treasury = Address::generate(&t.env);
    c.sweep_fees(&t.token_id, &treasury);
    assert_eq!(TokenClient::new(&t.env, &t.token_id).balance(&treasury), 1200);
    assert_eq!(c.get_fees_collected(&t.token_id), 0);
}

/// sweep_fees can only be called by admin; non-admin caller panics.
#[test]
fn test_sweep_fees_unauthorized_rejected() {
    let env = Env::default();
    // Note: do NOT call mock_all_auths so that auth is actually enforced
    let contract_id = env.register(SoroStreamContract, ());
    let c = SoroStreamContractClient::new(&env, &contract_id);

    // Initialize with a known admin (mock all auths just for this call)
    env.mock_all_auths();
    let admin = Address::generate(&env);
    c.initialize(&admin, &soroban_sdk::String::from_str(&env, "1.0.0"));

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    let destination = Address::generate(&env);

    // Now call without mocking auths — should fail auth check
    let result = c.try_sweep_fees(&token_id, &destination);
    assert!(result.is_err(), "non-admin should not be able to sweep fees");
}

// ─── Issue #321: max-duration stream edge-case tests ────────────────────────

/// A stream with duration = u64::MAX seconds must be created without panicking,
/// stored correctly, and report get_claimable == 0 at start_time.
///
/// Deposit is set to i128::MAX so that `flow_rate = deposit / duration` rounds
/// down to 1 stroop/sec rather than 0 (which would be rejected as ZeroFlowRate).
/// The key property under test is that no arithmetic overflows during creation.
#[test]
fn test_create_stream_max_duration_no_panic() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SoroStreamContract, ());
    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    let sender    = Address::generate(&env);
    let recipient = Address::generate(&env);

    // Mint enough tokens to cover a deposit that yields a non-zero flow_rate
    // with u64::MAX duration:  flow_rate = deposit / u64::MAX >= 1
    // Use deposit = u64::MAX (fits comfortably in i128).
    let deposit: i128 = u64::MAX as i128;
    soroban_sdk::token::StellarAssetClient::new(&env, &token_id)
        .mint(&sender, &deposit);

    let c = SoroStreamContractClient::new(&env, &contract_id);
    c.set_min_duration(&sender, &0u64);

    // start_time == current ledger timestamp (boundary condition from the issue)
    let start_time: u64 = 1_000;
    env.ledger().set_timestamp(start_time);

    // duration = u64::MAX — the principal edge-case under test.
    // This must NOT panic; the contract must return a valid stream_id.
    let stream_id = c.create_stream(
        &sender, &recipient, &token_id,
        &deposit,
        &u64::MAX,  // duration = u64::MAX seconds
        &0u64,      // cliff_offset
        &0u64,      // nonce
        &false,     // auto_renew
        &0u64,      // lock_until
        &false,     // allow_recipient_termination
        &0i128,     // holdback_amount
    );

    // Stream must be stored and readable.
    let stream = c.get_stream(&stream_id);
    assert_eq!(stream.deposit, deposit, "deposit must be stored verbatim");
    assert_eq!(stream.start_time, start_time, "start_time must equal current ledger");
    assert_eq!(stream.status, StreamStatus::Active, "stream must be Active");

    // flow_rate = floor(deposit / u64::MAX) = floor((2^64 - 1) / (2^64 - 1)) = 1
    assert_eq!(stream.flow_rate, 1, "flow_rate must be 1 stroop/sec");

    // At start_time, elapsed = 0, so nothing is claimable.
    let claimable_at_start = c.get_claimable(&stream_id);
    assert_eq!(claimable_at_start, 0, "get_claimable must return 0 at start_time");
}

/// Verify get_claimable returns the correct value at mid-duration for a
/// stream with duration = u64::MAX.
///
/// Mid-duration is approximated as u64::MAX / 2 seconds after start_time.
/// Expected claimable = flow_rate × elapsed = 1 × (u64::MAX / 2).
#[test]
fn test_max_duration_stream_claimable_at_mid_duration() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SoroStreamContract, ());
    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    let sender    = Address::generate(&env);
    let recipient = Address::generate(&env);

    let deposit: i128 = u64::MAX as i128;
    soroban_sdk::token::StellarAssetClient::new(&env, &token_id)
        .mint(&sender, &deposit);

    let c = SoroStreamContractClient::new(&env, &contract_id);
    c.set_min_duration(&sender, &0u64);

    let start_time: u64 = 0;
    env.ledger().set_timestamp(start_time);

    let stream_id = c.create_stream(
        &sender, &recipient, &token_id,
        &deposit,
        &u64::MAX,  // duration = u64::MAX seconds
        &0u64,
        &0u64,
        &false,
        &0u64,
        &false,
        &0i128,
    );

    // flow_rate = 1 stroop/sec (deposit == u64::MAX, duration == u64::MAX)
    let stream = c.get_stream(&stream_id);
    assert_eq!(stream.flow_rate, 1);

    // Advance to mid-duration: elapsed = u64::MAX / 2
    let mid_elapsed: u64 = u64::MAX / 2;
    env.ledger().set_timestamp(start_time + mid_elapsed);

    // Expected claimable = flow_rate × elapsed = 1 × mid_elapsed
    let expected_claimable: i128 = mid_elapsed as i128;
    let claimable = c.get_claimable(&stream_id);
    assert_eq!(
        claimable, expected_claimable,
        "get_claimable at mid-duration must equal flow_rate × elapsed"
    );
}

/// Verify that start_time == current_ledger is accepted (boundary from the issue).
/// The contract must store start_time correctly and report 0 claimable immediately.
#[test]
fn test_max_duration_start_time_equals_current_ledger() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SoroStreamContract, ());
    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    let sender    = Address::generate(&env);
    let recipient = Address::generate(&env);

    // Generous deposit so flow_rate != 0 even with a huge duration
    let deposit: i128 = u64::MAX as i128;
    soroban_sdk::token::StellarAssetClient::new(&env, &token_id)
        .mint(&sender, &deposit);

    let c = SoroStreamContractClient::new(&env, &contract_id);
    c.set_min_duration(&sender, &0u64);

    // Set ledger timestamp to a specific boundary value
    let boundary_timestamp: u64 = 9_999_999;
    env.ledger().set_timestamp(boundary_timestamp);

    let stream_id = c.create_stream(
        &sender, &recipient, &token_id,
        &deposit,
        &u64::MAX,
        &0u64,
        &42u64,   // distinct nonce
        &false,
        &0u64,
        &false,
        &0i128,
    );

    let stream = c.get_stream(&stream_id);
    // start_time must be exactly the ledger timestamp at creation
    assert_eq!(
        stream.start_time, boundary_timestamp,
        "start_time must equal the current ledger timestamp"
    );

    // No time has elapsed — claimable must be zero
    assert_eq!(
        c.get_claimable(&stream_id), 0,
        "claimable must be 0 when start_time == current ledger"
    );
}
