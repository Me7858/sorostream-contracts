# Admin Access Control Implementation

## Overview

This document describes the admin access control system for SoroStream contracts. All privileged operations (fee configuration, emergency pause, whitelist management, and other administrative functions) are restricted to the contract owner via the `check_admin()` function.

## Admin Storage

The admin address is stored in **instance storage** (persistent across contract upgrades) using the key `"admin"`.

### Storage Functions

```rust
pub fn write_admin(env: &Env, admin: &Address)          // Store admin
pub fn read_admin(env: &Env) -> Option<Address>         // Retrieve admin
pub fn check_admin(env: &Env)                           // Assert caller is admin (panics if not)
```

The `check_admin()` function:
1. Reads the stored admin address
2. Calls `.require_auth()` to verify the caller is the admin
3. Returns nothing on success, panics via soroban-sdk on auth failure

## Admin-Only Entry Points

All entry points marked below require admin authorization. Admin must call the function with their own signature.

### Initialization & Lifecycle

| Function | Purpose | Implementation |
|----------|---------|-----------------|
| `initialize(env, admin, version)` | Initialize contract, set admin | One-time only; `AlreadyInitialized` error if called again |
| `set_admin(env, new_admin)` | Transfer admin to new address | `check_admin()` required |
| `upgrade(env, new_wasm_hash)` | Deploy new contract code | Admin `.require_auth()` + upgrade via Soroban deployer |
| `emergency_pause(env)` | Pause all contract operations | `check_admin()` required; also updates audit log |
| `emergency_resume(env)` | Resume after emergency pause | `check_admin()` required; also updates audit log |
| `migrate(env, from_version, to_version)` | Perform contract migration | `check_admin()` required; records migration in audit log |

### Fee Management

| Function | Purpose | Implementation |
|----------|---------|-----------------|
| `set_protocol_fee(env, fee_bps)` | Set global protocol fee (basis points) | `check_admin()` required |
| `propose_fee_change(env, admin, new_fee_bps)` | Propose fee change with 7-day timelock | Admin `.require_auth()` + `check_admin()` required |
| `execute_fee_change(env)` | Execute pending fee change after timelock | Callable by anyone after 7-day period expires |
| `set_token_fee_tier(env, admin, token, fee_bps)` | Set per-token fee override | Admin `.require_auth()` + `check_admin()` required |
| `remove_token_fee_tier(env, admin, token)` | Remove per-token fee override | Admin `.require_auth()` + `check_admin()` required |
| `set_creation_fee(env, fee, xlm_token)` | Set XLM creation fee | `check_admin()` required |
| `sweep_fees(env, admin, token, destination)` | Withdraw accumulated fees | Admin `.require_auth()` + `check_admin()` required |

### Whitelist & Blocklist Management

| Function | Purpose | Implementation |
|----------|---------|-----------------|
| `set_whitelist_enabled(env, admin, enabled)` | Enable/disable recipient whitelist | Admin `.require_auth()` + `check_admin()` required |
| `add_to_whitelist(env, admin, recipient)` | Add recipient to whitelist | Admin `.require_auth()` + `check_admin()` required |
| `remove_from_whitelist(env, admin, recipient)` | Remove recipient from whitelist | Admin `.require_auth()` + `check_admin()` required |
| `add_to_blocklist(env, addr)` | Block sender/recipient from creating streams | `check_admin()` required |
| `remove_from_blocklist(env, addr)` | Remove address from blocklist | `check_admin()` required |

### Stream Configuration

| Function | Purpose | Implementation |
|----------|---------|-----------------|
| `set_max_streams(env, max_streams)` | Set global stream cap per sender | `check_admin()` required |
| `set_sender_stream_limit(env, sender, limit)` | Override per-sender stream limit | `check_admin()` required |
| `set_withdrawal_cooldown(env, admin, cooldown_seconds)` | Set minimum time between withdrawals | Admin `.require_auth()` + `check_admin()` required |
| `set_min_duration(env, admin, seconds)` | Set minimum allowed stream duration | Admin `.require_auth()` required (no `.check_admin()` call in current code) |
| `set_max_duration(env, admin, seconds)` | Set maximum allowed stream duration | Admin `.require_auth()` required (no `.check_admin()` call in current code) |
| `set_max_future_start_offset(env, admin, offset_seconds)` | Limit future-dated stream scheduling | Admin `.require_auth()` required (no `.check_admin()` call in current code) |
| `set_stream_creation_cooldown(env, admin, cooldown_seconds)` | Limit stream creation rate | Admin `.require_auth()` + `check_admin()` required |

### Token & Rate Limiting Configuration

| Function | Purpose | Implementation |
|----------|---------|-----------------|
| `set_max_streams_per_token(env, max)` | Limit streams per token | `check_admin()` required |
| `set_token_whitelist_enabled(env, enabled)` | Enable/disable token whitelist | `check_admin()` required |
| `add_token_to_whitelist(env, token)` | Whitelist a token | `check_admin()` required |
| `remove_token_from_whitelist(env, admin, token)` | Remove token from whitelist | Admin `.require_auth()` + `check_admin()` required |
| `add_rate_limit_exempt(env, addr)` | Exempt address from rate limits | `check_admin()` required |
| `remove_rate_limit_exempt(env, addr)` | Remove rate limit exemption | `check_admin()` required |

