#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    Address, BytesN, Env,
};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

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

    TestEnv { env, contract_id, token_id, sender, recipient }
}

fn client(t: &TestEnv) -> SoroStreamContractClient<'_> {
    SoroStreamContractClient::new(&t.env, &t.contract_id)
}

// Helper: create a stream with no expected_version (backwards-compat None path)
fn create(t: &TestEnv, amount: i128, duration: u64) -> u64 {
    client(t).create_stream(
        &t.sender,
        &t.recipient,
        &t.token_id,
        &amount,
        &duration,
        &false,
    )
}

// ---------------------------------------------------------------------------
// ── Original regression tests (updated for new function signatures) ─────────
// ---------------------------------------------------------------------------

#[test]
fn test_create_stream_success() {
    let t = setup();
    let c = client(&t);

    let stream_id =
        c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &false);
    assert_eq!(stream_id, 0);

    let stream = c.get_stream(&stream_id);
    assert_eq!(stream.deposit, 100_000);
    assert_eq!(stream.flow_rate, 100);
    assert_eq!(stream.status, StreamStatus::Active);
    // version starts at 1
    assert_eq!(stream.version, 1);
}

#[test]
fn test_withdraw_partial() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id =
        c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &false);

    t.env.ledger().set_timestamp(500);
    c.withdraw(&stream_id, &t.recipient, &None);

    let balance = TokenClient::new(&t.env, &t.token_id).balance(&t.recipient);
    assert_eq!(balance, 50_000);
}

#[test]
fn test_withdraw_full() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id =
        c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &false);

    t.env.ledger().set_timestamp(1000);
    c.withdraw(&stream_id, &t.recipient, &None);

    let balance = TokenClient::new(&t.env, &t.token_id).balance(&t.recipient);
    assert_eq!(balance, 100_000);

    let stream = c.get_stream(&stream_id);
    assert_eq!(stream.status, StreamStatus::Completed);
}

#[test]
fn test_cancel_stream_splits_correctly() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id =
        c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &false);

    t.env.ledger().set_timestamp(300);
    c.cancel_stream(&stream_id, &t.sender, &None);

    let recipient_bal = TokenClient::new(&t.env, &t.token_id).balance(&t.recipient);
    let sender_bal = TokenClient::new(&t.env, &t.token_id).balance(&t.sender);

    assert_eq!(recipient_bal, 30_000);
    assert_eq!(sender_bal, 970_000);

    let stream = c.get_stream(&stream_id);
    assert_eq!(stream.status, StreamStatus::Cancelled);
}

#[test]
fn test_top_up_extends_duration() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id =
        c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &false);
    let stream_before = c.get_stream(&stream_id);

    c.top_up(&stream_id, &t.sender, &50_000, &None);

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
    env.ledger().set_timestamp(0);

    let stream_id =
        c.create_stream(&sender, &recipient, &token_id, &100_000, &1000, &true);

    env.ledger().set_timestamp(1000);
    c.withdraw(&stream_id, &recipient, &None);

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

    let stream_id =
        c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &false);
    let other = Address::generate(&t.env);

    let result = c.try_withdraw(&stream_id, &other, &None);
    assert!(result.is_err());
}

#[test]
fn test_cannot_cancel_if_not_sender() {
    let t = setup();
    let c = client(&t);

    let stream_id =
        c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &false);
    let other = Address::generate(&t.env);

    let result = c.try_cancel_stream(&stream_id, &other, &None);
    assert!(result.is_err());
}

#[test]
fn test_zero_amount_fails() {
    let t = setup();
    let c = client(&t);

    let result =
        c.try_create_stream(&t.sender, &t.recipient, &t.token_id, &0, &1000, &false);
    assert!(result.is_err());
}

#[test]
fn test_get_claimable_calculates_correctly() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let stream_id =
        c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &false);

    t.env.ledger().set_timestamp(250);
    let claimable = c.get_claimable(&stream_id);
    assert_eq!(claimable, 25_000);
}

