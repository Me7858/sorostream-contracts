# SoroStream Parameter Validation Guide

Comprehensive guide to parameter validation, constraints, and edge cases.

## Stream Creation Parameters

### amount (Deposit)

**Type:** `i128` (stroops)

**Constraints:**
- Must be > 0
- Must not cause flow_rate to be 0: `amount / duration_seconds > 0`
- Must not exceed token balance of sender
- Maximum safe value: ~10^15 stroops (prevents overflow in calculations)

**Example - Validation:**
```rust
// ✅ Valid: 0.1 USDC for 1 month
let amount = 100_000_000;  // stroops
let duration = 2_592_000;  // seconds
assert!(amount / duration > 0);  // flow_rate won't be 0

// ❌ Invalid: 1 stroop for 1 year (flow_rate = 0)
let amount = 1;
let duration = 31_536_000;
// amount / duration = 0 → ZeroFlowRate error
```

### duration_seconds

**Type:** `u64` (seconds)

**Constraints:**
- Must be ≥ `min_duration` (default: 1 second, configurable)
- Must be ≤ `max_duration` (default: 315,360,000 = ~10 years)
- Combined with amount: `amount / duration_seconds > 0`

**Typical Values:**
```
1 minute = 60
1 hour = 3,600
1 day = 86,400
1 week = 604,800
1 month (30 days) = 2,592,000
1 year = 31,536,000
10 years = 315,360,000
```

**Error Cases:**
```rust
// ❌ Duration too short
let result = create_stream(..., duration_seconds: 0, ...);
// Err: InvalidDuration

// ❌ Duration too long
let result = create_stream(..., duration_seconds: 500_000_000, ...);
// Err: DurationExceedsMax
```

### cliff_seconds

**Type:** `u64` (seconds)

**Constraints:**
- Must be ≤ `duration_seconds`
- Typically 0 (no cliff) for immediate streaming

**Behavior:**
- No tokens are claimable before `start_time + cliff_seconds`
- After cliff, all previous earned tokens become available at once
- Recommended for vesting schedules

**Examples:**
```rust
// Immediate streaming (no cliff)
cliff_seconds: 0

// 1-year cliff (3-year total)
duration_seconds: 126_144_000,  // 4 years
cliff_seconds: 31_536_000,      // 1 year

// Error: cliff > duration
cliff_seconds: 100_000_000,
duration_seconds: 50_000_000,
// Err: InvalidCliff
```

### lock_until

**Type:** `u64` (Unix timestamp)

**Constraints:**
- Can be any future timestamp
- Typically 0 (no lock)

**Behavior:**
- Recipient cannot withdraw before this timestamp
- Even if tokens are earned, withdrawal is blocked until lock expires

**Use Cases:**
```rust
// No lock period
lock_until: 0

// Lock for 1 month
lock_until: env.ledger().timestamp() + 2_592_000

// Lock for specific date
lock_until: 1735689600  // Jan 1, 2025
```

### nonce

**Type:** `u64`

**Constraints:**
- Must be unique per (sender, recipient, timestamp) combination
- Prevents duplicate stream creation with same parameters

**Safe Values:**
```rust
// Sequential per stream
let nonce = 0u64;  // First stream
let nonce = 1u64;  // Second stream

// Random
let nonce = random::<u64>();

// Timestamp-based
let nonce = env.ledger().timestamp();
```

**Error:**
```rust
// ❌ Duplicate nonce with same sender+recipient
create_stream(..., nonce: 0, ...)?;
create_stream(..., nonce: 0, ...)?;  // Err: DuplicateStream
```

### auto_renew

**Type:** `bool`

**Default:** `false`

**Behavior:**
- If `true`: stream automatically renews at end_time
- If `false`: stream completes, tokens not spent are refunded

**With renew_count:**
```rust
auto_renew: true,
renew_count: Some(3),  // Renew max 3 times, then stop

auto_renew: true,
renew_count: None,     // Renew indefinitely

auto_renew: false,
renew_count: None,     // No renewal (renew_count ignored)
```

### renew_count

**Type:** `Option<u32>`

**Constraints:**
- Only meaningful if `auto_renew = true`
- `None` = unlimited renewals
- `Some(0)` = no renewals (contradicts auto_renew=true, completion at end)
- `Some(n)` = renew maximum n times

**Error Cases:**
```rust
// ❌ renew_count with auto_renew=false (ignored but confusing)
auto_renew: false,
renew_count: Some(5),
// renew_count is silently ignored

// ✅ Correct: Limited renewals
auto_renew: true,
renew_count: Some(12),  // Renew up to 12 times
```

### holdback_amount

**Type:** `i128` (stroops)

