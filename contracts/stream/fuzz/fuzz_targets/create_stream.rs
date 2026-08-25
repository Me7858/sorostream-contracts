#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::token::StellarAssetClient;
use soroban_sdk::{Address, Env};
use sorostream_stream::{SoroStreamContract, SoroStreamContractClient};

/// Boundary-biased combination of every integer/bool parameter `create_stream`
/// accepts, plus the ledger timestamp at call time. `Arbitrary` draws these
/// from raw fuzzer bytes, so libFuzzer's mutation engine explores the full
/// range of each field (including extremes like 0, negative, `u64::MAX`)
/// rather than only the values a human reviewer would think to try.
#[derive(Debug, Arbitrary)]
struct CreateStreamInput {
    now: u64,
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
}

fuzz_target!(|input: CreateStreamInput| {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(input.now);

    let contract_id = env.register(SoroStreamContract, ());
    let client = SoroStreamContractClient::new(&env, &contract_id);

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    // Generous, fixed balance: we want the fuzzer exploring create_stream's
    // own validation/arithmetic, not tripping over "insufficient balance"
    // transfer failures for every large `amount` it tries.
    StellarAssetClient::new(&env, &token_id).mint(&sender, &(i128::MAX / 2));

    client.set_min_duration(&sender, &0u64);

    // A validation failure returning `Err` is expected and fine -- it means
    // create_stream's own checks caught the bad combination of parameters.
    // What we're hunting for is anything that makes the host panic instead
    // of returning a declared `StreamError` (arithmetic overflow outside a
    // `checked_*` call, an `unwrap()` on an unexpected `None`, an
    // out-of-bounds index, ...). libFuzzer records any such panic as a crash.
    let _ = client.try_create_stream(
        &sender,
        &recipient,
        &token_id,
        &input.amount,
        &input.duration_seconds,
        &input.cliff_seconds,
        &input.nonce,
        &input.auto_renew,
        &input.lock_until,
        &input.allow_recipient_termination,
        &input.holdback_amount,
        &input.withdrawal_steps,
        &input.min_withdrawal_amount,
    );
});
