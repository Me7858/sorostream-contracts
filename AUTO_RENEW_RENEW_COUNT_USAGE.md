# Auto-Renewal with Renew Count - Quick Reference

## Summary

When creating a stream with `auto_renew = true`, you can now optionally specify a `renew_count` to limit how many times the stream will automatically renew:

- **`renew_count = None`** → Unlimited renewals (existing behavior)
- **`renew_count = Some(0)`** → No renewals (stream completes after duration)
- **`renew_count = Some(n)`** → Renew up to n times, then complete permanently

## Changes to createStream Function

### New Parameter: `renew_count`

The `renew_count` parameter is inserted after `auto_renew` in the function signature:

```rust
fn create_stream(
    env: Env,
    sender: Address,
    recipient: Address,
    token: Address,
    amount: i128,
    duration_seconds: u64,
    cliff_seconds: u64,
    nonce: u64,
    auto_renew: bool,              // <- existing
    renew_count: Option<u32>,      // <- NEW
    lock_until: u64,
    allow_recipient_termination: bool,
    holdback_amount: i128,
    withdrawal_steps: Option<u32>,
    min_withdrawal_amount: Option<i128>,
    non_transferable: bool,
    requires_recipient_approval: bool,
    enforce_recipient_allowlist: bool,
) -> Result<u64, StreamError>
```

## Examples

### Example 1: Limited Renewals (Salary paid for 12 months with quarterly renewals)

```rust
// Create a stream that will automatically renew 3 times (quarterly)
// Total duration: 4 quarters (12 months)
let stream_id = client.create_stream(
    &employee,           // sender (employer)
    &worker,             // recipient
    &usdc_token,
    &120_000 * 10_000_000,  // 120,000 stroops per quarter
    &7_776_000,          // 90 days in seconds (1 quarter)
    &0,                  // no cliff
    &nonce,
    &true,               // auto_renew enabled
    &Some(3u32),         // can renew 3 times (4 total periods)
    &0,
    &false,
    &0,
    &None,
    &None,
    &false,
    &false,
    &false,
)?;
// After 1st renewal: Q2
// After 2nd renewal: Q3
// After 3rd renewal: Q4
// After 4th end_time: COMPLETED (no more renewals)
```

### Example 2: Unlimited Renewals (Monthly subscription)

```rust
// Create a stream that will renew indefinitely every month
let stream_id = client.create_stream(
    &subscription_service,
    &customer,
    &usdc_token,
    &30 * 10_000_000,    // 30 stroops per month
    &2_592_000,          // 30 days in seconds
    &0,
    &nonce,
    &true,               // auto_renew enabled
    &None,               // unlimited renewals
    &0,
    &false,
    &0,
    &None,
    &None,
    &false,
    &false,
    &false,
)?;
// Renews automatically every month indefinitely
```

### Example 3: No Renewal (One-time stream)

```rust
// Create a stream that will NOT renew
let stream_id = client.create_stream(
    &granter,
    &grantee,
    &usdc_token,
    &1_000 * 10_000_000,
    &31_536_000,         // 1 year in seconds
    &0,
    &nonce,
    &true,               // auto_renew is true, but...
    &Some(0u32),         // ...with 0 renewals allowed
    &0,
    &false,
    &0,
    &None,
    &None,
    &false,
    &false,
    &false,
)?;
// Streams for 1 year, then completes permanently
```

## Stream State During Lifecycle

### With `renew_count = Some(2)`

```
Time: T=0
└─ Stream created with renew_count=Some(2), renewals_used=0

Time: T=duration
└─ Stream reaches end_time
   ├─ Recipient calls withdraw()
   └─ Check: renewals_used (0) < renew_count (2)? YES
      └─ Renewal happens → renewals_used becomes 1

Time: T=2*duration  
└─ Stream reaches end_time again
   ├─ Recipient calls withdraw()
   └─ Check: renewals_used (1) < renew_count (2)? YES
      └─ Renewal happens → renewals_used becomes 2

Time: T=3*duration
└─ Stream reaches end_time again
   ├─ Recipient calls withdraw()
   └─ Check: renewals_used (2) < renew_count (2)? NO
      └─ Event: RenewalLimitReached emitted
      └─ Stream marked as Completed
```

## Events

### RenewalLimitReached Event

Emitted when:
- Stream has `auto_renew = true`
- Stream reaches its `end_time`
- `renewals_used >= renew_count` (the limit has been reached)

Event structure:
```rust
(
    Symbol: "RenewalLimitReached",
    stream_id: u64,
    sender: Address,
    renewals_used: u32
)
```

Indexers can use this to:
- Alert senders when renewal limits are reached
- Distinguish between different stream completion reasons
- Track renewal patterns

## Migration Guide for Existing Code

### Old Code (without renew_count)
```rust
c.create_stream(
    &sender, 
    &recipient, 
    &token, 
    &amount, 
    &duration, 
    &cliff, 
    &nonce, 
    &true,      // auto_renew
    &lock_until,
    &false,     // allow_recipient_termination
    // ... other params
)
```

### New Code (with renew_count)
```rust
c.create_stream(
    &sender, 
    &recipient, 
    &token, 
    &amount, 
    &duration, 
    &cliff, 
    &nonce, 
    &true,           // auto_renew
    &None,           // renew_count - INSERT HERE (None = unlimited, like before)
    &lock_until,
    &false,          // allow_recipient_termination
    // ... other params
)
```

## Implementation Details

### Storage Impact
- **renew_count**: 4-5 bytes (Option + u32)
- **renewals_used**: 4 bytes (u32)
- Total: ~9 bytes per stream added to on-ledger storage

### Overflow Protection
- `renewals_used` uses `saturating_add(1)` on renewal
- Maximum value: u32::MAX (4,294,967,295)
- Practical limit: far exceeds any real-world usage

### Sender Balance Check
- Still applies after renewal limit check
- If sender lacks balance at renewal time, stream completes with `AutoRenewFailed` event
- Renewal count limit is checked first, so if limit reached, `RenewalLimitReached` takes precedence

## Common Use Cases

| Use Case | renew_count | auto_renew | Duration |
|---|---|---|---|
| Monthly subscription | None | true | 30 days |
| Quarterly payroll | Some(3) | true | 90 days |
| One-time grant (auto-stream) | Some(0) | true | any |
| Vesting schedule | Some(n) | true | period |
| Employee salary (12 months) | Some(11) | true | 1 month |
| Fixed-term lease payment | Some(n) | true | period |
| Subscription trial | Some(0) | true | trial_period |

## Troubleshooting

### Stream completes unexpectedly early
- Check: Has `renewals_used == renew_count`?
- Check: Is sender balance insufficient for renewal?
- Solution: Increase `renew_count` or ensure sender has balance

### Stream doesn't renew as expected
- Check: Is `auto_renew = true`?
- Check: Is `renew_count = Some(0)`? (prevents all renewals)
- Check: Have we already used all allowed renewals?

### How to monitor renewals
- Watch for `RenewalLimitReached` event
- Track `renewals_used` value from stream query
- Compare `renewals_used < renew_count` to predict future completions

