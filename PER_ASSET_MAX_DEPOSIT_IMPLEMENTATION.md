# Per-Asset Maximum Deposit Limit Implementation

## Overview
This document describes the implementation of a per-asset maximum deposit limit admin control for the SoroStream contract. This feature prevents users from locking an unbounded amount of a particular asset in a single stream, providing risk management control for administrators.

## Implementation Details

### 1. Storage Layer (`contracts/stream/src/storage.rs`)

Added Feature (j): Per-asset maximum deposit limit

**Storage Functions:**
- `max_deposit_per_token_key()` - Returns the storage key for a token's max deposit limit
- `get_max_deposit_per_token(env: &Env, token: &Address) -> i128` - Retrieves the max deposit limit for a token (0 = unlimited)
- `set_max_deposit_per_token(env: &Env, token: &Address, max_deposit: i128)` - Sets the max deposit limit for a token

**Storage Pattern:**
- Uses persistent storage with key format: `(Symbol::new("max_dep"), token_address)`
- Returns 0 when no limit is configured, indicating "unlimited"

### 2. Error Handling (`contracts/stream/src/errors.rs`)

Added new error variant:
```rust
MaxDepositExceeded = 64,
```

This error is returned when a stream creation attempt exceeds the configured per-token deposit limit.

### 3. Admin Interface (`contracts/stream/src/lib.rs`)

Added two admin-only functions:

#### `set_max_deposit_per_token()`
```rust
pub fn set_max_deposit_per_token(
    env: Env, 
    token: Address, 
    max_deposit: i128
) -> Result<(), StreamError>
```

**Features:**
- Admin-only (enforced via `check_admin()`)
- Validates that `max_deposit >= 0` (rejects negative values with `ZeroAmount` error)
- Setting to 0 disables the limit for that token
- Provides risk management control

#### `get_max_deposit_per_token()`
```rust
pub fn get_max_deposit_per_token(env: Env, token: Address) -> i128
```

**Features:**
- Public getter (no admin requirement)
- Returns 0 if no limit is configured (unlimited)
- Allows off-chain systems to query current limits

### 4. Validation Integration

The per-asset maximum deposit check is enforced in all stream creation methods:

#### `create_stream()`
- Location: After per-token stream cap check, before nonce marking
- Validation: `if max_deposit > 0 && amount > max_deposit { return MaxDepositExceeded }`

#### `create_stream_with_schedule()` (Step-vesting)
- Location: After sender limit check, before nonce marking
- Validates against `deposit` amount
- Ensures tranche-based streams also respect the limit

#### `create_stream_with_curve()` (Time-decay vesting)
- Location: After sender limit check, before nonce marking
- Validates against `amount` parameter

#### `batch_create_stream()`
- Location: Phase 1 validation loop (before any state mutation)
- Validates each stream in the batch against per-token limit
- Entire batch is rejected if any stream exceeds limit

### 5. Public Interface (`contracts/stream/src/interface.rs`)

Added trait methods for the `SoroStreamInterface`:

```rust
/// Sets the maximum deposit amount for a single stream using a specific token.
/// Setting to 0 disables the limit for that token. Admin only.
fn set_max_deposit_per_token(env: Env, token: Address, max_deposit: i128) -> Result<(), StreamError>;

/// Returns the maximum deposit amount for a single stream using the given token.
/// Returns 0 if no limit is set (unlimited).
fn get_max_deposit_per_token(env: Env, token: Address) -> i128;
```

## Usage Example

### Setting a maximum deposit limit (Admin action)
```
set_max_deposit_per_token(env, USDC_token_address, 1_000_000_000_000) // 1 million USDC stroops
```

### Getting the current limit
```
limit = get_max_deposit_per_token(env, USDC_token_address)
// Returns: 1_000_000_000_000, or 0 if unlimited
```

### Attempting to create a stream exceeding the limit
```
create_stream(env, sender, recipient, USDC_token, 2_000_000_000_000, ...)
// Returns: Err(StreamError::MaxDepositExceeded)
```

## Design Decisions

1. **Per-Token Configuration**: Limits are configured per token, allowing different risk profiles for different assets
2. **Zero = Unlimited**: Using 0 as the "no limit" sentinel value is consistent with other per-token controls in the contract (e.g., `set_max_streams_per_token`)
3. **Admin-Only**: Only administrators can set limits, ensuring centralized risk management
4. **Early Validation**: Checks occur early in stream creation, before any state mutation or token transfer
5. **Atomic Batch Validation**: In batch operations, entire batch is validated before any changes, preventing partial failures
6. **Consistent Error**: Uses new `MaxDepositExceeded` error for clear, specific rejection reasons

## Backward Compatibility

- Default limit is 0 (unlimited) for all tokens
- Existing streams are not affected
- New streams can only be created if within limit
- No breaking changes to existing API

## Testing Considerations

Key test scenarios:
1. Setting and retrieving limits for different tokens
2. Creating streams within limit (should succeed)
3. Creating streams exceeding limit (should fail with `MaxDepositExceeded`)
4. Batch creation where some streams exceed limits (entire batch rejected)
5. Step-vesting and time-decay streams also respecting limits
6. Disabling limits by setting to 0
7. Updating limits for existing tokens
8. Negative limit values rejected with proper error

## Risk Management Benefits

1. **Asset-Specific Control**: Different tokens can have different risk profiles
2. **Single-Stream Limits**: Prevents concentration of risk in individual streams
3. **Granular Control**: Admins can adjust limits without redeploying contract
4. **Clear Failure**: Rejected streams fail immediately with specific error
5. **Audit Trail**: Deployment logs/events show when limits are set/modified
