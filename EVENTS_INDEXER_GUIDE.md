# SoroStream Events - Indexer Quick Reference

Complete guide for building off-chain indexers using SoroStream events.

---

## Quick Start: 5-Minute Setup

### 1. Subscribe to Events

```typescript
import { SorobanRpc } from '@stellar/js-stellar-sdk';

const server = new SorobanRpc.Server('https://soroban-testnet.stellar.org');

// Subscribe to contract events
const eventStream = server.subscribeContractEvents({
  contractId: 'CAA3XNSN7V3DZQV5EMJU5MUK3QZPWQZ7',
  cursor: 'latest',
});

eventStream.on('open', () => console.log('Connected to event stream'));
eventStream.on('message', (event) => {
  console.log('Event received:', event.type);
});
```

### 2. Parse Stream Events

```typescript
function parseStreamEvent(event) {
  const [eventType, streamId] = event.topics;
  
  switch (eventType) {
    case 'StreamCreated':
      return {
        type: 'created',
        streamId,
        sender: event.data[0],
        recipient: event.data[1],
        amount: BigInt(event.data[2]),
        flowRate: BigInt(event.data[3]),
        endTime: BigInt(event.data[4]),
      };

    case 'StreamWithdrawn':
      return {
        type: 'withdrawn',
        streamId,
        recipient: event.data[0],
        amount: BigInt(event.data[1]),
        timestamp: BigInt(event.data[2]),
        totalWithdrawn: BigInt(event.data[3]),
      };

    // ... handle other event types
  }
}
```

### 3. Rebuild Stream State

```typescript
class StreamIndexer {
  streams = new Map(); // stream_id → state

  processEvent(event) {
    const parsed = parseStreamEvent(event);
    
    if (!this.streams.has(parsed.streamId)) {
      this.streams.set(parsed.streamId, {
        id: parsed.streamId,
        created: null,
        withdrawals: [],
        balance: 0n,
        status: 'active',
      });
    }

    const state = this.streams.get(parsed.streamId);

    switch (parsed.type) {
      case 'created':
        state.created = {
          sender: parsed.sender,
          recipient: parsed.recipient,
          amount: parsed.amount,
          flowRate: parsed.flowRate,
          startTime: BigInt(Date.now() / 1000),
          endTime: parsed.endTime,
        };
        state.balance = parsed.amount;
        break;

      case 'withdrawn':
        state.withdrawals.push({
          amount: parsed.amount,
          timestamp: parsed.timestamp,
        });
        state.balance = parsed.amount - BigInt(parsed.totalWithdrawn);
        break;

      case 'cancelled':
        state.status = 'cancelled';
        break;

      case 'completed':
        state.status = 'completed';
        break;
    }
  }

  getStreamState(streamId) {
    return this.streams.get(streamId);
  }
}
```

### 4. Store in Database

```typescript
// Example: Store in PostgreSQL
import pg from 'pg';

const db = new pg.Client({
  connectionString: 'postgresql://...',
});
await db.connect();

// Create tables
await db.query(`
  CREATE TABLE IF NOT EXISTS streams (
    id BIGINT PRIMARY KEY,
    sender TEXT NOT NULL,
    recipient TEXT NOT NULL,
    token TEXT NOT NULL,
    deposit BIGINT NOT NULL,
    flow_rate BIGINT NOT NULL,
    start_time BIGINT NOT NULL,
    end_time BIGINT NOT NULL,
    status TEXT DEFAULT 'active',
    created_at TIMESTAMPTZ DEFAULT NOW()
  );

  CREATE TABLE IF NOT EXISTS stream_events (
    id SERIAL PRIMARY KEY,
    stream_id BIGINT NOT NULL,
    event_type TEXT NOT NULL,
    data JSONB,
    ledger_seq BIGINT NOT NULL,
    timestamp BIGINT NOT NULL,
    FOREIGN KEY (stream_id) REFERENCES streams(id)
  );
`);

// Store event
async function storeEvent(event) {
  const parsed = parseStreamEvent(event);
  
  await db.query(
    `INSERT INTO stream_events (stream_id, event_type, data, ledger_seq, timestamp)
     VALUES ($1, $2, $3, $4, $5)`,
    [
      parsed.streamId,
      parsed.type,
      JSON.stringify(parsed),
      event.ledger.sequence,
      Date.now(),
    ]
  );
}
```

