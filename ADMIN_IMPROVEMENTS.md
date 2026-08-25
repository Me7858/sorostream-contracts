# Admin Access Control - Recommended Improvements

## Overview

While the SoroStream contract has comprehensive admin access control in place, this document outlines 4 specific functions that should be hardened with consistent `check_admin()` calls for security and consistency.

---

## Issue #1: `set_protocol_fee()` Lacks Admin Check

### Current Implementation
**Location**: `lib.rs:3410`

```rust
pub fn set_protocol_fee(env: Env, fee_bps: u32) -> Result<(), StreamError> {
    if fee_bps > 10_000 {
        return Err(StreamError::InvalidDuration);
    }
    set_protocol_fee(&env, fee_bps);
    Ok(())
}
```

### Problem
- ❌ No admin authorization check
- ❌ Anyone can modify the protocol fee
- ❌ Inconsistent with `propose_fee_change()` and `execute_fee_change()`

### Recommended Fix

```rust
pub fn set_protocol_fee(env: Env, fee_bps: u32) -> Result<(), StreamError> {
    check_admin(&env);  // ← ADD THIS
    if fee_bps > 10_000 {
        return Err(StreamError::InvalidDuration);
    }
    set_protocol_fee(&env, fee_bps);
    Ok(())
}
```

### Rationale
- Matches pattern used in `propose_fee_change()` and `execute_fee_change()`
- Prevents unauthorized fee manipulation
- Protects contract revenue and user expectations

---

## Issue #2: `set_min_duration()` Uses Only `require_auth()`, Not `check_admin()`

### Current Implementation
**Location**: `lib.rs:1062`

```rust
pub fn set_min_duration(env: Env, admin: Address, seconds: u64) {
    admin.require_auth();
    write_min_duration(&env, seconds);
}
```

### Problem
- ⚠️ Only verifies caller holds `admin`'s signing key
- ⚠️ Does NOT verify `admin` is the stored contract admin
- ⚠️ Allows anyone to set min_duration if they can sign the transaction

### Recommended Fix

```rust
pub fn set_min_duration(env: Env, admin: Address, seconds: u64) -> Result<(), StreamError> {
    check_admin(&env);      // ← ADD THIS (verifies stored admin)
    admin.require_auth();   // ← KEEP THIS (signature proof)
    write_min_duration(&env, seconds);
    Ok(())
}
```

### Rationale
- `require_auth()` only proves possession of a key, not identity
- `check_admin()` verifies the caller is the stored contract admin
- Pattern matches other admin functions like `propose_fee_change()`
- Prevents accidental authorization mismatches

---

## Issue #3: `set_max_duration()` Uses Only `require_auth()`, Not `check_admin()`

### Current Implementation
**Location**: `lib.rs:1074`

```rust
pub fn set_max_duration(env: Env, admin: Address, seconds: u64) {
    admin.require_auth();
    write_max_duration(&env, seconds);
}
```

### Problem
- Same as Issue #2

### Recommended Fix

```rust
pub fn set_max_duration(env: Env, admin: Address, seconds: u64) -> Result<(), StreamError> {
    check_admin(&env);      // ← ADD THIS
    admin.require_auth();   // ← KEEP THIS
    write_max_duration(&env, seconds);
    Ok(())
}
```

---

## Issue #4: `set_max_future_start_offset()` Uses Only `require_auth()`, Not `check_admin()`

### Current Implementation
**Location**: (found in storage.rs functions)

```rust
pub fn set_max_future_start_offset(env: Env, admin: Address, offset_seconds: u64) {
    admin.require_auth();
    write_max_future_start_offset(&env, offset_seconds);
}
```

### Problem
- Same as Issues #2 and #3

### Recommended Fix

```rust
pub fn set_max_future_start_offset(env: Env, admin: Address, offset_seconds: u64) -> Result<(), StreamError> {
    check_admin(&env);      // ← ADD THIS
    admin.require_auth();   // ← KEEP THIS
    write_max_future_start_offset(&env, offset_seconds);
    Ok(())
}
```

---

## Auth Pattern Comparison

### Pattern A: Recommended (Used by `propose_fee_change()`)
```rust
pub fn set_governance(env: Env, governance: Address) -> Result<(), StreamError> {
    check_admin(&env);              // Identity check
    write_governance(&env, &governance);  // Storage write
    Ok(())
}
```

**Advantages:**
- ✅ `check_admin()` verifies stored admin + calls `require_auth()`
- ✅ Single call does both checks
- ✅ Explicit identity verification

---

### Pattern B: Double Check (Used by `set_token_fee_tier()`)
```rust
pub fn set_token_fee_tier(env: Env, admin: Address, token: Address, fee_bps: u32)
    -> Result<(), StreamError>
{
    admin.require_auth();  // Signature proof
    let current_admin = read_admin(&env).ok_or(StreamError::NotInitialized)?;
    if admin != current_admin {
        return Err(StreamError::NotAuthorized);  // Identity verification
    }
    // ... do work ...
    Ok(())
}
```

