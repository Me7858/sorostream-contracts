# SoroStream Event XDR Encoding Reference

This document describes the **XDR encoding** of every event emitted by the SoroStream stream contract. It is intended for indexer developers who consume raw Horizon/RPC event records and need to know exactly how to decode the `topics` and `data` byte arrays.

For a high-level field-by-field schema without encoding details, see [`docs/events.md`](./docs/events.md).

---

## How Soroban Events Are XDR-Encoded

When a contract calls `env.events().publish(topics, data)`:

- **`topics`** is a `Vec<ScVal>` — each element is XDR-encoded as an `ScVal`.
- **`data`** is a single `ScVal` — if the Rust expression is a tuple `(a, b, c)`, it is encoded as `ScVal::ScvVec` containing three `ScVal` elements. A unit `()` becomes `ScVal::ScvVoid`.

### Rust → XDR Primitive Mapping

| Rust type | `ScVal` variant | Notes |
|-----------|----------------|-------|
| `Symbol` | `ScVal::ScvSymbol` | UTF-8, max 32 chars |
| `u32` | `ScVal::ScvU32` | Little-endian in the binary XDR |
| `u64` | `ScVal::ScvU64` | |
| `i128` | `ScVal::ScvI128` | Encoded as `Int128Parts { hi: i64, lo: u64 }` |
| `Address` | `ScVal::ScvAddress` | Either `ScAddress::ScAddressTypeAccount(AccountId)` or `ScAddress::ScAddressTypeContract(Hash)` |
| `String` (SDK) | `ScVal::ScvString` | Raw bytes, not null-terminated |
| `Bytes` (SDK) | `ScVal::ScvBytes` | Raw bytes |
| tuple `(A, B, …)` | `ScVal::ScvVec` | One `ScVal` per element |
| `()` | `ScVal::ScvVoid` | |

### Locating Events in a Horizon Response

A Horizon `/events` record looks like:

```json
{
  "type": "contract",
  "contract_id": "CAM753...",
  "topic": ["AAAAAA==", "AAAAC..."],
  "value": "AAAAAQ=="
}
```

Each `topic[i]` and `value` is a **base64-encoded XDR `ScVal`**.

---

## Event Catalogue

### 1. `StreamCreated`

Emitted by `create_stream` and `batch_create_stream`.

**Rust call:**
```rust
env.events().publish(
    (Symbol::new(env, "StreamCreated"), stream_id),
    (sender, recipient, amount, flow_rate, end_time),
);
```

| Position | Field | `ScVal` variant | XDR notes |
|----------|-------|----------------|-----------|
| topics[0] | `"StreamCreated"` | `ScvSymbol` | 13 ASCII chars |
| topics[1] | `stream_id` | `ScvU64` | SHA-256 derived; first 8 bytes of `sha256(sender_xdr \|\| recipient_xdr \|\| start_time_be \|\| nonce_be)` cast to `u64` |
| data[0] | `sender` | `ScvAddress` | |
| data[1] | `recipient` | `ScvAddress` | |
| data[2] | `amount` | `ScvI128` | Total deposit in stroops |
| data[3] | `flow_rate` | `ScvI128` | `amount / duration_seconds`, integer floor |
| data[4] | `end_time` | `ScvU64` | Unix timestamp |

**XDR hex example** (illustrative — not a live transaction):

```
topics[0] (ScvSymbol "StreamCreated"):
  00000006 0000000d 53747265616d43726561746564

topics[1] (ScvU64 12345678901234):
  0000000f 00000000 000b3a9c fd7df2

data (ScvVec of 5):
  00000012 00000005
    [ScvAddress sender]   [ScvAddress recipient]
    [ScvI128 amount]      [ScvI128 flow_rate]
    [ScvU64 end_time]
```

---

### 2. `StreamWithdrawn`

Emitted by `withdraw` and `batch_withdraw`.

```rust
env.events().publish(
    (Symbol::new(env, "StreamWithdrawn"), stream_id),
    (recipient, amount, timestamp),
);
```