**Constraints:**
- Must be ≥ 0
- Must be < `amount`
- If `amount - holdback_amount` rounds to zero flow_rate: error

**Behavior:**
- Held in escrow, separate from streaming amount
- Sender can `release_holdback()` to recipient or `claw_back()` to self

**Example:**
```rust
// 1M USDC with 100K held back
amount: 1_000_000_000,
holdback_amount: 100_000_000,
// Streams 900K over duration, 100K in escrow
```

### withdrawal_steps

**Type:** `Option<u32>`

**Constraints:**
- `None` (default): unlimited withdrawals
- `Some(0)`: error (InvalidDuration)
- `Some(n)`: n withdrawal boundaries

**Behavior:**
- Recipient can only withdraw at step boundaries
- Duration divided into n equal intervals

**Example:**
```rust
// Quarterly withdrawals (4 per year)
duration_seconds: 31_536_000,  // 1 year
withdrawal_steps: Some(4),     // 4 quarters
// Boundaries: 25%, 50%, 75%, 100%

// ❌ Zero steps error
withdrawal_steps: Some(0),
// Err: InvalidDuration
```

### min_withdrawal_amount

**Type:** `Option<i128>`

**Constraints:**
- `None` (default): no minimum
- `Some(n)` where n > 0

**Behavior:**
- Rejects withdrawals with claimable < n stroops
- Exception: final withdrawal (claimable >= remaining) always allowed

**Example:**
```rust
// Minimum 10K stroops per withdrawal
min_withdrawal_amount: Some(10_000_000),

// Would fail unless it's the final claim
let claimable = 5_000_000;  // < 10M
// Err: AmountBelowMinimum

// Unless claimable drains the stream
let remaining = 5_000_000;
let claimable = 5_000_000;
// claimable >= remaining → allowed (final claim)
```

### allow_recipient_termination

**Type:** `bool`

**Behavior:**
- If `true`: recipient can call `recipient_terminate()` to cancel
- If `false`: only sender can cancel

**Use Cases:**
```rust
// Employment-style (employer controls)
allow_recipient_termination: false

// Grant-style (grantee can exit)
allow_recipient_termination: true
```

### non_transferable

**Type:** `bool`

**Behavior:**
- If `true`: `transfer_recipient()` disabled; recipient locked
- If `false`: recipient can be changed

**Use Cases:**
```rust
// Personal vesting (can't transfer rights)
non_transferable: true

// Payment stream (can transfer to another address)
non_transferable: false
```

### requires_recipient_approval

**Type:** `bool`

**Behavior:**
- If `true`: stream starts in PendingApproval; recipient must approve
- If `false`: stream starts in Active immediately

**Initial State:**
```rust
requires_recipient_approval: true
// → StreamStatus::PendingApproval
// → Recipient calls approve_stream()
// → StreamStatus::Active

requires_recipient_approval: false
// → StreamStatus::Active immediately
```

### enforce_recipient_allowlist

**Type:** `bool`

**Constraints:**
- Only applies if recipient allowlist is enabled globally
- Recipient must be on allowlist if true

**Error:**
```rust
enforce_recipient_allowlist: true,
recipient: blockedAddress,
// Err: RecipientNotAllowed (if not on allowlist)
```

## Token Parameters

### token (SAC Token Address)

**Type:** `Address`

**Constraints:**
- Must be a valid Stellar Asset Contract
- Must be deployed and accessible
- Sender must have balance ≥ amount

**Validation:**
```rust
// ✅ Valid USDC mainnet
token: Address::from_string("GBUQWP3BOUZX34ULNQG23RQ6F4BFXWGDITOJLEF2D56RVZKMTAKSAGI7"),

// ❌ Invalid address format
token: Address::from_string("not_an_address"),
// Err: InvalidTokenAddress

// ❌ Token without balance
sender_balance < amount
// Err: InsufficientBalance (at transfer time)
```

## Query Parameters

### start (Pagination)

**Type:** `u32`

**Typical Values:**
```rust
start: 0      // First page
start: 100    // Skip first 100, start at 101
start: u32::MAX  // Near end of results
```

### limit (Pagination)

**Type:** `u32`

**Typical Values:**
```rust
limit: 10     // 10 results per page
limit: 100    // 100 results per page
limit: 1000   // Large batch
```

**Performance Note:**
- Higher limit = more gas cost
- Recommended: start with 50, adjust based on needs

## Advanced Parameters

### VestingCurve

**Linear (Default):**
```rust
VestingCurve::Linear
// claimable = flow_rate × elapsed_seconds
```

