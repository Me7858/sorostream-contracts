# Clone Stream Implementation Guide

## Overview

The `cloneStream` entry point enables callers to quickly create new streams by copying an existing stream's configuration and optionally overriding specific fields.

## Use Cases

### 1. Recurring Streams
```
Alice creates monthly salary stream for Bob
├─ Stream A: 1000 USDC, Jan 1 → Jan 31
└─ Alice clones Stream A → Stream B: 1000 USDC, Feb 1 → Feb 28
   └─ Only changes recipient_override = None, start_time = Feb 1
```

### 2. Multi-Recipient Campaigns
```
Company creates stream template
├─ Template: 100 USDC/month to Employee #1
└─ Clone 99 times with recipient_override = [Employee #2 through #100]
```

### 3. Testing/Staging
```
Create production stream
└─ Clone for testing with override for testnet recipient
   └─ Same parameters, different recipient
```

### 4. Cross-Token Equivalents
```
Stream with USDC
└─ Clone with token_override = EURC
   └─ Same rate, different token
```

## Function Signature

```rust
pub fn clone_stream(
    env: Env,
    source_stream_id: u64,
    caller: Address,
    recipient_override: Option<Address>,
    token_override: Option<Address>,
    rate_override: Option<i128>,
    duration_override: Option<u64>,
) -> Result<u64, StreamError>
```

## Parameters

| Parameter | Type | Purpose |
|-----------|------|---------|
| `source_stream_id` | u64 | ID of stream to clone |
| `caller` | Address | User initiating clone (must auth) |
| `recipient_override` | Option<Address> | New recipient (None = use source) |
| `token_override` | Option<Address> | New token (None = use source) |
| `rate_override` | Option<i128> | New flow rate (None = use source) |
| `duration_override` | Option<u64> | New duration (None = use source) |

## Return Value

- **Success**: New stream ID
- **Error**: StreamError variant (see below)

## Error Handling

| Error | Trigger |
|-------|---------|
| `StreamNotFound` | Source stream doesn't exist |
| `ContractPaused` | Contract in emergency pause |
| `NotAuthorized` | Caller can't clone this stream |
| `SenderStreamLimitExceeded` | Would exceed sender's stream cap |
| `ZeroAmount` | Overrides result in invalid amount |
| `ZeroFlowRate` | Overrides result in zero flow rate |
| `InvalidDuration` | Duration out of valid range |
| `RecipientNotWhitelisted` | New recipient not whitelisted |
| `TokenNotWhitelisted` | New token not whitelisted |
| `AddressBlocked` | Caller or recipient blocked |

## Implementation Strategy

### Step 1: Authorization

```rust
caller.require_auth();

// Read source stream
let source = load_stream(&env, source_stream_id)
    .ok_or(StreamError::StreamNotFound)?;

// Only sender (or delegate) can clone
let is_sender = source.sender == caller;
let is_delegate = get_delegate(&env, source_stream_id)
    .map_or(false, |d| d == caller);

if !is_sender && !is_delegate {
    return Err(StreamError::NotAuthorized);
}
```

### Step 2: Build Configuration

```rust
// Apply overrides, fallback to source values
let new_recipient = recipient_override.unwrap_or(source.recipient.clone());
let new_token = token_override.unwrap_or(source.token.clone());
let new_flow_rate = rate_override.unwrap_or(source.flow_rate);
let new_duration = duration_override.unwrap_or(
    source.end_time - source.start_time
);
```

### Step 3: Validate New Configuration

```rust
// Check whitelists, blocklists, caps, etc
if is_whitelist_enabled(&env) && !is_whitelisted(&env, &new_recipient) {
    return Err(StreamError::RecipientNotWhitelisted);
}

if is_blocked(&env, &caller) || is_blocked(&env, &new_recipient) {
    return Err(StreamError::AddressBlocked);
}

// Validate amount and rate
let new_amount = new_flow_rate * new_duration as i128;
if new_amount <= 0 {
    return Err(StreamError::ZeroAmount);
}
```

