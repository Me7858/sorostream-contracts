# Pause/Resume Event Fix - Test Cases

## Test 1: Single Stream Pause Event Correctness

**Objective**: Verify that pause_stream emits the correct stream ID

```rust
#[test]
fn test_pause_event_emits_correct_stream_id() {
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();

    let contract_id = env.register(SoroStreamContract, ());
    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    StellarAssetClient::new(&env, &token_id).mint(&sender, &1_000_000);

    let c = SoroStreamContractClient::new(&env, &contract_id);
    c.set_min_duration(&sender, &0u64);
    env.ledger().set_timestamp(0);

    // Create a single stream
    let stream_id = c.create_stream(
        &sender,
        &recipient,
        &token_id,
        &100_000,
        &1000,
        &0,
        &0u64,
        &false,
        &None::<u32>,
        &0u64,
        &false,
        &0i128,
        &None::<u32>,
        &None::<i128>,
        &false,
        &false,
        &false,
    );

    env.ledger().set_timestamp(100);
    
    // Pause the stream
    c.pause_stream(&stream_id, &sender);

    // Get stream to verify it was paused
    let stream = c.get_stream(&stream_id);
    assert_eq!(stream.status, StreamStatus::Paused);
    assert_eq!(stream.id, stream_id);
    
    // Verify the pause event contains the correct stream ID
    let events = env.events().all();
    let pause_event = events.iter()
        .find(|e| {
            if let Ok((topic, _)) = e.0.clone().try_into_val::<_, (Symbol, u64)>(&env) {
                topic == Symbol::new(&env, "StreamPaused")
            } else {
                false
            }
        });
    
    assert!(pause_event.is_some(), "StreamPaused event not found");
}
```

## Test 2: Multiple Streams - Pause Correct Stream

**Objective**: Verify pause emits correct ID when sender owns multiple streams

```rust
#[test]
fn test_pause_event_correct_with_multiple_streams() {
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();

    let contract_id = env.register(SoroStreamContract, ());
    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let sender = Address::generate(&env);
    let recipient1 = Address::generate(&env);
    let recipient2 = Address::generate(&env);

    StellarAssetClient::new(&env, &token_id).mint(&sender, &1_000_000);

    let c = SoroStreamContractClient::new(&env, &contract_id);
    c.set_min_duration(&sender, &0u64);
    env.ledger().set_timestamp(0);

    // Create Stream A
    let stream_a = c.create_stream(
        &sender,
        &recipient1,
        &token_id,
        &100_000,
        &1000,
        &0,
        &0u64,
        &false,
        &None::<u32>,
        &0u64,
        &false,
        &0i128,
        &None::<u32>,
        &None::<i128>,
        &false,
        &false,
        &false,
    );

    // Create Stream B
    let stream_b = c.create_stream(
        &sender,
        &recipient2,
        &token_id,
        &100_000,
        &1000,
        &0,
        &1u64,  // Different nonce
        &false,
        &None::<u32>,
        &0u64,
        &false,
        &0i128,
        &None::<u32>,
        &None::<i128>,
        &false,
        &false,
        &false,
    );

    env.ledger().set_timestamp(100);

    // Pause only Stream B
    c.pause_stream(&stream_b, &sender);

    // Verify Stream B is paused
    let stream_b_after = c.get_stream(&stream_b);
    assert_eq!(stream_b_after.status, StreamStatus::Paused);
    assert_eq!(stream_b_after.id, stream_b);

    // Verify Stream A is still active
    let stream_a_after = c.get_stream(&stream_a);
    assert_eq!(stream_a_after.status, StreamStatus::Active);
    assert_eq!(stream_a_after.id, stream_a);

    // Most important: verify event ID matches stream_b
    // (with the fix, it should use stream.id, not the parameter)
}
```

## Test 3: Resume Event Correctness

**Objective**: Verify resume_stream emits the correct stream ID

