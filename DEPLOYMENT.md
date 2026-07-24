# SoroStream Deployment Runbook

This document is the step-by-step runbook for deploying the SoroStream contract suite to Stellar **testnet** and **mainnet**. It covers prerequisites, build, upload, instantiate, initialize, post-deploy verification, and rollback procedures.

---

## Table of Contents

1. [Prerequisites](#1-prerequisites)
2. [Network Reference](#2-network-reference)
3. [Build](#3-build)
4. [Deploy: Stream Contract](#4-deploy-stream-contract)
5. [Deploy: Treasury Contract](#5-deploy-treasury-contract)
6. [Initialize: Stream Contract](#6-initialize-stream-contract)
7. [Initialize: Treasury Contract](#7-initialize-treasury-contract)
8. [Configure Protocol Fees](#8-configure-protocol-fees)
9. [Post-Deploy Verification](#9-post-deploy-verification)
10. [Updating an Existing Deployment (WASM Upgrade)](#10-updating-an-existing-deployment-wasm-upgrade)
11. [Rollback Procedure](#11-rollback-procedure)
12. [CI/CD via GitHub Actions](#12-cicd-via-github-actions)
13. [Deployment Manifest](#13-deployment-manifest)

---

## 1. Prerequisites

### Software

| Tool | Minimum version | Install |
|------|----------------|---------|
| Rust | 1.84+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| `wasm32v1-none` target | — | `rustup target add wasm32v1-none` |
| stellar-cli | latest | `cargo install --locked stellar-cli --features opt` |

Verify your tools:
```bash
rustc --version         # rustc 1.84.x
stellar --version       # stellar 22.x.x
```

### Funded Deployer Account

You need a Stellar account with sufficient XLM to pay transaction fees and storage rent.

**Testnet (generate and fund automatically):**
```bash
stellar keys generate deployer --network testnet
stellar keys fund deployer --network testnet
```

**Mainnet:** Fund an existing account via an exchange or transfer. The deployer key should be stored in the `MAINNET_DEPLOYER_SECRET_KEY` GitHub Actions secret.

Check balance:
```bash
stellar account show deployer --network testnet
```

---

## 2. Network Reference

| Parameter | Testnet | Mainnet |
|-----------|---------|---------|
| Network passphrase | `Test SDF Network ; September 2015` | `Public Global Stellar Network ; September 2015` |
| RPC URL | `https://soroban-testnet.stellar.org` | `https://soroban.stellar.org` |
| Horizon URL | `https://horizon-testnet.stellar.org` | `https://horizon.stellar.org` |
| stellar-cli `--network` flag | `testnet` | (use `--rpc-url` and `--network-passphrase` directly) |

**Set up named network config (optional shortcut):**
```bash
stellar network add mainnet \
  --rpc-url https://soroban.stellar.org \
  --network-passphrase "Public Global Stellar Network ; September 2015"
```

---

## 3. Build

Build the WASM binaries for all contracts. This must be done before uploading.

```bash
# Standard release build
cargo build --target wasm32v1-none --release

# Size-optimised build (used in production deployments)
cargo build --target wasm32v1-none --profile release-size
```

Output artifacts:
```
target/wasm32v1-none/release/sorostream_stream.wasm
target/wasm32v1-none/release/sorostream_treasury.wasm
target/wasm32v1-none/release/sorostream_proxy.wasm
target/wasm32v1-none/release/sorostream_governance.wasm
target/wasm32v1-none/release/sorostream_multisig.wasm
```

Check WASM sizes:
```bash
wc -c target/wasm32v1-none/release/sorostream_stream.wasm
```

---

## 4. Deploy: Stream Contract

`stellar contract deploy` uploads the WASM and instantiates the contract in one step, returning the contract address.

### Testnet

```bash
STREAM_CONTRACT_ID=$(stellar contract deploy \
  --wasm target/wasm32v1-none/release/sorostream_stream.wasm \
  --source deployer \
  --network testnet)

echo "Stream contract: $STREAM_CONTRACT_ID"
```

### Mainnet

```bash
STREAM_CONTRACT_ID=$(stellar contract deploy \
  --wasm target/wasm32v1-none/release/sorostream_stream.wasm \
  --source "$MAINNET_DEPLOYER_SECRET_KEY" \
  --rpc-url https://soroban.stellar.org \
  --network-passphrase "Public Global Stellar Network ; September 2015")

echo "Stream contract: $STREAM_CONTRACT_ID"
```

Save the contract ID immediately — it cannot be recovered from the WASM hash alone.

---

## 5. Deploy: Treasury Contract

```bash
# Testnet
TREASURY_CONTRACT_ID=$(stellar contract deploy \
  --wasm target/wasm32v1-none/release/sorostream_treasury.wasm \
  --source deployer \
  --network testnet)

echo "Treasury contract: $TREASURY_CONTRACT_ID"
```

> Repeat the mainnet variant from step 4, substituting the treasury WASM path.

---

## 6. Initialize: Stream Contract

`initialize` must be called **exactly once** after deploy. It sets the admin address and version string.

```bash
stellar contract invoke \
  --id "$STREAM_CONTRACT_ID" \
  --source deployer \
  --network testnet \
  -- initialize \
  --admin "$ADMIN_ADDRESS" \
  --version "1.0.0"
```

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `admin` | `Address` | The Stellar address that will hold the admin role |
| `version` | `String` | Semantic version string (e.g. `"1.0.0"`) |

**Expected response:** `Ok(())` (or empty output)

**Error to watch for:** `AlreadyInitialized` (code 9) — means `initialize` was already called. Do **not** call it again.

**Verify:**
```bash
stellar contract invoke \
  --id "$STREAM_CONTRACT_ID" \
  --network testnet \
  -- get_version
# expected: "1.0.0"

stellar contract invoke \
  --id "$STREAM_CONTRACT_ID" \
  --network testnet \
  -- get_admin
# expected: your $ADMIN_ADDRESS
```

---

## 7. Initialize: Treasury Contract

```bash
stellar contract invoke \
  --id "$TREASURY_CONTRACT_ID" \
  --source deployer \
  --network testnet \
  -- initialize \
  --admin "$ADMIN_ADDRESS"
```

**Verify:**
```bash
stellar contract invoke \
  --id "$TREASURY_CONTRACT_ID" \
  --network testnet \
  -- get_admin
# expected: your $ADMIN_ADDRESS
```

---

## 8. Configure Protocol Fees

These steps are optional at deploy time but required before streams generate fee revenue.

### 8.1 Link the treasury to the stream contract

```bash
stellar contract invoke \
  --id "$STREAM_CONTRACT_ID" \
  --source "$ADMIN_KEY" \
  --network testnet \
  -- set_treasury_address \
  --treasury "$TREASURY_CONTRACT_ID"
```

### 8.2 Set the protocol fee (basis points)

Use the timelock flow for production. For a fresh testnet deployment you can use `set_protocol_fee` directly (no timelock).

```bash
# Direct set (testnet / initial setup only — no timelock)
stellar contract invoke \
  --id "$STREAM_CONTRACT_ID" \
  --source "$ADMIN_KEY" \
  --network testnet \
  -- set_protocol_fee \
  --fee_bps 50

# Timelocked flow (recommended for mainnet)
stellar contract invoke \
  --id "$STREAM_CONTRACT_ID" \
  --source "$ADMIN_KEY" \
  --network mainnet \
  -- propose_fee_change \
  --admin "$ADMIN_ADDRESS" \
  --new_fee_bps 50
# ... wait 7 days ...
stellar contract invoke \
  --id "$STREAM_CONTRACT_ID" \
  --network mainnet \
  -- execute_fee_change
```

### 8.3 Set the XLM creation fee (optional)

```bash
stellar contract invoke \
  --id "$STREAM_CONTRACT_ID" \
  --source "$ADMIN_KEY" \
  --network testnet \
  -- set_creation_fee \
  --fee 5000000 \
  --xlm_token "$XLM_SAC_ADDRESS"
# 5,000,000 stroops = 0.5 XLM
```

### 8.4 Configure treasury LP split (optional)

```bash
# 70% treasury, 30% LP pool
stellar contract invoke \
  --id "$TREASURY_CONTRACT_ID" \
  --source "$ADMIN_KEY" \
  --network testnet \
  -- set_lp_pool \
  --lp_pool "$LP_POOL_ADDRESS"

stellar contract invoke \
  --id "$TREASURY_CONTRACT_ID" \
  --source "$ADMIN_KEY" \
  --network testnet \
  -- set_treasury_split \
  --treasury_bps 7000
```

---

## 9. Post-Deploy Verification

Run all checks before announcing the deployment.

```bash
# 1. Contract version
stellar contract invoke \
  --id "$STREAM_CONTRACT_ID" \
  --network testnet \
  -- get_version
# expected: "1.0.0"

# 2. Admin address
stellar contract invoke \
  --id "$STREAM_CONTRACT_ID" \
  --network testnet \
  -- get_admin
# expected: $ADMIN_ADDRESS

# 3. Paused state (must be false)
stellar contract invoke \
  --id "$STREAM_CONTRACT_ID" \
  --network testnet \
  -- is_paused
# expected: false

# 4. Fee configuration
stellar contract invoke \
  --id "$STREAM_CONTRACT_ID" \
  --network testnet \
  -- get_protocol_fee_info
# expected: (fee_bps, Some($TREASURY_CONTRACT_ID))

# 5. Stats (should show 0 streams initially)
stellar contract invoke \
  --id "$STREAM_CONTRACT_ID" \
  --network testnet \
  -- get_stats
# expected: Stats { total_streams: 0, active_streams: 0, total_volume: 0 }

# 6. Smoke test: create a minimal stream and withdraw
stellar contract invoke \
  --id "$STREAM_CONTRACT_ID" \
  --source deployer \
  --network testnet \
  -- create_stream \
  --sender "$DEPLOYER_ADDRESS" \
  --recipient "$RECIPIENT_ADDRESS" \
  --token "$USDC_ADDRESS" \
  --amount 1000 \
  --duration_seconds 3600 \
  --cliff_seconds 0 \
  --nonce 0 \
  --auto_renew false \
  --lock_until 0 \
  --allow_recipient_termination false
```

Record the returned `stream_id` and confirm a `StreamCreated` event appears in Horizon:

```bash
curl "https://horizon-testnet.stellar.org/events?\
contract_id=$STREAM_CONTRACT_ID&limit=5&order=desc"
```

---

## 10. Updating an Existing Deployment (WASM Upgrade)

A WASM upgrade replaces the contract bytecode without changing the contract address. All storage is preserved.

### Step-by-step

```bash
# 1. Build the new WASM
cargo build --target wasm32v1-none --profile release-size

# 2. Upload the new WASM and get its hash
NEW_WASM_HASH=$(stellar contract upload \
  --wasm target/wasm32v1-none/release/sorostream_stream.wasm \
  --source "$ADMIN_KEY" \
  --network testnet)

echo "New WASM hash: $NEW_WASM_HASH"

# 3. Call upgrade() on the existing contract (admin-gated)
stellar contract invoke \
  --id "$STREAM_CONTRACT_ID" \
  --source "$ADMIN_KEY" \
  --network testnet \
  -- upgrade \
  --new_wasm_hash "$NEW_WASM_HASH"

# 4. Run the migration step (if applicable)
stellar contract invoke \
  --id "$STREAM_CONTRACT_ID" \
  --source "$ADMIN_KEY" \
  --network testnet \
  -- migrate \
  --from_version "1.0.0" \
  --to_version "1.1.0"

# 5. Verify the new version
stellar contract invoke \
  --id "$STREAM_CONTRACT_ID" \
  --network testnet \
  -- get_version
# expected: "1.1.0"
```

> **Note:** `migrate` is idempotent — calling it again for the same `to_version` returns `MigrationAlreadyApplied` (code 26) without side effects.

---

## 11. Rollback Procedure

Soroban WASM upgrades are **irreversible at the ledger level** — once a new WASM is uploaded and `upgrade` is called, the old bytecode is no longer referenced by the contract. However, you can restore previous behaviour by re-deploying the old WASM.

### Strategy A — Re-upload old WASM

If you have the old `sorostream_stream.wasm` artifact:

```bash
# 1. Upload the previous WASM binary
OLD_WASM_HASH=$(stellar contract upload \
  --wasm path/to/old/sorostream_stream.wasm \
  --source "$ADMIN_KEY" \
  --network testnet)

# 2. Call upgrade() with the old hash — this downgrades the bytecode
stellar contract invoke \
  --id "$STREAM_CONTRACT_ID" \
  --source "$ADMIN_KEY" \
  --network testnet \
  -- upgrade \
  --new_wasm_hash "$OLD_WASM_HASH"

# 3. Update the version string to match
stellar contract invoke \
  --id "$STREAM_CONTRACT_ID" \
  --source "$ADMIN_KEY" \
  --network testnet \
  -- set_admin \
  --new_admin "$ADMIN_ADDRESS"
# (admin call to force an audit log entry; version is managed by migrate)
```

> Keep WASM artifacts from every successful deployment in a version-controlled store (e.g., GitHub releases or an S3 bucket).

### Strategy B — Emergency pause while diagnosing

If a bad upgrade is live but you are not yet ready to roll back:

```bash
# Pause immediately to stop all stream activity
stellar contract invoke \
  --id "$STREAM_CONTRACT_ID" \
  --source "$ADMIN_KEY" \
  --network testnet \
  -- emergency_pause

# Confirm paused
stellar contract invoke \
  --id "$STREAM_CONTRACT_ID" \
  --network testnet \
  -- is_paused
# expected: true

# Auto-unpause fires after 72 hours if not manually resumed.
# When ready to resume:
stellar contract invoke \
  --id "$STREAM_CONTRACT_ID" \
  --source "$ADMIN_KEY" \
  --network testnet \
  -- emergency_resume
```

While paused:
- `withdraw`, `create_stream`, `cancel_stream`, `top_up`, `pause_stream`, and `resume_stream` all return `ContractPaused` (error 14).
- Read-only calls (`get_stream`, `get_claimable`, `get_admin`) continue to work.

### Strategy C — Deploy a fresh contract (last resort)

If storage is corrupted or a critical bug cannot be patched via upgrade:

1. Deploy a new contract address following the full runbook above.
2. Update `deployments/testnet.json` or `deployments/mainnet.json` with the new address.
3. Notify all downstream integrators (SDKs, indexers, front-end) of the new address.
4. Direct users to re-create their streams on the new contract.

> There is no migration path for existing stream storage between two different contract addresses.

---

## 12. CI/CD via GitHub Actions

### Testnet — Automatic on push to `main`

The CI pipeline (`.github/workflows/ci.yml`) builds, lints, and tests on every PR and push. There is no automated testnet deployment; deploy manually following this runbook.

### Mainnet — Manual trigger

The mainnet deployment workflow (`.github/workflows/deploy-mainnet.yml`) is triggered manually via `workflow_dispatch`.

Required secrets:
- `MAINNET_DEPLOYER_SECRET_KEY` — the Stellar secret key for the deployer account.

Steps performed by the workflow:
1. Check out code.
2. Build `wasm32v1-none` release WASM.
3. Install `stellar-cli`.
4. Run `stellar contract deploy` and capture the contract address.
5. Write the address to `deployments/mainnet.json` and auto-commit.

**Trigger from GitHub UI:**
1. Navigate to **Actions → Deploy Mainnet**.
2. Click **Run workflow**.
3. Confirm the `network_passphrase` input (pre-filled with the mainnet value).
4. Click **Run workflow**.

After completion, `initialize` and fee configuration must still be run manually.

---

## 13. Deployment Manifest

Contract addresses are tracked in version-controlled JSON files.

`deployments/testnet.json`:
```json
{
  "StreamContract": "CAM753QTDMNRWJ7XI5B77QUEQBTI2FTOAWQJHWMFFHO54R36AFUUVR72"
}
```

`deployments/mainnet.json`:
```json
{
  "StreamContract": ""
}
```

After a successful deployment, commit the updated manifest:
```bash
git add deployments/
git commit -m "chore(deploy): update testnet contract address"
git push
```

---

## Quick-Reference Checklist

### Testnet deploy
- [ ] `cargo build --target wasm32v1-none --release`
- [ ] `stellar contract deploy` → capture `$STREAM_CONTRACT_ID`
- [ ] `stellar contract deploy` (treasury) → capture `$TREASURY_CONTRACT_ID`
- [ ] `initialize` stream contract
- [ ] `initialize` treasury contract
- [ ] `set_treasury_address` on stream contract
- [ ] `set_protocol_fee` (optional)
- [ ] `set_creation_fee` (optional)
- [ ] Run post-deploy verification checks
- [ ] Update `deployments/testnet.json` and commit

### Mainnet deploy (additional steps)
- [ ] Use `--profile release-size` build
- [ ] Store WASM artifact in a versioned archive before deploying
- [ ] Use `propose_fee_change` + `execute_fee_change` (not `set_protocol_fee` directly)
- [ ] Confirm no active streams are running before an upgrade
- [ ] Announce planned maintenance window to users before `emergency_pause`

> Closes [#261](https://github.com/SoroStream/sorostream-contracts/issues/261).