### Step 4: Create New Stream

```rust
// Generate new stream ID
let now = env.ledger().timestamp();
let new_stream_id = derive_stream_id(
    &env,
    &source.sender,
    &new_recipient,
    now,
    get_batch_nonce(&env, &source.sender)
);

// Build new stream struct
let mut new_stream = source.clone();
new_stream.id = new_stream_id;
new_stream.recipient = new_recipient.clone();
new_stream.token = new_token.clone();
new_stream.flow_rate = new_flow_rate;
new_stream.deposit = new_amount;
new_stream.start_time = now;
new_stream.end_time = now + new_duration;
new_stream.cliff_time = now + (source.cliff_time - source.start_time);
new_stream.last_withdraw_time = now;
new_stream.total_withdrawn = 0;
new_stream.status = StreamStatus::Active;
// Preserve: auto_renew, allow_recipient_termination, etc.
```

### Step 5: Execute and Index

```rust
// Transfer tokens
token::Client::new(&env, &new_token).transfer(
    &source.sender,
    &env.current_contract_address(),
    &new_amount
);

// Persist
save_stream(&env, &new_stream);
index_by_sender(&env, &source.sender, new_stream_id);
index_by_recipient(&env, &new_recipient, new_stream_id);
index_global_stream(&env, new_stream_id);

// Update counters
increment_active_stream_count(&env);
increment_token_stream_count(&env, &new_token);

// Emit event
events::stream_cloned(
    &env,
    source_stream_id,
    new_stream_id,
    &source.sender,
    &new_recipient,
    new_flow_rate,
    new_duration
);

Ok(new_stream_id)
```

## What Gets Cloned

### Preserved from Source
✅ Auto-renew setting
✅ Allow recipient termination
✅ Lock until timestamp
✅ Holdback configuration
✅ Step-vesting settings (if any)
✅ Oracle settings (if any)
✅ Withdrawal steps (if any)
✅ Non-transferable flag
✅ Vesting curve
✅ Minimum withdrawal amount

### Override Options
🔄 Recipient (new person receiving)
🔄 Token (different token)
🔄 Flow rate (different speed)
🔄 Duration (different length)

### Reset/Generated
🆕 Stream ID (new unique ID)
🆕 Start time (now)
🆕 End time (now + duration)
🆕 Status (Active)
🆕 Total withdrawn (0)
🆕 Last withdraw time (now)

## Authorization Rules

| Caller | Can Clone? | Notes |
|--------|-----------|-------|
| Stream sender | ✅ Yes | Original creator |
| Delegate | ✅ Yes | If set via `set_delegate()` |
| Other user | ❌ No | Would create unauthorized stream |

## Example Usage

### Clone with Different Recipient

```typescript
const newStreamId = await client.clone_stream(
  env,
  123,  // source_stream_id
  caller,
  {
    recipient_override: new_recipient_address,
    token_override: null,
    rate_override: null,
    duration_override: null,
  }
);
```

### Clone with Different Rate

```typescript
const newStreamId = await client.clone_stream(
  env,
  123,
  caller,
  {
    recipient_override: null,
    token_override: null,
    rate_override: BigInt(2000),  // 2x rate
    duration_override: null,
  }
);
```

### Clone with Multiple Overrides

```typescript
const newStreamId = await client.clone_stream(
  env,
  123,
  caller,
  {
    recipient_override: new_recipient,
    token_override: different_token,
    rate_override: BigInt(5000),
    duration_override: 2592000n,  // 30 days
  }
);
```

## Event Emission