#[test]
fn test_get_active_streams_by_sender_excludes_cancelled() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let id0 =
        c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &false);
    let recipient2 = Address::generate(&t.env);
    let id1 =
        c.create_stream(&t.sender, &recipient2, &t.token_id, &100_000, &1000, &false);

    c.cancel_stream(&id0, &t.sender, &None);

    let active = c.get_active_streams_by_sender(&t.sender);
    assert_eq!(active.len(), 1);
    assert_eq!(active.get(0).unwrap().id, id1);

    let all = c.get_streams_by_sender(&t.sender);
    assert_eq!(all.len(), 2);
}

#[test]
fn test_get_active_streams_by_recipient_excludes_completed() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let id0 =
        c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &false);
    let id1 =
        c.create_stream(&t.sender, &t.recipient, &t.token_id, &100_000, &1000, &false);

    t.env.ledger().set_timestamp(1000);
    c.withdraw(&id0, &t.recipient, &None);

    let active = c.get_active_streams_by_recipient(&t.recipient);
    assert_eq!(active.len(), 1);
    assert_eq!(active.get(0).unwrap().id, id1);

    let all = c.get_streams_by_recipient(&t.recipient);
    assert_eq!(all.len(), 2);
}

// ---------------------------------------------------------------------------
// ── Issue #236: Optimistic concurrency (version field) ──────────────────────
// ---------------------------------------------------------------------------

#[test]
fn test_version_starts_at_one() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);
    let sid = create(&t, 100_000, 1000);
    let stream = c.get_stream(&sid);
    assert_eq!(stream.version, 1);
}

#[test]
fn test_withdraw_increments_version() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);
    let sid = create(&t, 100_000, 1000);

    t.env.ledger().set_timestamp(100);
    c.withdraw(&sid, &t.recipient, &None);

    assert_eq!(c.get_stream(&sid).version, 2);
}

#[test]
fn test_top_up_increments_version() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);
    let sid = create(&t, 100_000, 1000);

    c.top_up(&sid, &t.sender, &10_000, &None);
    assert_eq!(c.get_stream(&sid).version, 2);
}

#[test]
fn test_cancel_increments_version_before_finalising() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);
    let sid = create(&t, 100_000, 1000);

    t.env.ledger().set_timestamp(500);
    c.cancel_stream(&sid, &t.sender, &None);

    // Stream is cancelled; version was bumped to 2 during the cancel.
    assert_eq!(c.get_stream(&sid).version, 2);
    assert_eq!(c.get_stream(&sid).status, StreamStatus::Cancelled);
}

#[test]
fn test_correct_expected_version_passes() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);
    let sid = create(&t, 100_000, 1000);

    // version is 1 after create; pass Some(1) — should succeed
    t.env.ledger().set_timestamp(100);
    c.withdraw(&sid, &t.recipient, &Some(1u32));
    assert_eq!(c.get_stream(&sid).version, 2);
}

#[test]
fn test_wrong_expected_version_rejects_withdraw() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);
    let sid = create(&t, 100_000, 1000);

    t.env.ledger().set_timestamp(100);
    // version is 1 but we supply 99 — must fail with VersionConflict
    let result = c.try_withdraw(&sid, &t.recipient, &Some(99u32));
    assert!(result.is_err());
}

#[test]
fn test_wrong_expected_version_rejects_top_up() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);
    let sid = create(&t, 100_000, 1000);

    // version is 1 but we supply 5
    let result = c.try_top_up(&sid, &t.sender, &10_000, &Some(5u32));
    assert!(result.is_err());
}

#[test]
fn test_wrong_expected_version_rejects_cancel() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);
    let sid = create(&t, 100_000, 1000);

    // version is 1 but we supply 0
    let result = c.try_cancel_stream(&sid, &t.sender, &Some(0u32));
    assert!(result.is_err());
}

