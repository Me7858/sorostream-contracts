# Security

## Formally Verified Properties

The vesting arithmetic in [`contracts/stream/src/vesting_math.rs`](contracts/stream/src/vesting_math.rs) is formally verified using [Kani](https://model-checking.github.io/kani/), a Rust model checker backed by CBMC. Verification runs automatically in CI and any invariant violation fails the build.

### Verified Invariants

| # | Property | Function | Description |
|---|----------|----------|-------------|
| 1 | **claimable ≤ deposit** | `compute_claimable` | The claimable amount at any point in time never exceeds the original deposit, regardless of when the query occurs. |
| 1b | **claimable ≤ deposit (post-withdrawal)** | `compute_claimable` | After partial withdrawals (last_withdraw_time advanced), the claimable amount still cannot exceed the deposit. |
| 2 | **claimable is non-decreasing** | `compute_claimable` | For any two timestamps t₁ ≤ t₂ with identical stream parameters, claimable(t₂) ≥ claimable(t₁). The recipient's entitlement never decreases over time. |
| 3 | **claimable = 0 before cliff** | `compute_claimable` | For any timestamp strictly before `cliff_time`, the claimable amount is exactly zero. |
| 4 | **balance conservation on cancel** | `compute_total_streamed`, `compute_refund` | total_streamed + refund = deposit. No tokens are created or destroyed during cancellation. |
| 5 | **earned ≥ 0** | `compute_earned` | The earned amount is always non-negative. |
| 6 | **refund ≥ 0** | `compute_refund` | The refund amount is always non-negative. |

### Proof Bounds

Kani proofs use bounded symbolic inputs covering realistic Stellar parameters:
- **Deposit**: 1 to 1,000,000,000,000 stroops (up to 100,000 XLM equivalent)
- **Duration**: 1 to 315,360,000 seconds (up to 10 years)
- **Flow rate**: 1 to 1,000,000,000 stroops/second

### How It Works

The contract's vesting arithmetic is extracted into pure functions in `vesting_math.rs` with zero Soroban dependencies. The contract calls these functions directly, ensuring the verified code is the code that runs on-chain. Kani generates symbolic inputs covering all values within the bounds and uses SAT/SMT solving to exhaustively check each assertion.

## Additional Verification

### Property-Based Testing (proptest)

[`contracts/stream/src/proptest_tests.rs`](contracts/stream/src/proptest_tests.rs) uses the `proptest` crate to verify invariants at the contract level (with full Soroban VM execution) across 10,000+ random inputs per property:

- **Balance conservation**: create, cancel, and top-up preserve total token supply
- **Monotonic withdrawal**: recipient balance only increases
- **State machine validity**: pause/resume transitions are correct
- **Field correctness**: stream parameters match inputs

### Differential Fuzzing

[`contracts/stream/src/differential_fuzz.rs`](contracts/stream/src/differential_fuzz.rs) runs 1,000,000 iterations comparing the contract's vesting math against an independent reference ("SDK") implementation. Any divergence greater than 1 stroop fails the test, along with the seed and parameters for reproducibility.

### Cross-Contract Integration Tests

[`contracts/stream/src/integration_tests.rs`](contracts/stream/src/integration_tests.rs) deploys SoroStream alongside a real SAC token contract and tests the full lifecycle:

- mint → create stream → withdraw → verify balances
- Treasury/fee integration with batch withdrawals
- Auto-renewal with real token transfers
- Partial cancellation balance conservation

---

## Admin Trust Assumptions

The SoroStream contract has a privileged **admin** role. This section documents every instruction that requires admin authorisation, what it does, what happens if the admin key is compromised, and the planned mitigations.

> **Current status:** The admin is a single externally owned account (EOA). Multi-sig and DAO governance are planned mitigations (see Roadmap below).

### Admin-Gated Instructions

| Instruction | Effect | Blast Radius if Abused |
|-------------|--------|------------------------|
| `set_admin` | Transfers the admin role to a new address | Permanent loss of admin control to an attacker |
| `emergency_pause` | Pauses all user-facing instructions (`create_stream`, `withdraw`, `cancel_stream`, etc.) | Freezes all user activity; auto-expires after `MAX_PAUSE_DURATION` |
| `emergency_resume` | Lifts an active pause before the auto-expiry | Resumes a pause the admin may have intended to hold |
| `upgrade` | Replaces the contract WASM bytecode immediately | Can introduce arbitrary logic; affects all funds under management |
| `migrate` | Runs a named migration step post-upgrade | Can alter storage layout; replay-guarded by applied-migration registry |
| `propose_fee_change` | Proposes a new protocol fee (basis points); starts a **7-day timelock** | No immediate effect; users have 7 days to cancel streams if the fee is unacceptable |
| `execute_fee_change` | Applies a timelocked fee proposal after the 7-day window | Increases fee up to 100% (10,000 bps) of withdrawals |
| `set_treasury_address` | Redirects accumulated protocol fees to a new address | Future fee income diverted; does not affect already-collected funds |
| `set_max_streams` | Sets the global cap on active streams | Can set cap to 0, preventing any new streams from being created |
| `set_sender_stream_limit` | Sets a per-sender stream count cap | Can set a sender's cap to 0, blocking that sender specifically |
| `set_min_duration` | Sets the minimum allowed stream duration in seconds | Can be raised arbitrarily, preventing short-term streams |
| `set_withdrawal_cooldown` | Sets a minimum time between consecutive withdrawals per stream | Can be raised to delay recipient withdrawals |
| `set_whitelist_enabled` | Enables or disables the recipient whitelist gate | Enabling the whitelist without pre-approving recipients locks out all withdrawals |
| `add_to_whitelist` | Adds a recipient address to the allowed list | No direct financial risk; controls who can receive streams |
| `remove_from_whitelist` | Removes a recipient from the allowed list | Prevents that recipient from being the target of new streams |
| `add_fee_exempt` | Exempts an address from paying protocol fees | Reduces protocol revenue; can be used to benefit specific actors |
| `remove_fee_exempt` | Removes a fee exemption | Starts charging fees to a previously exempt address |
| `set_guardian` | Sets a secondary address with pause-only capability | A compromised guardian can pause but cannot upgrade or drain funds |
| `set_governance` | Sets the governance contract address that can call `unpause` | Determines who can lift a guardian-initiated pause |
| `set_creation_fee` | Sets the flat XLM creation fee charged per new stream | Can price out small senders; revenue goes to `xlm_token` treasury |
| `set_delegate` | Grants another address permission to call certain instructions on the admin's behalf | Expands the blast radius of a key compromise |

### What a Compromised Admin Can Do

If the admin private key is stolen or the admin address is transferred to a malicious actor, the attacker can:

1. **Immediately upgrade the WASM** — There is no timelock on `upgrade`. An attacker can replace the contract bytecode with code that steals all funds in a single transaction.
2. **Redirect fee revenue** — `set_treasury_address` takes effect immediately. All future fee income flows to the attacker's wallet.
3. **Freeze all activity** — `emergency_pause` with a maximum-duration pause freezes the contract for up to `MAX_PAUSE_DURATION` seconds. After expiry the contract auto-unpauses, so this cannot be permanent.
4. **Manipulate fee rates** — Via `propose_fee_change` + `execute_fee_change` (requires 7-day wait). Users have a 7-day window to notice and exit.
5. **Block specific senders or recipients** — Via `set_sender_stream_limit` (to 0) or `add_to_whitelist` / `remove_from_whitelist` (with whitelist enabled).
6. **Add a delegate** — `set_delegate` can quietly grant a second attacker-controlled address elevated permissions.

### What a Compromised Admin Cannot Do

- **Steal funds from existing active streams** — Stream deposits are locked in the contract. Without a WASM upgrade, there is no instruction that lets the admin transfer a stream's deposit to an arbitrary address.
- **Override recipient or sender authentication** — All stream operations (`withdraw`, `cancel_stream`, etc.) require the caller to be the authorised participant. The admin cannot impersonate them without a WASM upgrade.
- **Bypass the fee-change timelock** — The 7-day timelock on `propose_fee_change` → `execute_fee_change` cannot be bypassed. Even a compromised admin must wait 7 days to raise fees.
- **Permanently pause the contract** — `emergency_pause` sets an expiry at `ts + MAX_PAUSE_DURATION`. After expiry, the contract auto-unpauses via `is_paused_or_auto_unpause`.

### Current Mitigations

- All admin actions emit `AdminAction` events and are written to a circular on-chain audit log (readable via `get_admin_log`). Off-chain monitors can detect unexpected admin activity in real time.
- The fee-change path has a mandatory 7-day timelock, giving users time to react.
- The guardian role (`set_guardian`) allows a secondary key with pause-only capability, reducing the blast radius of the main admin key for operational pauses.
- Auto-pause expiry (`MAX_PAUSE_DURATION`) prevents an indefinite freeze even with a compromised key.

### Governance Roadmap

The following improvements are planned to reduce the trust surface of the admin role:

| Milestone | Description | Status |
|-----------|-------------|--------|
| **Multi-sig admin** | Replace the single-key admin with a 2-of-3 (or N-of-M) multisig using `contracts/multisig/` | Planned |
| **Upgrade timelock** | Require a mandatory delay (e.g. 7 days) before a WASM upgrade takes effect, allowing users to withdraw | Planned |
| **DAO governance** | Transfer admin role to `contracts/governance/` so fee and upgrade proposals require token-holder votes | Planned |
| **Guardian expand** | Fully implement the guardian role with on-chain scope restrictions (pause-only, no upgrade) | In progress |
| **Admin rotation audit** | Require on-chain evidence of multi-sig quorum before `set_admin` can be executed | Planned |

Until multi-sig is deployed, the admin key should be kept in cold storage, and the `set_guardian` key should be kept separately to ensure the admin and guardian cannot both be compromised in a single incident.

For a full analysis of the admin threat vector, see [docs/THREAT_MODEL.md — §5.4 Compromised Admin Key](docs/THREAT_MODEL.md#54-compromised-admin-key).
