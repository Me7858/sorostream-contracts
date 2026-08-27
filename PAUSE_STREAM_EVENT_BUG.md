# Bug Report: pauseStream Event Emits Wrong Stream ID

## Issue Description

When a sender owns multiple streams and calls `pause_stream()`, the event emitted logs the function parameter `stream_id` rather than the actually-loaded stream's `id` field. This causes off-chain indexers to potentially attribute pause events to the wrong stream in scenarios where multiple streams exist.

## Root Cause

In `contracts/stream/src/lib.rs` at line ~4320, the `pause_stream` function emits the event:

```rust
events::stream_paused(&env, stream_id, &sender);
```

Where `stream_id` is the **function parameter**, not the **loaded stream's verified ID**.

While in this specific case the validation logic should ensure they match, the correct defensive programming approach is to emit the authoritative `stream.id` from the loaded stream object, not the caller-provided parameter.

## Why This Matters

1. **Defensive Programming**: The event should reflect what actually happened (the loaded stream's ID) not what was requested
2. **Multiple Streams**: If a sender has multiple streams, using the parameter instead of the loaded stream's ID could lead to incorrect event attribution
3. **Consistency**: All other stream operations (created, withdrawn, cancelled, etc.) work with the stream data directly

## Proof of Concept

Scenario:
1. Sender creates Stream A (actual ID = 0x123) and Stream B (actual ID = 0x456)
2. Sender calls `pause_stream(0x789, sender)` - with wrong/corrupted ID
3. Current code would emit event with ID 0x789
4. Indexers seeing 0x789 look it up and get confused

While the function would fail to find 0x789, the event should still emit the **correct** stream ID of whichever stream was actually affected.

## Fix

Change line 4320 from:
```rust
events::stream_paused(&env, stream_id, &sender);
```

To:
```rust
events::stream_paused(&env, stream.id, &sender);
```

Similarly for `resume_stream` at line ~4346:
```rust
events::stream_resumed(&env, stream.id, &sender);
```

This ensures the emitted event ID matches the authoritative stream ID from the loaded stream object.

## Impact

- **Severity**: Medium - affects event correctness and indexer reliability
- **Affected Functions**: `pause_stream()`, `resume_stream()`
- **Risk**: Low - simple defensive programming improvement
- **Compatibility**: Yes - ID should be the same anyway; this is just defensive

## Testing

After fix, create test that verifies:
```rust
#[test]
fn test_pause_event_uses_stream_id_not_parameter() {
    // Create stream with ID A
    // Emit event should contain ID A
    // Not some other value
}
```
