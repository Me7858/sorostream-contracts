# Contract Upgrade Implementation

## Overview

The SoroStream contract implements a secure upgrade mechanism that allows the admin to deploy new contract code without redeploying the entire contract. This enables seamless feature updates, bug fixes, and security patches while preserving all contract state and data.

---

## Current Implementation

### Location
**File**: `contracts/stream/src/lib.rs:358-363`

```rust
pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), StreamError> {
    let admin = read_admin(&env).ok_or(StreamError::NotInitialized)?;
    admin.require_auth();
    env.deployer().update_current_contract_wasm(new_wasm_hash);
    Ok(())
}
```

### How It Works

1. **Admin Verification**: Reads stored admin address from contract storage
2. **Auth Check**: Calls `require_auth()` to verify caller holds admin's signing key
3. **WASM Update**: Calls Soroban SDK's `update_current_contract_wasm()` with new code hash
4. **State Preservation**: All persistent data remains unchanged

---

## Security Properties

### ✅ What's Protected
- **Admin-only**: Only the stored admin address can invoke upgrade
- **Cryptographic proof**: `require_auth()` verifies signature
- **One-time per contract**: Each upgrade overwrites previous bytecode
- **Immutable state**: All contract storage persists across upgrades

### ⚠️ Assumptions
- Admin's signing key is secure (no compromise)
- New WASM hash is verified off-chain before submission
- Governance process validates changes before upgrade
- Version tracking coordinates multi-contract deployments

---

## Usage Guide

### Prerequisite: Build New Contract

```bash
# 1. Build the new contract WASM
cd contracts/stream
cargo build --target wasm32v1-none --release

# 2. Get the new WASM file
NEW_WASM="target/wasm32v1-none/release/sorostream_stream.wasm"

# 3. Calculate SHA256 hash (this is what goes to the upgrade function)
HASH=$(sha256sum "$NEW_WASM" | cut -d' ' -f1 | xxd -r -p | base64)
echo "WASM SHA256 Hash (Base64): $HASH"
```

### Invoke Upgrade via CLI

```bash
# Using Stellar CLI
stellar contract invoke \
  --network testnet \
  --id <CONTRACT_ADDRESS> \
  -- upgrade \
  --new_wasm_hash <BASE64_HASH>

# The admin's signing key must be available for authentication
```

### Invoke Upgrade via SDK

```rust
// Using sorostream-sdk
let client = SoroStreamClient::new(&env, &contract_address);

let new_wasm_hash = BytesN::<32>::from_array(&env, &hash_bytes);
client.upgrade(&env, new_wasm_hash)?;
```

---

## Upgrade Process Checklist

### Pre-Upgrade

- [ ] Code review completed by security team
- [ ] All tests pass: `cargo test`
- [ ] Linting passes: `cargo clippy -- -D warnings`
- [ ] WASM size acceptable (< 256KB typically)
- [ ] Contract behavior compatible with existing state
- [ ] Changelog updated
- [ ] Release notes prepared
- [ ] Backup of current contract address documented

### During Upgrade

- [ ] Build new WASM: `cargo build --target wasm32v1-none --release`
- [ ] Verify build is reproducible
- [ ] Calculate SHA256 hash of WASM binary
- [ ] Call `upgrade()` with hash and admin signature
- [ ] Wait for transaction confirmation
- [ ] Verify upgrade succeeded (check via contract call)

### Post-Upgrade

- [ ] Verify `get_version()` returns new version
- [ ] Verify admin can still call functions
- [ ] Check recent streams still accessible: `get_stream(stream_id)`
- [ ] Test critical operations (create stream, withdraw, etc)
- [ ] Monitor on-chain activity for errors
- [ ] Publish release notes

---

## State Preservation Across Upgrades

### ✅ Preserved
All contract storage persists:
- Admin address
- Existing streams
- Fee configuration
- Whitelist/blocklist
- All indices (sender, recipient, global)
- Audit log
- Migration history
- Per-token statistics

### Example: Migration Flow
```
Old Contract              New Contract
(v1.0.0)                 (v1.1.0)
├── Admin: 0xABC...      Admin: 0xABC... ✅ preserved
├── Stream #1 data       Stream #1 data ✅ preserved
├── Fee config: 100 bps  Fee config: 100 bps ✅ preserved
└── Indices              Indices ✅ preserved
```

---

## Risk Mitigation

### Risk: Incompatible Code Changes

**Scenario**: New code expects different storage layout

