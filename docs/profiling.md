# SoroStream Contract Profiling Guide

This guide shows how to capture CPU flame graphs for SoroStream contracts using
[`cargo-flamegraph`](https://github.com/flamegraph-rs/flamegraph), interpret them
in the context of Soroban instruction limits, and act on common findings.

---

## Prerequisites

| Tool | Purpose | Install |
|------|---------|---------|
| Rust stable ≥ 1.84 | Build toolchain | `rustup update stable` |
| `cargo-flamegraph` | Capture + render flame graphs | `cargo install flamegraph` |
| `perf` (Linux) | Sampling profiler back-end | `sudo apt-get install linux-perf` |
| `dtrace` (macOS) | Sampling profiler back-end | Bundled with Xcode CLI tools |
| `stellar-cli` | Simulate contract invocations | `cargo install --locked stellar-cli --features opt` |

> **macOS note:** `cargo flamegraph` on macOS uses DTrace. Run with `sudo` or grant
> the required entitlement:
> ```bash
> sudo codesign -s - -f --entitlements entitlements.plist $(which cargo-flamegraph)
> ```

---

## 1. Build the Contract in Profiling Mode

Flame graphs require debug symbols. Add a dedicated Cargo profile that preserves them
while keeping optimisations close to the release build:

```toml
# Cargo.toml (workspace root)
[profile.profiling]
inherits = "release"
debug = true     # keep full debug info
strip = "none"   # do not strip symbols
```

Build the native test harness (the flame graph is captured against it, not the WASM):

```bash
# Build the contract WASM for size checks (optional)
stellar contract build --profile profiling

# Build the native test binary with debug symbols
cargo build --profile profiling --tests -p sorostream-stream
```

---

## 2. Write a Representative Benchmark Workload

Create or use a test that exercises the hot path. The example below stresses
`create_stream`, `withdraw`, and `cancel_stream` in a loop — the three most
instruction-heavy operations.

```rust
// contracts/stream/src/cost_bench.rs  (or any #[cfg(test)] module)

#[test]
fn bench_create_withdraw_cancel_loop() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SoroStreamContract, ());
    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let sender    = Address::generate(&env);
    let recipient = Address::generate(&env);

    StellarAssetClient::new(&env, &token_id).mint(&sender, &100_000_000_000i128);

    let c = SoroStreamContractClient::new(&env, &contract_id);
    c.set_min_duration(&sender, &0u64);

    // 500 iterations gives the profiler enough samples to be statistically meaningful.
    for i in 0u64..500 {
        env.ledger().set_timestamp(i * 2000);

        let stream_id = c.create_stream(
            &sender, &recipient, &token_id,
            &1_000_000i128, // deposit
            &1000u64,       // duration seconds
            &0u64,          // cliff_offset
            &i,             // nonce (unique per iteration)
            &false,         // auto_renew
            &0u64,          // lock_until
            &false,         // allow_recipient_termination
            &0i128,         // holdback_amount
        );

        env.ledger().set_timestamp(i * 2000 + 500); // mid-stream
        c.withdraw(&stream_id, &recipient);

        env.ledger().set_timestamp(i * 2000 + 999);
        c.cancel_stream(&stream_id, &sender);
    }
}
```

---

## 3. Capture a Flame Graph

### Linux (perf)

```bash
CARGO_PROFILE_PROFILING_DEBUG=true \
cargo flamegraph \
  --profile profiling \
  --test sorostream_stream \
  --root \
  -- bench_create_withdraw_cancel_loop --nocapture
```

If `--root` prompts for a password:

```bash
sudo -E cargo flamegraph \
  --profile profiling \
  --test sorostream_stream \
  -- bench_create_withdraw_cancel_loop --nocapture
```

### macOS (DTrace)

```bash
cargo flamegraph \
  --profile profiling \
  --test sorostream_stream \
  -- bench_create_withdraw_cancel_loop --nocapture
```

### Increase sampling frequency for short-running functions

```bash
CARGO_FLAMEGRAPH_FREQUENCY=2000 \
cargo flamegraph \
  --profile profiling \
  --test sorostream_stream \
  -- bench_create_withdraw_cancel_loop
```

`--frequency 2000` samples at 2 kHz instead of the default 997 Hz.

Output is written to `flamegraph.svg` in the current directory.

---

## 4. Reading the Flame Graph

Open `flamegraph.svg` in any modern browser. The SVG is interactive — click a frame
to zoom in, press `Ctrl+F` to search by function name.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    bench_create_withdraw_cancel_loop                          │  ← root
├──────────────────────────┬─────────────────────────────┬─────────────────────┤
│   create_stream  (42 %)  │     withdraw  (38 %)        │  cancel_stream (20%)│
├──────┬───────────────────┼──────────────────┬──────────┤                     │
│ XDR  │  sha256 + storage │ claimable_calc   │ token::  │                     │
│ ser  │  index writes     │ (flow_rate math) │ transfer │                     │
│(12%)│      (18 %)       │     (8 %)        │  (22 %)  │                     │
└──────┴───────────────────┴──────────────────┴──────────┴─────────────────────┘
```

**Annotated observations from `bench_create_withdraw_cancel_loop`:**

| Frame | Typical width | What it means |
|-------|---------------|---------------|
| `derive_stream_id` → `sha256` | ~8 % | SHA-256 of sender+recipient XDR on every `create_stream`. |
| `index_global_stream` → `persistent().set` | ~10 % | Two persistent write slots per `create_stream`. |
| `load_stream` → `persistent().get` | ~6 % | Read-modify-write on every `withdraw` and `cancel_stream`. |
| `token::Client::transfer` | ~22 % | Cross-contract SAC call — the single largest contributor. |
| `Address::to_xdr` | ~7 % | XDR serialisation for stream-ID derivation and index keys. |

---

## 5. Relating Flame Graph Width to Soroban Instruction Limits

Flame graph width is proportional to wall-clock CPU samples, not instruction counts
directly. To measure actual on-chain instruction cost, use the budget API:

```rust
let budget = env.cost_estimate().budget();
println!("CPU instructions: {}", budget.cpu_insns_consumed());
println!("Memory bytes:     {}", budget.mem_bytes_consumed());
```

Soroban Protocol 22 per-transaction limits:

| Resource | Limit |
|----------|-------|
| CPU instructions | 100,000,000 |
| Memory | 41,943,040 bytes (40 MB) |
| Read ledger entries | 40 |
| Write ledger entries | 25 |
| Read bytes | 200,000 |
| Write bytes | 66,560 |

Typical SoroStream operation costs (measured via `cost_estimate`):

| Operation | CPU instructions | Write entries |
|-----------|-----------------|---------------|
| `create_stream` | ~3–5 M | 2 (stream + global index) |
| `withdraw` (mid-stream) | ~12–18 M | 1 (stream update) + SAC transfer |
| `cancel_stream` | ~10–14 M | 1 (stream remove) + SAC transfers |

---

## 6. Common Optimization Patterns in Soroban Contracts

### 6.1 Reduce Cross-Contract Calls

Each `token::Client::transfer` is a full cross-contract invocation. Batch fee
accounting in-contract and settle in a single transfer per withdrawal:

```rust
// Costly: two separate SAC calls
token_client.transfer(&contract, &recipient, &net_amount);
token_client.transfer(&contract, &treasury, &fee_amount);

// Better: accumulate fee locally, one SAC call per withdrawal
accumulate_fees(env, fee_amount);
token_client.transfer(&contract, &recipient, &net_amount);
// drain fees periodically via a separate treasury instruction
```

### 6.2 Use Instance Storage for Hot Read-Only Data

Instance storage is loaded once per transaction and is significantly cheaper than
persistent storage for repeated reads within the same call:

```rust
// Cheap — loaded once per tx
env.storage().instance().get(&Symbol::new(env, "fee_bps"))

// Expensive — separate ledger entry lookup
env.storage().persistent().get(&stream_id)
```

Move frequently-read config (fee rate, pause flag, admin address) to instance storage.

### 6.3 Cache XDR Bytes Across Batch Operations

`address.to_xdr(env)` allocates a full serialised XDR blob on each call. In
`batch_create_stream`, the sender XDR is identical for every stream — compute it once:

```rust
let sender_xdr = sender.to_xdr(env);  // once
for (recipient, amount, duration, nonce) in batch {
    let id = derive_stream_id_from_bytes(&sender_xdr, &recipient.to_xdr(env), ...);
    ...
}
```

### 6.4 Early Return on Zero-Claimable Withdrawals

Before running vesting math and issuing a storage write + token transfer, check
whether anything is actually claimable:

```rust
let claimable = compute_claimable(env, &stream);
if claimable == 0 {
    return Ok(()); // avoids storage write and token SAC call entirely
}
```

### 6.5 Pack Co-Read Values into a Single Storage Key

Each distinct persistent key is a separate ledger entry read. Co-locate values that
are always read together under a single key to reduce entry reads per call:

```rust
// 2 ledger entry reads:
let fee_bps: u32 = instance_get("fee_bps");
let treasury: Address = instance_get("treasury");

// 1 ledger entry read:
let (fee_bps, treasury): (u32, Address) = instance_get("fee_cfg");
```

### 6.6 Iterate Tranches from the Cursor, Not from Index 0

The `tranches_claimed` cursor already tracks which tranches have been paid. Start the
iteration there rather than scanning from the beginning of the `Vec`:

```rust
// Good: O(remaining tranches)
for i in stream.tranches_claimed..tranches.len() {
    let tranche = tranches.get(i).unwrap();
    if now < tranche.unlock_time { break; }
    // ... process tranche
}

// Wasteful: O(all tranches)
for tranche in tranches.iter() { ... }
```

---

## 7. Continuous Profiling in CI

Add a step that uploads the flame graph as a CI artifact on every push to `main`:

```yaml
# .github/workflows/ci.yml
- name: Build profiling test binary
  run: cargo build --profile profiling --tests -p sorostream-stream

- name: Generate flame graph
  run: |
    cargo flamegraph --profile profiling --test sorostream_stream \
      -- bench_create_withdraw_cancel_loop --nocapture
  continue-on-error: true   # don't fail CI if perf is unavailable in the runner

- name: Upload flame graph artifact
  uses: actions/upload-artifact@v4
  with:
    name: flamegraph-${{ github.sha }}
    path: flamegraph.svg
    retention-days: 30
```

---

## 8. Further Reading

- [Soroban Budget and Fee Model](https://developers.stellar.org/docs/learn/fundamentals/fees-resource-limits-metering)
- [cargo-flamegraph README](https://github.com/flamegraph-rs/flamegraph)
- [Brendan Gregg — Flame Graphs](https://www.brendangregg.com/flamegraphs.html)
- [`docs/cost-benchmarks.md`](./docs/cost-benchmarks.md) — existing SoroStream cost baselines
- [Soroban SDK `cost_estimate` API](https://docs.rs/soroban-sdk/latest/soroban_sdk/struct.Env.html)
