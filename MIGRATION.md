# SoroStream Contract Migration Guide: v1 → v2

This document describes the complete upgrade path for migrating a deployed SoroStream
v1 contract instance to v2. Follow every step in order. Skipping steps may result in
lost funds or an inconsistent contract state.

---

## Table of Contents

1. [Overview of Breaking Changes](#1-overview-of-breaking-changes)
2. [Pre-Upgrade Checklist](#2-pre-upgrade-checklist)
3. [Storage Key Changes](#3-storage-key-changes)
4. [Step-by-Step Migration Instructions](#4-step-by-step-migration-instructions)
5. [Post-Upgrade Verification](#5-post-upgrade-verification)
6. [Rollback Procedure](#6-rollback-procedure)

---

## 1. Overview of Breaking Changes

v2 introduces several new fields on the `Stream` struct and new storage keys. Because
Soroban persistent storage entries are XDR-encoded, adding fields to a `#[contracttype]`
struct is a **breaking schema change**: a v2 binary reading a v1 stream entry will fail
to deserialise unless a migration step back-fills the new fields.

Key changes between v1 and v2:

| Area | v1 | v2 |
|------|----|----|
| `Stream.curve` | not present | `VestingCurve` enum (Linear / TimeDecay) |
| `Stream.withdrawal_steps` | not present | `Option<u32>` |
| `Stream.min_withdrawal_amount` | not present | `Option<i128>` |
| `Stream.non_transferable` | not present | `bool` |
| `Stream.requires_recipient_approval` | not present | `bool` |
| `Stream.approval_timestamp` | not present | `u64` |
| `Stream.sender_locked` | not present | `bool` |
| `Stream.oracle` | not present | `Option<Address>` |
| `Stream.max_price_deviation_bps` | not present | `u32` |
| `Stream.creation_price` | not present | `i128` |
| Fee-exempt index | not present | per-address persistent flag |
| Blocklist index | not present | per-address persistent flag |
| Rate-limit state | not present | per-sender window counter |

---

## 2. Pre-Upgrade Checklist

Complete every item before deploying the v2 wasm. Each step is a safety gate; if any
check fails, stop and resolve the issue before proceeding.

### 2.1 Pause All Active Streams

1. Call `emergency_pause()` on the v1 contract to halt all new withdrawals and
   creations. This sets the global `paused` flag and gives you a safe window.
2. Confirm the flag is set:
   ```
   soroban contract invoke --id <CONTRACT_ID> -- is_paused
   # expected: true
   ```
3. Record the ledger sequence at which you paused — you will need it to calculate the
   automatic unpause window when you resume.

### 2.2 Snapshot On-Chain Storage

Export a full snapshot of all persistent storage entries before the upgrade:

```bash
soroban contract read --id <CONTRACT_ID> --output json > v1-snapshot-$(date +%s).json
```

Store this file off-chain in at least two independent locations (e.g. IPFS and a
team-controlled S3 bucket). It is your recovery baseline.

### 2.3 Verify Admin Key

Confirm you hold the private key matching the contract admin address:

```bash
soroban contract invoke --id <CONTRACT_ID> -- read_admin
# compare with your key list
```

You will need admin authority to invoke `migrate_storage` and to resume the contract
after upgrade.

### 2.4 Verify Treasury Balance

Check that the protocol treasury address holds enough XLM to cover any creation fees
you plan to test immediately after upgrade:

```bash
soroban contract invoke --id <CONTRACT_ID> -- get_creation_fee_xlm
```

### 2.5 Communicate Downtime Window

Notify stream participants (senders and recipients) of the planned maintenance window.
Because the contract is paused during migration, no withdrawals are possible. Aim for
a window of no more than 30 minutes.

---

## 3. Storage Key Changes

The following storage keys are new in v2. They do not conflict with v1 keys.

| Key pattern | Storage type | Purpose |
|-------------|-------------|---------|
| `("fe", Address)` | Persistent | Fee-exempt flag per address |
| `("bl", Address)` | Persistent | Blocklist flag per address |
| `("rl_w",)` | Instance | Rate-limit window length (seconds) |
| `("rl_m",)` | Instance | Rate-limit max creations per window |
| `("rl", Address)` | Persistent | Per-sender rate-limit state (count + window_start) |
| `("ep_w",)` | Instance | Expiry-warning window (ledgers) |
| `("ns_cap",)` | Instance | New-sender stream cap |
| `("s_prom",)` | Instance | Sender promotion threshold |
| `("gp_l",)` | Instance | Grace-period ledger count |
| `("mf_start",)` | Instance | Max future start-time offset |

### Fields Back-Filled by `migrate_storage`

For every existing `Stream` entry (keyed by `stream_id: u64`), `migrate_storage` will
write a new XDR blob that includes the v2 fields with safe zero/default values:

| Field | Default applied |
|-------|----------------|
| `curve` | `VestingCurve::Linear` |
| `withdrawal_steps` | `None` |
| `min_withdrawal_amount` | `None` |
| `non_transferable` | `false` |
| `requires_recipient_approval` | `false` |
| `approval_timestamp` | `0` |
| `sender_locked` | `false` |
| `oracle` | `None` |
| `max_price_deviation_bps` | `0` |
| `creation_price` | `0` |

These defaults preserve the original stream semantics — existing streams continue to
behave as linear, freely-withdrawable streams with no oracle checks.

---

## 4. Step-by-Step Migration Instructions

### Step 1 — Build and Verify the v2 Wasm

```bash
cd contracts/stream
cargo build --target wasm32-unknown-unknown --release
ls -lh ../../target/wasm32-unknown-unknown/release/sorostream_stream.wasm
```

Record the SHA-256 hash:

```bash
sha256sum ../../target/wasm32-unknown-unknown/release/sorostream_stream.wasm
```

Verify this hash against the value published in the v2 release notes.

### Step 2 — Upload the New Wasm

```bash
soroban contract upload \
  --wasm ../../target/wasm32-unknown-unknown/release/sorostream_stream.wasm \
  --source <ADMIN_KEY_ALIAS> \
  --network mainnet
# note the returned wasm hash
WASM_HASH=<returned-hash>
```

### Step 3 — Deploy the Upgrade

Use Soroban's `upgrade` host function via the contract's `upgrade` entry point (if
exposed) or directly through the CLI:

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source <ADMIN_KEY_ALIAS> \
  --network mainnet \
  -- upgrade \
  --new_wasm_hash "$WASM_HASH"
```

At this point the contract binary is replaced but the storage schema is still v1.
The contract is still paused — no user-facing calls will succeed yet.

### Step 4 — Invoke `migrate_storage`

`migrate_storage` iterates over all stream entries, deserialises each one under the v1
schema, and re-serialises it under the v2 schema with defaults applied. If a stream is
already at the v2 schema (detected by a migration-applied flag) it is skipped.

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source <ADMIN_KEY_ALIAS> \
  --network mainnet \
  -- migrate_storage \
  --from_version "1.0.0" \
  --to_version "2.0.0"
```

Expected output: `ContractMigrated` event emitted; return value `Ok(())`.

> **Batch size note.** If you have more than ~500 streams, `migrate_storage` may
> hit the 100 M instruction budget in a single transaction. The function accepts an
> optional `batch_size` parameter (default 200). Call it repeatedly until it returns
> `MigrationAlreadyApplied`:
>
> ```bash
> soroban contract invoke ... -- migrate_storage \
>   --from_version "1.0.0" --to_version "2.0.0" --batch_size 100
> # repeat until MigrationAlreadyApplied
> ```

### Step 5 — Resume the Contract

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source <ADMIN_KEY_ALIAS> \
  --network mainnet \
  -- resume_contract
```

Confirm:

```bash
soroban contract invoke --id <CONTRACT_ID> -- is_paused
# expected: false
```

---

## 5. Post-Upgrade Verification

Run these checks immediately after resuming the contract. If any check fails, proceed
to the rollback procedure.

### 5.1 Verify Stream Count

```bash
soroban contract invoke --id <CONTRACT_ID> -- get_stats
```

The `total_streams` field must match the count from your v1 snapshot.

### 5.2 Sample `get_claimable` Calls

Pick 3–5 stream IDs that were active at the time of the snapshot and confirm the
claimable amounts are sensible (non-zero, non-negative):

```bash
for STREAM_ID in 1 42 100 337 1000; do
  soroban contract invoke --id <CONTRACT_ID> -- get_claimable --stream_id $STREAM_ID
done
```

All calls should succeed (`Ok` variant). A `StreamNotFound` or deserialisation error
indicates the migration did not back-fill that entry.

### 5.3 Verify New Default Field Values

Query a known stream and confirm the new fields carry their expected defaults:

```bash
soroban contract invoke --id <CONTRACT_ID> -- get_stream --stream_id 1
# check: curve=Linear, non_transferable=false, requires_recipient_approval=false
```

### 5.4 Test a Live Withdrawal

Ask a recipient of a known active stream to execute a withdrawal in a testnet dry-run
(or with a minimal live stream you control) and confirm the `StreamWithdrawn` event is
emitted without error.

### 5.5 Verify Admin Functions

```bash
soroban contract invoke --id <CONTRACT_ID> -- read_admin
# should return the same admin address as before the upgrade
```

---

## 6. Rollback Procedure

If a critical issue is discovered after upgrade, you can redeploy the v1 wasm. Note
that any streams created between the v2 deployment and the rollback will use the v2
schema and will **not** be readable by the v1 binary — this is why the contract must
remain paused until post-upgrade verification is complete.

### 6.1 Keep Contract Paused

Do not resume the contract if verification fails. The pause is your safety net.

### 6.2 Re-upload v1 Wasm

```bash
V1_WASM=path/to/v1/sorostream_stream.wasm
soroban contract upload \
  --wasm "$V1_WASM" \
  --source <ADMIN_KEY_ALIAS> \
  --network mainnet
V1_WASM_HASH=<returned-hash>
```

### 6.3 Redeploy v1 Binary

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source <ADMIN_KEY_ALIAS> \
  --network mainnet \
  -- upgrade \
  --new_wasm_hash "$V1_WASM_HASH"
```

### 6.4 Clear Migration Flag

The migration flag stored under the `"mig"` instance key must be cleared so that
`migrate_storage` can be re-run after a future re-attempt:

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source <ADMIN_KEY_ALIAS> \
  --network mainnet \
  -- reset_migration_flag
```

### 6.5 Resume on v1

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source <ADMIN_KEY_ALIAS> \
  --network mainnet \
  -- resume_contract
```

Streams that were active before the upgrade attempt are now readable again under v1.
Investigate and fix the root cause before attempting a second upgrade.

---

## Support

For questions or to report issues with this migration guide, open a GitHub issue in the
[SoroStream/sorostream-contracts](https://github.com/SoroStream/sorostream-contracts)
repository or reach out in the project Discord.
