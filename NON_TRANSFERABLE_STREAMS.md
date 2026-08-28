# Non-Transferable Streams Feature

## Overview

The `non_transferable` flag allows senders to lock the recipient field of a payment stream, preventing any subsequent `transfer_recipient` calls. This is essential for regulated payment flows where the beneficiary must remain fixed and cannot be changed.

## Use Cases

1. **Regulated Payments**: Compliance-driven systems where payments must go to a specific recipient
2. **Vesting Schedules**: Employee vesting where the recipient cannot be changed to someone else
3. **Institutional Grants**: Grant disbursements where the recipient is contractually bound
4. **Controlled Distributions**: Scenarios where immutability of recipient is a requirement
5. **Custody Arrangements**: Custodial relationships requiring fixed payment targets

## Implementation

### Function Signatures

#### `create_stream()`
```rust
pub fn create_stream(
    env: Env,
    sender: Address,
    recipient: Address,
    token: Address,
    amount: i128,
    duration_seconds: u64,
    cliff_seconds: u64,
    nonce: u64,
    auto_renew: bool,
    lock_until: u64,
    allow_recipient_termination: bool,
    non_transferable: bool,  // NEW PARAMETER
) -> Result<u64, StreamError>
```

#### `create_stream_with_federation()`
```rust
fn create_stream_with_federation(
    env: Env,
    sender: Address,
    federation_name: String,
    token: Address,
    amount: i128,
    duration_seconds: u64,
    cliff_seconds: u64,
    nonce: u64,
    auto_renew: bool,
    lock_until: u64,
    allow_recipient_termination: bool,
    non_transferable: bool,  // NEW PARAMETER
) -> Result<u64, StreamError>
```

#### `batch_create_stream()`
```rust
pub fn batch_create_stream(
    env: Env,
    sender: Address,
    recipients: Vec<Address>,
    amounts: Vec<i128>,
    tokens: Vec<Address>,
    duration_seconds: u64,
    auto_renew: bool,
    lock_untils: Vec<u64>,
    nonce: u64,
    non_transferable: bool,  // NEW PARAMETER
) -> Result<Vec<u64>, StreamError>
```

### Behavior

#### Stream Creation
- When `non_transferable = true`, the stream is created with the recipient field locked
- The parameter is stored in the `Stream` struct and persists for the stream's lifetime
- Applies uniformly to all streams in batch operations for efficiency

#### Transfer Prevention
- When `transfer_recipient()` is called on a non-transferable stream, it returns `StreamError::StreamNonTransferable`
- The check occurs early in the function, before any state mutation or token transfer
- Both the current recipient and any authorized caller attempting transfer will receive the error

#### Events
- Stream creation events (`stream_created`) include the `non_transferable` flag
- This allows off-chain indexers to identify non-transferable streams
- Improves discoverability and compliance auditing

### Error Handling

The `StreamError::StreamNonTransferable` error (value 59) is returned when:
1. `transfer_recipient()` is called on a stream with `non_transferable = true`
2. No other conditions prevent the transfer attempt
3. The error is the first check after authorization verification

## Usage Examples

### Creating a Non-Transferable Stream
```
create_stream(
    env,
    sender,
    recipient,
    token,
    1_000_000_000,  // amount
    31_536_000,     // 1 year duration
    0,              // no cliff
    nonce,
    false,          // auto_renew
    0,              // no lock
    false,          // allow_recipient_termination
    true            // NON_TRANSFERABLE - recipient is locked
)
```

### Attempting to Transfer (Will Fail)
```
transfer_recipient(
    env,
    stream_id,
    current_recipient,
    new_recipient
)
// Returns: Err(StreamError::StreamNonTransferable)
```

### Creating a Transferable Stream (Default Behavior)
```
create_stream(
    env,
    sender,
    recipient,
    token,
    1_000_000_000,  // amount
    31_536_000,     // 1 year duration
    0,              // no cliff
    nonce,
    false,          // auto_renew
    0,              // no lock
    false,          // allow_recipient_termination
    false           // TRANSFERABLE - default behavior
)
// Recipient CAN be changed later via transfer_recipient()
```

### Batch Create with Non-Transferable Flag
```
batch_create_stream(
    env,
    sender,
    vec![recipient1, recipient2, recipient3],
    vec![amount1, amount2, amount3],
    vec![token1, token2, token3],
    duration_seconds,
    auto_renew,
    vec![lock1, lock2, lock3],
    nonce,
    true            // All streams in batch are non-transferable
)
```

## Design Decisions

1. **Boolean Flag**: Simple yes/no semantics for immutability
2. **Sender-Controlled**: Only the sender decides if a stream is transferable at creation time
3. **Immutable Once Set**: Cannot be changed after stream creation
4. **Batch Uniformity**: All streams in a batch operation share the same `non_transferable` setting for simplicity
5. **Early Validation**: Check occurs first in `transfer_recipient()` before any side effects
6. **Clear Error**: Specific error variant for compliance and debugging

## Compatibility

- **Backward Compatible**: Default value is `false` (transferable), preserving existing behavior
- **Non-Breaking**: Existing code continues to work; callers must opt-in to immutability
- **Event Structure**: `stream_created` event already includes the flag, no breaking changes

## Security Considerations

1. **Immutability**: Once set to `true`, cannot be changed for the stream's lifetime
2. **Authorization**: `transfer_recipient()` still requires current recipient authentication
3. **Bypass Prevention**: No admin or emergency override can change a non-transferable stream's recipient
4. **Audit Trail**: Events document which streams have immutable recipients

## Interface Changes

### Updated Trait Methods
- `SoroStreamInterface::create_stream()` - Added `non_transferable: bool` parameter
- `SoroStreamInterface::create_stream_with_federation()` - Added `non_transferable: bool` parameter
- `SoroStreamInterface::batch_create_stream()` - Added `non_transferable: bool` parameter

### No Changes Required To
- `transfer_recipient()` - Logic already in place, just documented
- `withdraw()` - No impact on withdrawal behavior
- `cancel_stream()` - No impact on cancellation
- Other stream operations

## Testing Recommendations

1. Create transferable and non-transferable streams and verify flag storage
2. Attempt transfer on non-transferable stream (should fail)
3. Attempt transfer on transferable stream (should succeed)
4. Batch create with non_transferable=true and verify all streams are locked
5. Verify events correctly reflect the flag value
6. Test with various recipients and tokens
7. Ensure error code matches StreamNonTransferable (59)
8. Verify non-transferable streams can still be withdrawn from and cancelled

## Migration Path

For existing systems:
1. New streams can opt-in to non-transferable by passing `true`
2. Existing streams remain transferable (default `false`)
3. No contract redeployment needed
4. Gradual adoption as use cases require immutability
