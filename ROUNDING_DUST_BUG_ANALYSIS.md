# Rounding Dust Accumulation Bug - Detailed Analysis

## Issue Summary

When streams have odd stroop-per-second (flow) rates that create rounding discrepancies through integer division, the DUST_THRESHOLD check causes intermediate withdrawals to silently skip 1-stroop dust amounts. This accumulates over long durations, causing the final withdrawal to either under-pay the recipient or leave a residual stroop in the contract.

## Root Cause

### Part 1: Flow Rate Rounding

When creating a stream:
```rust
flow_rate = deposit / duration_seconds  // Integer division floors the result
```

Example:
- `deposit = 100`, `duration = 3` seconds
- `flow_rate = 100 / 3 = 33` stroops/second
- `flow_rate * duration = 33 * 3 = 99` stroops
- **Rounding dust = 100 - 99 = 1 stroop**

### Part 2: Dust Threshold Silently Skips Micro-withdrawals

**Location**: `contracts/stream/src/lib.rs` lines 2523-2530

```rust
const DUST_THRESHOLD: i128 = 1;

if claimable <= DUST_THRESHOLD {
    // Still update last_withdraw_time to avoid spamming
    stream.last_withdraw_time = effective_now;
    save_stream(&env, &stream);
    clear_reentrancy_lock(&env);
    return Ok(());  // ← EARLY RETURN, NO TRANSFER!
}
```

**Problem**: When `claimable == 1`, the function returns **without**:
1. Transferring the 1 stroop to recipient
2. Updating `stream.total_withdrawn`

This 1 stroop just vanishes from accounting!

## Concrete Bug Scenario

### Setup
- Stream with `deposit = 1001`, `duration = 1000` seconds
- `flow_rate = 1001 / 1000 = 1` stroop/second (integer division)
- `flow_rate * duration = 1 * 1000 = 1000` stroops
- **Expected dust at end = 1001 - 1000 = 1 stroop**

### Timeline

**T=999 (1 second before end)**
- `elapsed = 999 seconds`
- `claimable = flow_rate * elapsed = 1 * 999 = 999` stroops
- Recipient withdraws 999 stroops
- `stream.total_withdrawn = 999`
- **Contract balance: 1001 - 999 = 2 stroops remaining**

**T=1000 (at stream end)**
- `elapsed = 1000 seconds` (from last withdraw at T=999)
- `claimable = flow_rate * 1 = 1` stroop
- **DUST_THRESHOLD check: claimable (1) <= DUST_THRESHOLD (1)? YES**
- **Function returns early WITHOUT transferring the 1 stroop!**
- `stream.total_withdrawn` stays at 999
- **Contract balance still: 2 stroops**

**T=1001 (final settlement, stream_ended = true)**
- `dust = deposit - (flow_rate * duration) = 1001 - 1000 = 1` stroop
- Dust is refunded to sender: 1 stroop
- **Contract balance: 2 - 1 = 1 stroop STUCK**

### Result
- Recipient should have received: 1000 stroops (gets 999)
- Sender should get refunded: 1 stroop (gets 1)
- **Under-payment to recipient: 1 stroop**
- **Stranded in contract: 1 stroop**

## Why It Accumulates Over Long Durations

The DUST_THRESHOLD logic silently swallows any intermediate withdrawals where `claimable == 1`. On a stream with a low flow_rate:

- Example: `deposit = 1,000,000`, `duration = 1,000,000` seconds
- `flow_rate = 1` stroop/second
- User could withdraw multiple times, each time 1 stroop is due
- Each intermediate withdrawal near stream end silently loses 1 stroop
- Final settlement can't recover these lost stroops

## The Real Problem Statement

The DUST_THRESHOLD was intended to prevent failed micro-transactions, but it violates **balance conservation** by silently discarding tokens rather than:
1. **Transferring them to the recipient**, or
2. **Accumulating them in the stream for the final settlement**, or
3. **Refunding them to the sender**

## Impact

- **Severity**: High - violates fundamental token accounting
- **Affected Scenarios**: 
  - Any stream with `flow_rate * duration < deposit` (all rounded streams)
  - Long-duration streams with low flow rates
  - Multiple intermediate withdrawals near stream end
- **Symptom**: 
  - Recipient under-receives by 1-N stroops
  - Contract holds orphaned 1-N stroops indefinitely
  - Balance conservation violation

## Current Code Flow Issues

**File**: `contracts/stream/src/lib.rs`

### Path A: Intermediate Withdrawal (lines 2523-2530)
```rust
if claimable <= DUST_THRESHOLD {
    stream.last_withdraw_time = effective_now;
    save_stream(&env, &stream);
    return Ok(());  // ← BUG: 1 stroop vanishes
}
```

### Path B: Stream End Settlement (lines 2598-2605)
```rust
let dust = stream.deposit.saturating_sub(
    stream.flow_rate.saturating_mul(duration as i128),
);

if dust > 0 {
    token_client.transfer(..., &stream.sender, &dust);
}
```

**Issue**: Dust calculation doesn't account for stroops that were skipped by DUST_THRESHOLD logic!

## Proposed Fix

### Option 1: Don't Skip at Dust Threshold, Always Transfer
Remove the early return and allow even 1-stroop transfers:
```rust
// Remove this check entirely or modify it
// if claimable <= DUST_THRESHOLD {
//     return Ok(());
// }
```

**Pros**: Simple, maintains balance conservation  
**Cons**: Allows micro-transactions

### Option 2: Accumulate Dust in Stream for Final Settlement
Track skipped dust and refund at stream end:
```rust
if claimable <= DUST_THRESHOLD {
    stream.accumulated_dust = stream.accumulated_dust.saturating_add(claimable);
    stream.last_withdraw_time = effective_now;
    save_stream(&env, &stream);
    return Ok(());
}

// At stream end:
let dust = ... + stream.accumulated_dust;
```

**Pros**: Prevents micro-txns, maintains conservation  
**Cons**: More complex, adds field to Stream struct

### Option 3: Adjust Final Settlement Calculation
Ensure final withdrawal captures any skipped dust:
```rust
// At stream end:
let dust = stream.deposit.saturating_sub(stream.total_withdrawn);
token_client.transfer(..., &stream.sender, &dust);
```

**Pros**: Simple, doesn't require new fields  
**Cons**: Refunds dust to sender, not recipient (may be intended)

## Recommendation

**Option 3** is the cleanest fix:
- Change the dust calculation from `flow_rate * duration` to `deposit - total_withdrawn`
- This naturally captures ANY missing stroops, whether from rounding or DUST_THRESHOLD skipping
- Maintains balance conservation
- No new fields or complex tracking needed

## Test Case

Create a test that reproduces the bug:
```rust
#[test]
fn test_dust_accumulation_with_low_flow_rate() {
    // Create stream: deposit=1001, duration=1000
    // flow_rate = 1 stroop/sec
    // Expected dust = 1 stroop
    
    // Withdraw at T=999: gets 999 stroops
    // Withdraw at T=1000: claimable=1 (gets skipped by DUST_THRESHOLD)
    // Final settlement: should get remaining 2 stroops total
    
    // Verify recipient got exactly 1000 stroops
    // Verify no stroops stranded in contract
}
```
