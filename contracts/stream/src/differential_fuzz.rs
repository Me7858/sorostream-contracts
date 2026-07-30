#![cfg(test)]

extern crate std;

use crate::vesting_math;
use proptest::prelude::*;

/// Independent "SDK" reference implementation of the vesting claimable calculation.
/// Written with different control flow from the contract's version to maximize
/// the chance of catching implementation divergence.
fn sdk_compute_claimable(
    deposit: i128,
    duration_secs: u64,
    now: u64,
    cliff_time: u64,
    end_time: u64,
    last_withdraw_time: u64,
) -> i128 {
    if now < cliff_time {
        return 0;
    } 
    let rate = deposit / (duration_secs as i128);
    if rate == 0 {
        return 0;
    }
    let capped = core::cmp::min(now, end_time);
    if capped <= last_withdraw_time {
        return 0;
    }
    let secs = capped - last_withdraw_time;
    rate * (secs as i128)
}

/// Independent "SDK" reference for earned amount (no cliff).
fn sdk_compute_earned(
    deposit: i128,
    duration_secs: u64,
    now: u64,
    end_time: u64,
    last_withdraw_time: u64,
) -> i128 {
    let rate = deposit / (duration_secs as i128);
    if rate == 0 {
        return 0;
    }
    let capped = if now > end_time { end_time } else { now };
    if last_withdraw_time >= capped {
        return 0;
    }
    rate * ((capped - last_withdraw_time) as i128)
}

/// Independent "SDK" reference for refund on cancellation.
fn sdk_compute_refund(
    deposit: i128,
    duration_secs: u64,
    now: u64,
    end_time: u64,
    start_time: u64,
) -> i128 {
    let rate = deposit / (duration_secs as i128);
    if rate == 0 {
        return deposit;
    }
    let capped = if now > end_time { end_time } else { now };
    let streamed_secs = if capped > start_time {
        capped - start_time
    } else {
        0
    };
    let total_streamed = rate * (streamed_secs as i128);
    if deposit > total_streamed {
        deposit - total_streamed
    } else {
        0
    }
}

// ── Differential fuzz tests: 1,000,000 iterations ───────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1_000_000))]

    /// Contract vs SDK: compute_claimable must agree within 1 stroop.
    #[test]
    fn differential_fuzz_claimable(
        deposit in 1_i128..=1_000_000_000_i128,
        duration in 1_u64..=10_000_000_u64,
        start_time in 0_u64..=1_000_000_u64,
        cliff_offset in 0_u64..=10_000_000_u64,
        elapsed_offset in 0_u64..=20_000_000_u64,
    ) {
        let flow_rate = deposit / duration as i128;
        if flow_rate == 0 { return Ok(()); }

        let cliff_offset = cliff_offset.min(duration);
        let end_time = start_time.saturating_add(duration);
        let cliff_time = start_time.saturating_add(cliff_offset);
        let last_withdraw_time = start_time;
        let now = start_time.saturating_add(elapsed_offset);

        let contract_result = vesting_math::compute_claimable(
            flow_rate, now, cliff_time, end_time, last_withdraw_time,
        );
        let sdk_result = sdk_compute_claimable(
            deposit, duration, now, cliff_time, end_time, last_withdraw_time,
        );

        let diff = (contract_result - sdk_result).abs();
        prop_assert!(
            diff <= 1,
            "claimable divergence {} > 1 stroop: contract={}, sdk={}, deposit={}, duration={}, now={}, cliff={}, end={}, lwt={}",
            diff, contract_result, sdk_result, deposit, duration, now, cliff_time, end_time, last_withdraw_time
        );
    }

    /// Contract vs SDK: compute_earned must agree within 1 stroop.
    #[test]
    fn differential_fuzz_earned(
        deposit in 1_i128..=1_000_000_000_i128,
        duration in 1_u64..=10_000_000_u64,
        start_time in 0_u64..=1_000_000_u64,
        elapsed_offset in 0_u64..=20_000_000_u64,
        lwt_offset in 0_u64..=10_000_000_u64,
    ) {
        let flow_rate = deposit / duration as i128;
        if flow_rate == 0 { return Ok(()); }

        let end_time = start_time.saturating_add(duration);
        let lwt_offset = lwt_offset.min(duration);
        let last_withdraw_time = start_time.saturating_add(lwt_offset);
        let now = start_time.saturating_add(elapsed_offset);

        let contract_result = vesting_math::compute_earned(
            flow_rate, now, end_time, last_withdraw_time,
        );
        let sdk_result = sdk_compute_earned(
            deposit, duration, now, end_time, last_withdraw_time,
        );

        let diff = (contract_result - sdk_result).abs();
        prop_assert!(
            diff <= 1,
            "earned divergence {} > 1 stroop: contract={}, sdk={}, deposit={}, duration={}, now={}, end={}, lwt={}",
            diff, contract_result, sdk_result, deposit, duration, now, end_time, last_withdraw_time
        );
    }

    /// Contract vs SDK: compute_refund must agree within 1 stroop.
    #[test]
    fn differential_fuzz_refund(
        deposit in 1_i128..=1_000_000_000_i128,
        duration in 1_u64..=10_000_000_u64,
        start_time in 0_u64..=1_000_000_u64,
        elapsed_offset in 0_u64..=20_000_000_u64,
    ) {
        let flow_rate = deposit / duration as i128;
        if flow_rate == 0 { return Ok(()); }

        let end_time = start_time.saturating_add(duration);
        let now = start_time.saturating_add(elapsed_offset);

        let contract_result = vesting_math::compute_refund(
            deposit, flow_rate, now, end_time, start_time,
        );
        let sdk_result = sdk_compute_refund(
            deposit, duration, now, end_time, start_time,
        );

        let diff = (contract_result - sdk_result).abs();
        prop_assert!(
            diff <= 1,
            "refund divergence {} > 1 stroop: contract={}, sdk={}, deposit={}, duration={}, now={}, end={}, start={}",
            diff, contract_result, sdk_result, deposit, duration, now, end_time, start_time
        );
    }
}

// ── Cross-function consistency: 1,000,000 iterations ────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1_000_000))]

    /// earned + refund = deposit for any valid stream parameters.
    #[test]
    fn differential_fuzz_earned_plus_refund_eq_deposit(
        deposit in 1_i128..=1_000_000_000_i128,
        duration in 1_u64..=10_000_000_u64,
        start_time in 0_u64..=1_000_000_u64,
        elapsed_offset in 0_u64..=20_000_000_u64,
    ) {
        let flow_rate = deposit / duration as i128;
        if flow_rate == 0 { return Ok(()); }

        let end_time = start_time.saturating_add(duration);
        let now = start_time.saturating_add(elapsed_offset);

        let total_streamed = vesting_math::compute_total_streamed(
            flow_rate, now, end_time, start_time,
        );
        let refund = vesting_math::compute_refund(
            deposit, flow_rate, now, end_time, start_time,
        );

        prop_assert_eq!(
            total_streamed + refund, deposit,
            "total_streamed + refund must equal deposit"
        );
    }
}
