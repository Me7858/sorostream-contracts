# Protocol Stats Data Structure Reference

## Complete Data Model

```
ProtocolStats {
  total_streams: u64              // All streams ever created
  active_streams: u64             // Currently active only
  total_volume: i128              // Total value locked (stroops)
  status_breakdown: StatusStats   // Count by status
  asset_breakdown: Vec<AssetStats> // Per-token metrics
}
```

## Detailed Structure Hierarchy

```
Protocol Level
└─ ProtocolStats
   ├─ total_streams (u64)
   ├─ active_streams (u64)
   ├─ total_volume (i128)
   │
   ├─ StatusStats (status_breakdown)
   │  ├─ active (u64)
   │  ├─ cancelled (u64)
   │  ├─ completed (u64)
   │  ├─ paused (u64)
   │  ├─ expired (u64)
   │  └─ pending_approval (u64)
   │
   └─ Vec<AssetStats> (asset_breakdown) [sorted by total_volume DESC]
      └─ AssetStats[0..N]
         ├─ token (Address)              // Token contract address
         ├─ stream_count (u64)           // Total streams using this token
         ├─ total_volume (i128)          // Total value for this token (stroops)
         └─ active_streams (u64)         // Active streams for this token
```

## Status State Machine

```
StreamStatus variants tracked in StatusStats:

    ┌─────────────────────────────────────────────────────┐
    │                  Stream States                       │
    └─────────────────────────────────────────────────────┘

    ┌──────────────┐
    │   PENDING    │ ← Awaiting recipient approval
    │ APPROVAL     │
    └──────┬───────┘
           │ (recipient approves)
           ▼
    ┌──────────────┐
    │    ACTIVE    │ ← Tokens flowing
    └──────┬───────┘
           │
      ┌────┴──────┬──────────┐
      │           │          │
      ▼           ▼          ▼
   PAUSED    CANCELLED   COMPLETED
   (sender)  (before    (natural
             end time)   end time)
      │
      └──► ACTIVE (resume)
           │
           ▼
        EXPIRED
      (post end_time)
```

## Sample Data

### Minimal Protocol (No Streams)
```json
{
  "total_streams": 0,
  "active_streams": 0,
  "total_volume": 0,
  "status_breakdown": {
    "active": 0,
    "cancelled": 0,
    "completed": 0,
    "paused": 0,
    "expired": 0,
    "pending_approval": 0
  },
  "asset_breakdown": []
}
```

### Single Token Protocol
```json
{
  "total_streams": 342,
  "active_streams": 285,
  "total_volume": 1200000000000,
  "status_breakdown": {
    "active": 285,
    "cancelled": 12,
    "completed": 42,
    "paused": 2,
    "expired": 1,
    "pending_approval": 0
  },
  "asset_breakdown": [
    {
      "token": "CAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4",
      "stream_count": 342,
      "total_volume": 1200000000000,
      "active_streams": 285
    }
  ]
}
```

### Multi-Token Protocol
```json
{
  "total_streams": 450,
  "active_streams": 380,
  "total_volume": 2500000000000,
  "status_breakdown": {
    "active": 380,
    "cancelled": 20,
    "completed": 45,
    "paused": 3,
    "expired": 2,
    "pending_approval": 0
  },
  "asset_breakdown": [
    {
      "token": "CAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4",
      "stream_count": 280,
      "total_volume": 1600000000000,
      "active_streams": 235
    },
    {
      "token": "CBQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4",
      "stream_count": 120,
      "total_volume": 700000000000,
      "active_streams": 105
    },
    {
      "token": "CCQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4",
      "stream_count": 50,
      "total_volume": 200000000000,
      "active_streams": 40
    }
  ]
}
```

## Field Descriptions

### ProtocolStats Top Level

| Field | Type | Description |
|-------|------|-------------|
| `total_streams` | u64 | Cumulative count of all streams ever created, including cancelled/expired |
| `active_streams` | u64 | Current count of streams with `status == Active` |
| `total_volume` | i128 | Sum of all stream deposits in stroops |
| `status_breakdown` | StatusStats | Nested object with count per status |
| `asset_breakdown` | Vec<AssetStats> | Array of per-token metrics, sorted by volume DESC |

### StatusStats (status_breakdown)

| Field | Type | Description |
|-------|------|-------------|
| `active` | u64 | Streams actively flowing tokens |
| `cancelled` | u64 | Streams ended early by sender |
| `completed` | u64 | Streams that reached natural end time |
| `paused` | u64 | Streams temporarily paused |
| `expired` | u64 | Streams past end_time marked as expired |
| `pending_approval` | u64 | Streams awaiting recipient approval (no tokens flowing) |

### AssetStats (asset_breakdown element)

| Field | Type | Description |
|-------|------|-------------|
| `token` | Address | Stellar contract address of token (SAC format) |
| `stream_count` | u64 | Total streams using this token |
| `total_volume` | i128 | Total value locked in this token (stroops) |
| `active_streams` | u64 | Active streams for this token |

## Invariants

The following properties should always hold:

1. **Sum of status counts = total_streams**
   ```
   active + cancelled + completed + paused + expired + pending_approval == total_streams
   ```

2. **Active streams count matches status breakdown**
   ```
   active_streams == status_breakdown.active
   ```

3. **Sum of asset volumes = total volume**
   ```
   sum(asset.total_volume for asset in asset_breakdown) == total_volume
   ```

4. **Sum of asset active streams = total active**
   ```
   sum(asset.active_streams for asset in asset_breakdown) == active_streams
   ```

5. **Asset breakdown sorted by volume descending**
   ```
   asset_breakdown[i].total_volume >= asset_breakdown[i+1].total_volume
   ```

6. **No duplicate tokens in breakdown**
   ```
   All asset_breakdown[*].token values are unique
   ```

## Usage in Dashboards

### KPI Metrics
```
Total Value Locked = total_volume / 10^7 (assuming 7-decimal token)
Streams Created = total_streams
Active Streams = active_streams
Completed Rate = completed / total_streams (%)
```

### Status Distribution
```
Active %      = active / total_streams
Completed %   = completed / total_streams
Cancelled %   = cancelled / total_streams
Paused %      = paused / total_streams
Expired %     = expired / total_streams
Pending %     = pending_approval / total_streams
```

### Per-Asset Insights
```
USDC Market Share = USDC.total_volume / total_volume (%)
USDC Active Rate  = USDC.active_streams / USDC.stream_count (%)
USDC Avg Stream   = USDC.total_volume / USDC.stream_count (stroops)
```

## Type Definitions in Rust

```rust
#[contracttype]
#[derive(Clone, Debug)]
pub struct StatusStats {
    pub active: u64,
    pub cancelled: u64,
    pub completed: u64,
    pub paused: u64,
    pub expired: u64,
    pub pending_approval: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct AssetStats {
    pub token: Address,
    pub stream_count: u64,
    pub total_volume: i128,
    pub active_streams: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct ProtocolStats {
    pub total_streams: u64,
    pub active_streams: u64,
    pub total_volume: i128,
    pub status_breakdown: StatusStats,
    pub asset_breakdown: Vec<AssetStats>,
}
```

## Size Estimates

| Component | Typical Size |
|-----------|--------------|
| StatusStats | 48 bytes (6 × u64) |
| Single AssetStats | 40 bytes (Address + 3 × u64) |
| ProtocolStats Header | 32 bytes (3 × u64) + StatusStats |
| Per Asset in Breakdown | 40 bytes |
| **Total (1 asset)** | ~160 bytes |
| **Total (5 assets)** | ~360 bytes |
| **Total (20 assets)** | ~1,040 bytes |