### Fee Exemption & Other Admin Operations

| Function | Purpose | Implementation |
|----------|---------|-----------------|
| `add_fee_exempt(env, addr)` | Exempt address from protocol fees | `check_admin()` required |
| `remove_fee_exempt(env, addr)` | Remove fee exemption | `check_admin()` required |
| `set_guardian(env, guardian)` | Set emergency pause guardian | `check_admin()` required |
| `set_governance(env, governance)` | Set emergency unpause authority | `check_admin()` required |
| `set_treasury_address(env, treasury)` | Set treasury for fee collection | `check_admin()` required |
| `set_expiry_warning_window(env, window_ledgers)` | Configure stream expiry warning threshold | `check_admin()` required |
| `set_new_sender_stream_cap(env, cap)` | Set new-sender concurrent stream cap | `check_admin()` required |
| `set_sender_promotion_threshold(env, threshold)` | Set lifetime stream count for promotion | `check_admin()` required |
| `set_grace_period_ledgers(env, ledgers)` | Set post-expiry grace period for fund recovery | `check_admin()` required |
| `register_federation(env, admin, federation_name, stellar_address)` | Register federation name | Admin `.require_auth()` + `check_admin()` required |
| `unregister_federation(env, admin, federation_name)` | Unregister federation name | Admin `.require_auth()` + `check_admin()` required |
| `recalibrate_stats(env, admin)` | Recalibrate active stream counters | Admin `.require_auth()` + `check_admin()` required |

### List Manipulation & Emergency

| Function | Purpose | Implementation |
|----------|---------|-----------------|
| `get_admin_log(env)` | Retrieve admin action audit log | No auth required; returns circular buffer of last 20 admin actions |
| `get_admin(env)` | Get current admin address | No auth required; read-only |

## Design Patterns

### Pattern 1: Direct Admin Check
```rust
pub fn some_admin_function(env: Env) -> Result<(), StreamError> {
    check_admin(&env);  // Panics if caller is not admin
    // ... perform privileged operation ...
    Ok(())
}
```

### Pattern 2: Admin with Auth Signature
```rust
pub fn some_admin_function(env: Env, admin: Address) -> Result<(), StreamError> {
    admin.require_auth();           // Verify caller holds admin's key
    check_admin(&env);              // Verify caller is the stored admin
    // ... perform privileged operation ...
    Ok(())
}
```

### Pattern 3: Audit Logging
```rust
let admin = read_admin(&env).unwrap();
let ts = env.ledger().timestamp();
let entry = AuditEntry {
    instruction: String::from_str(&env, "set_protocol_fee"),
    admin: admin.clone(),
    timestamp: ts,
    params: format_params(...),
};
append_audit_entry(&env, &entry);
```

## Audit Trail

All significant admin actions are recorded in a **circular buffer** (capacity 20) stored in instance storage:

- Key: `"al_head"` (write head), `"al_len"` (current length)
- Entry type: `AuditEntry { instruction, admin, timestamp, params }`
- Accessed via: `get_admin_log(env)` → `Vec<AuditEntry>`

Entries recorded automatically for:
- `initialize`
- `emergency_pause`
- `emergency_resume`
- `migrate`

Entries can be manually added in custom admin functions using `append_audit_entry()`.

## Security Considerations

1. **Admin as Superuser**: The admin is a single superuser with exclusive control over all privileged operations. Consider governance models if decentralization is desired.

2. **Auth Requirements**: The `require_auth()` call ensures the caller cryptographically proves possession of the admin's signing key. However, some functions only have `check_admin()` without explicit auth (relies on Soroban's invocation auth chain).

3. **Timelock on Fees**: Fee changes use a 7-day timelock to allow stakeholders to observe the pending change and exit if desired.

4. **Guardian/Governance Split**: `emergency_pause` requires admin, but `pause()` can be called by a guardian, and `unpause()` by a governance address. This allows separation of duties.

5. **Audit Trail**: All admin actions are recorded but not immutable (admin could theoretically claw back logs if contract allows). Proper access control ensures logs are tamper-evident.

## Incident Response

If an admin key is compromised:
1. The compromised admin should immediately call `set_admin()` with a new trusted address
2. Or call `emergency_pause()` to lock the contract while a governance process recovers
3. A new admin can then perform recovery operations (e.g., `sweep_fees()`, `recover_expired()`)

## Testing Admin Functions

All admin-only functions should be tested with:
- ✅ Caller = legitimate admin (should succeed)
- ✅ Caller = non-admin account (should fail with `NotAuthorized` or panic)
- ✅ Uninitialized contract (should fail with `NotInitialized`)

Example test:
```rust
#[test]
fn test_set_protocol_fee_requires_admin() {
    let env = Env::default();
    let admin = Address::random(&env);
    SoroStreamContract::initialize(&env, admin.clone(), String::from_str(&env, "1.0.0")).unwrap();
    
    // Admin can set fee
    SoroStreamContract::set_protocol_fee(&env, 100).unwrap();
    
    // Non-admin cannot set fee
    let non_admin = Address::random(&env);
    // This would fail with auth error or similar
}
```