| Position | Field | `ScVal` variant |
|----------|-------|----------------|
| topics[0] | `"StreamWithdrawn"` | `ScvSymbol` |
| topics[1] | `stream_id` | `ScvU64` |
| data[0] | `recipient` | `ScvAddress` |
| data[1] | `amount` | `ScvI128` | Total claimable amount (before fee deduction) |
| data[2] | `timestamp` | `ScvU64` | `env.ledger().timestamp()` at time of call |

> **Note:** `amount` is the gross claimable. The net amount received by the recipient is `amount - fee`. The corresponding `FeeCollected` event carries the fee.

---

### 3. `StreamCancelled`

Emitted by `cancel_stream` and `batch_cancel_stream`.

```rust
env.events().publish(
    (Symbol::new(env, "StreamCancelled"), stream_id),
    (sender, refund_amount, recipient_amount),
);
```

| Position | Field | `ScVal` variant |
|----------|-------|----------------|
| topics[0] | `"StreamCancelled"` | `ScvSymbol` |
| topics[1] | `stream_id` | `ScvU64` |
| data[0] | `sender` | `ScvAddress` |
| data[1] | `refund_amount` | `ScvI128` | Tokens returned to sender |
| data[2] | `recipient_amount` | `ScvI128` | Earned tokens sent to recipient |

---

### 4. `StreamToppedUp`

Emitted by `top_up`.

```rust
env.events().publish(
    (Symbol::new(env, "StreamToppedUp"), stream_id),
    (added_amount, new_end_time),
);
```

| Position | Field | `ScVal` variant |
|----------|-------|----------------|
| topics[0] | `"StreamToppedUp"` | `ScvSymbol` |
| topics[1] | `stream_id` | `ScvU64` |
| data[0] | `added_amount` | `ScvI128` | Actual tokens added (rounded down to nearest `flow_rate` multiple) |
| data[1] | `new_end_time` | `ScvU64` | Updated stream end timestamp |

---

### 5. `StreamCompleted`

Emitted when a stream reaches its `end_time` naturally.

```rust
env.events().publish(
    (Symbol::new(env, "StreamCompleted"), stream_id),
    (),
);
```

| Position | Field | `ScVal` variant |
|----------|-------|----------------|
| topics[0] | `"StreamCompleted"` | `ScvSymbol` |
| topics[1] | `stream_id` | `ScvU64` |
| data | _(none)_ | `ScvVoid` |

---

### 6. `StreamPaused`

Emitted by `pause_stream`.

```rust
env.events().publish(
    (Symbol::new(env, "StreamPaused"), stream_id),
    sender,
);
```

| Position | Field | `ScVal` variant |
|----------|-------|----------------|
| topics[0] | `"StreamPaused"` | `ScvSymbol` |
| topics[1] | `stream_id` | `ScvU64` |
| data | `sender` | `ScvAddress` | Scalar (not wrapped in a vec) |

---

### 7. `StreamResumed`

Emitted by `resume_stream`.

```rust
env.events().publish(
    (Symbol::new(env, "StreamResumed"), stream_id),
    sender,
);
```

| Position | Field | `ScVal` variant |
|----------|-------|----------------|
| topics[0] | `"StreamResumed"` | `ScvSymbol` |
| topics[1] | `stream_id` | `ScvU64` |
| data | `sender` | `ScvAddress` | Scalar |

---

### 8. `StreamPartialCancelled`

Emitted by `partial_cancel_stream`. The original stream is marked `Cancelled` and a new stream is created.

```rust
env.events().publish(
    (Symbol::new(env, "StreamPartialCancelled"), old_stream_id),
    (new_stream_id, sender, refund_amount, new_deposit),
);
```

| Position | Field | `ScVal` variant |
|----------|-------|----------------|
| topics[0] | `"StreamPartialCancelled"` | `ScvSymbol` |
| topics[1] | `old_stream_id` | `ScvU64` |
| data[0] | `new_stream_id` | `ScvU64` |
| data[1] | `sender` | `ScvAddress` |
| data[2] | `refund_amount` | `ScvI128` | Tokens returned to sender immediately |
| data[3] | `new_deposit` | `ScvI128` | Deposit locked in the replacement stream |

---

### 9. `StreamTerminatedByRecipient`

