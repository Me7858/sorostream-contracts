# Protocol Stats Entry Point - Usage Guide

## Overview

The `get_protocol_stats()` entry point provides a single, efficient call to retrieve comprehensive protocol-level metrics. This replaces the need for dashboards to iterate through all streams individually.

## Function Signature

```rust
pub fn get_protocol_stats(env: Env) -> ProtocolStats
```

## Return Type: ProtocolStats

```rust
pub struct ProtocolStats {
    pub total_streams: u64,          // Total streams ever created
    pub active_streams: u64,         // Currently active streams
    pub total_volume: i128,          // Total value locked (stroops)
    pub status_breakdown: StatusStats,    // Count per status
    pub asset_breakdown: Vec<AssetStats>, // Per-token metrics (sorted by volume)
}

pub struct StatusStats {
    pub active: u64,              // Active streams
    pub cancelled: u64,           // Cancelled streams
    pub completed: u64,           // Completed (naturally ended)
    pub paused: u64,             // Paused streams
    pub expired: u64,            // Expired streams
    pub pending_approval: u64,   // Awaiting recipient approval
}

pub struct AssetStats {
    pub token: Address,           // Token contract address
    pub stream_count: u64,        // Total streams with this token
    pub total_volume: i128,       // Total value locked (stroops)
    pub active_streams: u64,      // Currently active with this token
}
```

## Usage Examples

### Stellar CLI

```bash
stellar contract invoke \
  --id CONTRACT_ID \
  --source-account ACCOUNT \
  --network testnet \
  -- get_protocol_stats
```

### JavaScript SDK

```javascript
const client = new SoroStreamClient({ publicKey, rpcUrl });

const stats = await client.get_protocol_stats();

console.log(`Total value locked: ${stats.total_volume / 1e7} USDC`);
console.log(`Active streams: ${stats.active_streams}`);
console.log(`Total streams: ${stats.total_streams}`);

// Status breakdown
console.log(`Status distribution:`, {
  active: stats.status_breakdown.active,
  completed: stats.status_breakdown.completed,
  cancelled: stats.status_breakdown.cancelled,
  paused: stats.status_breakdown.paused,
});

// Top 3 assets by volume
stats.asset_breakdown.slice(0, 3).forEach((asset, i) => {
  console.log(`${i+1}. Token: ${asset.token}`);
  console.log(`   Volume: ${asset.total_volume / 1e7}`);
  console.log(`   Streams: ${asset.stream_count} (${asset.active_streams} active)`);
});
```

### Python/Rust Integration

```python
from sorostream_client import SoroStreamClient

client = SoroStreamClient(contract_id, env)
stats = client.get_protocol_stats()

# Protocol health dashboard
dashboard = {
    "tvl_usd": stats.total_volume / 1e7,  # assuming USDC (7 decimals)
    "total_streams": stats.total_streams,
    "active_streams": stats.active_streams,
    "efficiency": stats.total_volume / stats.total_streams if stats.total_streams > 0 else 0,
}

# Status distribution
status_dist = {
    "active": stats.status_breakdown.active,
    "completed": stats.status_breakdown.completed,
    "cancelled": stats.status_breakdown.cancelled,
}

# Top assets
top_assets = [
    {
        "token": asset.token,
        "tvl": asset.total_volume / 1e7,
        "stream_count": asset.stream_count,
        "active_count": asset.active_streams,
    }
    for asset in stats.asset_breakdown[:5]
]
```

## Dashboard Components

### 1. Protocol Summary Card
```
Total Value Locked: $1.2M USDC
Total Streams: 342
Active Streams: 285
Average Stream Value: $3,509
```

### 2. Status Distribution Pie Chart
```
Active:     285 (83%)
Completed:  42  (12%)
Cancelled:  10  (3%)
Paused:     5   (1%)
```