```rust
pub fn stream_cloned(
    env: &Env,
    source_stream_id: u64,
    new_stream_id: u64,
    sender: &Address,
    new_recipient: &Address,
    new_flow_rate: i128,
    new_duration: u64,
) {
    env.events().publish(
        (Symbol::new(env, "StreamCloned"), source_stream_id),
        (new_stream_id, sender.clone(), new_recipient.clone(), new_flow_rate, new_duration),
    );
}
```

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_clone_stream_basic() {
    let env = Env::default();
    let sender = Address::random(&env);
    let recipient1 = Address::random(&env);
    let recipient2 = Address::random(&env);
    
    // Create source stream
    let stream1 = create_stream(...);
    
    // Clone with different recipient
    let stream2 = clone_stream(
        stream1,
        sender,
        Some(recipient2),  // override
        None, None, None
    ).unwrap();
    
    // Verify stream2 has recipient2 but same other params
    assert_eq!(get_stream(stream2).recipient, recipient2);
    assert_eq!(get_stream(stream2).sender, sender);
    assert_eq!(get_stream(stream2).flow_rate, get_stream(stream1).flow_rate);
}

#[test]
fn test_clone_stream_non_sender_fails() {
    let env = Env::default();
    let sender = Address::random(&env);
    let non_sender = Address::random(&env);
    
    let stream1 = create_stream(sender.clone(), ...);
    
    let result = clone_stream(stream1, non_sender, None, None, None, None);
    assert_eq!(result, Err(StreamError::NotAuthorized));
}

#[test]
fn test_clone_stream_with_rate_override() {
    let env = Env::default();
    let sender = Address::random(&env);
    
    let stream1 = create_stream(..., flow_rate: 1000, ...);
    
    let stream2 = clone_stream(
        stream1,
        sender,
        None,
        None,
        Some(2000),  // rate override
        None
    ).unwrap();
    
    assert_eq!(get_stream(stream2).flow_rate, 2000);
}

#[test]
fn test_clone_stream_invalid_overrides() {
    // Test invalid amount (rate * duration = 0)
    // Test invalid token
    // Test blocked recipient
    // Test rate limit exceeded
}
```

### Integration Tests

```rust
#[test]
fn test_clone_stream_creates_independent_streams() {
    // Create stream1
    // Clone to stream2
    // Verify stream1 and stream2 are independent
    // Withdraw from stream1
    // Verify stream2 balance unchanged
}

#[test]
fn test_clone_stream_inherits_auto_renew() {
    // Create stream with auto_renew = true
    // Clone it
    // Verify cloned stream has auto_renew = true
}
```

## Performance Considerations

| Operation | Cost | Notes |
|-----------|------|-------|
| Read source stream | O(1) | Single persistent storage read |
| Generate stream ID | O(1) | Hash operation |
| Validate configuration | O(n) | n = number of overrides |
| Transfer tokens | Variable | Token contract call |
| Index operations | O(1) | Each index append is constant |
| **Total** | O(n) | n = overrides + indices |

## Security Considerations

### ✅ Protected
- Only sender or delegate can clone
- All validation rules applied to new stream
- Proper authorization checks
- Nonce used to prevent ID collisions

### ⚠️ Assumptions
- Source stream configuration is valid
- Token contract is functional
- Caller has sufficient balance

## Edge Cases Handled

1. **Source stream no longer exists** → StreamNotFound error
2. **Contract paused during clone** → ContractPaused error
3. **Whitelist changed after reading source** → New stream uses current rules
4. **Multiple overrides create invalid stream** → ZeroAmount error
5. **Sender hits stream cap** → SenderStreamLimitExceeded error
6. **Token whitelist only includes some tokens** → TokenNotWhitelisted error

## Related Functions

- `create_stream()` - Main stream creation
- `get_stream()` - Read stream configuration
- `set_delegate()` - Allow delegation for cloning
- `partial_cancel_stream()` - Similar ID generation pattern

## Future Enhancements

Possible extensions:
- Batch clone multiple streams at once
- Clone all streams for a sender to new recipient
- Template repository for common stream configs
- Scheduled cloning (auto-clone on stream completion)