Emitted by `recipient_terminate` (requires `allow_recipient_termination = true`).

```rust
env.events().publish(
    (Symbol::new(env, "StreamTerminatedByRecipient"), stream_id),
    (recipient, recipient_amount, refund_amount),
);
```

| Position | Field | `ScVal` variant |
|----------|-------|----------------|
| topics[0] | `"StreamTerminatedByRecipient"` | `ScvSymbol` |
| topics[1] | `stream_id` | `ScvU64` |
| data[0] | `recipient` | `ScvAddress` |
| data[1] | `recipient_amount` | `ScvI128` | Earned tokens sent to recipient |
| data[2] | `refund_amount` | `ScvI128` | Remainder returned to sender |

---

### 10. `RecipientTransferred`

Emitted by `transfer_recipient`. Any earned tokens are auto-swept to the old recipient first, then ownership changes.

```rust
env.events().publish(
    (Symbol::new(env, "RecipientTransferred"), stream_id),
    (old_recipient, new_recipient),
);
```

| Position | Field | `ScVal` variant |
|----------|-------|----------------|
| topics[0] | `"RecipientTransferred"` | `ScvSymbol` |
| topics[1] | `stream_id` | `ScvU64` |
| data[0] | `old_recipient` | `ScvAddress` |
| data[1] | `new_recipient` | `ScvAddress` |

---

### 11. `StreamArchived`

Emitted by `archive_stream` after the stream is fully settled and its storage entry is deleted.

```rust
env.events().publish(
    (Symbol::new(env, "StreamArchived"), stream_id),
    (sender, recipient, total_amount),
);
```

| Position | Field | `ScVal` variant |
|----------|-------|----------------|
| topics[0] | `"StreamArchived"` | `ScvSymbol` |
| topics[1] | `stream_id` | `ScvU64` |
| data[0] | `sender` | `ScvAddress` |
| data[1] | `recipient` | `ScvAddress` |
| data[2] | `total_amount` | `ScvI128` | Original deposit |

---

### 12. `MetadataUpdated`

Emitted by `update_metadata`.

```rust
env.events().publish(
    (Symbol::new(env, "MetadataUpdated"), stream_id),
    metadata,
);
```

| Position | Field | `ScVal` variant |
|----------|-------|----------------|
| topics[0] | `"MetadataUpdated"` | `ScvSymbol` |
| topics[1] | `stream_id` | `ScvU64` |
| data | `metadata` | `ScvBytes` | Raw bytes, max 64 bytes |

---

### 13. `AutoRenewCancelled`

Emitted by `cancel_auto_renew`.

```rust
env.events().publish(
    (Symbol::new(env, "AutoRenewCancelled"), stream_id),
    (),
);
```

| Position | Field | `ScVal` variant |
|----------|-------|----------------|
| topics[0] | `"AutoRenewCancelled"` | `ScvSymbol` |
| topics[1] | `stream_id` | `ScvU64` |
| data | _(none)_ | `ScvVoid` |

---

### 14. `AutoRenewFailed`

Emitted when the sender has insufficient token balance to fund the next auto-renew cycle.

```rust
env.events().publish(
    (Symbol::new(env, "AutoRenewFailed"), stream_id),
    (sender, required),
);
```

| Position | Field | `ScVal` variant |
|----------|-------|----------------|
| topics[0] | `"AutoRenewFailed"` | `ScvSymbol` |
| topics[1] | `stream_id` | `ScvU64` |
| data[0] | `sender` | `ScvAddress` |
| data[1] | `required` | `ScvI128` | `stream.deposit` — full deposit needed for renewal |

---

### 15. `StreamRenewed`

Emitted on a successful auto-renew cycle start.

```rust
env.events().publish(
    (Symbol::new(env, "StreamRenewed"), old_stream_id),
    new_stream_id,
);
```

| Position | Field | `ScVal` variant |
|----------|-------|----------------|
| topics[0] | `"StreamRenewed"` | `ScvSymbol` |
| topics[1] | `old_stream_id` | `ScvU64` |
| data | `new_stream_id` | `ScvU64` | Scalar |

---

### 16. `FeeCollected`

