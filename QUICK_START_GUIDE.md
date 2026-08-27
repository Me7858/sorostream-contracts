# SoroStream Quick Start Guide

Get up and running with SoroStream in 5 minutes.

## Installation

### Node.js / TypeScript

```bash
npm install @sorostream/sdk soroban-sdk
```

### Initialize Client

```typescript
import { SoroStreamClient } from '@sorostream/sdk';

const client = new SoroStreamClient({
  rpc: 'https://soroban-testnet.stellar.org',
  contractId: 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4',
  // Network: testnet, mainnet, etc.
});
```

## Create Your First Stream

### 1. Basic Linear Stream (30 days)

```typescript
const streamId = await client.createStream({
  sender: senderAddress,           // Your account
  recipient: recipientAddress,     // Who receives tokens
  token: usdcContractAddress,     // Stellar asset contract
  amount: '100000000',            // 1 USDC (in stroops)
  durationSeconds: 2_592_000,     // 30 days in seconds
  cliffSeconds: 0,                // No cliff (tokens flow immediately)
  nonce: 0,                       // Unique per stream
  autoRenew: false,               // Don't auto-renew
  renewCount: null,               // N/A
  lockUntil: 0,                   // No lock period
  allowRecipientTermination: false, // Recipient can't cancel
  holdbackAmount: 0,              // No escrow holdback
  withdrawalSteps: null,          // No step withdrawals
  minWithdrawalAmount: null,      // No minimum
  nonTransferable: false,         // Recipient can be changed
  requiresRecipientApproval: false, // No approval needed
  enforceRecipientAllowlist: false, // No allowlist check
});

console.log(`Stream created: ${streamId}`);
```

### 2. Withdraw Tokens

```typescript
// Check how much is available
const claimable = await client.getClaimable(streamId);
console.log(`You can withdraw: ${claimable} stroops`);

// Withdraw all earned tokens
await client.withdraw({
  streamId,
  recipient: recipientAddress,
});

console.log('Tokens withdrawn');
```

### 3. Check Stream Status

```typescript
const stream = await client.getStream(streamId);

console.log(`
  Status: ${stream.status}
  Flow Rate: ${stream.flowRate} stroops/sec
  End Time: ${new Date(stream.endTime * 1000)}
  Total Withdrawn: ${stream.totalWithdrawn}
`);
```

## Common Use Cases

### Salary Payments (Monthly Subscription)

```typescript
// Employee receives salary streamed over 1 month, auto-renews yearly
const streamId = await client.createStream({
  sender: company,
  recipient: employee,
  token: usdc,
  amount: String(100_000_000 * 12),  // Annual salary, paid monthly
  durationSeconds: 2_592_000 * 12,   // 12 months
  nonce: Math.random(),
  autoRenew: true,
  renewCount: 5,  // Renew up to 5 times (5 years max)
});
```

### Vesting Schedule (3-Year Cliff + Linear)

```typescript
// 1M USDC, 3-year cliff, then streams over 1 year
const streamId = await client.createStream({
  sender: company,
  recipient: employee,
  token: usdc,
  amount: '1000000000000',           // 10M USDC
  durationSeconds: 126_144_000,      // 4 years total
  cliffSeconds: 94_608_000,          // 3-year cliff
  nonce: Math.random(),
  autoRenew: false,
});
```

### Cancel a Stream

```typescript
const { refund, earned } = await client.cancelStream({
  streamId,
  sender: senderAddress,
});

console.log(`
  Recipient earned: ${earned}
  Sender refunded: ${refund}
`);
```

### Top-Up a Stream

```typescript
// Add more funds to an active stream
await client.topUp({
  streamId,
  sender: senderAddress,
  token: usdcAddress,
  amount: '100000000',  // 1 USDC
});

console.log('Stream topped up');
```

## Batch Operations

### Create Multiple Streams (Payroll)

```typescript
const employees = [
  { address: emp1, salary: '100000000' },     // 1 USDC
  { address: emp2, salary: '150000000' },     // 1.5 USDC
  { address: emp3, salary: '120000000' },     // 1.2 USDC
];

const streamIds = await client.batchCreateStream({
  sender: company,
  recipients: employees.map(e => e.address),
  amounts: employees.map(e => e.salary),
  tokens: [usdc, usdc, usdc],
  durationSeconds: 2_592_000 * 12,  // 1 year
  autoRenew: true,
  renewCount: null,
  lockUntils: [0, 0, 0],
  nonce: Math.random(),
});

console.log(`Created ${streamIds.length} streams`);
```

### Withdraw from Multiple Streams

```typescript
const amounts = await client.batchWithdraw({
  streamIds: [stream1, stream2, stream3],
  recipient: address,
});

console.log(`Withdrew: ${amounts}`);
```

## Query Streams

### Get All Streams for an Address

```typescript
// Streams I send
const sentStreams = await client.getStreamsBySender(myAddress, 0, 100);

// Streams I receive
const receivedStreams = await client.getStreamsByRecipient(myAddress, 0, 100);

// Only active streams
const activeStreams = await client.getActiveStreamsByRecipient(myAddress);
```

### Advanced Filtering

```typescript
const streams = await client.queryStreams({
  filter: {
    status: 'Active',
    asset: usdcAddress,
    sender: companyAddress,
    recipient: null,  // Any recipient
  },
  start: 0,
  limit: 50,
});
```

## Pause & Resume

```typescript
// Pause a stream (sender only)
await client.pauseStream(streamId, senderAddress);

// Recipient can't withdraw while paused

// Resume (sender restarts, end time extends by pause duration)
await client.resumeStream(streamId, senderAddress);
```

## Manage Stream Redirect

```typescript
// Recipient redirects withdrawals to top-up another stream
await client.setRedirect(streamId, targetStreamId, recipientAddress);

// Now withdrawing from streamId tops up targetStreamId

// Clear redirect
await client.clearRedirect(streamId, recipientAddress);
```

## Handle Errors

```typescript
try {
  await client.withdraw(streamId, recipientAddress);
} catch (error) {
  if (error.code === 'StreamNotFound') {
    console.log('Stream does not exist');
  } else if (error.code === 'NotRecipient') {
    console.log('You are not the recipient');
  } else if (error.code === 'StreamNotActive') {
    console.log('Stream is paused or completed');
  } else {
    console.log(`Error: ${error.message}`);
  }
}
```

## Monitor Stream Health

```typescript
const health = await client.getStreamHealth(streamId);

console.log(`
  TTL Status: ${health.status}
  Ledgers Remaining: ${health.ttlRemainingLedgers}
`);

if (health.status === 'AtRisk') {
  console.log('WARNING: Stream storage expiring soon!');
  await client.bumpStreamTtl(streamId);  // Extend TTL
}
```

## Next Steps

- Read the [Comprehensive API Reference](./COMPREHENSIVE_API_REFERENCE.md) for all methods
- Check [ARCHITECTURE.md](./ARCHITECTURE.md) for design details
- Explore [examples/](./examples/) for more sample code
- File an issue on [GitHub](https://github.com/SoroStream/sorostream-contracts) with questions

## Tips

1. **Always use nonce values** - Prevents duplicate stream creation
2. **Check `getClaimable()` before withdraw** - Confirm available balance
3. **Use batch operations** - Save gas with multi-stream operations
4. **Plan for rounding** - Flow rate uses integer division; dust refunded at end
5. **Set appropriate parameters** - Cliffs, locks, and approvals enhance security
