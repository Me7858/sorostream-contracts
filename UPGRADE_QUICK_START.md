# Contract Upgrade - Quick Start Guide

## One-Minute Summary

**What**: Upgrade SoroStream contract code without redeployment
**Who**: Admin only
**Why**: Bug fixes, features, security patches
**How**: Call `upgrade(new_wasm_hash)` with admin signature
**Result**: New code runs, all data preserved

---

## Quick Reference

```
BEFORE UPGRADE              AFTER UPGRADE
┌─────────────┐            ┌─────────────┐
│ Code v1.0.0 │            │ Code v1.1.0 │
├─────────────┤  upgrade() ├─────────────┤
│ All data    │  ────────→ │ All data    │ ✅
│ All state   │            │ All state   │
└─────────────┘            └─────────────┘
Contract Address: Same
```

---

## Step-by-Step

### 1. Build New Version
```bash
cd contracts/stream
cargo build --target wasm32v1-none --release
```

### 2. Get WASM Hash
```bash
sha256sum target/wasm32v1-none/release/sorostream_stream.wasm
```

### 3. Call Upgrade
```bash
stellar contract invoke \
  --network testnet \
  --id <CONTRACT_ADDRESS> \
  -- upgrade \
  --new_wasm_hash <SHA256_HASH_HERE>
```

### 4. Verify Success
```bash
# Check version changed
stellar contract invoke \
  --network testnet \
  --id <CONTRACT_ADDRESS> \
  -- get_version

# Test a function
stellar contract invoke \
  --network testnet \
  --id <CONTRACT_ADDRESS> \
  -- get_admin
```

---

## Common Tasks

### Get Current WASM Hash

```bash
# After building:
HASH=$(sha256sum target/wasm32v1-none/release/sorostream_stream.wasm | cut -d' ' -f1)
echo "WASM Hash: $HASH"

# Convert to base64 if needed:
HASH_B64=$(echo "$HASH" | xxd -r -p | base64)
echo "Hash (Base64): $HASH_B64"
```

### Test Upgrade on Testnet

```bash
# 1. Deploy test version
cargo build --target wasm32v1-none --release
HASH=$(sha256sum target/wasm32v1-none/release/sorostream_stream.wasm | cut -d' ' -f1)

# 2. Call upgrade on testnet contract
stellar contract invoke \
  --network testnet \
  --id <TESTNET_CONTRACT> \
  -- upgrade \
  --new_wasm_hash $HASH

# 3. Verify
stellar contract invoke --network testnet --id <TESTNET_CONTRACT> -- get_version
```

### Check If Upgrade Succeeded

```bash
# Version changed?
OLD_VERSION="1.0.0"
NEW_VERSION=$(stellar contract invoke \
  --network testnet \
  --id <CONTRACT_ADDRESS> \
  -- get_version)

if [ "$OLD_VERSION" != "$NEW_VERSION" ]; then
  echo "✅ Upgrade succeeded: $NEW_VERSION"
else
  echo "❌ Upgrade failed"
fi
```

### Verify All Data Preserved

```bash
# Check key stream still exists
stellar contract invoke \
  --network testnet \
  --id <CONTRACT_ADDRESS> \
  -- get_stream \
  --stream_id <STREAM_ID>

# Check admin unchanged
stellar contract invoke \
  --network testnet \
  --id <CONTRACT_ADDRESS> \
  -- get_admin

# Check a stream count
stellar contract invoke \
  --network testnet \
  --id <CONTRACT_ADDRESS> \
  -- get_stats
```

---

## Key Points

✅ **State Preserved**
- Admin address stays same
- Existing streams intact
- Fee configuration unchanged
- All indices preserved

✅ **Admin-Only**
- Requires admin's signing key
- No one else can call upgrade
- Cryptographic verification

✅ **Atomic**
- Upgrade succeeds fully or fails completely
- No partial upgrades
- Safe to retry if it fails

✅ **No Redeployment**
- Contract address stays same
- No code migration needed
- Streams remain at same address

---

## Troubleshooting

| Problem | Solution |
|---------|----------|
| "Not Initialized" | Run `initialize()` first |
| "Not Authorized" | Use admin's signing key |
| "Invalid Hash" | Verify hash format (hex/base64) |
| Version didn't change | Check hash is correct, wait for confirmation |
| Data missing | Data isn't lost; check you're querying correct contract |

---

## Pre-Upgrade Checklist

- [ ] Code reviewed
- [ ] Tests pass: `cargo test`
- [ ] Linter passes: `cargo clippy`
- [ ] Build succeeds
- [ ] Tested on testnet
- [ ] All data verified post-upgrade
- [ ] Admin key secure
- [ ] Users notified

---

## Files & References

- **Implementation**: See UPGRADE_IMPLEMENTATION.md
- **Enhancement**: See UPGRADE_ENHANCEMENT.md
- **Admin Control**: See ADMIN_ACCESS_CONTROL.md
- **Soroban Docs**: https://soroban.stellar.org/docs

---

## Example: Complete Upgrade

```bash
#!/bin/bash
set -e

CONTRACT="CPRODUCTION_ADDRESS"
NETWORK="public"
ADMIN_KEY="$HOME/.config/stellar/keys/admin"

echo "📦 Building new version..."
cargo build --target wasm32v1-none --release

echo "🔐 Getting WASM hash..."
HASH=$(sha256sum target/wasm32v1-none/release/sorostream_stream.wasm | cut -d' ' -f1)
echo "Hash: $HASH"

echo "⏫ Calling upgrade..."
stellar contract invoke \
  --network $NETWORK \
  --source-account $ADMIN_KEY \
  --id $CONTRACT \
  -- upgrade \
  --new_wasm_hash $HASH

echo "✅ Upgrade submitted. Waiting for confirmation..."
sleep 5

echo "📋 Verifying upgrade..."
stellar contract invoke \
  --network $NETWORK \
  --id $CONTRACT \
  -- get_version

echo "🎉 Upgrade complete!"
```

---

**More Details**: Read UPGRADE_IMPLEMENTATION.md for comprehensive guide.
