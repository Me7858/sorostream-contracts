# SoroStream Threat Model

**Version:** 1.0.0  
**Last updated:** 2026-07-24  
**Contract version:** see `get_version()`  
**Status:** Living document — update whenever the attack surface changes.

---

## Table of Contents

1. [Scope](#1-scope)
2. [Assets Under Protection](#2-assets-under-protection)
3. [Trust Boundaries](#3-trust-boundaries)
4. [Actors and Trust Levels](#4-actors-and-trust-levels)
5. [Attack Vectors and Mitigations](#5-attack-vectors-and-mitigations)
   - 5.1 [Arithmetic and Overflow](#51-arithmetic-and-overflow)
   - 5.2 [Reentrancy](#52-reentrancy)
   - 5.3 [Storage Exhaustion and DoS](#53-storage-exhaustion-and-dos)
   - 5.4 [Compromised Admin Key](#54-compromised-admin-key)
   - 5.5 [Malicious or Non-Standard Token](#55-malicious-or-non-standard-token)
   - 5.6 [MEV and Transaction Ordering](#56-mev-and-transaction-ordering)
   - 5.7 [Replay and Nonce Attacks](#57-replay-and-nonce-attacks)
   - 5.8 [Frontend and Off-Chain Manipulation](#58-frontend-and-off-chain-manipulation)
   - 5.9 [Oracle and Timestamp Manipulation](#59-oracle-and-timestamp-manipulation)
   - 5.10 [Contract Upgrade Risk](#510-contract-upgrade-risk)
   - 5.11 [Cross-Contract Callback Risk](#511-cross-contract-callback-risk)
6. [Residual Risks](#6-residual-risks)
7. [Governance Roadmap](#7-governance-roadmap)

---

## 1. Scope

This threat model covers the on-chain SoroStream payment-streaming contract deployed on Stellar's Soroban platform (`contracts/stream/`). It does not cover:

- The SoroStream frontend web application
- Off-chain monitoring or alerting infrastructure
- Wallet software used by senders and recipients
- Third-party token issuers
- Stellar core validator behaviour

The adjacent contracts (`contracts/governance/`, `contracts/multisig/`, `contracts/treasury/`, `contracts/proxy/`) are referenced where they interact with the stream contract but are not fully analysed here.

---

## 2. Assets Under Protection

| Asset | Description | Value at Risk |
|-------|-------------|---------------|
| **User funds (stream deposits)** | SAC tokens locked by senders and awaiting withdrawal by recipients | Full deposit amount per active stream |
| **Protocol treasury** | Accumulated protocol fees held by the treasury contract | Sum of all collected fees |
| **Admin key** | The address authorised to pause, upgrade, change fees, and manage the whitelist | Indirect — compromise enables drain of all future streams |
| **Contract WASM** | The bytecode executing on-chain; controls all logic | All funds under management if replaced |
| **Stream state** | Per-stream records (deposit, flow_rate, status, etc.) in persistent storage | Manipulation could enable over-withdrawal |
| **Audit log** | Circular on-chain audit log of admin actions | Integrity needed for incident reconstruction |

---

## 3. Trust Boundaries

```
┌──────────────────────────────────────────────────────────────────┐
│                       Stellar Ledger                             │
│                                                                  │
│  ┌─────────────────┐    ┌──────────────────────────────────┐    │
│  │   SAC Token     │◄───│      SoroStream Contract         │    │
│  │   Contract      │    │  (stream state, vesting math)    │    │
│  │  (trusted:      │    │                                  │    │
│  │   protocol SAC) │    │   ┌─────────────┐               │    │
│  └─────────────────┘    │   │  Treasury   │               │    │
│                          │   │  Contract   │               │    │
│  ┌─────────────────┐    │   └─────────────┘               │    │
│  │   Governance /  │───►│                                  │    │
│  │   Multisig      │    │   ┌─────────────┐               │    │
│  │   Contract      │    │   │  Proxy      │               │    │
│  └─────────────────┘    │   │  Contract   │               │    │
│                          │   └─────────────┘               │    │
│  ┌─────────────────┐    └──────────────────────────────────┘    │
│  │  Admin EOA /    │                                            │
│  │  Multisig       │                                            │
│  └─────────────────┘                                            │
└──────────────────────────────────────────────────────────────────┘
         ▲                      ▲
         │ (untrusted)          │ (untrusted)
  ┌──────┴──────┐        ┌──────┴──────┐
  │  Sender     │        │  Recipient  │
  │  (EOA)      │        │  (EOA or    │
  └─────────────┘        │  contract)  │
                         └─────────────┘
         ▲
  ┌──────┴──────┐
  │  Frontend   │
  │  (untrusted │
  │   relay)    │
  └─────────────┘
```

**Boundary crossings of interest:**

| Boundary | Direction | Risk |
|----------|-----------|------|
| Contract → SAC token | Outbound `transfer` call | Token behaviour must conform to SEP-0041 |
| Contract → Treasury | Outbound `deposit` call | Treasury contract failure should not freeze the stream contract |
| Contract → Recipient (callback) | Outbound `on_stream_withdraw` via `try_invoke` | Must not allow reentrancy |
| Admin EOA → Contract | Admin instructions | Requires strict authorisation check |
| Frontend → Contract | Transaction construction | Frontend can only mislead the user; it cannot forge authorisations |

---

## 4. Actors and Trust Levels

| Actor | Trust Level | Capabilities |
|-------|-------------|-------------|
| **Stellar Protocol / Soroban VM** | Fully trusted | Executes WASM deterministically; enforces auth; manages storage |
| **Admin** | Elevated trust (operator) | `emergency_pause`, `emergency_resume`, `upgrade`, `set_protocol_fee` (via timelock), `set_treasury_address`, `set_admin`, `set_whitelist_enabled`, `add_to_whitelist`, `remove_from_whitelist`, `set_creation_fee`, `set_min_duration`, `set_max_streams`, `set_sender_stream_limit`, `set_guardian`, `set_governance`, `set_withdrawal_cooldown`, `add_fee_exempt`, `remove_fee_exempt`, `set_delegate` |
| **Guardian** | Limited elevated trust | Emergency pause only (subset of admin, not yet fully implemented) |
| **Sender** | User trust (self-service) | `create_stream`, `cancel_stream`, `top_up`, `pause_stream`, `resume_stream`, `partial_cancel_stream`, `cancel_auto_renew`, `update_metadata`, `transfer_recipient`, `batch_create_stream`, `batch_cancel_stream` |
| **Recipient** | User trust (self-service) | `withdraw`, `batch_withdraw`, `recipient_terminate`, `archive_stream` |
| **Anyone** | No trust required | Read-only queries (`get_stream`, `get_claimable`, `get_all_stream_ids`, `get_stats`, `is_paused`, etc.) |
| **Frontend** | Untrusted relay | Can construct transactions but cannot forge authorisation signatures |
| **SAC Token Contract** | Protocol trust | Trusted to implement SEP-0041 correctly; non-SAC tokens are out of scope |

---

## 5. Attack Vectors and Mitigations

### 5.1 Arithmetic and Overflow

**Threat:** Integer overflow in `flow_rate × elapsed` (both `i128`) could allow a recipient to claim more than the deposit.

**Mitigations:**
- All vesting math is extracted into `vesting_math.rs` pure functions.
- Kani model-checking proofs exhaustively verify that `claimable ≤ deposit` for all inputs within realistic Soroban bounds (see `SECURITY.md`).
- All `checked_mul` / `checked_add` calls return `Err(StreamError::Overflow)` on overflow — there are no unchecked arithmetic operations in the vesting path.
- `flow_rate` is truncating integer division (`amount / duration`), so the deposit is never under-collected.

**Residual risk:** Inputs outside Kani's symbolic bounds (duration > 10 years, deposit > 100,000 XLM) are not exhaustively proved. Deposits of this scale should be split across multiple streams.

---

### 5.2 Reentrancy

**Threat:** The `withdraw` function makes outbound token transfers. A malicious recipient contract implementing `on_stream_withdraw` could call `withdraw` again before the first invocation completes, claiming funds twice.

**Mitigations:**
- A reentrancy guard (`is_reentrancy_locked` / `set_reentrancy_lock` / `clear_reentrancy_lock`) blocks any re-entrant call to `withdraw` during settlement.
- All state mutations (balance accounting, status updates) occur in the EFFECTS phase before the INTERACTIONS (token transfers) phase, following the checks-effects-interactions pattern.
- The `on_stream_withdraw` callback is invoked using `env.try_invoke_contract`, which does not propagate panics, preventing the callback from reverting the stream's state.

**Residual risk:** A reentrancy guard in persistent storage persists across transactions. If a transaction panics after setting the lock but before clearing it, the contract will be permanently locked. The guard is cleared in all code paths, including the auto-renew failure path, but thorough review of all exit paths is recommended.

---

### 5.3 Storage Exhaustion and DoS

**Threat:** An attacker could create millions of streams to exhaust Soroban storage limits, inflate index sizes, or drive up storage rent costs for legitimate participants.

**Mitigations:**
- `set_max_streams` caps the global number of active streams.
- `set_sender_stream_limit` caps streams per sender address.
- Stream IDs require a nonce provided by the sender — creating a stream costs at least one ledger operation and one token transfer.
- Paginated query functions (`get_all_stream_ids` with `start`/`limit`) prevent unbounded scans.
- Completed and archived streams are removed from storage.

**Residual risk:** A well-funded attacker could still fill stream slots up to the cap, preventing new streams from being created until the admin increases the cap or existing streams are archived. The global cap should be set conservatively and monitored.

---

### 5.4 Compromised Admin Key

**Threat:** If the admin private key is stolen, an attacker can: pause the contract indefinitely, change the protocol fee to 100%, redirect the treasury to a wallet they control, upgrade the WASM to arbitrary code, or drain treasury funds.

**Mitigations:**
- Protocol fee changes require a 7-day timelock (`propose_fee_change` → `execute_fee_change`), giving users time to cancel streams.
- Auto-pause expiry (`MAX_PAUSE_DURATION`) prevents indefinite pauses from a compromised key — the pause expires after a fixed duration.
- All admin actions are recorded in a circular on-chain audit log and emit `AdminAction` events, enabling rapid detection.
- The guardian address can be set to a second key with pause-only capability, reducing the blast radius of the main admin key.

**Residual risk:** WASM upgrades and treasury redirection are not timelocked. A compromised admin can immediately upgrade to malicious WASM or redirect fees. Multi-sig governance and an upgrade timelock are planned (see Section 7).

See [SECURITY.md — Admin Trust Assumptions](../SECURITY.md#admin-trust-assumptions) for the full admin capability matrix.

---

### 5.5 Malicious or Non-Standard Token

**Threat:** A non-SAC token with fee-on-transfer, rebasing, or upgradeable behaviour could cause the contract to hold less than the recorded `deposit`, making full withdrawal impossible or breaking vesting invariants.

**Mitigations:**
- Vesting math is proved correct for standard `i128` token semantics — the proofs break if token transfers have side effects.
- The admin due diligence process (`docs/ADDING_TOKENS.md`) requires WASM hash verification against the canonical SAC WASM before a token is recommended.
- No on-chain allowlist of approved tokens exists today (streams accept any `Address` as `token`), so the control is procedural rather than technical.

**Residual risk:** Any sender can pass any `Address` as the `token` argument to `create_stream`. A sender could stream a non-SAC token and harm themselves or their recipient. An on-chain token allowlist is a planned mitigation (see Section 7).

---

### 5.6 MEV and Transaction Ordering

**Threat:** On Soroban, validators can theoretically reorder transactions within a ledger close. A recipient's `withdraw` transaction could be front-run or sandwiched.

**Mitigations:**
- `withdraw` delivers `flow_rate × elapsed` tokens where `elapsed` is computed from `env.ledger().timestamp()`. Front-running only changes _when_ the withdrawal is confirmed, not whether funds are stolen.
- The contract does not use AMMs, price oracles, or slippage-sensitive operations, eliminating the main MEV attack surface.

**Residual risk:** Reordering a sender's `cancel_stream` ahead of a recipient's `withdraw` could cause the recipient to receive their earned amount via the cancel path rather than the withdraw path — both are correct, so this is not an exploitable condition.

---

### 5.7 Replay and Nonce Attacks

**Threat:** A valid signed `create_stream` transaction could be replayed to create duplicate streams, double-spending sender funds.

**Mitigations:**
- Every `create_stream` call requires a `nonce` parameter; used nonces are recorded in persistent storage per sender address.
- Duplicate streams are rejected with `DuplicateStream` (error 11).
- Soroban's native transaction sequence numbers also prevent replay at the ledger level.

**Residual risk:** If a sender's nonce space is treated as a global counter rather than a per-intent value, an attacker who observes a pending transaction can submit it on a different network (e.g., testnet) to drain testnet funds. Always use separate keys for testnet and mainnet.

---

### 5.8 Frontend and Off-Chain Manipulation

**Threat:** A compromised frontend could display incorrect stream parameters, trick users into approving higher amounts, or submit transactions to a malicious contract address.

**Mitigations:**
- All authorisation is enforced on-chain — the contract verifies `sender.require_auth()` and `recipient.require_auth()`. A malicious frontend cannot forge signatures.
- Stream parameters are verifiable via `get_stream` queries directly against the contract, independent of the frontend.
- Contract addresses are published in `deployments/testnet.json` and `deployments/mainnet.json` for independent verification.

**Residual risk:** Social engineering (phishing a user into approving a transaction to a different contract) is an off-chain risk that cannot be fully mitigated on-chain. Users should always verify contract addresses before signing.

---

### 5.9 Oracle and Timestamp Manipulation

**Threat:** The streaming rate depends on `env.ledger().timestamp()`. If validators can manipulate the ledger timestamp, a recipient could claim more tokens than intended by making the ledger appear to advance faster.

**Mitigations:**
- Stellar ledger timestamps are set by a supermajority of validators and are bounded to advance monotonically within ±5 seconds of real time. Manipulation requires compromising a quorum of validators.
- Kani proofs verify correctness for any `u64` timestamp — an adversarial timestamp within realistic bounds cannot cause `claimable > deposit`.

**Residual risk:** Short streams with very high flow rates are more sensitive to single-second timestamp rounding. For high-value short streams, senders should use cliff periods to delay the start of vesting.

---

### 5.10 Contract Upgrade Risk

**Threat:** An admin calling `upgrade` can replace the contract WASM with arbitrary code, immediately affecting all funds under management.

**Mitigations:**
- Upgrades require admin authorisation via `check_admin`, which verifies against the stored admin address.
- Applied migrations are recorded in persistent storage (`read_applied_migrations`) to prevent re-application.
- A `ContractMigrated` event and `AdminAction` event are emitted, enabling immediate off-chain detection.

**Residual risk:** There is no timelock on upgrades. A compromised admin key can upgrade immediately. Adding a timelock or requiring multi-sig approval for upgrades is the highest-priority governance improvement (see Section 7).

---

### 5.11 Cross-Contract Callback Risk

**Threat:** The `withdraw` function attempts to call `on_stream_withdraw` on the recipient address after paying out funds. If the recipient is a malicious contract, it could attempt reentrancy, panic to revert state, or consume excessive compute.

**Mitigations:**
- The callback is wrapped in `env.try_invoke_contract`, which catches panics and ignores errors — a failing callback does not revert the withdrawal.
- The reentrancy guard (see §5.2) blocks any re-entrant `withdraw` calls during the callback.
- The callback is invoked _after_ state has been finalised and the stream's `last_withdraw_time` updated, so even a successful re-entrant call would find zero claimable balance.

**Residual risk:** A callback that consumes excessive compute increases transaction fees for the recipient. This is a DoS risk against the recipient themselves, not against other users.

---

## 6. Residual Risks

The following risks are acknowledged and accepted pending the governance improvements described in Section 7:

| Risk | Severity | Likelihood | Status |
|------|----------|-----------|--------|
| Compromised admin key enables immediate WASM upgrade | Critical | Low | Mitigated by audit log; timelock planned |
| Non-SAC token passed to `create_stream` breaks vesting math | High | Low | Procedural control only; on-chain allowlist planned |
| Storage exhaustion via stream spam at global cap | Medium | Low | Rate-limited by token cost; cap is configurable |
| Reentrancy guard stuck in locked state after panic | Medium | Very Low | Cleared in all code paths; requires audit review |
| Sender constructs stream with zero flow rate | Low | Low | `ZeroFlowRate` error (16) prevents this |

---

## 7. Governance Roadmap

The following mitigations are planned but not yet implemented. They are tracked as issues in this repository.

| Mitigation | Target State | Priority |
|------------|-------------|---------|
| **Multi-sig admin** | Replace EOA admin with a 2-of-3 multisig using `contracts/multisig/` | High |
| **Upgrade timelock** | Require a 7-day timelock before any WASM upgrade takes effect | High |
| **On-chain token allowlist** | Restrict `create_stream` to a contract-enforced list of approved SAC tokens | Medium |
| **DAO governance** | Transfer admin role to `contracts/governance/` for community voting on fee and upgrade proposals | Medium |
| **Guardian emergency pause** | Fully implement the guardian role with pause-only capability to reduce admin blast radius | Medium |
| **Formal verification expansion** | Extend Kani proofs to cover streaming edge cases above current symbolic bounds | Low |

---

## References

- [SoroStream SECURITY.md](../SECURITY.md)
- [SoroStream Contract Reference](./contract-reference.md)
- [SoroStream Events Schema](./events.md)
- [SEP-0041 Token Interface](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0041.md)
- [Soroban Auth Model](https://developers.stellar.org/docs/build/smart-contracts/example-contracts/auth)
- [Kani Model Checker](https://model-checking.github.io/kani/)
- [OWASP Smart Contract Top 10](https://owasp.org/www-project-smart-contract-top-10/)
