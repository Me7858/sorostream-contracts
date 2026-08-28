# partialCancelStream Entry Point Specification

## Overview

`partialCancelStream` provides a clean, semantic operation to immediately stop an active stream at the current ledger and split the balance:
- **Recipient** receives earned tokens accrued up to now
- **Sender** receives remaining unstreamed tokens
- **Stream** is terminated (not continued)

## Differences from Similar Operations

| Operation | Creates New Stream? | Removes Original? | Use Case |
|-----------|-------------------|-------------------|----------|
| `cancel_stream()` | No | Yes | Cancel with full semantics, handle special cases |
| `partial_cancel_stream(amount)` | **Yes** | Changes to Cancelled | Reduce amount but keep streaming |
| `partialCancelStream()` | No | Yes | Simple immediate stop with balance split |

## Function Signature

```rust
pub fn partial_cancel_stream(
    env: Env,
    stream_id: u64,
    sender: Address,  // Must be stream sender or delegate
) -> Result<(), StreamError>
```

Note: This is different from the existing `partial_cancel_stream` which takes `cancel_amount` and returns `u64` (new stream ID).

## Semantics

### Preconditions
- Stream exists
- Caller is sender, recipient, or sender's delegate
- Stream status is Active or Paused
- Stream is not sender-locked (unless being called by recipient)

### Execution
1. **Calculate balances at current ledger time:**
   - Earned amount for recipient (based on flow rate and elapsed time)
   - Remaining unstreamed amount for sender

2. **Transfer tokens:**
   - To recipient: earned amount
   - To sender: remaining unstreamed amount

3. **Remove stream:**
   - Delete stream record
   - Remove from sender/recipient indexes
   - Decrement active stream count
   - Release holdback if applicable

4. **Emit event:**
   - `StreamPartialCancelled` with stream_id, earned, and remainder

### Edge Cases

#### PendingApproval Streams
- Can be cancelled with zero earned
- Recipient gets nothing
- Sender gets full refund

#### Step-Vesting (Tranches)
- Recipient gets all tranches that have unlocked (unlock_time <= now)
- Sender gets all future (unlocked) tranches

#### Linear Vesting with Cliff
- If now < cliff_time: recipient gets 0
- Otherwise: recipient gets earned amount

#### Paused Streams
- Use last_pause_time instead of current timestamp
- Otherwise same logic

#### Locked Streams (sender_locked = true)
- Sender cannot call partialCancelStream
- Only recipient can call (to claim earnings)
- Or admin could unlock first

#### Holdback Amount
- If not yet claimed, include in sender refund
- Remove holdback record

#### Dual-Stream
- Handle cleanup of both components

## Authorization

Callable by:
- **Sender** (or delegate): To stop stream and recover unstreamed portion
- **Recipient**: To claim earned portion and stop stream (only if not locked)
- **Admin**: Could enforce stop (future enhancement)

## Returns

`Result<(), StreamError>`

### Errors
- `StreamNotFound`: Stream doesn't exist
- `NotAuthorized`: Caller is not sender/recipient/delegate
- `StreamNotActive`: Stream is not Active or Paused (already completed/cancelled/expired)
- `StreamLocked`: Sender-locked and caller is sender
- `Overflow`: Arithmetic overflow calculating amounts

## Events Emitted

```rust
events::stream_partial_cancelled(
    &env,
    stream_id,      // Original stream ID being stopped
    &stream.sender,
    earned_amount,  // To recipient
    unstreamed,     // To sender (remainder)
)
```

## State Changes

| State | Before | After |
|-------|--------|-------|
| Stream exists | Yes | Removed |
| Active count | 1 (if Active) | 0 |
| Recipient balance | Balance + 0 | Balance + earned |
| Sender balance | Balance + 0 | Balance + unstreamed |
| Holdback escrow | Held (if applicable) | Released to sender |

## Gas Cost

- O(1) for linear streams
- O(T) for step-vesting streams where T = number of tranches

## Use Cases

1. **Early Termination Agreement**: Parties agree to end stream early
2. **Budget Management**: Sender stops stream when budget is depleted
3. **Emergency Stop**: If stream parameters become invalid
4. **Recipient Claim**: Recipient pulls earnings and stops stream

## Comparison with cancel_stream

### Similarities
- Same balance split logic
- Same token transfers
- Same stream removal
- Same event semantics

### Differences
- **Simpler naming**: "partial" suggests immediate stop, not stream reduction
- **Clearer semantics**: No "cancel_amount" parameter to confuse with reduction
- **Recipient callable**: Allows recipient to claim and stop without sender involvement
- **Future-proofed**: Cleaner API for dashboard/UI integration

## Implementation Strategy

Reuse existing `cancel_stream` logic for:
- Cliff enforcement
- Earned calculation
- Available balance computation
- Holdback handling
- Tranche processing
- Dual-stream cleanup

New or modified:
- Authorization: Allow recipient to call
- Remove "full cancellation" special handling that might not apply
- Clear error messages for "stream stopped" vs "stream cancelled"