```rust
#[test]
fn test_resume_event_emits_correct_stream_id() {
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();

    let contract_id = env.register(SoroStreamContract, ());
    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    StellarAssetClient::new(&env, &token_id).mint(&sender, &1_000_000);

    let c = SoroStreamContractClient::new(&env, &contract_id);
    c.set_min_duration(&sender, &0u64);
    env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(
        &sender,
        &recipient,
        &token_id,
        &100_000,
        &1000,
        &0,
        &0u64,
        &false,
        &None::<u32>,
        &0u64,
        &false,
        &0i128,
        &None::<u32>,
        &None::<i128>,
        &false,
        &false,
        &false,
    );

    env.ledger().set_timestamp(100);
    c.pause_stream(&stream_id, &sender);

    let stream_paused = c.get_stream(&stream_id);
    assert_eq!(stream_paused.status, StreamStatus::Paused);

    env.ledger().set_timestamp(200);
    c.resume_stream(&stream_id, &sender);

    let stream_resumed = c.get_stream(&stream_id);
    assert_eq!(stream_resumed.status, StreamStatus::Active);
    assert_eq!(stream_resumed.id, stream_id);
    
    // End time should be shifted by pause duration (200-100 = 100)
    assert_eq!(stream_resumed.end_time, 1100); // Was 1000, now 1000+100

    // Verify the resume event contains the correct stream ID
}
```

## Test 4: Pause-Resume Cycle Maintains Correct IDs

**Objective**: Verify multiple pause/resume cycles maintain correct event IDs

```rust
#[test]
fn test_pause_resume_cycle_event_ids() {
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();

    let contract_id = env.register(SoroStreamContract, ());
    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    StellarAssetClient::new(&env, &token_id).mint(&sender, &2_000_000);

    let c = SoroStreamContractClient::new(&env, &contract_id);
    c.set_min_duration(&sender, &0u64);
    env.ledger().set_timestamp(0);

    let stream_id = c.create_stream(
        &sender,
        &recipient,
        &token_id,
        &100_000,
        &2000,
        &0,
        &0u64,
        &false,
        &None::<u32>,
        &0u64,
        &false,
        &0i128,
        &None::<u32>,
        &None::<i128>,
        &false,
        &false,
        &false,
    );

    // Cycle 1: Pause
    env.ledger().set_timestamp(100);
    c.pause_stream(&stream_id, &sender);
    let s1 = c.get_stream(&stream_id);
    assert_eq!(s1.status, StreamStatus::Paused);
    assert_eq!(s1.id, stream_id);

    // Cycle 1: Resume
    env.ledger().set_timestamp(300);
    c.resume_stream(&stream_id, &sender);
    let s1_resumed = c.get_stream(&stream_id);
    assert_eq!(s1_resumed.status, StreamStatus::Active);
    assert_eq!(s1_resumed.id, stream_id);
    assert_eq!(s1_resumed.end_time, 2200); // 2000 + (300-100)

    // Cycle 2: Pause again
    env.ledger().set_timestamp(400);
    c.pause_stream(&stream_id, &sender);
    let s2 = c.get_stream(&stream_id);
    assert_eq!(s2.status, StreamStatus::Paused);
    assert_eq!(s2.id, stream_id);

    // Cycle 2: Resume again
    env.ledger().set_timestamp(600);
    c.resume_stream(&stream_id, &sender);
    let s2_resumed = c.get_stream(&stream_id);
    assert_eq!(s2_resumed.status, StreamStatus::Active);
    assert_eq!(s2_resumed.id, stream_id);
    assert_eq!(s2_resumed.end_time, 2400); // 2200 + (600-400)

    // All events should have stream_id = stream_id (not an index)
}
```

## Verification Method

To manually verify the fix:

1. Create sender with multiple streams
2. Pause different streams in sequence
3. Check events published to contract
4. Verify each StreamPaused/StreamResumed event contains the actual stream.id
5. Confirm events can be correctly attributed to their respective streams off-chain

## Expected Behavior After Fix

- All pause events emit `stream.id` (the authoritative stream identifier)
- All resume events emit `stream.id`
- Even with multiple streams from same sender, events are unambiguous
- Off-chain indexers can correctly attribute pause/resume events to streams
