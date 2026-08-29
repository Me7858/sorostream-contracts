//! Unit tests for the four protocol-economics features:
//! - Issue #465: configurable protocol fee recipient + sweep_fees
//! - Issue #462: per-stream fee override (tiered pricing)
//! - Issue #464: referral tracking with on-chain attribution
//! - Issue #463: insurance pool for cancelled-stream recovery

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, String as SdkString,
};

struct PETestEnv {
    env: Env,
    contract_id: Address,
    token_id: Address,
    admin: Address,
    sender: Address,
    recipient: Address,
}

fn default_options() -> StreamCreateOptions {
    StreamCreateOptions {
        renew_count: None,
        allow_recipient_termination: false,
        non_transferable: false,
    }
}

fn setup() -> PETestEnv {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000);

    let contract_id = env.register(SoroStreamContract, ());
    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    StellarAssetClient::new(&env, &token_id).mint(&sender, &10_000_000);

    let c = SoroStreamContractClient::new(&env, &contract_id);
    c.initialize(&admin, &SdkString::from_str(&env, "1.0.0"));
    c.set_min_duration(&sender, &0u64);

    PETestEnv {
        env,
        contract_id,
        token_id,
        admin,
        sender,
        recipient,
    }
}

fn client(t: &PETestEnv) -> SoroStreamContractClient<'_> {
    SoroStreamContractClient::new(&t.env, &t.contract_id)
}

// ═══════════════════════════════════════════════════════════════════════════
// Issue #465: Configurable protocol fee recipient
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_set_and_get_fee_recipient() {
    let t = setup();
    let c = client(&t);
    assert_eq!(c.get_fee_recipient(), None);

    let recipient = Address::generate(&t.env);
    c.set_fee_recipient(&recipient);
    assert_eq!(c.get_fee_recipient(), Some(recipient));
}

#[test]
fn test_sweep_fees_requires_fee_recipient_configured() {
    let t = setup();
    let c = client(&t);
    // No fee recipient configured yet.
    let result = c.try_sweep_fees(&t.token_id);
    assert_eq!(result, Err(Ok(StreamError::NotInitialized)));
}

#[test]
fn test_sweep_fees_pays_configured_recipient_not_arbitrary_destination() {
    let t = setup();
    let c = client(&t);

    c.set_protocol_fee(&1000u32); // 10%
    let fee_recipient = Address::generate(&t.env);
    c.set_fee_recipient(&fee_recipient);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &1_000_000, &1000, &0, &0u64, &false, &0u64,
        &default_options(),
    );
    t.env.ledger().set_timestamp(1_000 + 1000);
    c.withdraw(&stream_id, &t.recipient);

    assert!(c.get_fees_collected(&t.token_id) > 0);

    c.sweep_fees(&t.token_id);

    assert_eq!(c.get_fees_collected(&t.token_id), 0);
    assert_eq!(TokenClient::new(&t.env, &t.token_id).balance(&fee_recipient), 100_000);
}

// ═══════════════════════════════════════════════════════════════════════════
// Issue #462: Per-stream fee override
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_stream_fee_override_rejects_invalid_bps() {
    let t = setup();
    let c = client(&t);
    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &1_000_000, &1000, &0, &0u64, &false, &0u64,
        &default_options(),
    );
    let result = c.try_set_stream_fee_override(&stream_id, &10_001u32);
    assert_eq!(result, Err(Ok(StreamError::InvalidFeeRate)));
}

#[test]
fn test_stream_fee_override_applied_instead_of_default_on_withdraw() {
    let t = setup();
    let c = client(&t);
    c.set_protocol_fee(&1000u32); // global default: 10%

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &1_000_000, &1000, &0, &0u64, &false, &0u64,
        &default_options(),
    );
    // Premium tier: 1% instead of the 10% default.
    c.set_stream_fee_override(&stream_id, &100u32);
    assert_eq!(c.get_stream_fee_override(&stream_id), Some(100u32));

    t.env.ledger().set_timestamp(1_000 + 1000);
    c.withdraw(&stream_id, &t.recipient);

    // 1% of 1_000_000 = 10_000, not the 100_000 the 10% default would charge.
    assert_eq!(c.get_fees_collected(&t.token_id), 10_000);
    assert_eq!(TokenClient::new(&t.env, &t.token_id).balance(&t.recipient), 990_000);
}

#[test]
fn test_clear_stream_fee_override_reverts_to_default() {
    let t = setup();
    let c = client(&t);
    c.set_protocol_fee(&1000u32);

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &1_000_000, &1000, &0, &0u64, &false, &0u64,
        &default_options(),
    );
    c.set_stream_fee_override(&stream_id, &100u32);
    c.clear_stream_fee_override(&stream_id);
    assert_eq!(c.get_stream_fee_override(&stream_id), None);

    t.env.ledger().set_timestamp(1_000 + 1000);
    c.withdraw(&stream_id, &t.recipient);
    assert_eq!(c.get_fees_collected(&t.token_id), 100_000);
}