Emitted on every `withdraw` (and `transfer_recipient`) where the protocol fee is non-zero and the recipient is not fee-exempt.

```rust
env.events().publish(
    (Symbol::new(env, "FeeCollected"), stream_id),
    (amount, treasury),
);
```

| Position | Field | `ScVal` variant |
|----------|-------|----------------|
| topics[0] | `"FeeCollected"` | `ScvSymbol` |
| topics[1] | `stream_id` | `ScvU64` |
| data[0] | `amount` | `ScvI128` | Fee in stroops: `claimable * fee_bps / 10_000` |
| data[1] | `treasury` | `ScvAddress` | Recipient of the fee |

---

### 17. `CreationFeeCollected`

Emitted by `create_stream` when a flat XLM creation fee is configured (`cf_xlm > 0`).

```rust
env.events().publish(
    (Symbol::new(env, "CreationFeeCollected"),),
    (fee_amount, treasury),
);
```

| Position | Field | `ScVal` variant |
|----------|-------|----------------|
| topics[0] | `"CreationFeeCollected"` | `ScvSymbol` |
| data[0] | `fee_amount` | `ScvI128` | XLM in stroops (1 XLM = 10,000,000 stroops) |
| data[1] | `treasury` | `ScvAddress` | |

> **No `stream_id` in topics.** This event has only one topic element.

---

### 18. `FeeChangeProposed`

Emitted by `propose_fee_change`, starting the 7-day timelock.

```rust
env.events().publish(
    (Symbol::new(env, "FeeChangeProposed"),),
    (new_fee, unlock_time),
);
```

| Position | Field | `ScVal` variant |
|----------|-------|----------------|
| topics[0] | `"FeeChangeProposed"` | `ScvSymbol` |
| data[0] | `new_fee` | `ScvU32` | Proposed fee in basis points |
| data[1] | `unlock_time` | `ScvU64` | `now + 604_800` (7 days in seconds) |

---

### 19. `FeeChangeExecuted`

Emitted by `execute_fee_change` after the timelock expires.

```rust
env.events().publish(
    (Symbol::new(env, "FeeChangeExecuted"),),
    (new_fee,),
);
```

| Position | Field | `ScVal` variant |
|----------|-------|----------------|
| topics[0] | `"FeeChangeExecuted"` | `ScvSymbol` |
| data[0] | `new_fee` | `ScvU32` | New active fee in basis points |

> `data` is a `ScvVec` with one element (a single-element tuple), not a scalar `ScvU32`.

---

### 20. `ContractDeployed`

Emitted once during `initialize`.

```rust
env.events().publish(
    (Symbol::new(env, "ContractDeployed"),),
    (version, admin),
);
```

| Position | Field | `ScVal` variant |
|----------|-------|----------------|
| topics[0] | `"ContractDeployed"` | `ScvSymbol` |
| data[0] | `version` | `ScvString` | e.g. `"1.0.0"` |
| data[1] | `admin` | `ScvAddress` | |

---

### 21. `ContractPaused`

Emitted by `emergency_pause`.

```rust
env.events().publish(
    (Symbol::new(env, "ContractPaused"), admin),
    timestamp,
);
```

| Position | Field | `ScVal` variant |
|----------|-------|----------------|
| topics[0] | `"ContractPaused"` | `ScvSymbol` |
| topics[1] | `admin` | `ScvAddress` | **Indexed** |
| data | `timestamp` | `ScvU64` | Scalar |

---

### 22. `ContractResumed`

Emitted by `emergency_resume`.

```rust
env.events().publish(
    (Symbol::new(env, "ContractResumed"), admin),
    timestamp,
);
```

| Position | Field | `ScVal` variant |
|----------|-------|----------------|
| topics[0] | `"ContractResumed"` | `ScvSymbol` |
| topics[1] | `admin` | `ScvAddress` | **Indexed** |
| data | `timestamp` | `ScvU64` | Scalar |

---

### 23. `ContractMigrated`

Emitted by `migrate`.

```rust
env.events().publish(
    (Symbol::new(env, "ContractMigrated"),),
    (from_version, to_version, admin),
);
```

