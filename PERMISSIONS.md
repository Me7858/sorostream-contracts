# SoroStream Contract — Authorization Model & Permission Matrix

This document is the authoritative reference for **who may call which contract
instruction** and **how that authorization is enforced** in the Rust source.

Maintainers must update this table whenever a new instruction or role is added.
The PR template includes a reminder to do so.

---

## Table of Contents

1. [Roles](#roles)
2. [Permission Matrix](#permission-matrix)
   - [Admin / Lifecycle](#admin--lifecycle-instructions)
   - [Stream Operations](#stream-operation-instructions)
   - [Query / Read-only](#query--read-only-instructions)
   - [Batch Operations](#batch-operation-instructions)
   - [Fee Management](#fee-management-instructions)
   - [Rate Limiting](#rate-limiting-instructions)
   - [Whitelist Management](#whitelist-management-instructions)
   - [Token Whitelist](#token-whitelist-instructions)
   - [Federation Registry](#federation-registry-instructions)
   - [Delegation](#delegation-instructions)
   - [Milestone Gating](#milestone-gating-instructions)
3. [Authorization Enforcement Functions](#authorization-enforcement-functions)
4. [Role Definitions in Storage](#role-definitions-in-storage)
5. [Notes on Implicit vs. Explicit Auth](#notes-on-implicit-vs-explicit-auth)

---

## Roles

| Role Symbol | Description |
|---|---|
| **Admin** | Address stored via `write_admin()`. Set at `initialize()` and transferable via `set_admin()`. Has full administrative powers. |
| **Guardian** | Optional address stored via `write_guardian()`. Can pause the contract; lower-privilege than Admin. |
| **Governance** | Optional address stored via `write_governance()`. Can unpause the contract. Intended to be a multisig or DAO address. |
| **Sender** | The stream creator / payer. Authorized on a per-stream basis (stored as `stream.sender`). |
| **Recipient** | The stream beneficiary. Authorized on a per-stream basis (stored as `stream.recipient`). |
| **Delegate** | An address granted withdrawal rights for a specific stream by the sender via `set_delegate()`. |
| **Anyone** | Any account on the network — no authorization required. |

---

## Permission Matrix

Legend: ✅ Allowed · ❌ Denied · — Not applicable

### Admin / Lifecycle Instructions

| Instruction | Admin | Guardian | Governance | Sender | Recipient | Anyone | Auth Check (Rust function) |
|---|:---:|:---:|:---:|:---:|:---:|:---:|---|
| `initialize` | ✅ | — | — | — | — | ❌ | `read_admin(&env).is_some()` guard; first caller becomes admin |
| `get_admin` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | Read-only, no auth |
| `get_version` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | Read-only, no auth |
| `set_admin` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | `check_admin(&env)` |
| `emergency_pause` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | `check_admin(&env)` |
| `emergency_resume` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | `check_admin(&env)` |
| `pause` | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ | `guardian.require_auth()` + `stored_guardian == guardian` |
| `unpause` | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | `governance.require_auth()` + `stored_governance == governance` |
| `is_paused` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | Read-only, no auth |
| `get_pause_expiry` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | Read-only, no auth |
| `set_guardian` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | `check_admin(&env)` |
| `get_guardian` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | Read-only, no auth |
| `set_governance` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | `check_admin(&env)` |
| `get_governance` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | Read-only, no auth |
| `upgrade` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | `admin.require_auth()` (read via `read_admin`) |
| `migrate` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | `check_admin(&env)` |
| `get_admin_log` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | Read-only, no auth |
| `set_max_streams` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | `check_admin(&env)` |
| `set_sender_stream_limit` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | `check_admin(&env)` |
| `set_min_duration` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | `check_admin(&env)` |
| `set_withdrawal_cooldown` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | `check_admin(&env)` + `admin.require_auth()` |
| `set_stream_creation_cooldown` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | `check_admin(&env)` + `admin.require_auth()` |

---

### Stream Operation Instructions

| Instruction | Admin | Guardian | Governance | Sender | Recipient | Delegate | Anyone | Auth Check (Rust function) |
|---|:---:|:---:|:---:|:---:|:---:|:---:|:---:|---|
| `create_stream` | — | — | — | ✅ | ❌ | ❌ | ❌ | `sender.require_auth()` |
| `create_stream_with_federation` | — | — | — | ✅ | ❌ | ❌ | ❌ | delegates to `create_stream` → `sender.require_auth()` |
| `create_stream_with_curve` | — | — | — | ✅ | ❌ | ❌ | ❌ | `sender.require_auth()` |
| `create_step_vesting_stream` | — | — | — | ✅ | ❌ | ❌ | ❌ | `sender.require_auth()` |
| `withdraw` | ❌ | ❌ | ❌ | ❌ | ✅ | ✅¹ | ❌ | `recipient.require_auth()` + `stream.recipient == recipient`; delegate via `get_delegate` |
| `cancel_stream` | ❌ | ❌ | ❌ | ✅ | ✅² | ❌ | ❌ | `sender.require_auth()` + `stream.sender == sender`; recipient allowed if `allow_recipient_termination` |
| `partial_cancel_stream` | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | `sender.require_auth()` + `stream.sender == sender` |
| `top_up` | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | `sender.require_auth()` + `stream.sender == sender` |
| `pause_stream` | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | `sender.require_auth()` + `stream.sender == sender` |
| `resume_stream` | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | `sender.require_auth()` + `stream.sender == sender` |
| `cancel_auto_renew` | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | `sender.require_auth()` + `stream.sender == sender` |
| `update_metadata` | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | `sender.require_auth()` + `stream.sender == sender` |
| `update_metadata_uri` | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | `sender.require_auth()` + `stream.sender == sender` |
| `set_slippage_params` | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | `sender.require_auth()` + `stream.sender == sender` |
| `archive_stream` | ❌ | ❌ | ❌ | ✅ | ✅ | ❌ | ❌ | `caller.require_auth()` + `stream.sender == caller ‖ stream.recipient == caller` |
| `release_milestone` | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | `sender.require_auth()` + `stream.sender == sender` |
| `release_holdback` | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | `sender.require_auth()` + `stream.sender == sender` |
| `clawback_holdback` | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | `sender.require_auth()` + `stream.sender == sender` |

> ¹ Delegate: a separate address granted withdrawal rights by the sender for a specific stream. See `set_delegate`.  
> ² Recipient cancel: only allowed when the stream was created with `allow_recipient_termination = true`.

---

### Query / Read-only Instructions

All query instructions are **permissionless** (no `require_auth` call). Anyone may call them.

| Instruction | Auth Check (Rust function) |
|---|---|
| `get_stream` | None — read-only |
| `get_claimable` | None — read-only |
| `simulate_claimable` | None — read-only |
| `is_participant` | None — read-only |
| `get_all_stream_ids` | None — read-only |
| `get_streams_by_sender` | None — read-only |
| `get_streams_by_recipient` | None — read-only |
| `get_active_streams_by_sender` | None — read-only |
| `get_active_streams_by_recipient` | None — read-only |
| `get_stats` | None — read-only |
| `get_protocol_fee_info` | None — read-only |
| `remaining_quota` | None — read-only |
| `is_fee_exempt` | None — read-only |
| `get_fees_collected` | None — read-only |
| `resolve_federation` | None — read-only |

---

### Batch Operation Instructions

| Instruction | Admin | Sender | Recipient | Anyone | Auth Check (Rust function) |
|---|:---:|:---:|:---:|:---:|---|
| `batch_create_stream` | — | ✅ | ❌ | ❌ | `sender.require_auth()` (single auth covers all streams in the batch) |
| `batch_withdraw` | ❌ | ❌ | ✅ | ❌ | `recipient.require_auth()` + each stream's `stream.recipient == recipient` |
| `batch_cancel_stream` | ❌ | ✅ | ❌ | ❌ | `sender.require_auth()` + each stream's `stream.sender == sender` |

---

### Fee Management Instructions

| Instruction | Admin | Anyone | Auth Check (Rust function) |
|---|:---:|:---:|---|
| `set_protocol_fee` | ✅ | ❌ | `check_admin(&env)` |
| `set_treasury_address` | ✅ | ❌ | `check_admin(&env)` |
| `sweep_fees` | ✅ | ❌ | `check_admin(&env)` + `admin.require_auth()` |
| `add_fee_exempt` | ✅ | ❌ | `check_admin(&env)` |
| `remove_fee_exempt` | ✅ | ❌ | `check_admin(&env)` |
| `set_fee_tier` | ✅ | ❌ | `check_admin(&env)` |
| `set_creation_fee_xlm` | ✅ | ❌ | `check_admin(&env)` |
| `propose_fee_change` | ✅ | ❌ | `check_admin(&env)` |
| `apply_fee_change` | ✅ | ❌ | `check_admin(&env)` — also enforces timelock |
| `cancel_fee_proposal` | ✅ | ❌ | `check_admin(&env)` |

---

### Rate Limiting Instructions

| Instruction | Admin | Anyone | Auth Check (Rust function) |
|---|:---:|:---:|---|
| `set_rate_limit_window` | ✅ | ❌ | `check_admin(&env)` + `admin.require_auth()` |
| `set_rate_limit_max` | ✅ | ❌ | `check_admin(&env)` + `admin.require_auth()` |
| `add_rate_limit_exempt` | ✅ | ❌ | `check_admin(&env)` + `admin.require_auth()` |
| `remove_rate_limit_exempt` | ✅ | ❌ | `check_admin(&env)` + `admin.require_auth()` |
| `remaining_quota` | ✅ | ✅ | None — read-only |

---

### Whitelist Management Instructions

| Instruction | Admin | Anyone | Auth Check (Rust function) |
|---|:---:|:---:|---|
| `set_whitelist_enabled` | ✅ | ❌ | `check_admin(&env)` + `admin.require_auth()` |
| `add_to_whitelist` | ✅ | ❌ | `check_admin(&env)` + `admin.require_auth()` |
| `remove_from_whitelist` | ✅ | ❌ | `check_admin(&env)` + `admin.require_auth()` |

---

### Token Whitelist Instructions

| Instruction | Admin | Anyone | Auth Check (Rust function) |
|---|:---:|:---:|---|
| `set_token_whitelist_enabled` | ✅ | ❌ | `check_admin(&env)` + `admin.require_auth()` |
| `add_token_to_whitelist` | ✅ | ❌ | `check_admin(&env)` + `admin.require_auth()` |
| `remove_token_from_whitelist` | ✅ | ❌ | `check_admin(&env)` + `admin.require_auth()` |

---

### Federation Registry Instructions

| Instruction | Admin | Anyone | Auth Check (Rust function) |
|---|:---:|:---:|---|
| `register_federation` | ✅ | ❌ | `check_admin(&env)` + `admin.require_auth()` |
| `unregister_federation` | ✅ | ❌ | `check_admin(&env)` + `admin.require_auth()` |
| `resolve_federation` | ✅ | ✅ | None — read-only |

---

### Delegation Instructions

| Instruction | Admin | Sender | Anyone | Auth Check (Rust function) |
|---|:---:|:---:|:---:|---|
| `set_delegate` | ❌ | ✅ | ❌ | `sender.require_auth()` + `stream.sender == sender` |
| `remove_delegate` | ❌ | ✅ | ❌ | `sender.require_auth()` + `stream.sender == sender` |
| `get_delegate` | ✅ | ✅ | ✅ | None — read-only |

---

### Milestone Gating Instructions

| Instruction | Admin | Sender | Recipient | Auth Check (Rust function) |
|---|:---:|:---:|:---:|---|
| `release_milestone` | ❌ | ✅ | ❌ | `sender.require_auth()` + `stream.sender == sender` |

---

## Authorization Enforcement Functions

These are the Rust functions in `contracts/stream/src/` that implement the
authorization checks referenced in the tables above.

| Function | Location | Description |
|---|---|---|
| `check_admin` | `storage.rs` | Reads the stored admin address and calls `admin.require_auth()`. Panics if not initialized. |
| `sender.require_auth()` | `lib.rs` (inline) | Soroban SDK built-in. Fails the invocation if the transaction is not signed by `sender`. |
| `recipient.require_auth()` | `lib.rs` (inline) | Same as above for `recipient`. |
| `guardian.require_auth()` | `lib.rs` (inline) | Same as above for the guardian address. |
| `governance.require_auth()` | `lib.rs` (inline) | Same as above for the governance address. |
| `caller.require_auth()` | `lib.rs` (inline) | Used for archive_stream; caller must be sender or recipient. |
| `stream.sender == sender` | `lib.rs` (inline) | Secondary ownership check after auth, guards against passing someone else's stream ID. |
| `stream.recipient == recipient` | `lib.rs` (inline) | Same as above for recipient. |
| `is_paused_or_auto_unpause` | `storage.rs` | Returns `true` if the contract is paused. Called at the top of every state-mutating instruction. |

---

## Role Definitions in Storage

| Role | Storage Key | Set by | Enforced by |
|---|---|---|---|
| Admin | `DataKey::Admin` (persistent) | `write_admin()` | `check_admin()` |
| Guardian | `DataKey::Guardian` (persistent) | `write_guardian()` | `guardian.require_auth()` + equality check |
| Governance | `DataKey::Governance` (persistent) | `write_governance()` | `governance.require_auth()` + equality check |
| Stream Sender | `stream.sender` field (persistent, per stream) | `create_stream()` | `stream.sender == sender` |
| Stream Recipient | `stream.recipient` field (persistent, per stream) | `create_stream()` | `stream.recipient == recipient` |
| Delegate | `DataKey::Delegate(stream_id)` (persistent) | `set_delegate()` | `get_delegate()` in `withdraw()` |

---

## Notes on Implicit vs. Explicit Auth

- **`check_admin`** reads the stored admin from persistent storage and calls
  `require_auth()` on it. It does **not** accept an admin parameter from the
  caller — this prevents spoofing.

- **Dual-check pattern**: several admin functions use both `check_admin(&env)`
  (which calls `require_auth` internally) and an explicit `admin.require_auth()`
  on a passed-in address parameter. This is redundant but harmless; the
  `check_admin` call is the authoritative gate. When adding new admin
  instructions, prefer `check_admin(&env)` alone — passing `admin: Address` as
  a parameter and calling `require_auth()` on it is the legacy pattern and
  should not be extended.

- **Recipient cancel**: `cancel_stream` checks `allow_recipient_termination`
  flag stored in the stream. If `false`, only the sender may cancel. If `true`,
  both sender and recipient may cancel.

- **Delegate withdrawal**: `withdraw` first checks `stream.recipient == recipient`.
  If that fails, it looks up `get_delegate(stream_id)` and allows the delegate
  to withdraw on the recipient's behalf. The delegate still needs to sign the
  transaction (`recipient.require_auth()` is called on the passed-in address,
  which may be the delegate).

- **Paused contract**: `is_paused_or_auto_unpause` is checked **before** any
  auth check in `create_stream`, `withdraw`, `cancel_stream`, `top_up`, and all
  batch operations. A paused contract rejects all state-mutating calls regardless
  of who is calling.