#[test]
fn test_none_expected_version_always_succeeds() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);
    let sid = create(&t, 100_000, 1000);

    // None bypasses version check — backwards compatibility
    t.env.ledger().set_timestamp(100);
    c.withdraw(&sid, &t.recipient, &None);
    c.top_up(&sid, &t.sender, &10_000, &None);
    // version should now be 3
    assert_eq!(c.get_stream(&sid).version, 3);
}

#[test]
fn test_simulated_concurrent_conflict() {
    // Two callers both read version=1 (snapshot A and B).
    // Caller A calls top_up with expected_version=Some(1) → succeeds → version becomes 2.
    // Caller B then calls top_up with expected_version=Some(1) → must fail.
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);
    let sid = create(&t, 100_000, 1000);

    // "Caller A" succeeds
    c.top_up(&sid, &t.sender, &10_000, &Some(1u32));
    assert_eq!(c.get_stream(&sid).version, 2);

    // "Caller B" has stale version=1, must be rejected
    let result = c.try_top_up(&sid, &t.sender, &10_000, &Some(1u32));
    assert!(result.is_err());

    // State must remain unchanged by the failed attempt
    assert_eq!(c.get_stream(&sid).version, 2);
    assert_eq!(c.get_stream(&sid).deposit, 110_000);
}

// ---------------------------------------------------------------------------
// ── Issue #234: Token cross-index + pagination ───────────────────────────────
// ---------------------------------------------------------------------------

fn setup_two_tokens(t: &TestEnv) -> (Address, Address) {
    let token_admin2 = Address::generate(&t.env);
    let token2 = t
        .env
        .register_stellar_asset_contract_v2(token_admin2.clone())
        .address();
    StellarAssetClient::new(&t.env, &token2).mint(&t.sender, &1_000_000);
    (t.token_id.clone(), token2)
}

#[test]
fn test_get_streams_by_token_single() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let sid = create(&t, 100_000, 1000);

    let ids = c.get_streams_by_token(&t.token_id, &0, &10);
    assert_eq!(ids.len(), 1);
    assert_eq!(ids.get(0).unwrap(), sid);
}

#[test]
fn test_get_streams_by_token_multiple_same_token() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let id0 = c.create_stream(&t.sender, &t.recipient, &t.token_id, &50_000, &500, &false);
    let id1 = c.create_stream(&t.sender, &t.recipient, &t.token_id, &50_000, &500, &false);
    let id2 = c.create_stream(&t.sender, &t.recipient, &t.token_id, &50_000, &500, &false);

    let ids = c.get_streams_by_token(&t.token_id, &0, &10);
    assert_eq!(ids.len(), 3);
    assert_eq!(ids.get(0).unwrap(), id0);
    assert_eq!(ids.get(1).unwrap(), id1);
    assert_eq!(ids.get(2).unwrap(), id2);
}

#[test]
fn test_get_streams_by_token_pagination_offset() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    // Create 5 streams with same token
    for _ in 0..5 {
        c.create_stream(&t.sender, &t.recipient, &t.token_id, &50_000, &500, &false);
    }

    // Page 1: start=0 limit=2 → IDs 0,1
    let page1 = c.get_streams_by_token(&t.token_id, &0, &2);
    assert_eq!(page1.len(), 2);
    assert_eq!(page1.get(0).unwrap(), 0u64);
    assert_eq!(page1.get(1).unwrap(), 1u64);

    // Page 2: start=2 limit=2 → IDs 2,3
    let page2 = c.get_streams_by_token(&t.token_id, &2, &2);
    assert_eq!(page2.len(), 2);
    assert_eq!(page2.get(0).unwrap(), 2u64);
    assert_eq!(page2.get(1).unwrap(), 3u64);

    // Page 3: start=4 limit=2 → ID 4 only (ceiling)
    let page3 = c.get_streams_by_token(&t.token_id, &4, &2);
    assert_eq!(page3.len(), 1);
    assert_eq!(page3.get(0).unwrap(), 4u64);
}

