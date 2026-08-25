# Query Streams Implementation

## Overview

This document describes the implementation of the `queryStreams` entry point for the SoroStream contract, which enables efficient on-chain queries with optional filters for status, asset, sender, and recipient without requiring iteration over all stream records.

## Changes Made

### 1. Type Definition: StreamQueryFilter

**File**: `contracts/stream/src/types.rs`

Added a new `#[contracttype]` struct `StreamQueryFilter` with the following optional filter fields:
- `status: Option<StreamStatus>` - Filter by stream status (Active, Cancelled, Completed, etc.)
- `asset: Option<Address>` - Filter by token contract address
- `sender: Option<Address>` - Filter by stream creator address
- `recipient: Option<Address>` - Filter by stream beneficiary address

All fields are optional (`None` values are ignored), and multiple filters are combined with AND logic.

### 2. Query Implementation

**File**: `contracts/stream/src/lib.rs`

Implemented the `query_streams` function with the following features:
- Accepts a `StreamQueryFilter` and pagination parameters (`start` and `limit`)
- Iterates through all global streams and applies all non-None filter conditions
- Supports efficient pagination with a capped limit of 20 results per query
- Returns a `Vec<Stream>` with matching results

Algorithm:
1. Retrieve total global stream count from storage
2. Iterate through all streams sequentially
3. For each stream, check if all specified filter criteria match
4. Collect matching streams
5. Apply pagination: skip first `start` results, return up to `min(limit, 20)` results

### 3. Interface Definition

**File**: `contracts/stream/src/interface.rs`

Added the `query_streams` method to the `SoroStreamInterface` trait:
```rust
fn query_streams(env: Env, filter: StreamQueryFilter, start: u32, limit: u32) -> Vec<Stream>;
```

This enables:
- Type-safe client generation via `#[contractclient]`
- Proper type signatures for remote calls
- Seamless integration with the Stellar SDK

### 4. Exports

**File**: `contracts/stream/src/lib.rs`

Exported `StreamQueryFilter` from the types module so it's accessible to SDK clients:
```rust
pub use types::{..., StreamQueryFilter};
```

## Usage Examples

### Query all active streams from a specific sender:
```rust
let filter = StreamQueryFilter {
    status: Some(StreamStatus::Active),
    asset: None,
    sender: Some(sender_address),
    recipient: None,
};
let results = query_streams(env, filter, 0, 20);
```

### Query all streams involving USDC:
```rust
let filter = StreamQueryFilter {
    status: None,
    asset: Some(usdc_token_address),
    sender: None,
    recipient: None,
};
let results = query_streams(env, filter, 0, 10);
```

### Query completed streams to a specific recipient (with pagination):
```rust
let filter = StreamQueryFilter {
    status: Some(StreamStatus::Completed),
    asset: None,
    sender: None,
    recipient: Some(recipient_address),
};

// Get first 20 results
let page1 = query_streams(env, filter.clone(), 0, 20);

// Get next 20 results
let page2 = query_streams(env, filter, 20, 20);
```

## Test Coverage

**File**: `contracts/stream/src/test.rs`

Comprehensive test suite covering:

1. **Empty Filter** (`test_query_streams_empty_filter`)
   - Verifies all streams are returned when no filters are specified

2. **Single Filter Tests**:
   - `test_query_streams_by_status` - Filter by stream status
   - `test_query_streams_by_sender` - Filter by sender address
   - `test_query_streams_by_recipient` - Filter by recipient address
   - `test_query_streams_by_asset` - Filter by token contract

3. **Multiple Filters** (`test_query_streams_multiple_filters`)
   - Tests AND logic with 2+ filters active
   - Verifies only streams matching ALL criteria are returned

4. **Pagination** (`test_query_streams_pagination`)
   - Tests pagination with different start/limit values
   - Verifies no overlap between pages
   - Tests edge cases (partial final page)

5. **Limit Capping** (`test_query_streams_limit_capped_at_20`)
   - Ensures limit is capped at 20 even if higher value requested
   - Tests with > 20 streams in contract

## Performance Characteristics

- **Time Complexity**: O(n) where n is the total number of streams in the contract
  - Iterates through all streams once
  - Each filter check is O(1) (address comparison)
  - Pagination is O(min(limit, 20))

- **Storage**: O(limit) - only returns up to 20 streams at a time

- **Gas Cost**: Proportional to number of streams iterated
  - Recommended: use more specific filters when possible
  - For contracts with many streams, combine sender + status for optimal results

## Design Notes

### Why iterate all streams?
The implementation iterates all global streams rather than using pre-indexed lookups because:
1. Enables arbitrary combinations of independent filters
2. Avoids maintaining complex multi-dimensional indexes
3. Supports future filter criteria without schema changes
4. Pagination is simple and deterministic

### Pagination Strategy
- Standard offset-based pagination (start, limit)
- Limit hard-capped at 20 to prevent excessive gas consumption
- Clients can make multiple calls to retrieve larger result sets
- Deterministic ordering based on global stream index

### Filter Combination
- All non-None filters are combined with AND logic
- Empty filter (`all None`) returns all streams
- Allows for progressive refinement of queries

## Future Enhancements

Potential improvements for future versions:
1. Add compound index support for common filter combinations
2. Support query result ordering (by ID, status, amount, etc.)
3. Add time-range filtering (by start_time, end_time)
4. Implement stream amount filters (min/max deposit)
5. Add tag-based filtering integration

## Integration with Existing Functions

The `query_streams` function complements existing query functions:
- `get_streams_by_sender()` - optimized sender-only lookup
- `get_streams_by_recipient()` - optimized recipient-only lookup
- `get_streams_by_tag()` - optimized tag-based lookup
- `get_active_streams_by_sender()` - optimized active sender lookup
- `query_streams()` - **flexible multi-filter queries** (NEW)

Clients should use specific functions when querying by single dimension, and `query_streams()` when combining multiple filter criteria.
