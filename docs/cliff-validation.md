# Cliff Validation — Edge Cases and Worked Examples

This document describes the **cliff semantics** of the SoroStream payment
streaming contract, explains every validation rule, and provides annotated
worked examples with concrete timestamps for each edge case. It is intended
for contributors adding or modifying cliff-related logic and for auditors
verifying the correctness of cliff enforcement.

Cross-references:
- Validation logic: `contracts/stream/src/lib.rs` → `create_stream()`
- Claimable computation: `contracts/stream/src/vesting_math.rs` → `compute_claimable()`
- Error codes: `contracts/stream/src/errors.rs` → `StreamError`

---

## Table of Contents

1. [Cliff Semantics Overview](#cliff-semantics-overview)
2. [Validation Rules at Stream Creation](#validation-rules-at-stream-creation)
3. [Cliff Enforcement at Withdrawal Time](#cliff-enforcement-at-withdrawal-time)
4. [Worked Examples](#worked-examples)
   - [Case 1 — Valid cliff (normal operation)](#case-1--valid-cliff-normal-operation)
   - [Case 2 — Zero cliff (no cliff)](#case-2--zero-cliff-no-cliff)
   - [Case 3 — Cliff at the boundary of stream end (cliff == end\_time)](#case-3--cliff-at-the-boundary-of-stream-end-cliff--end_time)
   - [Case 4 — Cliff after stream end (cliff > end\_time) — INVALID](#case-4--cliff-after-stream-end-cliff--end_time--invalid)
   - [Case 5 — Withdrawal attempted before cliff](#case-5--withdrawal-attempted-before-cliff)
   - [Case 6 — Withdrawal exactly at cliff](#case-6--withdrawal-exactly-at-cliff)
   - [Case 7 — Multiple withdrawals straddling the cliff](#case-7--multiple-withdrawals-straddling-the-cliff)
5. [Cliff Interaction with cancel\_stream](#cliff-interaction-with-cancel_stream)
6. [Summary Table](#summary-table)
7. [Known Gotchas and Prior Bugs](#known-gotchas-and-prior-bugs)

---

## Cliff Semantics Overview

A **cliff** is a minimum waiting period before the recipient may claim any
tokens. Tokens continue to accrue during the cliff period but are **not
withdrawable** until `now >= cliff_time`.

```
time axis →

start_time          cliff_time              end_time
    │                    │                       │
    │←── cliff period ──►│←── linear vesting ───►│
    │   (tokens accrue   │                       │
    │   but not claimable│                       │
    │   before this point│                       │
```

Key fields stored in the `Stream` struct:

| Field | Type | Meaning |
|---|---|---|
| `start_time` | `u64` | Ledger timestamp when the stream was created |
| `cliff_time` | `u64` | Timestamp before which `get_claimable` returns 0 |
| `end_time` | `u64` | Timestamp when all tokens are fully vested |
| `cliff_seconds` (input) | `u64` | Seconds after `start_time` until the cliff — passed to `create_stream`; stored as `cliff_time = start_time + cliff_seconds` |

---

## Validation Rules at Stream Creation

`create_stream` applies the following checks **in order** before persisting
the stream. The first failing check returns the associated error immediately.

```
Rust location: contracts/stream/src/lib.rs
Function:      SoroStreamContract::create_stream
```

### Rule 1 — `cliff_seconds` must not exceed `duration_seconds`

```rust
if cliff_seconds > duration_seconds {
    return Err(StreamError::InvalidCliff);   // error code 8
}
```

This ensures `cliff_time <= end_time`. A cliff that extends past the stream's
end would mean the recipient can never claim — which is almost certainly a
user error.

### Rule 2 — Derived times are computed after validation

```rust
let end_time   = now.checked_add(duration_seconds).ok_or(StreamError::Overflow)?;
let cliff_time = now.checked_add(cliff_seconds).ok_or(StreamError::Overflow)?;
```

Both are relative to `now` (the current ledger timestamp at the moment
`create_stream` is executed). There is no separate check for
`cliff_time == start_time` at creation time — a `cliff_seconds` of 0 is
explicitly allowed and means "no cliff".

### What is NOT validated

| Condition | Behaviour |
|---|---|
| `cliff_seconds == 0` | Allowed — means no cliff at all |
| `cliff_seconds == duration_seconds` | Allowed — cliff exactly at stream end (boundary case; see Case 3) |
| `cliff_time == start_time` | Allowed — equivalent to `cliff_seconds == 0` |

---

## Cliff Enforcement at Withdrawal Time

```
Rust location: contracts/stream/src/vesting_math.rs
Function:      compute_claimable
```

```rust
pub fn compute_claimable(
    flow_rate: i128,
    now: u64,
    cliff_time: u64,
    end_time: u64,
    last_withdraw_time: u64,
) -> Option<i128> {
    if now < cliff_time {          // ← cliff guard
        return Some(0);
    }
    let effective_now = if now < end_time { now } else { end_time };
    let elapsed = effective_now.saturating_sub(last_withdraw_time);
    flow_rate.checked_mul(elapsed as i128)
}
```

Semantics:

1. If `now < cliff_time` → return 0 (recipient gets nothing; call succeeds without error).
2. Otherwise → compute `flow_rate × (min(now, end_time) − last_withdraw_time)`.

Note that **`cancel_stream` uses `compute_earned` instead of `compute_claimable`**
(see `vesting_math.rs`). `compute_earned` does **not** apply the cliff guard.
This means a sender who cancels before the cliff still owes the recipient the
tokens that have time-accrued up to cancellation, even though the recipient
could not have withdrawn them yet.

---

## Worked Examples

All examples use these conventions:

- Timestamps are seconds since the Unix epoch for readability; in Soroban
  they are ledger timestamps (also unsigned 64-bit seconds).
- `deposit = 1_000_000` stroops, `duration_seconds = 1000 s`.
- `flow_rate = deposit / duration_seconds = 1_000 stroops/s`.
- `start_time = 1_000_000` (arbitrary epoch anchor).

---

### Case 1 — Valid cliff (normal operation)

**Setup:**

```
start_time  = 1_000_000
cliff_secs  = 200           (20% of duration)
end_time    = 1_001_000     (start + 1000)
cliff_time  = 1_000_200     (start + 200)
flow_rate   = 1_000 stroops/s
deposit     = 1_000_000 stroops
```

**Validation:** `cliff_seconds (200) <= duration_seconds (1000)` → ✅ passes.

**Claimable at various query times:**

| `now` | Cliff passed? | Elapsed since `start_time` | `get_claimable` result | Notes |
|---|---|---|---|---|
| 1_000_100 | ❌ No (now < 1_000_200) | 100 s | **0** | Tokens accrued but cliff not reached |
| 1_000_200 | ✅ Yes (now == cliff_time) | 200 s | **200_000** | First moment recipient can claim; 200 s of flow is immediately claimable |
| 1_000_600 | ✅ Yes | 600 s | **400_000** | Assumes no prior withdrawals (last_withdraw_time = start_time); `1_000 × (600 − 0)` — but wait, elapsed = now − last_withdraw_time = 600 − 0 = 600 s, capped at cliff... see note ↓ |
| 1_001_000 | ✅ Yes | 1_000 s | **1_000_000** | Full deposit claimable at end_time |

> **Note on Case 1, row 3:** `compute_claimable` computes elapsed as
> `effective_now − last_withdraw_time`. If the recipient has never withdrawn,
> `last_withdraw_time = start_time = 1_000_000`. At `now = 1_000_600`:
> `elapsed = 1_000_600 − 1_000_000 = 600`. `claimable = 1_000 × 600 = 600_000`.
> The cliff only gates *when* withdrawal is allowed, not *how much* accrued;
> all accrual since `start_time` is available in the first post-cliff withdrawal.

---

### Case 2 — Zero cliff (no cliff)

**Setup:**

```
start_time  = 1_000_000
cliff_secs  = 0             (no cliff)
cliff_time  = 1_000_000     (= start_time)
end_time    = 1_001_000
flow_rate   = 1_000 stroops/s
```

**Validation:** `cliff_seconds (0) <= duration_seconds (1000)` → ✅ passes.

**Claimable immediately after creation:**

| `now` | `get_claimable` result | Notes |
|---|---|---|
| 1_000_000 | **0** | Same timestamp as start; 0 elapsed |
| 1_000_001 | **1_000** | 1 second of flow |
| 1_000_500 | **500_000** | 500 s × 1_000 stroops/s |

The `compute_claimable` cliff guard passes immediately because
`now >= cliff_time (= start_time)` from the very first second.

---

### Case 3 — Cliff at the boundary of stream end (`cliff == end_time`)

**Setup:**

```
start_time  = 1_000_000
cliff_secs  = 1_000         (= duration_seconds — cliff at the very end)
cliff_time  = 1_001_000     (= end_time)
end_time    = 1_001_000
flow_rate   = 1_000 stroops/s
deposit     = 1_000_000 stroops
```

**Validation:** `cliff_seconds (1000) <= duration_seconds (1000)` → ✅ passes
(equal is allowed).

**Behaviour:**

```
now = 1_000_999  →  now < cliff_time (1_001_000)  →  claimable = 0
now = 1_001_000  →  now == cliff_time == end_time  →  cliff passed
                    effective_now = min(now, end_time) = 1_001_000
                    elapsed = 1_001_000 − 1_000_000 = 1_000 s
                    claimable = 1_000 × 1_000 = 1_000_000
```

**Result:** The entire deposit becomes claimable at exactly `end_time`.
This is a valid use case for "all-or-nothing" vesting (e.g., a 1-year
cliff that releases 100% of a grant at the anniversary).

---

### Case 4 — Cliff after stream end (`cliff > end_time`) — INVALID

**Setup:**

```
duration_seconds = 1_000
cliff_secs       = 1_001     (> duration_seconds)
```

**Validation:**

```rust
if cliff_seconds > duration_seconds {        // 1_001 > 1_000 → true
    return Err(StreamError::InvalidCliff);   // error code 8
}
```

**Result:** `create_stream` returns `Err(StreamError::InvalidCliff)` (code 8).
The stream is never created. No tokens are transferred.

**Why this is invalid:** A cliff past `end_time` would mean the cliff is
unreachable — the stream expires before the cliff is ever passed. All tokens
would be permanently locked with no path for the recipient to claim them and
no refund mechanism (the sender could only cancel, not withdraw). The contract
treats this as a programmer error.

---

### Case 5 — Withdrawal attempted before cliff

**Setup** (same as Case 1):

```
cliff_time  = 1_000_200
now         = 1_000_100   (100 s before cliff)
```

**What happens:**

```rust
// In SoroStreamContract::withdraw:
c.withdraw(&stream_id, &b.recipient);  // called at now = 1_000_100

// Inside compute_claimable:
if now < cliff_time {   // 1_000_100 < 1_000_200 → true
    return Some(0);     // returns zero — NOT an error
}
```

**Result:** `withdraw` **succeeds** but transfers 0 tokens. It does not return
an error. The `last_withdraw_time` is updated to `now` (1_000_100), which
means subsequent calls will compute elapsed from 1_000_100, not from
`start_time`.

> ⚠️ **Important subtlety:** A recipient who calls `withdraw` before the cliff
> and receives 0 tokens will lose the accrual from `start_time` to
> `last_withdraw_time`. After the zero-withdraw at 1_000_100, the next
> successful withdrawal at 1_000_500 computes:
> `elapsed = 1_000_500 − 1_000_100 = 400 s` → `400_000 stroops`.
> The 100 s of accrual between `start_time` and the early call is silently
> "advanced" — not lost from the contract's perspective (it stays in `deposit`)
> but effectively donated back to the stream's unclaimed tail.
>
> Off-chain callers should check `get_claimable > 0` before submitting a
> withdrawal transaction to avoid this.

---

### Case 6 — Withdrawal exactly at cliff

**Setup:**

```
start_time        = 1_000_000
cliff_time        = 1_000_200
last_withdraw_time = 1_000_000   (initial value = start_time)
now               = 1_000_200   (exactly at cliff)
flow_rate         = 1_000 stroops/s
```

**Computation:**

```
now >= cliff_time → cliff passed ✅
effective_now = min(1_000_200, 1_001_000) = 1_000_200
elapsed = 1_000_200 − 1_000_000 = 200 s
claimable = 1_000 × 200 = 200_000 stroops
```

**Result:** 200_000 stroops transferred. All accrual since stream creation
(including the cliff period) is immediately available at the cliff instant.
`last_withdraw_time` is updated to `1_000_200`.

---

### Case 7 — Multiple withdrawals straddling the cliff

**Setup:**

```
start_time = 1_000_000
cliff_time = 1_000_300
end_time   = 1_001_000
flow_rate  = 1_000 stroops/s
```

**Withdrawal sequence:**

| Call # | `now` | Cliff passed? | `last_withdraw_time` before call | `elapsed` | `claimable` | `last_withdraw_time` after |
|---|---|---|---|---|---|---|
| 1 | 1_000_100 | ❌ | 1_000_000 | — (returns 0 immediately) | 0 | **1_000_100** |
| 2 | 1_000_200 | ❌ | 1_000_100 | — (returns 0 immediately) | 0 | **1_000_200** |
| 3 | 1_000_400 | ✅ | 1_000_200 | 400 − 200 = 200 s | 200_000 | 1_000_400 |
| 4 | 1_000_700 | ✅ | 1_000_400 | 700 − 400 = 300 s | 300_000 | 1_000_700 |

**Total claimed after 4 calls: 500_000 stroops.**

Tokens that accrued between `start_time` (1_000_000) and the last
pre-cliff withdrawal (1_000_200) — i.e., 200 s × 1_000 = 200_000 stroops —
are **not claimed** because the cliff calls advanced `last_withdraw_time`
without transferring anything. That 200_000 remains in the contract deposit
and can only be reclaimed by the sender via `cancel_stream` (since it is
beyond what the recipient can earn from `last_withdraw_time` forward).

This is the **most dangerous cliff edge case** and the source of prior bugs.
The invariant to remember:

> `compute_claimable` returns `flow_rate × (effective_now − last_withdraw_time)`.
> If `last_withdraw_time` is advanced during the cliff (by zero-value withdrawals),
> some accrual is silently stranded.

---

## Cliff Interaction with `cancel_stream`

`cancel_stream` calls `compute_earned` (not `compute_claimable`):

```rust
// vesting_math.rs
pub fn compute_earned(
    flow_rate: i128,
    now: u64,
    end_time: u64,
    last_withdraw_time: u64,
) -> Option<i128> {
    let effective_now = if now < end_time { now } else { end_time };
    let elapsed = effective_now.saturating_sub(last_withdraw_time);
    flow_rate.checked_mul(elapsed as i128)
}
```

**No cliff guard.** The recipient receives everything that has accrued since
`last_withdraw_time`, regardless of whether the cliff has been reached.

**Example:**

```
now       = 1_000_100   (before cliff_time = 1_000_200)
flow_rate = 1_000 stroops/s
last_withdraw_time = 1_000_000

compute_earned → elapsed = 100 s → recipient gets 100_000 stroops
sender gets    → deposit − earned = 900_000 stroops
```

Rationale: cancellation is an irrevocable termination event. Denying accrued
tokens on cancellation would allow a malicious sender to cancel streams
immediately before the cliff to deny recipients their earned compensation.

---

## Summary Table

| Scenario | `cliff_seconds` vs `duration_seconds` | Valid at creation? | Error code | `get_claimable` before cliff | `cancel_stream` before cliff |
|---|---|---|---|---|---|
| Normal cliff | `cliff_seconds < duration_seconds` | ✅ Yes | — | 0 (cliff guard) | Accrued tokens sent to recipient |
| No cliff | `cliff_seconds == 0` | ✅ Yes | — | Normal (cliff == start_time) | Accrued tokens sent to recipient |
| Cliff at stream end | `cliff_seconds == duration_seconds` | ✅ Yes | — | 0 until `end_time` | Accrued tokens sent to recipient |
| Cliff after stream end | `cliff_seconds > duration_seconds` | ❌ No | `InvalidCliff` (8) | Stream not created | Stream not created |

---

## Known Gotchas and Prior Bugs

### 1. Zero-value withdrawals advance `last_withdraw_time`

**Symptom:** A recipient who calls `withdraw` before the cliff receives 0
tokens but their `last_withdraw_time` is silently advanced. Any accrual
before that timestamp is inaccessible on future withdrawals.

**Fix applied (in tests):** Off-chain callers must check `get_claimable() > 0`
before submitting a withdrawal.

**Status:** Known design trade-off. No contract change planned. Documented in
`docs/contract-reference.md` under the `withdraw` section.

### 2. `cliff_seconds == duration_seconds` is silently allowed

**Symptom:** Creating a stream with `cliff_seconds == duration_seconds` is valid
but means the recipient can only claim at exactly `end_time`. Any call before
that moment returns 0. This surprised early integrators who expected an error.

**Status:** By design. Enables "cliff only" vesting (100% unlocks at one date).
Documented explicitly in [Case 3](#case-3--cliff-at-the-boundary-of-stream-end-cliff--end_time) above.

### 3. Cancellation ignores the cliff; `withdraw` respects it

**Symptom:** A stream cancelled before the cliff still pays out accrued tokens
to the recipient. This is inconsistent with the cliff's stated purpose of
making tokens "not claimable" before a date.

**Rationale:** Cancellation is an active choice by the sender (or, with
`allow_recipient_termination`, the recipient). Denying accrued tokens on
cancellation would be exploitable (sender cancels 1 second before cliff to
deny earned pay). The cliff is a *withdrawal gate*, not an *accrual gate*.

**Status:** By design. Documented in [Cliff Interaction with cancel\_stream](#cliff-interaction-with-cancel_stream) above.

### 4. `cliff_time` is computed relative to `now` at creation, not a user-supplied absolute timestamp

**Symptom:** Two `create_stream` calls with identical parameters but different
ledger timestamps produce different `cliff_time` values. This is expected but
has surprised integrators who expected idempotency.

**Status:** By design. `cliff_time = ledger.timestamp() + cliff_seconds`.
