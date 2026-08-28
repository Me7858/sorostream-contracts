# Fix: Flow Rate Overflow During Withdraw

## Problem Statement
Setting the stream rate to an extremely large value could cause the accrued amount calculation to produce an incorrect result or runtime error during withdraw. Specifically:
- A stream could be created with an unsafe `flow_rate` value
- During withdrawal, multiplying `flow_rate * elapsed_time` could overflow
- The overflow would be caught as `StreamError::Overflow`, but only at withdraw time
- This delayed error discovery is inefficient and confusing

## Root Cause
During stream creation (`create_stream` function, line 438):
```rust
let flow_rate = streaming_amount / duration_seconds as i128;
if flow_rate == 0 {
    return Err(StreamError::ZeroFlowRate);
}
// ⚠️ NO validation that flow_rate is within safe bounds!
```

There was no upper bound validation on `flow_rate`. This allowed creation of streams where:
- `flow_rate` could be very close to `i128::MAX`
- Any multiplication `flow_rate * elapsed_time` would overflow
- This would cause ALL future withdrawals to fail with `StreamError::Overflow`

## Solution
Added flow_rate bounds validation at stream creation time:

### 1. Define Safe Flow Rate Bound (line 79)
```rust
const MAX_SAFE_FLOW_RATE: i128 = i128::MAX / (MAX_STREAM_DURATION_SECONDS as i128);
```
This ensures: `flow_rate * any_valid_duration <= i128::MAX`

### 2. Validation Function (lines 81-89)
```rust
fn validate_flow_rate_bounds(flow_rate: i128) -> Result<(), StreamError> {
    if flow_rate <= 0 {
        return Err(StreamError::ZeroFlowRate);
    }
    if flow_rate > MAX_SAFE_FLOW_RATE {
        return Err(StreamError::Overflow);
    }
    Ok(())
}
```

### 3. Validation Call in create_stream (line 450)
```rust
// After computing flow_rate:
validate_flow_rate_bounds(flow_rate)?;
```

## Benefits

✅ **Early Error Detection**: Unsafe flow rates are caught at stream creation time, not during withdraw  
✅ **Deterministic Behavior**: Prevents "runtime errors" - all validation is explicit  
✅ **Clear Error Messages**: Developers get immediate feedback if their stream parameters are invalid  
✅ **Safe Arithmetic**: Guarantees that all future withdrawals will have safe arithmetic operations  
✅ **Backward Compatible**: Only rejects streams that would have failed at withdrawal anyway  

## Safety Properties Maintained

- ✅ **Type Safety**: Returns typed `StreamError::Overflow` instead of panicking
- ✅ **Bounds Guaranteed**: `flow_rate <= i128::MAX / (100 years)` ensures safe multiplication
- ✅ **No Gaps**: All overflow scenarios now fail at creation or are mathematically impossible

## Test Cases Added

### test_flow_rate_bounds_validation_prevents_overflow
- Attempts creation with `amount = i128::MAX` and `duration = 1 second`
- Verifies rejection at creation time with `StreamError::Overflow`

### test_large_flow_rate_with_long_duration_succeeds  
- Creates stream with realistic large amount (10^18 stroops) and 1-year duration
- Verifies creation succeeds and withdraw operations work correctly

### test_extremely_large_flow_rate_causes_creation_error
- Tests another unsafe scenario with amount close to `i128::MAX`
- Confirms validation catches edge cases

## Impact Analysis

| Aspect | Impact |
|--------|--------|
| **Functional** | None - only prevents creation of impossible streams |
| **Gas** | Negligible - adds one comparison operation per stream creation |
| **Security** | Positive - removes potential panic vector |
| **Backward Compat** | Full - existing valid streams unaffected |

## Mathematical Basis

Given:
- `MAX_STREAM_DURATION_SECONDS = 100 * 365 * 24 * 60 * 60 ≈ 3.15e9 seconds`
- `MAX_SAFE_FLOW_RATE = i128::MAX / (3.15e9) ≈ 2.93e18 stroops/second`

For any stream:
- Duration ≤ 100 years
- Flow rate ≤ 2.93e18 stroops/sec
- Maximum streamed = flow_rate * duration ≤ i128::MAX ✓

## Files Modified
1. `/workspaces/sorostream-contracts/contracts/stream/src/lib.rs`
   - Added `MAX_SAFE_FLOW_RATE` constant
   - Added `validate_flow_rate_bounds()` function
   - Added validation call in `create_stream()`

2. `/workspaces/sorostream-contracts/contracts/stream/src/test.rs`
   - Added 3 comprehensive test cases

## Verification Steps

1. Flow rate bounds are validated at creation: ✅
2. Overflow errors are caught with typed error: ✅
3. Safe streams still work correctly: ✅
4. Edge cases are tested: ✅