**Advantages:**
- ✅ Explicit error message if address mismatch
- ✅ Can take parameters but still check admin identity
- ✅ More verbose but educational

---

### Pattern C: Not Recommended (Used by `set_min_duration()`)
```rust
pub fn set_min_duration(env: Env, admin: Address, seconds: u64) {
    admin.require_auth();  // Only signature proof, no identity check
    write_min_duration(&env, seconds);
}
```

**Problems:**
- ❌ No verification that `admin` is the stored contract admin
- ❌ Could accept any address if attacker controls `admin.require_auth()` call
- ❌ Potential authorization bypass

---

## Implementation Plan

### Step 1: Fix `set_protocol_fee()`
```rust
// lib.rs:3410
pub fn set_protocol_fee(env: Env, fee_bps: u32) -> Result<(), StreamError> {
    check_admin(&env);
    if fee_bps > 10_000 {
        return Err(StreamError::InvalidDuration);
    }
    set_protocol_fee(&env, fee_bps);
    Ok(())
}
```

### Step 2: Fix `set_min_duration()`
```rust
// lib.rs:1062
pub fn set_min_duration(env: Env, admin: Address, seconds: u64) -> Result<(), StreamError> {
    check_admin(&env);
    admin.require_auth();
    write_min_duration(&env, seconds);
    Ok(())
}
```

### Step 3: Fix `set_max_duration()`
```rust
// lib.rs:1074
pub fn set_max_duration(env: Env, admin: Address, seconds: u64) -> Result<(), StreamError> {
    check_admin(&env);
    admin.require_auth();
    write_max_duration(&env, seconds);
    Ok(())
}
```

### Step 4: Fix `set_max_future_start_offset()`
```rust
// lib.rs or storage.rs
pub fn set_max_future_start_offset(env: Env, admin: Address, offset_seconds: u64) -> Result<(), StreamError> {
    check_admin(&env);
    admin.require_auth();
    write_max_future_start_offset(&env, offset_seconds);
    Ok(())
}
```

---

## Testing Strategy

For each function, test these scenarios:

### Test 1: Admin succeeds
```rust
#[test]
fn test_set_protocol_fee_as_admin() {
    let env = Env::default();
    let admin = Address::random(&env);
    initialize(&env, admin.clone(), "1.0.0").unwrap();
    
    // Admin can set fee
    set_protocol_fee(&env, 100).unwrap();
    assert_eq!(get_protocol_fee(&env), 100);
}
```

### Test 2: Non-admin fails
```rust
#[test]
fn test_set_protocol_fee_non_admin_fails() {
    let env = Env::default();
    let admin = Address::random(&env);
    let non_admin = Address::random(&env);
    
    initialize(&env, admin.clone(), "1.0.0").unwrap();
    
    // Non-admin cannot set fee
    // This should fail at check_admin() -> require_auth()
}
```

### Test 3: Uninitialized fails
```rust
#[test]
fn test_set_protocol_fee_uninitialized() {
    let env = Env::default();
    
    // Should fail with NotInitialized
    let result = set_protocol_fee(&env, 100);
    assert_eq!(result, Err(StreamError::NotInitialized));
}
```

### Test 4: Invalid values rejected
```rust
#[test]
fn test_set_protocol_fee_invalid_bps() {
    let env = Env::default();
    let admin = Address::random(&env);
    initialize(&env, admin.clone(), "1.0.0").unwrap();
    
    // > 10,000 bps should fail
    let result = set_protocol_fee(&env, 15_000);
    assert_eq!(result, Err(StreamError::InvalidDuration));
}
```

---

## Security Implications

### Before Fix
- ⚠️ Unauthorized fee modifications possible
- ⚠️ Admin parameter identity unverified in some functions
- ⚠️ Inconsistent authorization patterns across the codebase

### After Fix
- ✅ All admin functions use consistent `check_admin()` pattern
- ✅ Two-layer verification: identity (check_admin) + signature (require_auth)
- ✅ Clear admin-only vs. non-admin function distinction
- ✅ Prevents authorization bypass attacks

---

## Backward Compatibility

These changes are **backward compatible**:
- ✅ Function signatures remain the same (adding Result return is optional)
- ✅ Existing admin calls will still work
- ✅ Non-admin calls that were failing will continue to fail
- ✅ No changes to storage or data structures

---

## Review Checklist

Before deployment:

- [ ] All 4 functions have `check_admin()` calls added
- [ ] Functions return `Result<(), StreamError>` for consistency
- [ ] Error handling matches other admin functions
- [ ] Audit trail updated if needed
- [ ] Unit tests added for each function
- [ ] Integration tests verify admin-only behavior
- [ ] Documentation updated to reflect changes
- [ ] Code review completed by security team

---

## Conclusion

These 4 improvements will standardize admin access control across the SoroStream contract, eliminating potential authorization gaps and ensuring consistent security patterns throughout the codebase.

**Estimated Impact**: Low risk, high security benefit. All changes are defensive enhancements to existing patterns.