---

## Core Events to Index

### Tier 1: Essential (Must Have)

These 7 events are sufficient to reconstruct stream state:

| Event | Purpose | Data |
|-------|---------|------|
| `StreamCreated` | Initial stream | sender, recipient, amount, rate, end_time |
| `StreamWithdrawn` | Withdrawal | recipient, amount, total_withdrawn |
| `StreamCancelled` | Early cancellation | sender, refund, recipient_amount |
| `StreamToppedUp` | Add tokens | added_amount, new_end_time |
| `StreamCompleted` | Natural end | (none) |
| `StreamPaused` | Pause start | sender |
| `StreamResumed` | Pause end | sender |

### Tier 2: Important (Recommended)

10 more events for complete audit trail:

- `FeeCollected` - Track fees charged
- `StreamTerminatedByRecipient` - Recipient cancellation
- `RecipientTransferred` - Rights transfer
- `StreamExpired` - Marked expired
- `HoldbackReleased` - Holdback settled
- `TranchesWithdrawn` - Step-vesting withdrawal
- `StreamRedirectSet` - Redirect created
- `StreamApproved` - Approval pending→active
- `StreamSenderLocked` - Sender locked stream
- `CreationFeeCollected` - Creation fee charged

### Tier 3: Optional (Analytics)

Additional 20 events for advanced features:

- `AdminAction`, `ContractMigrated` - Contract updates
- `PriceCheckPassed` - Oracle checks
- `WithdrawalStepCompleted` - Step completed
- `DelegateSet`, `DelegateRevoked` - Delegation
- And 15 more...

---

## Database Schema

### Minimal Schema (Tier 1 Events)

```sql
CREATE TABLE streams (
  id BIGINT PRIMARY KEY,
  sender TEXT NOT NULL,
  recipient TEXT NOT NULL,
  token TEXT NOT NULL,
  amount BIGINT NOT NULL,
  flow_rate BIGINT NOT NULL,
  start_time BIGINT NOT NULL,
  end_time BIGINT NOT NULL,
  status TEXT DEFAULT 'active',
  total_withdrawn BIGINT DEFAULT 0,
  paused BOOLEAN DEFAULT FALSE,
  created_at TIMESTAMPTZ DEFAULT NOW(),
  updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE stream_events (
  id SERIAL PRIMARY KEY,
  stream_id BIGINT REFERENCES streams(id),
  event_type TEXT,
  amount BIGINT,
  flow_rate BIGINT,
  timestamp BIGINT,
  ledger_seq BIGINT UNIQUE,
  created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_streams_sender ON streams(sender);
CREATE INDEX idx_streams_recipient ON streams(recipient);
CREATE INDEX idx_streams_status ON streams(status);
CREATE INDEX idx_events_stream_id ON stream_events(stream_id);
CREATE INDEX idx_events_ledger ON stream_events(ledger_seq);
```

### Full Schema (All Events)

```sql
-- Add more columns for advanced features
ALTER TABLE streams ADD COLUMN (
  non_transferable BOOLEAN DEFAULT FALSE,
  auto_renew BOOLEAN DEFAULT FALSE,
  lock_until BIGINT,
  holdback_amount BIGINT DEFAULT 0,
  is_step_vesting BOOLEAN DEFAULT FALSE,
  oracle_price BIGINT,
  price_deviation_bps SMALLINT
);

-- Track withdrawals in detail
CREATE TABLE withdrawals (
  id SERIAL PRIMARY KEY,
  stream_id BIGINT REFERENCES streams(id),
  recipient TEXT NOT NULL,
  amount BIGINT NOT NULL,
  fee_amount BIGINT DEFAULT 0,
  timestamp BIGINT NOT NULL,
  cumulative_withdrawn BIGINT NOT NULL,
  ledger_seq BIGINT NOT NULL,
  created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Track fees
CREATE TABLE fees (
  id SERIAL PRIMARY KEY,
  stream_id BIGINT REFERENCES streams(id),
  amount BIGINT NOT NULL,
  treasury TEXT NOT NULL,
  ledger_seq BIGINT NOT NULL,
  created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Track redirects
CREATE TABLE redirects (
  id SERIAL PRIMARY KEY,
  source_stream_id BIGINT REFERENCES streams(id),
  target_stream_id BIGINT REFERENCES streams(id),
  recipient TEXT NOT NULL,
  created_at TIMESTAMPTZ DEFAULT NOW()
);
```

