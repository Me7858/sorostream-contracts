/// Clones an existing stream with optional field overrides.
///
/// Reads the configuration of an existing stream and creates a new stream with
/// the same parameters, allowing the caller to override specific fields such as
/// recipient, token, flow_rate, or duration.
///
/// # Parameters
/// - `source_stream_id`: The ID of the stream to clone
/// - `caller`: The caller initiating the clone (must be sender or delegate)
/// - `recipient_override`: Optional new recipient (None = use source)
/// - `token_override`: Optional new token (None = use source)
/// - `rate_override`: Optional new flow_rate (None = use source)
/// - `duration_override`: Optional new duration in seconds (None = use source)
///
/// # Authorization
/// Only the original stream sender or their delegate can clone a stream.
///
/// # Returns
/// The ID of the newly created stream, or a StreamError.
///
/// # Errors
/// - `StreamNotFound`: Source stream doesn't exist
/// - `NotAuthorized`: Caller is neither sender nor delegate
/// - `ContractPaused`: Contract is in emergency pause
/// - `SenderStreamLimitExceeded`: Sender has reached max streams
/// - `RecipientNotWhitelisted`: New recipient not on whitelist
/// - `TokenNotWhitelisted`: New token not on whitelist
/// - `AddressBlocked`: Caller or recipient on blocklist
/// - `ZeroAmount`: Overrides result in zero or negative amount
/// - `ZeroFlowRate`: Overrides result in zero flow_rate
/// - `InvalidDuration`: Duration out of valid range
///
/// # Example
/// ```ignore
/// // Clone a stream with a different recipient
/// let new_stream_id = clone_stream(
///     env,
///     existing_stream_id,
///     caller,
///     Some(new_recipient),  // Override recipient
///     None,                 // Keep same token
///     None,                 // Keep same rate
///     None,                 // Keep same duration
/// )?;
/// ```
pub fn clone_stream(
    env: Env,
    source_stream_id: u64,
    caller: Address,
    recipient_override: Option<Address>,
    token_override: Option<Address>,
    rate_override: Option<i128>,
    duration_override: Option<u64>,
) -> Result<u64, StreamError> {
    if is_paused_or_auto_unpause(&env) {
        return Err(StreamError::ContractPaused);
    }

    caller.require_auth();

    // Load source stream
    let source = load_stream(&env, source_stream_id)
        .ok_or(StreamError::StreamNotFound)?;

    // Authorization: only sender or delegate can clone
    let is_sender = source.sender == caller;
    let is_delegate = get_delegate(&env, source_stream_id)
        .map_or(false, |d| d == caller);

    if !is_sender && !is_delegate {
        return Err(StreamError::NotAuthorized);
    }

    // Get current timestamp
    let now = env.ledger().timestamp();

    // Apply overrides, fallback to source values
    let new_recipient = recipient_override.unwrap_or(source.recipient.clone());
    let new_token = token_override.unwrap_or(source.token.clone());
    let new_flow_rate = rate_override.unwrap_or(source.flow_rate);
    
    // For duration, use the original stream's duration if not overridden
    let original_duration = source.end_time.saturating_sub(source.start_time);
    let new_duration = duration_override.unwrap_or(original_duration);

    // --- Validation Phase ---

    // Check if contract is initialized
    if read_admin(&env).is_none() {
        return Err(StreamError::NotInitialized);
    }

    // Check if contract is paused
    if is_paused_or_auto_unpause(&env) {
        return Err(StreamError::ContractPaused);
    }

    // Validate blocklist
    if is_blocked(&env, &caller) || is_blocked(&env, &new_recipient) {
        return Err(StreamError::AddressBlocked);
    }

    // Validate whitelist (recipient)
    if is_whitelist_enabled(&env) && !is_whitelisted(&env, &new_recipient) {
        return Err(StreamError::RecipientNotWhitelisted);
    }

    // Validate token whitelist
    if is_token_whitelist_enabled(&env) && !is_token_whitelisted(&env, &new_token) {
        return Err(StreamError::TokenNotWhitelisted);
    }

    // Validate duration
    let min_dur = read_min_duration(&env);
    if new_duration < min_dur {
        return Err(StreamError::StreamDurationTooShort);
    }

    let max_dur = read_max_duration(&env);
    if max_dur > 0 && new_duration > max_dur {
        return Err(StreamError::DurationExceedsMax);
    }

    // Validate token address
    validate_token_address(&env, &new_token)?;

    // Calculate new amount
    let new_amount = new_flow_rate
        .checked_mul(new_duration as i128)
        .ok_or(StreamError::Overflow)?;

    // Validate amount
    if new_amount <= 0 {
        return Err(StreamError::ZeroAmount);
    }

    // Validate flow_rate
    if new_flow_rate == 0 {
        return Err(StreamError::ZeroFlowRate);
    }

    // Check sender stream limit
    let sender_count = get_sender_stream_count(&env, &source.sender);
    let limit = effective_sender_limit(&env, &source.sender);
    if sender_count >= limit {
        return Err(StreamError::SenderStreamLimitExceeded);
    }

    // Check per-token stream cap
    let max_per_token = get_max_streams_per_token(&env);
    if max_per_token > 0 && get_token_stream_count(&env, &new_token) >= max_per_token {
        return Err(StreamError::TokenStreamCapExceeded);
    }

    // --- Generate Stream ID ---
    
    // Use next batch nonce to avoid collisions
    let nonce = get_batch_nonce(&env, &source.sender);
    
    // Defensive collision check (same as create_stream)
    const MAX_ID_RETRIES: u64 = 3;
    let mut new_stream_id = derive_stream_id(&env, &source.sender, &new_recipient, now, nonce);
    
    if stream_exists(&env, new_stream_id) {
        let mut found = false;
        for retry in 1u64..=MAX_ID_RETRIES {
            let candidate = derive_stream_id(
                &env,
                &source.sender,
                &new_recipient,
                now,
                nonce ^ (retry << 32),
            );
            if !stream_exists(&env, candidate) {
                new_stream_id = candidate;
                found = true;
                break;
            }
        }
        if !found {
            return Err(StreamError::IDCollision);
        }
    }

    // --- Transfer Tokens ---

    // Check if caller has sufficient balance (delegated check via token contract)
    token::Client::new(&env, &new_token).transfer(
        &source.sender,
        &env.current_contract_address(),
        &new_amount,
    );

    // --- Create New Stream ---

    // Build new stream preserving cloneable fields
    let mut new_stream = source.clone();
    
    // Update cloned fields
    new_stream.id = new_stream_id;
    new_stream.sender = source.sender.clone();
    new_stream.recipient = new_recipient.clone();
    new_stream.token = new_token.clone();
    new_stream.deposit = new_amount;
    new_stream.flow_rate = new_flow_rate;
    
    // Reset temporal fields
    new_stream.start_time = now;
    new_stream.end_time = now
        .checked_add(new_duration)
        .ok_or(StreamError::Overflow)?;
    
    // Adjust cliff time relative to new start time
    let cliff_offset = source.cliff_time.saturating_sub(source.start_time);
    new_stream.cliff_time = now.checked_add(cliff_offset)
        .ok_or(StreamError::Overflow)?;
    
    // Adjust lock_until relative to new start time
    if source.lock_until > source.start_time {
        let lock_offset = source.lock_until.saturating_sub(source.start_time);
        new_stream.lock_until = now.checked_add(lock_offset)
            .ok_or(StreamError::Overflow)?;
    } else {
        new_stream.lock_until = now;
    }
    
    new_stream.last_withdraw_time = now;
    
    // Reset withdrawal state
    new_stream.status = StreamStatus::Active;
    new_stream.total_withdrawn = 0;
    new_stream.last_pause_time = 0;
    new_stream.locked = false;
    
    // Reset approval timestamp if approval was required
    if new_stream.requires_recipient_approval {
        new_stream.approval_timestamp = 0;
        new_stream.status = StreamStatus::PendingApproval;
    } else {
        new_stream.approval_timestamp = 0;
    }

    // Preserved fields from source:
    // - auto_renew
    // - allow_recipient_termination
    // - holdback_amount
    // - holdback_claimed (reset to false)
    // - is_step_vesting, tranches_claimed (if applicable)
    // - oracle, max_price_deviation_bps, creation_price
    // - curve
    // - withdrawal_steps, current_step
    // - min_withdrawal_amount
    // - non_transferable
    // - requires_recipient_approval
    // - sender_locked (reset to false)
    // - metadata, metadata_uri
    // - milestones

    // Reset holdback claim state
    new_stream.holdback_claimed = false;

    // Reset sender lock state
    new_stream.sender_locked = false;

    // --- Persist & Index ---

    save_stream(&env, &new_stream);
    extend_instance_ttl(&env);
    
    index_by_sender(&env, &source.sender, new_stream_id);
    index_by_recipient(&env, &new_recipient, new_stream_id);
    index_global_stream(&env, new_stream_id);

    // Update counters
    increment_active_stream_count(&env);
    increment_token_stream_count(&env, &new_token);
    
    // Update sender's last creation time
    set_sender_last_creation_time(&env, &source.sender, now);

    // Post-creation sender accounting
    post_create_sender_accounting(&env, &source.sender);

    // --- Emit Event ---

    events::stream_cloned(
        &env,
        source_stream_id,
        new_stream_id,
        &source.sender,
        &new_recipient,
        new_flow_rate,
        new_duration,
    );

    Ok(new_stream_id)
}


// ───────────────────────────────────────────────────────────────────────────
// Event emission for clone_stream
// ───────────────────────────────────────────────────────────────────────────

/// Emitted when a stream is cloned to create a new stream.
pub fn stream_cloned(
    env: &Env,
    source_stream_id: u64,
    new_stream_id: u64,
    sender: &Address,
    new_recipient: &Address,
    new_flow_rate: i128,
    new_duration: u64,
) {
    env.events().publish(
        (Symbol::new(env, "StreamCloned"), source_stream_id),
        (
            new_stream_id,
            sender.clone(),
            new_recipient.clone(),
            new_flow_rate,
            new_duration,
        ),
    );
}


// ───────────────────────────────────────────────────────────────────────────
// Interface trait implementation
// ───────────────────────────────────────────────────────────────────────────

// Add to SoroStreamInterface trait:
fn clone_stream(
    env: Env,
    source_stream_id: u64,
    caller: Address,
    recipient_override: Option<Address>,
    token_override: Option<Address>,
    rate_override: Option<i128>,
    duration_override: Option<u64>,
) -> Result<u64, StreamError>;