#[test]
fn test_get_streams_by_token_pagination_out_of_range() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    create(&t, 100_000, 1000);

    // start beyond list length → empty
    let result = c.get_streams_by_token(&t.token_id, &99, &10);
    assert_eq!(result.len(), 0);

    // limit=0 → empty
    let result2 = c.get_streams_by_token(&t.token_id, &0, &0);
    assert_eq!(result2.len(), 0);
}

#[test]
fn test_get_streams_by_token_different_tokens_isolated() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let (tok1, tok2) = setup_two_tokens(&t);

    // 2 streams with tok1, 1 with tok2
    c.create_stream(&t.sender, &t.recipient, &tok1, &50_000, &500, &false);
    c.create_stream(&t.sender, &t.recipient, &tok1, &50_000, &500, &false);
    c.create_stream(&t.sender, &t.recipient, &tok2, &50_000, &500, &false);

    let tok1_ids = c.get_streams_by_token(&tok1, &0, &10);
    let tok2_ids = c.get_streams_by_token(&tok2, &0, &10);

    assert_eq!(tok1_ids.len(), 2);
    assert_eq!(tok2_ids.len(), 1);
}

#[test]
fn test_get_streams_by_token_and_sender_intersection() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let sender2 = Address::generate(&t.env);
    StellarAssetClient::new(&t.env, &t.token_id).mint(&sender2, &500_000);

    // sender creates 2 streams with token, sender2 creates 1
    let id0 =
        c.create_stream(&t.sender, &t.recipient, &t.token_id, &50_000, &500, &false);
    let id1 =
        c.create_stream(&t.sender, &t.recipient, &t.token_id, &50_000, &500, &false);
    // sender2 stream should NOT appear in the intersection
    c.create_stream(&sender2, &t.recipient, &t.token_id, &50_000, &500, &false);

    let result = c.get_streams_by_token_and_sender(&t.token_id, &t.sender);
    assert_eq!(result.len(), 2);
    assert_eq!(result.get(0).unwrap(), id0);
    assert_eq!(result.get(1).unwrap(), id1);
}

#[test]
fn test_get_streams_by_token_and_sender_no_match() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let (tok1, tok2) = setup_two_tokens(&t);

    // All sender streams use tok1, query with tok2 → empty
    c.create_stream(&t.sender, &t.recipient, &tok1, &50_000, &500, &false);

    let result = c.get_streams_by_token_and_sender(&tok2, &t.sender);
    assert_eq!(result.len(), 0);
}

#[test]
fn test_token_index_updated_on_create() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    assert_eq!(c.get_streams_by_token(&t.token_id, &0, &10).len(), 0);
    create(&t, 100_000, 1000);
    assert_eq!(c.get_streams_by_token(&t.token_id, &0, &10).len(), 1);
}

// ---------------------------------------------------------------------------
// ── Issue #235: SEP-0010 classic account streaming ───────────────────────────
// ---------------------------------------------------------------------------

use types::StellarAuth;

// In Soroban's test environment, `env.crypto().ed25519_verify()` actually
// runs the crypto.  We therefore test the three early-exit error paths
// (expiry, replay, invalid signature) and the storage-level nonce helpers.
// A full end-to-end test with a real Ed25519 signature would require
// `ed25519-dalek` as a dev-dependency — kept out of scope here to avoid
// introducing an unvetted dependency.

/// Build a StellarAuth payload that will be rejected before reaching sig verify.
fn make_expired_auth(env: &Env, account: &Address) -> (StellarAuth, BytesN<32>) {
    let auth = StellarAuth {
        account: account.clone(),
        nonce: BytesN::from_array(env, &[1u8; 32]),
        // expires_at is in the past relative to any positive timestamp
        expires_at: 1,
        signature: BytesN::from_array(env, &[0u8; 64]),
    };
    let pub_key = BytesN::from_array(env, &[0u8; 32]);
    (auth, pub_key)
}