// ═══════════════════════════════════════════════════════════════════════════
// Issue #464: Referral tracking
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_stream_referral_requires_stream_sender() {
    let t = setup();
    let c = client(&t);
    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &1_000_000, &1000, &0, &0u64, &false, &0u64,
        &default_options(),
    );
    let not_sender = Address::generate(&t.env);
    let referral = Address::generate(&t.env);
    let result = c.try_set_stream_referral(&stream_id, &not_sender, &referral);
    assert_eq!(result, Err(Ok(StreamError::NotSender)));
}

#[test]
fn test_referral_reward_paid_atomically_on_withdraw() {
    let t = setup();
    let c = client(&t);
    c.set_protocol_fee(&1000u32); // 10%
    c.set_referral_fee_share(&2000u32); // referral gets 20% of the protocol fee

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &1_000_000, &1000, &0, &0u64, &false, &0u64,
        &default_options(),
    );
    let referral = Address::generate(&t.env);
    c.set_stream_referral(&stream_id, &t.sender, &referral);
    assert_eq!(c.get_stream_referral(&stream_id), Some(referral.clone()));

    t.env.ledger().set_timestamp(1_000 + 1000);
    c.withdraw(&stream_id, &t.recipient);

    // Total fee = 10% of 1_000_000 = 100_000. Referral share = 20% of that = 20_000.
    assert_eq!(TokenClient::new(&t.env, &t.token_id).balance(&referral), 20_000);
    assert_eq!(c.get_referral_rewards(&referral, &t.token_id), 20_000);
    // The remainder (80_000) still goes to the protocol fee reserve.
    assert_eq!(c.get_fees_collected(&t.token_id), 80_000);
}

#[test]
fn test_no_referral_means_fees_fully_accumulated() {
    let t = setup();
    let c = client(&t);
    c.set_protocol_fee(&1000u32);
    c.set_referral_fee_share(&2000u32); // configured, but no referral attributed to this stream

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &1_000_000, &1000, &0, &0u64, &false, &0u64,
        &default_options(),
    );
    t.env.ledger().set_timestamp(1_000 + 1000);
    c.withdraw(&stream_id, &t.recipient);

    assert_eq!(c.get_fees_collected(&t.token_id), 100_000);
}

// ═══════════════════════════════════════════════════════════════════════════
// Issue #463: Insurance pool
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_create_stream_funds_insurance_reserve() {
    let t = setup();
    let c = client(&t);
    c.set_insurance_bps(&500u32); // 5% of deposit

    assert_eq!(c.get_insurance_reserve(&t.token_id), 0);

    c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &1_000_000, &1000, &0, &0u64, &false, &0u64,
        &default_options(),
    );

    // 5% of 1_000_000 = 50_000, collected in addition to the 1_000_000 deposit.
    assert_eq!(c.get_insurance_reserve(&t.token_id), 50_000);
    assert_eq!(TokenClient::new(&t.env, &t.token_id).balance(&t.sender), 10_000_000 - 1_000_000 - 50_000);
}

#[test]
fn test_cancel_stream_as_failure_requires_admin_or_guardian() {
    let t = setup();
    let c = client(&t);
    c.set_insurance_bps(&500u32);
    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &1_000_000, &1000, &0, &0u64, &false, &0u64,
        &default_options(),
    );
    let result = c.try_cancel_stream_as_failure(&stream_id, &t.sender);
    assert_eq!(result, Err(Ok(StreamError::NotAuthorized)));
}

#[test]
fn test_cancel_stream_as_failure_pays_insurance_claim_to_recipient() {
    let t = setup();
    let c = client(&t);
    c.set_insurance_bps(&2000u32); // 20% of deposit is covered

    let stream_id = c.create_stream(
        &t.sender, &t.recipient, &t.token_id, &1_000_000, &1000, &0, &0u64, &false, &0u64,
        &default_options(),
    );
    // Insurance reserve funded with 20% of 1_000_000 = 200_000.
    assert_eq!(c.get_insurance_reserve(&t.token_id), 200_000);

    // Cancel immediately (t == start_time): recipient has earned nothing yet,
    // so the entire 1_000_000 deposit is the "shortfall" — but insurance only
    // covers up to the 200_000 cap.
    c.cancel_stream_as_failure(&stream_id, &t.admin);

    assert_eq!(TokenClient::new(&t.env, &t.token_id).balance(&t.recipient), 200_000);
    // Sender gets their full deposit back (insurance payout is not clawed
    // back from the sender — it comes from the separately-funded reserve).
    assert_eq!(
        TokenClient::new(&t.env, &t.token_id).balance(&t.sender),
        10_000_000 - 200_000, // spent 1_000_000 + 200_000 insurance, got 1_000_000 back
    );
    assert_eq!(c.get_insurance_reserve(&t.token_id), 0);
}

#[test]
fn test_cancel_stream_as_failure_rejects_milestone_streams() {
    let t = setup();
    let c = client(&t);
    let milestones = soroban_sdk::vec![
        &t.env,
        (1_000_000i128, t.env.ledger().timestamp() + 1000, soroban_sdk::BytesN::from_array(&t.env, &[0u8; 32])),
    ];
    let stream_id = c.create_stream_with_milestones(
        &t.sender, &t.recipient, &t.token_id, &1_000_000, &milestones, &0u64, &0u64, &false,
    );
    let result = c.try_cancel_stream_as_failure(&stream_id, &t.admin);
    assert_eq!(result, Err(Ok(StreamError::StreamNotActive)));
}
