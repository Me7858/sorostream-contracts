# Auto-Renewal with Renew Count Limit Implementation

## Overview

This implementation adds an optional `renew_count` limit parameter to the `createStream` function, allowing senders to control how many times a stream can automatically renew after reaching its end_time.

## Changes Made

### 1. Stream Type Definition (types.rs)

Added two new fields to the `Stream` struct:

```rust
/// Optional limit on the number of auto-renewals. When set, the stream will automatically
/// renew up to this many times. Once reached, the stream will complete permanently and not renew.
/// `None` means unlimited auto-renewals (default behaviour when auto_renew is true).
pub renew_count: Option<u32>,

/// Number of times this stream has been renewed so far. Starts at 0 and increments each time
/// the stream auto-renews. Only meaningful when `auto_renew` is true.
pub renewals_used: u32,
```

### 2. Contract Interface (interface.rs)

Updated all stream creation methods to accept the `renew_count` parameter:

- `create_stream()` - Main stream creation with all parameters
- `create_stream_with_federation()` - Stream creation with federation name
- `create_stream_with_curve()` - Stream creation with vesting curve
- `batch_create_stream()` - Batch stream creation
- `create_stream_with_milestones()` - (Not modified, doesn't use auto_renew)
- `create_stream_with_schedule()` - (Not modified, doesn't use auto_renew)

### 3. Implementation Logic (lib.rs)

#### 3.1 Stream Creation

Updated all stream creation paths to:
- Accept `renew_count: Option<u32>` parameter
- Initialize new Stream instances with `renew_count` and `renewals_used: 0`

Modified functions:
- `create_stream()` - Added renew_count parameter and initialization
- `create_stream_with_federation()` - Added renew_count parameter forwarding
- `create_stream_with_curve()` - Added renew_count parameter and initialization
- `batch_create_stream()` - Added renew_count parameter and initialization

#### 3.2 Auto-Renewal Logic

Enhanced the auto-renewal check in both `withdraw()` and `batch_withdraw()` functions to:

1. **Check renewal count limit** - Before attempting renewal, check if `renew_count` limit has been reached:
   ```rust
   let can_renew = if let Some(max_renewals) = stream.renew_count {
       stream.renewals_used < max_renewals
   } else {
       true  // No limit set, can always renew
   };
   ```

2. **Handle limit reached** - If limit reached:
   - Mark stream as `Completed`
   - Decrement active and token stream counts
   - Transfer funds to recipient and sender
   - Emit `RenewalLimitReached` event

3. **Proceed with renewal** - If limit not reached:
   - Check sender balance (existing logic)
   - If insufficient balance, complete stream (existing logic)
   - If sufficient balance, proceed with renewal and **increment `renewals_used`**:
     ```rust
     stream.renewals_used = stream.renewals_used.saturating_add(1);
     ```

### 4. Events (events.rs)

Added new event function:

```rust
/// Emitted when a stream's renewal count limit is reached and the stream can no longer auto-renew.
pub fn renewal_limit_reached(env: &Env, stream_id: u64, sender: &Address, renewals_used: u32) {
    env.events().publish(
        (Symbol::new(env, "RenewalLimitReached"), stream_id),
        (sender.clone(), renewals_used),
    );
}
```

This event is emitted when:
- A stream has `auto_renew = true`
- The stream reaches its end_time
- `renewals_used >= renew_count` (limit reached)

### 5. Tests (test.rs)

Added comprehensive test cases:

1. **`test_auto_renew_respects_renew_count_limit()`** - Verifies:
   - Stream renews up to the limit
   - `renewals_used` increments correctly on each renewal
   - Stream completes when limit is reached

2. **`test_auto_renew_without_renew_count_unlimited()`** - Verifies:
   - Stream with `renew_count = None` can renew indefinitely
   - Multiple renewals succeed without hitting a limit

3. **`test_renew_count_with_zero_limit()`** - Verifies:
   - Stream with `renew_count = Some(0)` cannot renew at all
   - Stream completes immediately at end_time

4. **`test_cancel_auto_renew_before_expiry()`** - Verifies:
   - Canceling auto-renewal before expiry works correctly

## Behavior Specification

### Renewal Count Semantics

| renew_count | Behavior |
|---|---|
| `None` | Unlimited auto-renewals (default when `auto_renew = true`) |
| `Some(0)` | No renewals allowed; stream completes at end_time |
| `Some(n)` where n > 0 | Stream can renew up to n times, then completes permanently |

### State Transitions

```
Create Stream (renew_count=Some(n), renewals_used=0)
    ↓
[Active - Streaming]
    ↓ [end_time reached, recipient withdraws]
[Check: renewals_used < renew_count?]
    ├─ YES → Renewal possible
    │   ├─ [Check: sender balance sufficient?]
    │   │   ├─ YES → Renew stream (renew_count stays same, renewals_used++)
    │   │   └─ NO → Complete stream (RenewalLimitReached? No, AutoRenewFailed)
    │   └─ Goto [Active - Streaming]
    └─ NO → Complete stream (RenewalLimitReached event)
```

## API Usage Examples

### Create Stream with Limited Renewals

```rust
// Stream that can renew at most 5 times
let stream_id = client.create_stream(
    &sender,
    &recipient,
    &token,
    &1_000_000,    // amount in stroops
    &86400,        // duration in seconds (1 day)
    &0,            // cliff in seconds
    &nonce,
    &true,         // auto_renew enabled
    &Some(5u32),   // can renew 5 times max
    &0,            // lock_until
    &false,        // allow_recipient_termination
    &0,            // holdback_amount
    &None,         // withdrawal_steps
    &None,         // min_withdrawal_amount
    &false,        // non_transferable
    &false,        // requires_recipient_approval
    &false,        // enforce_recipient_allowlist
)?;
```

### Create Stream with Unlimited Renewals

```rust
// Stream that can renew indefinitely
let stream_id = client.create_stream(
    &sender,
    &recipient,
    &token,
    &1_000_000,
    &86400,
    &0,
    &nonce,
    &true,         // auto_renew enabled
    &None,         // unlimited renewals
    &0,
    &false,
    &0,
    &None,
    &None,
    &false,
    &false,
    &false,
)?;
```

## Technical Details

### Overflow Protection

The `renewals_used` counter uses `saturating_add(1)` to prevent overflow:
```rust
stream.renewals_used = stream.renewals_used.saturating_add(1);
```

At u32::MAX renewals, the counter stops incrementing, which is acceptable as it's an extremely large number and the stream would complete naturally before reaching this limit.

### Storage Considerations

- Two new fields added to Stream struct (one Option, one u32)
- These are stored on-ledger for every stream
- Minimal impact: ~4-8 bytes per stream in persistent storage
- `renew_count` is set at stream creation and never changes (immutable)
- `renewals_used` is updated on each auto-renewal

### Event Publishing

When renewal limit is reached, both events are emitted:
1. `RenewalLimitReached` - Specific to the limit being hit
2. `StreamCompleted` - Standard completion event

This allows indexers to distinguish between:
- Streams that completed naturally (no auto_renew)
- Streams that completed due to insufficient balance (auto_renew_failed)
- Streams that completed due to renewal limit (renewal_limit_reached)

## Compatibility Notes

- The `renew_count` parameter is optional in the function signature
- Existing code that doesn't use the feature can pass `None` for unlimited renewals
- The field is part of the Stream struct serialization, affecting on-ledger storage

## Future Enhancements

Potential improvements for future versions:

1. **Dynamic Renewal Management** - Allow sender to modify `renew_count` after stream creation
2. **Renewal Notifications** - Emit events when approaching renewal limit (e.g., when renewals_used == renew_count - 1)
3. **Renewal Costs** - Implement optional per-renewal fees to discourage excessive renewals
4. **Renewal Cooldowns** - Add minimum time between renewals to prevent rapid cycles