| Position | Field | `ScVal` variant |
|----------|-------|----------------|
| topics[0] | `"ContractMigrated"` | `ScvSymbol` |
| data[0] | `from_version` | `ScvString` |
| data[1] | `to_version` | `ScvString` |
| data[2] | `admin` | `ScvAddress` |

---

### 24. `AdminAction`

Emitted alongside specific admin operations (`emergency_pause`, `emergency_resume`, `migrate`).

```rust
env.events().publish(
    (Symbol::new(env, "AdminAction"),),
    (instruction, admin, timestamp),
);
```

| Position | Field | `ScVal` variant |
|----------|-------|----------------|
| topics[0] | `"AdminAction"` | `ScvSymbol` |
| data[0] | `instruction` | `ScvString` | e.g. `"emergency_pause"` |
| data[1] | `admin` | `ScvAddress` |
| data[2] | `timestamp` | `ScvU64` |

---

## Decoding Events from a Horizon Record

The following TypeScript snippet decodes a raw Horizon `/events` record into structured fields using the `@stellar/stellar-sdk`:

```typescript
import {
  xdr,
  Address,
  scValToNative,
} from "@stellar/stellar-sdk";

interface SoroStreamEvent {
  name: string;
  topics: unknown[];
  data: unknown;
}

/**
 * Decode a single Horizon contract event record into a typed SoroStream event.
 *
 * @param record  A raw Horizon event object with `topic: string[]` and `value: string`
 *                (both base64-encoded XDR ScVal).
 */
function decodeEvent(record: {
  topic: string[];
  value: string;
}): SoroStreamEvent {
  // Decode topics
  const topics = record.topic.map((t) =>
    scValToNative(xdr.ScVal.fromXDR(t, "base64"))
  );

  // Decode data payload
  const data = scValToNative(xdr.ScVal.fromXDR(record.value, "base64"));

  const name = topics[0] as string;
  return { name, topics, data };
}

/**
 * Decode a StreamCreated event and return typed fields.
 */
function decodeStreamCreated(record: { topic: string[]; value: string }) {
  const ev = decodeEvent(record);
  if (ev.name !== "StreamCreated") throw new Error("Wrong event type");

  const [, streamId] = ev.topics as [string, bigint];
  const [sender, recipient, amount, flowRate, endTime] = ev.data as [
    string,
    string,
    bigint,
    bigint,
    bigint,
  ];

  return { streamId, sender, recipient, amount, flowRate, endTime };
}

/**
 * Decode a FeeCollected event.
 */
function decodeFeeCollected(record: { topic: string[]; value: string }) {
  const ev = decodeEvent(record);
  if (ev.name !== "FeeCollected") throw new Error("Wrong event type");

  const [, streamId] = ev.topics as [string, bigint];
  const [amount, treasury] = ev.data as [bigint, string];

  return { streamId, amount, treasury };
}

// ── Example: poll all SoroStream events for a contract ──────────────────────
import { Horizon } from "@stellar/stellar-sdk";

async function pollEvents(contractId: string, cursor = "now") {
  const server = new Horizon.Server("https://horizon-testnet.stellar.org");

  const page = await server
    .effects()
    .forContract(contractId)
    // Horizon /events endpoint:
    // GET /events?contract_id=<ID>&cursor=<cursor>&limit=200
    .call();

  for (const record of page.records) {
    // record.type === "contract" for Soroban contract events
    if ((record as any).type !== "contract") continue;
    const ev = decodeEvent(record as any);
    console.log(`Event: ${ev.name}`, ev.data);
  }
}
```

### Python equivalent (using `stellar-sdk`)

```python
import base64
from stellar_sdk import xdr as stellar_xdr
from stellar_sdk.scval import from_xdr_sc_val

def decode_event(topic_list: list[str], value_b64: str) -> dict:
    """
    Decode a Horizon contract event record.

    topic_list: list of base64-encoded XDR ScVal strings
    value_b64:  base64-encoded XDR ScVal string for the data payload
    """
    topics = [
        from_xdr_sc_val(stellar_xdr.ScVal.from_xdr(t))
        for t in topic_list
    ]
    data = from_xdr_sc_val(stellar_xdr.ScVal.from_xdr(value_b64))
    return {"name": topics[0], "topics": topics, "data": data}


# Example: decode a StreamWithdrawn event
def decode_stream_withdrawn(topic_list, value_b64):
    ev = decode_event(topic_list, value_b64)
    assert ev["name"] == "StreamWithdrawn"
    stream_id = ev["topics"][1]               # int
    recipient, amount, timestamp = ev["data"] # str, int, int
    return {
        "stream_id": stream_id,
        "recipient": recipient,
        "amount": amount,         # gross claimable (stroops)
        "timestamp": timestamp,
    }
```