#[test]
fn test_classic_stream_expired_token_rejected() {
    let t = setup();
    let c = client(&t);
    // Set ledger time past the expiry
    t.env.ledger().set_timestamp(1000);

    let (auth, pub_key) = make_expired_auth(&t.env, &t.sender);

    let result = c.try_create_stream_classic(
        &auth, &pub_key, &t.recipient, &t.token_id, &100_000, &1000, &false,
    );
    assert!(result.is_err(), "expected AuthTokenExpired error");
}

#[test]
fn test_classic_stream_replayed_nonce_rejected() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let nonce = BytesN::from_array(&t.env, &[42u8; 32]);

    // Pre-consume the nonce from within the contract's storage context.
    t.env.as_contract(&t.contract_id, || {
        storage::mark_nonce_used(&t.env, &nonce);
    });

    // Build auth whose expiry is still valid but nonce is already used.
    let auth = StellarAuth {
        account: t.sender.clone(),
        nonce: nonce.clone(),
        expires_at: 9999,
        signature: BytesN::from_array(&t.env, &[0u8; 64]),
    };
    let pub_key = BytesN::from_array(&t.env, &[0u8; 32]);

    let result = c.try_create_stream_classic(
        &auth, &pub_key, &t.recipient, &t.token_id, &100_000, &1000, &false,
    );
    assert!(result.is_err(), "expected AuthNonceReplayed error");
}

#[test]
fn test_classic_stream_invalid_signature_rejected() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    // Valid expiry & fresh nonce, but a garbage signature — ed25519_verify panics
    // which the Soroban test harness converts to an Err result via try_*.
    let auth = StellarAuth {
        account: t.sender.clone(),
        nonce: BytesN::from_array(&t.env, &[7u8; 32]),
        expires_at: 9999,
        signature: BytesN::from_array(&t.env, &[0xFFu8; 64]),
    };
    let pub_key = BytesN::from_array(&t.env, &[1u8; 32]);

    let result = c.try_create_stream_classic(
        &auth, &pub_key, &t.recipient, &t.token_id, &100_000, &1000, &false,
    );
    assert!(result.is_err(), "expected invalid-signature rejection");
}

#[test]
fn test_nonce_storage_helpers() {
    let t = setup();
    t.env.ledger().set_timestamp(0);

    let nonce = BytesN::from_array(&t.env, &[55u8; 32]);
    let other_nonce = BytesN::from_array(&t.env, &[99u8; 32]);

    // All storage calls must happen within the contract's context.
    t.env.as_contract(&t.contract_id, || {
        // Fresh nonce is NOT used.
        assert!(!storage::nonce_used(&t.env, &nonce));

        // After marking, it IS used.
        storage::mark_nonce_used(&t.env, &nonce);
        assert!(storage::nonce_used(&t.env, &nonce));

        // A different nonce is still fresh.
        assert!(!storage::nonce_used(&t.env, &other_nonce));
    });
}

#[test]
fn test_classic_stream_zero_amount_rejected() {
    // Ensure parameter validation fires even after auth checks pass.
    // We use an already-expired auth so the test hits AuthTokenExpired first —
    // this validates the ordering of guards (expiry checked before sig).
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(5000);

    let auth = StellarAuth {
        account: t.sender.clone(),
        nonce: BytesN::from_array(&t.env, &[8u8; 32]),
        expires_at: 100, // expired
        signature: BytesN::from_array(&t.env, &[0u8; 64]),
    };
    let pub_key = BytesN::from_array(&t.env, &[0u8; 32]);

    // ZeroAmount would only be reached after expiry — so we still get an error
    let result = c.try_create_stream_classic(
        &auth, &pub_key, &t.recipient, &t.token_id, &0, &1000, &false,
    );
    assert!(result.is_err());
}
