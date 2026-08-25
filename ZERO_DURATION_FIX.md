# Fix: Zero-Duration Stream Validation

## Problem Statement
The input validation in `createStream` was unclear about rejecting zero-duration streams. A stream with zero duration (where `end_time == start_time`) would produce a problematic state:
- The deposit would supposedly accrue over zero time
- The flow_rate calculation would result in division by zero or ambiguous behavior  
- `flow_rate * 0 elapsed_seconds` would immediately equal 0, causing unexpected accrual results

While the validation technically prevented this (via minimum duration check or end_time validation), the error message wasn't clear, and the code logic could be confusing for developers.

## Root Cause
1. **Implicit reliance on minimum duration**: The default minimum duration (3600 seconds) prevented zero-duration streams, but this was implicit.
2. **No explicit zero-duration check**: The `end_time <= now` check would catch zero-duration streams, but the error (`InvalidEndTime`) wasn't semantically clear.
3. **Could be disabled**: Tests often disable minimum duration validation, allowing potential confusion about whether zero-duration is allowed.

## Solution
Added explicit zero-duration validation at stream creation time:

### 1. Explicit Zero-Duration Check (line 428-433)
```rust
// Explicit zero-duration check for clarity (Issue: allow end_time = start_time vulnerability)
// A stream must have positive duration. Zero duration would mean start_time == end_time,
// which is invalid: the deposit would immediately fully accrue with flow_rate * 0 = 0,
// but the constraint enforcement becomes ambiguous.
if duration_seconds == 0 {
    return Err(StreamError::InvalidDuration);
}
```

This check:
- Comes immediately after minimum duration validation
- Rejects zero duration with a semantically clear error (`InvalidDuration`)
- Provides a detailed comment explaining why zero duration is invalid
- Happens BEFORE any other processing

### 2. Enhanced end_time Validation (line 496-502)
Added comments to the existing `end_time <= now` check to document its role as a defensive check and clarify its purpose.

## Benefits

✅ **Explicit Validation**: Zero-duration streams are clearly and immediately rejected  
✅ **Clear Error Messages**: `InvalidDuration` error clearly indicates the problem  
✅ **Defense-in-Depth**: Both explicit check AND end_time validation provide layered protection  
✅ **Independence**: Works regardless of minimum duration setting  
✅ **Developer Clarity**: Code intent is unmistakable  

## Test Cases Added

### test_zero_duration_explicitly_rejected
- Verifies that zero-duration streams are rejected with `InvalidDuration` error
- Tests the primary scenario described in the issue

### test_minimal_duration_is_allowed
- Ensures that 1-second (minimal non-zero) streams are allowed
- Verifies that end_time > start_time as expected
- Confirms the validation only rejects zero, not minimal positive durations

### test_zero_duration_cannot_be_bypassed_with_minimum_duration_zero
- Tests that zero-duration is rejected even when minimum duration is set to 0
- Ensures the explicit check works independently of configuration
- Demonstrates robustness of the fix

## Impact Analysis

| Aspect | Impact |
|--------|--------|
| **Functional** | None - prevents creating invalid streams |
| **Gas** | Negligible - adds one comparison operation |
| **Security** | Positive - prevents ambiguous stream states |
| **Backward Compat** | Full - zero-duration streams were already prevented |
| **Error Messages** | Improved - more semantic error codes |

## Validation Order

The validation now follows a clear progression:

1. ✓ Check minimum duration
2. ✓ **Check for explicitly zero duration** ← NEW
3. ✓ Check maximum duration
4. ✓ Calculate flow_rate and validate bounds
5. ✓ Calculate end_time and validate it's in future
6. ✓ ... (other validations)

This order ensures:
- Early rejection of clearly invalid inputs
- Clear error messages at the point of violation
- Semantic correctness (zero duration is invalid, not just outside bounds)

## Mathematical Basis

For a valid stream:
- `duration_seconds > 0` (enforced by this fix)
- `end_time = start_time + duration_seconds`
- `end_time > start_time` (guaranteed by the above)
- `flow_rate = deposit / duration > 0` (already validated)
- `accrued = flow_rate * elapsed >= 0` (mathematically sound)

With `duration_seconds == 0`:
- `end_time = start_time` (creates ambiguous state)
- No time window for vesting to occur
- Accrual calculation becomes undefined

## Files Modified
1. `/workspaces/sorostream-contracts/contracts/stream/src/lib.rs`
   - Added explicit zero-duration check in `create_stream()` at line 428-433
   - Enhanced comments on `end_time` validation

2. `/workspaces/sorostream-contracts/contracts/stream/src/test.rs`
   - Added 3 comprehensive test cases
   - Tests cover: explicit rejection, minimal valid duration, and configuration independence

## Verification Steps

1. Zero-duration rejected: ✅
2. Clear error message (InvalidDuration): ✅
3. Minimal non-zero duration allowed: ✅
4. Works with minimum duration = 0: ✅
5. Defensive validation preserved: ✅