---

## Filtering Events by Contract

All SoroStream contract events are emitted with `type = "contract"` and carry the
`contract_id` of the stream contract. To filter by event name, match `topics[0]`
after base64-decoding.

### Horizon REST query

```bash
# Fetch the 200 most recent events for the testnet stream contract
curl "https://horizon-testnet.stellar.org/events?contract_id=CAM753QTDMNRWJ7XI5B77QUEQBTI2FTOAWQJHWMFFHO54R36AFUUVR72&limit=200&order=desc"
```

### stellar-cli

```bash
stellar events \
  --network testnet \
  --contract-id CAM753QTDMNRWJ7XI5B77QUEQBTI2FTOAWQJHWMFFHO54R36AFUUVR72 \
  --start-ledger 0
```

---

## Summary Table

| # | Event name | topics count | data type | Emitting instruction(s) |
|---|-----------|:---:|---|---|
| 1 | `StreamCreated` | 2 | `(Address, Address, i128, i128, u64)` | `create_stream`, `batch_create_stream` |
| 2 | `StreamWithdrawn` | 2 | `(Address, i128, u64)` | `withdraw`, `batch_withdraw`, `transfer_recipient` |
| 3 | `StreamCancelled` | 2 | `(Address, i128, i128)` | `cancel_stream`, `batch_cancel_stream`, `partial_cancel_stream` |
| 4 | `StreamToppedUp` | 2 | `(i128, u64)` | `top_up` |
| 5 | `StreamCompleted` | 2 | `()` | `withdraw` (at end_time) |
| 6 | `StreamPaused` | 2 | `Address` | `pause_stream` |
| 7 | `StreamResumed` | 2 | `Address` | `resume_stream` |
| 8 | `StreamPartialCancelled` | 2 | `(u64, Address, i128, i128)` | `partial_cancel_stream` |
| 9 | `StreamTerminatedByRecipient` | 2 | `(Address, i128, i128)` | `recipient_terminate` |
| 10 | `RecipientTransferred` | 2 | `(Address, Address)` | `transfer_recipient` |
| 11 | `StreamArchived` | 2 | `(Address, Address, i128)` | `archive_stream` |
| 12 | `MetadataUpdated` | 2 | `Bytes` | `update_metadata` |
| 13 | `AutoRenewCancelled` | 2 | `()` | `cancel_auto_renew` |
| 14 | `AutoRenewFailed` | 2 | `(Address, i128)` | `withdraw` (auto-renew path) |
| 15 | `StreamRenewed` | 2 | `u64` | `withdraw` (auto-renew path) |
| 16 | `FeeCollected` | 2 | `(i128, Address)` | `withdraw`, `transfer_recipient` |
| 17 | `CreationFeeCollected` | 1 | `(i128, Address)` | `create_stream` |
| 18 | `FeeChangeProposed` | 1 | `(u32, u64)` | `propose_fee_change` |
| 19 | `FeeChangeExecuted` | 1 | `(u32,)` | `execute_fee_change` |
| 20 | `ContractDeployed` | 1 | `(String, Address)` | `initialize` |
| 21 | `ContractPaused` | 2 | `u64` | `emergency_pause` |
| 22 | `ContractResumed` | 2 | `u64` | `emergency_resume` |
| 23 | `ContractMigrated` | 1 | `(String, String, Address)` | `migrate` |
| 24 | `AdminAction` | 1 | `(String, Address, u64)` | `emergency_pause`, `emergency_resume`, `migrate` |

> Closes [#264](https://github.com/SoroStream/sorostream-contracts/issues/264).