---

## Query Examples

### Get Current Stream State

```sql
SELECT
  id,
  sender,
  recipient,
  amount,
  flow_rate,
  start_time,
  end_time,
  total_withdrawn,
  status,
  (amount - total_withdrawn) as balance,
  EXTRACT(EPOCH FROM (to_timestamp(end_time) - NOW())) as seconds_remaining
FROM streams
WHERE id = $1;
```

### Calculate Claimable Amount

```sql
SELECT
  s.id,
  s.recipient,
  s.flow_rate,
  (EXTRACT(EPOCH FROM NOW()) - s.start_time)::BIGINT as elapsed_seconds,
  (s.flow_rate * (EXTRACT(EPOCH FROM NOW()) - s.start_time)::BIGINT) - s.total_withdrawn as claimable
FROM streams s
WHERE s.id = $1 AND s.status = 'active';
```

### Stream History

```sql
SELECT
  se.event_type,
  se.timestamp,
  se.amount,
  se.ledger_seq
FROM stream_events se
WHERE se.stream_id = $1
ORDER BY se.ledger_seq ASC;
```

### Sender Activity

```sql
SELECT
  COUNT(*) as total_streams,
  COUNT(CASE WHEN status = 'active' THEN 1 END) as active_streams,
  SUM(amount) as total_deposit,
  MAX(created_at) as latest_stream
FROM streams
WHERE sender = $1;
```

### Fee Collection

```sql
SELECT
  DATE_TRUNC('day', se.created_at)::DATE as date,
  SUM(amount) as daily_fees,
  COUNT(*) as fee_count
FROM fees se
GROUP BY DATE_TRUNC('day', se.created_at)
ORDER BY date DESC;
```

---

## Event Processing Flow

```
1. Subscribe to Events
   ↓
2. Listen for New Events
   ├─ Parse Event Data
   ├─ Validate Event
   └─ Extract Fields
   ↓
3. Process Event
   ├─ Update Stream State
   ├─ Calculate Balances
   └─ Track History
   ↓
4. Persist to Database
   ├─ Update streams table
   ├─ Insert event log
   └─ Update indices
   ↓
5. Query via API
   ├─ Get stream state
   ├─ Calculate claimable
   └─ Show history
```

---

## Real-Time Dashboard

```typescript
// Example: Stream dashboard updating in real-time

class StreamDashboard {
  constructor(indexer) {
    this.indexer = indexer;
    this.listeners = [];
  }

  subscribe(callback) {
    this.listeners.push(callback);
  }

  async getStreamData(streamId) {
    const state = this.indexer.getStreamState(streamId);
    const now = BigInt(Date.now() / 1000);
    
    return {
      stream_id: streamId,
      status: state.status,
      sender: state.created?.sender,
      recipient: state.created?.recipient,
      deposit: state.created?.amount,
      flow_rate: state.created?.flowRate,
      total_withdrawn: state.balance.totalWithdrawn,
      balance: state.balance.current,
      claimable: state.created
        ? (state.created.flowRate * (now - state.created.startTime)) - state.balance.totalWithdrawn
        : 0n,
      end_time: state.created?.endTime,
      time_remaining: state.created
        ? state.created.endTime - now
        : 0n,
      withdrawal_history: state.withdrawals,
    };
  }

  notifyListeners(streamId) {
    const data = this.getStreamData(streamId);
    this.listeners.forEach(cb => cb(data));
  }
}
```

---

## Testing Your Indexer

### Unit Tests

```typescript
describe('StreamIndexer', () => {
  let indexer;

  beforeEach(() => {
    indexer = new StreamIndexer();
  });

  test('handles StreamCreated event', () => {
    const event = {
      type: 'StreamCreated',
      streamId: 123n,
      sender: 'GAAAA',
      recipient: 'GBBBB',
      amount: 1000n,
      flowRate: 1n,
      endTime: 9999999n,
    };

    indexer.processEvent(event);
    const state = indexer.getStreamState(123n);

    expect(state.status).toBe('active');
    expect(state.balance).toBe(1000n);
  });

  test('handles StreamWithdrawn event', () => {
    // ... create stream first
    // ... then withdraw
    expect(state.balance).toBe(800n); // 1000 - 200
  });

  // ... more tests
});
```