**Mitigation**:
- ✅ Use migration entry points for schema changes
- ✅ Add `migrate()` function for version coordination
- ✅ Test extensively on testnet before mainnet
- ✅ Plan multi-step upgrades if needed

### Risk: Admin Key Compromise

**Scenario**: Attacker upgrades to malicious code

**Mitigation**:
- ✅ Secure admin key in secure enclave/HSM
- ✅ Use multisig governance (see guardian/governance roles)
- ✅ Monitor contract upgrades via external systems
- ✅ Can recover via governance consensus

### Risk: WASM Hash Mismatch

**Scenario**: Wrong hash submitted, code doesn't execute as expected

**Mitigation**:
- ✅ Verify WASM hash off-chain before submission
- ✅ Test new code on testnet first
- ✅ Use reproducible builds (Docker, etc)
- ✅ Compare hash with known good build

### Risk: Upgrade in Progress

**Scenario**: Multiple upgrade calls race each other

**Mitigation**:
- ✅ Soroban SDK handles atomicity (first wins)
- ✅ Failed upgrades revert immediately
- ✅ No partial state corruption possible
- ✅ Safe to retry failed upgrades

---

## Monitoring & Verification

### Check Current Version

```rust
let version = SoroStreamContract::get_version(&env)?;
println!("Current version: {}", version);
```

### Verify Upgrade Success

```rust
// Call a function to verify new code is running
let admin = SoroStreamContract::get_admin(&env)?;
assert_eq!(admin, expected_admin);

// Check version changed
let new_version = SoroStreamContract::get_version(&env)?;
assert_ne!(new_version, old_version);

// Verify streams accessible
let stream = SoroStreamContract::get_stream(&env, stream_id)?;
println!("Stream still exists: {:?}", stream);
```

### Audit Trail

```rust
// See admin action log
let log = SoroStreamContract::get_admin_log(&env);
for entry in log.iter() {
    if entry.instruction.as_str().map_or(false, |s| s == "migrate") {
        println!("Migration by {} at {}", entry.admin, entry.timestamp);
    }
}
```

---

## Advanced: Multi-Step Upgrades

Some upgrades require coordination with storage migrations:

### Pattern 1: Feature-Gated Rollout
```
Step 1: Deploy v1.1.0 with new functions (no state change)
        → All functions work, old and new code paths coexist
        
Step 2: Call migrate(v1.0.0 → v1.1.0) to update state
        → Storage format updated in background
        
Step 3: Deploy v1.2.0 that requires new state format
        → New format is already applied, works seamlessly
```

### Pattern 2: Versioned Structs
```rust
// In code, handle multiple stream formats
pub fn load_stream(env: &Env, stream_id: u64) -> Option<Stream> {
    // Check which version format exists
    // Automatically convert old → new format on read
    // Normalize to current Stream struct
}
```

---

## Implementation Notes

### Why `require_auth()` is Used

The current implementation uses `require_auth()` (signature check) instead of `check_admin()` (identity + signature) because:

1. **Soroban Framework**: Deployer contract is a privileged system contract
2. **Authorization Chain**: The admin's auth is proven implicitly through the call stack
3. **Additional Check**: However, also reads and validates admin address for redundancy

### Recommended Enhancement

Consider adding `check_admin()` for consistency with other admin functions:

```rust
pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), StreamError> {
    check_admin(&env);           // ← ADD THIS for consistency
    // update_current_contract_wasm is safe and idempotent
    env.deployer().update_current_contract_wasm(new_wasm_hash);
    Ok(())
}
```

This would make the authorization pattern consistent with other admin operations.

---

## Error Handling

### Error: `NotInitialized`
**Cause**: Contract not yet initialized

**Recovery**: Call `initialize()` first

```rust
SoroStreamContract::initialize(&env, admin, "1.0.0")?;
```

### Error: `NotAuthorized` (implicit)
**Cause**: Caller is not the admin

**Recovery**: Use admin's signing key

### Error: `Overflow` (if hash invalid)
**Cause**: Invalid WASM hash format

**Recovery**: Verify hash is valid SHA256

---

## Best Practices

### ✅ DO
- [ ] Test upgrade path on testnet first
- [ ] Get multiple approvals before mainnet upgrade
- [ ] Keep old WASM binary for rollback (if needed)
- [ ] Document changes in changelog
- [ ] Announce upgrade to users in advance
- [ ] Monitor network for issues post-upgrade

### ❌ DON'T
- [ ] Don't upgrade without testing
- [ ] Don't change storage layout without migration step
- [ ] Don't upgrade during peak usage without warning
- [ ] Don't lose admin key (upgrade will be impossible)
- [ ] Don't assume state will migrate automatically

