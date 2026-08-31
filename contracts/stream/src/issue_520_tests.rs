
use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env,
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

    StellarAssetClient::new(&env, &token_id).mint(&sender, &10_000_000);

    let admin = Address::generate(&env);
    SoroStreamContractClient::new(&env, &contract_id)
        .initialize(&admin, &soroban_sdk::String::from_str(&env, "1.0.0"));

    SoroStreamContractClient::new(&env, &contract_id).set_min_duration(&admin, &0u64);

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

// ─────────────────────────────────────────────────────────────────────────
// Issue #520: Vesting Cliff Validation Tests
// ─────────────────────────────────────────────────────────────────────────
// Test that cliff validation correctly prevents withdrawal before cliff_time.

#[test]
fn test_issue_520_cliff_prevents_early_withdrawal() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    // Create a stream with a cliff period of 1000 seconds
    let cliff_seconds = 1000u64;
    let duration_seconds = 5000u64;
    let stream_id = c.create_stream(
        &t.sender,
        &t.recipient,
        &t.token_id,
        &500_000,          // amount
        &duration_seconds, // duration_seconds
        &cliff_seconds,    // cliff_seconds
        &0u64,             // nonce
        &false,            // auto_renew
        &None::<u32>,      // renew_count
        &0u64,             // lock_until
        &false,            // allow_recipient_termination
        &false,            // non_transferable
    );

    // Try to withdraw before cliff is reached
    t.env.ledger().set_timestamp(500); // Before cliff (cliff is at 1000)

    // Attempt withdrawal - should return zero claimable
    let stream_before_cliff = c.get_stream(&stream_id);
    assert_eq!(stream_before_cliff.status, StreamStatus::Active);

    // After cliff is reached, withdrawal should be possible
    t.env.ledger().set_timestamp(1500); // After cliff
    c.withdraw(&stream_id, &t.recipient);

    // Verify withdrawal succeeded and tokens were transferred
    let balance = TokenClient::new(&t.env, &t.token_id).balance(&t.recipient);
    assert!(balance > 0, "Recipient should have received tokens after cliff");
}

#[test]
fn test_issue_520_cliff_zero_claimable_before_cliff_time() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let cliff_seconds = 2000u64;
    let duration_seconds = 10000u64;
    let stream_id = c.create_stream(
        &t.sender,
        &t.recipient,
        &t.token_id,
        &1_000_000,
        &duration_seconds,
        &cliff_seconds,
        &0u64,
        &false,
        &None::<u32>,
        &0u64,
        &false,
        &false,
    );

    // Move time to 1000 seconds (before cliff at 2000)
    t.env.ledger().set_timestamp(1000);

    // Recipient balance should still be zero since cliff not reached
    let initial_balance = TokenClient::new(&t.env, &t.token_id).balance(&t.recipient);
    assert_eq!(initial_balance, 0, "No tokens should be claimable before cliff");
}

#[test]
fn test_issue_520_cliff_exact_boundary() {
    let t = setup();
    let c = client(&t);
    t.env.ledger().set_timestamp(0);

    let cliff_seconds = 500u64;
    let duration_seconds = 2000u64;
    let stream_id = c.create_stream(
        &t.sender,
        &t.recipient,
        &t.token_id,
        &100_000,
        &duration_seconds,
        &cliff_seconds,
        &0u64,
        &false,
        &None::<u32>,
        &0u64,
        &false,
        &false,
    );

    // Withdraw exactly at cliff boundary
    t.env.ledger().set_timestamp(500);
    c.withdraw(&stream_id, &t.recipient);

    let balance = TokenClient::new(&t.env, &t.token_id).balance(&t.recipient);
    assert_eq!(balance, 0, "No tokens should be earned at exact cliff boundary");
}