### Integration Tests

```typescript
async function testIndexerIntegration() {
  // 1. Create stream on contract
  const streamId = await createStream(...);
  
  // 2. Wait for event
  await waitForEvent('StreamCreated', streamId);
  
  // 3. Verify in indexer
  const indexed = indexer.getStreamState(streamId);
  assert(indexed.status === 'active');
  
  // 4. Withdraw tokens
  await withdraw(streamId, ...);
  
  // 5. Verify updated
  await waitForEvent('StreamWithdrawn', streamId);
  const updated = indexer.getStreamState(streamId);
  assert(updated.balance < indexed.balance);
}
```

---

## Performance Tips

### 1. Batch Processing

```typescript
// Don't process events one-by-one
const events = await fetchEvents(batch_size = 1000);

// Instead, batch insert
const insertMany = async (events) => {
  const query = `
    INSERT INTO stream_events (stream_id, event_type, data, ledger_seq)
    VALUES ${events.map((_, i) => `($${i*4+1}, $${i*4+2}, $${i*4+3}, $${i*4+4})`).join(',')}
  `;
  await db.query(query, events.flatMap(e => [
    e.streamId, e.type, JSON.stringify(e), e.ledgerSeq
  ]));
};
```

### 2. Indexing Strategy

```sql
-- Priority indices for common queries
CREATE INDEX CONCURRENTLY idx_streams_status_recipient 
  ON streams(status, recipient);

CREATE INDEX CONCURRENTLY idx_events_stream_type
  ON stream_events(stream_id, event_type);

-- Partitioning for large tables
CREATE TABLE stream_events_2024_h1 PARTITION OF stream_events
  FOR VALUES FROM (1704067200) TO (1719792000);
```

### 3. Cache Hot Data

```typescript
// Cache frequently accessed streams
const cache = new Map();
const CACHE_TTL = 60_000; // 1 minute

function getStreamState(streamId) {
  const cached = cache.get(streamId);
  if (cached && Date.now() - cached.timestamp < CACHE_TTL) {
    return cached.state;
  }

  const state = this.indexer.getStreamState(streamId);
  cache.set(streamId, { state, timestamp: Date.now() });
  return state;
}
```

---

## Common Pitfalls

❌ **Don't**: Query contract for every stream
✅ **Do**: Use indexed event data

❌ **Don't**: Assume events arrive in order
✅ **Do**: Sort by ledger_seq before processing

❌ **Don't**: Miss events by starting indexing too late
✅ **Do**: Use archive nodes for historical data

❌ **Don't**: Calculate state from single events
✅ **Do**: Replay all events to build state

❌ **Don't**: Ignore pause/resume in calculations
✅ **Do**: Track paused time and adjust end times

---

## Checklist for Production

- [ ] All Tier 1 events processed
- [ ] Stream state correctly rebuilt
- [ ] Claimable amount calculation verified
- [ ] Event ordering by ledger_seq
- [ ] Database transactions atomic
- [ ] Indices created for common queries
- [ ] Batch processing implemented
- [ ] Error handling for missed events
- [ ] Fallback to contract queries if needed
- [ ] Monitoring and alerts set up
- [ ] Unit tests > 80% coverage
- [ ] Integration tests pass
- [ ] Load tested with realistic throughput
- [ ] Handles network interruptions
- [ ] Data validation and sanitization

---

## Support & Resources

- **Soroban RPC Docs**: https://soroban.stellar.org/docs/rpc-reference
- **Event Subscription**: Check soroban-rpc documentation
- **SoroStream Docs**: See EVENTS_SYSTEM.md
- **Example Indexer**: Available in SoroStream GitHub repo

---

## Summary

Building a SoroStream indexer requires:

✅ Subscribing to Soroban events
✅ Parsing event data
✅ Rebuilding stream state
✅ Storing in persistent database
✅ Providing query API to clients

With 63 events covering every state change, indexers can reconstruct complete history and provide real-time stream state to users.