### 3. Top Assets Table
```
| Token  | Volume      | Streams | Active | % of TVL |
|--------|-------------|---------|--------|----------|
| USDC   | $800,000    | 230     | 190    | 65%      |
| EUR    | $300,000    | 85      | 72     | 25%      |
| GBP    | $100,000    | 27      | 23     | 10%      |
```

### 4. Stream Lifecycle Metrics
```
New streams today:     24
Completed streams:     8
Cancelled streams:     2
Total activity:        34 transactions
```

## Performance Characteristics

| Metric | Value |
|--------|-------|
| Time Complexity | O(N) - single pass |
| Space Complexity | O(A) - number of unique assets |
| Gas Cost | Linear with stream count |
| Auth Required | None |
| State Mutations | None |
| Cacheable | Yes (data is point-in-time) |

## Recommendations for Dashboards

### Caching Strategy
```
- Cache for 30-60 seconds for high-traffic dashboards
- Invalidate on stream state changes (create, cancel, pause, resume)
- Use WebSocket subscriptions for real-time updates on key events
```

### Query Patterns
```
// Efficient: Single call gets all metrics
const stats = await client.get_protocol_stats();

// Inefficient: Iterating all streams (avoid)
for (const id of allStreamIds) {
  const stream = await client.get_stream(id);
  // ... manually aggregate
}
```

### Rate Limiting Recommendations
```
- General public dashboard: 1 call per minute
- Admin dashboard: 1 call per 10 seconds
- Real-time monitoring: Subscriptions + periodic validation
```

## Edge Cases

### Empty Protocol (No Streams)
```rust
ProtocolStats {
    total_streams: 0,
    active_streams: 0,
    total_volume: 0,
    status_breakdown: StatusStats { /* all zeros */ },
    asset_breakdown: Vec::new(),
}
```

### Single Token
```rust
// asset_breakdown contains exactly one entry
asset_breakdown: [
    AssetStats {
        token: USDC_ADDRESS,
        stream_count: 342,
        total_volume: 1_200_000_000_000, // 1.2M USDC in stroops
        active_streams: 285,
    }
]
```

### Multiple Tokens Sorted by Volume
```rust
// Largest first
asset_breakdown: [
    AssetStats { token: USDC, total_volume: 1_000_000_000_000, ... },
    AssetStats { token: EUR, total_volume: 300_000_000_000, ... },
    AssetStats { token: GBP, total_volume: 100_000_000_000, ... },
]
```

## Migration from get_stats()

The original `get_stats()` function is still available for backwards compatibility:

```rust
// Old way (still works)
let stats = client.get_stats(); // Returns Stats { total_streams, active_streams, total_volume }

// New way (recommended)
let protocol_stats = client.get_protocol_stats(); // Returns detailed ProtocolStats
```

Use `get_protocol_stats()` for new implementations. The `Stats` struct is deprecated but maintained for existing integrations.

## Testing the Integration

```rust
#[test]
fn test_dashboard_metrics() {
    let client = setup_test_client();
    
    // Create diverse streams
    client.create_stream(USDC, 100_000, 1000);
    client.create_stream(USDC, 50_000, 500);
    client.create_stream(EUR, 75_000, 800);
    
    let stats = client.get_protocol_stats();
    
    // Validate aggregates
    assert_eq!(stats.total_volume, 225_000);
    assert_eq!(stats.total_streams, 3);
    assert_eq!(stats.active_streams, 3);
    
    // Validate asset breakdown
    assert_eq!(stats.asset_breakdown.len(), 2);
    assert_eq!(stats.asset_breakdown[0].total_volume, 150_000); // USDC first (higher volume)
    assert_eq!(stats.asset_breakdown[0].stream_count, 2);
}
```

## Support and Questions

For integration support or questions about the protocol stats:
1. Check the [PROTOCOL_STATS_IMPLEMENTATION.md](./PROTOCOL_STATS_IMPLEMENTATION.md) for technical details
2. Review test cases in `contracts/stream/src/test.rs` for usage patterns
3. Open an issue in the repository for problems or feature requests
