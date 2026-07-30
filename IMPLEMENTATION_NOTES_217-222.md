# Implementation Notes: Issues #217-222

This document describes the implementation of four interconnected security and administration features for the SoroStream contract.

## Overview

- **Issue #217**: Per-address rate limiting to prevent stream creation spam
- **Issue #218**: Slippage protection for asset price deviation during stream settlement
- **Issue #221**: Token whitelist for approved SAC assets
- **Issue #222**: Treasury auto-sweep instruction to collect accumulated protocol fees

## Architecture

### Issue #217: Rate Limiting

Rate limiting is implemented as a rolling window counter per address.

**Storage:**
- `rate_limit_window`: Window size in seconds (instance, default: 3600)
- `rate_limit_max`: Max creations per window (instance, default: 20)
- `rate_limit_state(address)`: (window_start_time, count_in_window) tuple per address (persistent)
- `rate_limit_exempt(address)`: Boolean exempt list (persistent)

**Logic:**
1. On each `create_stream` call, `check_rate_limit()` is invoked
2. If address is exempt, check passes immediately
3. If current timestamp is outside the window, counter resets
4. If counter >= max_creations, return `RateLimitExceeded` error
5. Otherwise increment counter and allow stream creation

**Admin Functions:**
- `set_rate_limit_window(window_seconds)`: Update window size
- `set_rate_limit_max(max_creations)`: Update max creations per window
- `add_rate_limit_exempt(address)`: Add exempt address (useful for trusted integrators)
- `remove_rate_limit_exempt(address)`: Remove exempt address

**Read Function:**
- `remaining_quota(address)`: Returns remaining allowed stream creations in current window

### Issue #221: Token Whitelist

Token whitelist ensures only vetted SAC tokens can be used for streaming.

**Storage:**
- `token_whitelist_enabled`: Boolean toggle (instance, default: false for backward compatibility)
- `token_whitelisted(token)`: Per-token allowlist (persistent)

**Logic:**
1. In `create_stream`, `check_token_whitelist()` is called before stream creation
2. If token whitelist is enabled and token is not whitelisted, return `TokenNotWhitelisted`
3. Otherwise allow the token

**Admin Functions:**
- `set_token_whitelist_enabled(enabled)`: Enable/disable the feature
- `add_token_to_whitelist(token)`: Add token to whitelist
- `remove_token_from_whitelist(token)`: Remove token from whitelist

**Design Notes:**
- Disabled by default to maintain backward compatibility
- Separate from recipient whitelist (which uses `is_whitelisted`)
- Can be toggled on/off without modifying individual token entries

### Issue #222: Fee Sweep

Admin-gated function to collect accumulated fees from contract balance.

**Logic:**
1. `sweep_fees(token, destination)` is called by admin
2. Function queries contract's balance of the token
3. Transfers entire balance to destination address
4. Emits `FeeSwept` event with token, amount, and destination

**Implementation Notes:**
- Works with current contract token balance
- Can be called multiple times as fees accumulate
- Event emitted for audit trail
- No state modification (just transfers existing balance)

### Issue #218: Slippage Protection

Allows stream senders to set acceptable price deviation limits.

**Storage:**
- `slippage_params(stream_id)`: (reference_price, max_slippage_bps) tuple per stream (persistent)

**Logic:**
1. `set_slippage_params(stream_id, reference_price, max_slippage_bps)` stores parameters
2. At settlement time, if current price deviates from reference > max_slippage_bps, operation reverts
3. Slippage warning emitted when within 80% of limit

**Admin Function:**
- `set_slippage_params(stream_id, reference_price, max_slippage_bps)`: Set or update slippage limit

**Parameters:**
- `reference_price`: Base price for comparison (in basis points, typically from price oracle)
- `max_slippage_bps`: Maximum acceptable deviation in basis points (0-10000, where 10000 = 100%)

**Design Notes:**
- Default 0 means no slippage protection (backward compatible)
- Allows fine-grained control per stream
- Sender-only operation (only they can set parameters for their streams)

## Error Codes

New error codes added:

```rust
RateLimitExceeded = 37,      // Rate limit exceeded for stream creation
TokenNotWhitelisted = 38,    // Token not in whitelist
SlippageExceeded = 39,       // Price deviation exceeds max slippage
InvalidSlippage = 40,        // Invalid slippage parameter (> 10000)
```

## Events

New events emitted:

```rust
// Rate limiting
rate_limit_exceeded(sender: Address)
rate_limit_updated(window_seconds: u64, max_creations: u32)

// Token whitelist
token_whitelisted(token: Address)
token_dewhitelisted(token: Address)
token_whitelist_toggled(enabled: bool)

// Fee sweep
fee_swept(token: Address, amount: i128, destination: Address)

// Slippage
slippage_exceeded(stream_id: u64, current_price: i128, max_slippage_bps: u32)
slippage_warning(stream_id: u64, current_deviation_bps: u32, max_slippage_bps: u32)
```

## Integration Points

### create_stream Flow

1. Check pause status
2. Get current timestamp (moved earlier for validation)
3. Check rate limiting (NEW - Issue #217)
4. Check token whitelist (NEW - Issue #221)
5. Continue with existing validation

### Admin Interface Extensions

New admin methods:
- Rate limiting: 4 functions (set window, set max, add exempt, remove exempt)
- Token whitelist: 3 functions (enable/disable, add, remove)
- Fee sweep: 1 function (sweep_fees)
- Slippage: 1 function (set_slippage_params)

## Testing Considerations

### Rate Limiting Tests

- [ ] Verify rate limit blocks after N creations in time window
- [ ] Verify counter resets after window expires
- [ ] Verify exempt addresses bypass limit
- [ ] Verify admin can update window and max parameters
- [ ] Verify remaining_quota() returns correct value
- [ ] Test edge cases: exactly at limit, one over limit, window boundary

### Token Whitelist Tests

- [ ] Verify create_stream fails with non-whitelisted token when enabled
- [ ] Verify create_stream succeeds with whitelisted token
- [ ] Verify toggling whitelist on/off
- [ ] Verify adding/removing tokens from whitelist
- [ ] Verify backward compatibility (disabled by default)

### Fee Sweep Tests

- [ ] Verify sweep_fees transfers all balance
- [ ] Verify sweep_fees with zero balance is no-op
- [ ] Verify only admin can call sweep_fees
- [ ] Verify event is emitted with correct parameters

### Slippage Tests

- [ ] Verify set_slippage_params stores parameters
- [ ] Verify only sender can set parameters
- [ ] Verify invalid slippage (> 10000) rejected
- [ ] Verify settlement uses stored parameters
- [ ] Verify warning emitted at 80% threshold

## Deployment Checklist

- [ ] All new error codes tested
- [ ] All new events tested
- [ ] Rate limiting not too strict (default 20 per hour is reasonable)
- [ ] Token whitelist disabled by default for backward compatibility
- [ ] Fee sweep tested with various token balances
- [ ] Slippage parameters validated (max 10000)
- [ ] All new admin functions properly gated and audited
- [ ] Events logged for monitoring
- [ ] Documentation updated in contract reference