**TimeDecay (Front-Weighted):**
```rust
VestingCurve::TimeDecay(500)
// 500 basis points (5%) decay per 1000-second window
// Tokens heavily weighted to beginning of stream
// decay_factor range: 0-9999 bps
```

### StreamQueryFilter

**All Optional:**
```rust
StreamQueryFilter {
    status: Some(StreamStatus::Active),    // Filter by status
    asset: Some(usdc_address),             // Filter by token
    sender: Some(company_address),         // Filter by sender
    recipient: None,                       // No recipient filter
}
```

## Timestamp Constraints

### start_time (Scheduled Streams)

**Constraints:**
- Must satisfy: `now ≤ start_time ≤ now + max_future_start_offset`
- Default `max_future_start_offset`: 365 days
- Cannot be in the past

**Error:**
```rust
// ❌ Start time in the past
start_time: env.ledger().timestamp() - 1_000,
// Err: InvalidStartTime

// ❌ Start time too far in future
start_time: env.ledger().timestamp() + (400 * 24 * 3600),
// Err: StartTimeTooFar
```

## Dual Stream Parameters

### Dual Token Streams

**Constraints:**
- `token1` != `token2` (must be different)
- Both `amount1` and `amount2` > 0
- Both produce non-zero flow rates

**Error:**
```rust
// ❌ Same token twice
create_dual_stream(
    ...,
    token1: usdc,
    amount1: 1_000_000,
    token2: usdc,  // Same!
    amount2: 1_000_000,
)
// Err: DuplicateTokenInDualStream
```

## Rate Limiting

### nonce for Batch Operations

**get_nonce() for batches:**
```rust
// Get current expected nonce
let nonce = client.get_nonce(sender);

// Use for batch_create_stream
let stream_ids = client.batch_create_stream(
    ...,
    nonce,
)?;

// Next batch must use nonce + 1
```

## Fee Parameters

### Fee in Basis Points

**Type:** `u32`

**Typical Values:**
```rust
0      // 0% (no fee)
50     // 0.5%
100    // 1%
500    // 5%
1000   // 10%
10000  // 100% (all tokens to protocol)
```

**Conversion:**
```
basis_points / 10_000 = percentage
50 / 10_000 = 0.005 = 0.5%
```

## Common Pitfalls

### 1. Amount Too Small

```rust
// ❌ 1 stroop for 1 year (flow_rate rounds to 0)
create_stream(..., amount: 1, duration_seconds: 31_536_000, ...)
// Err: ZeroFlowRate

// ✅ Minimum amount = duration_seconds (if flow_rate must be 1)
amount: duration_seconds as i128
```

### 2. Contradictory Parameters

```rust
// ❌ Auto-renew but max 0 renewals
auto_renew: true,
renew_count: Some(0),
// Stream still completes, confusing logic

// ✅ Clear intent
auto_renew: false,
renew_count: None,
```

### 3. Cliff Larger Than Duration

```rust
// ❌ Invalid
duration_seconds: 1_000,
cliff_seconds: 2_000,
// Err: InvalidCliff

// ✅ Cliff ≤ duration
duration_seconds: 10_000,
cliff_seconds: 5_000,
```

### 4. Holdback Equals or Exceeds Amount

```rust
// ❌ Invalid
amount: 1_000_000,
holdback_amount: 1_000_000,  // >= amount
// Err: ZeroAmount

// ✅ Holdback < amount
amount: 1_000_000,
holdback_amount: 100_000,
```

### 5. Lock Until Before Now

```rust
// ❌ Lock in past
lock_until: env.ledger().timestamp() - 1000,
// Recipient can withdraw immediately (lock already expired)

// ✅ Lock in future
lock_until: env.ledger().timestamp() + 2_592_000,
```

## Summary Table

| Parameter | Type | Required | Range | Notes |
|-----------|------|----------|-------|-------|
| amount | i128 | Yes | > 0 | Must not cause ZeroFlowRate |
| duration_seconds | u64 | Yes | min-max | Configurable limits |
| cliff_seconds | u64 | No | 0-duration | No cliff if 0 |
| nonce | u64 | Yes | Any | Must be unique |
| auto_renew | bool | Yes | true/false | Default false |
| renew_count | Option<u32> | No | None or n | Only if auto_renew |
| lock_until | u64 | No | 0+ | 0 = no lock |
| holdback_amount | i128 | No | 0-amount | Escrow amount |
| withdrawal_steps | Option<u32> | No | None or n | n > 0 |
| min_withdrawal_amount | i128 | No | 0+ | Must be > 0 if Some |
| non_transferable | bool | No | true/false | Locks recipient |
| requires_approval | bool | No | true/false | PendingApproval if true |
| enforce_allowlist | bool | No | true/false | Checks recipient list |