---

## Example: Complete Upgrade Flow

### Step 1: Prepare New Version

```bash
# Update version in code
sed -i 's/"1.0.0"/"1.1.0"/g' src/lib.rs

# Make feature changes
# ... edit code ...

# Build
cargo build --target wasm32v1-none --release

# Get hash
HASH=$(sha256sum target/wasm32v1-none/release/sorostream_stream.wasm | cut -d' ' -f1)
echo "New WASM SHA256: $HASH"
```

### Step 2: Test on Testnet

```bash
# Deploy test version to testnet
stellar contract invoke \
  --network testnet \
  --id CAA3XNSN7V3DZQV5EMJU5MUK3QZPWQZ7 \
  -- upgrade \
  --new_wasm_hash <HASH>

# Verify upgrade
stellar contract invoke \
  --network testnet \
  --id CAA3XNSN7V3DZQV5EMJU5MUK3QZPWQZ7 \
  -- get_version
```

### Step 3: Validate State Preservation

```bash
# Check streams still exist
stellar contract invoke \
  --network testnet \
  --id CAA3XNSN7V3DZQV5EMJU5MUK3QZPWQZ7 \
  -- get_stream \
  --stream_id 12345

# Verify admin unchanged
stellar contract invoke \
  --network testnet \
  --id CAA3XNSN7V3DZQV5EMJU5MUK3QZPWQZ7 \
  -- get_admin
```

### Step 4: Upgrade Mainnet

```bash
# Same process, different network
stellar contract invoke \
  --network public \
  --id CPRODUCTION_ADDRESS \
  -- upgrade \
  --new_wasm_hash <HASH>

# Monitor
stellar contract invoke \
  --network public \
  --id CPRODUCTION_ADDRESS \
  -- get_version
```

---

## Troubleshooting

### Problem: Upgrade Fails with "Not Initialized"
**Solution**: Ensure contract was initialized before upgrade
```rust
let admin = get_admin(env)?;  // Should return Some, not None
```

### Problem: Upgrade Fails with "Not Authorized"
**Solution**: Use admin's signing key
```bash
stellar contract invoke \
  --source-account <ADMIN_KEY> \  # ← Specify admin key
  --network testnet \
  -- upgrade \
  --new_wasm_hash <HASH>
```

### Problem: New Code Doesn't Execute
**Solution**: Verify WASM hash is correct
```bash
# Check file hash
sha256sum target/wasm32v1-none/release/sorostream_stream.wasm

# Should match what was submitted
echo "<SUBMITTED_HASH>"
```

### Problem: State Lost After Upgrade
**Solution**: This shouldn't happen. If it does:
1. Rollback to previous version (keep old WASM)
2. Contact Stellar support
3. Restore from backup

---

## Governance Integration

For production deployments, consider wrapping upgrade in governance:

```rust
pub fn propose_upgrade(admin: Address, new_wasm_hash: BytesN<32>) -> Result<(), StreamError> {
    admin.require_auth();
    // Store proposal with voting period
    store_pending_upgrade(&env, new_wasm_hash, voting_period)?;
    Ok(())
}

pub fn execute_pending_upgrade(admin: Address) -> Result<(), StreamError> {
    check_admin(&env);
    
    let (new_wasm_hash, voting_end) = get_pending_upgrade(&env)?;
    if env.ledger().timestamp() < voting_end {
        return Err(StreamError::StreamLocked);  // Not enough time passed
    }
    
    env.deployer().update_current_contract_wasm(new_wasm_hash);
    clear_pending_upgrade(&env);
    Ok(())
}
```

This allows:
- ✅ Public announcement of pending upgrade
- ✅ Time for stakeholders to review
- ✅ Coordinated rollback if issues found
- ✅ Governance consensus on changes

---

## Summary

The SoroStream upgrade mechanism is **production-ready** and provides:

✅ **Admin-only access** — Only contract owner can upgrade
✅ **State preservation** — All data persists across upgrades
✅ **Atomic operations** — Upgrade succeeds or fails completely
✅ **Signature verification** — Cryptographic proof required
✅ **No redeployment** — Upgrade in place, preserves contract address

The implementation is secure and follows Soroban best practices.

---

## Related Documentation

- [Admin Access Control](./ADMIN_ACCESS_CONTROL.md) — Authorization patterns
- [Contract Architecture](./ARCHITECTURE.md) — Overall design
- [Soroban Deployer Documentation](https://soroban.stellar.org/docs)
