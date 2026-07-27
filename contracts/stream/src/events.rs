use soroban_sdk::{Address, Bytes, Env, String, Symbol};

/// Emitted when a new stream is created.
pub fn stream_created(
    env: &Env,
    stream_id: u64,
    sender: &Address,
    recipient: &Address,
    amount: i128,
    flow_rate: i128,
    end_time: u64,
) {
    env.events().publish(
        (Symbol::new(env, "StreamCreated"), stream_id),
        (
            sender.clone(),
            recipient.clone(),
            amount,
            flow_rate,
            end_time,
        ),
    );
}

/// Emitted when a recipient withdraws claimable tokens.
///
/// `total_withdrawn` reflects the cumulative amount withdrawn from this stream
/// including the current withdrawal, computed after the stream state has been
/// updated (checks-effects-interactions order).
pub fn stream_withdrawn(
    env: &Env,
    stream_id: u64,
    recipient: &Address,
    amount: i128,
    timestamp: u64,
    total_withdrawn: i128,
) {
    env.events().publish(
        (Symbol::new(env, "StreamWithdrawn"), stream_id),
        (recipient.clone(), amount, timestamp, total_withdrawn),
    );
}

/// Emitted when a sender cancels a stream.
pub fn stream_cancelled(
    env: &Env,
    stream_id: u64,
    sender: &Address,
    refund_amount: i128,
    recipient_amount: i128,
) {
    env.events().publish(
        (Symbol::new(env, "StreamCancelled"), stream_id),
        (sender.clone(), refund_amount, recipient_amount),
    );
}

/// Emitted when a sender tops up an existing stream.
pub fn stream_topped_up(env: &Env, stream_id: u64, added_amount: i128, new_end_time: u64) {
    env.events().publish(
        (Symbol::new(env, "StreamToppedUp"), stream_id),
        (added_amount, new_end_time),
    );
}

/// Emitted when a stream naturally reaches its end time.
pub fn stream_completed(env: &Env, stream_id: u64) {
    env.events()
        .publish((Symbol::new(env, "StreamCompleted"), stream_id), ());
}

/// Emitted when an auto-renew re-lock fails because the sender has insufficient balance.
pub fn auto_renew_failed(env: &Env, stream_id: u64, sender: &Address, required: i128) {
    env.events().publish(
        (Symbol::new(env, "AutoRenewFailed"), stream_id),
        (sender.clone(), required),
    );
}

/// Emitted when the contract is initialized with a version.
pub fn contract_deployed(env: &Env, version: &String, admin: &Address) {
    env.events().publish(
        (Symbol::new(env, "ContractDeployed"),),
        (version.clone(), admin.clone()),
    );
}

/// Emitted when a sender partially cancels a stream, spawning a new smaller stream.
pub fn stream_partial_cancelled(
    env: &Env,
    old_stream_id: u64,
    new_stream_id: u64,
    sender: &Address,
    refund_amount: i128,
    new_deposit: i128,
) {
    env.events().publish(
        (Symbol::new(env, "StreamPartialCancelled"), old_stream_id),
        (new_stream_id, sender.clone(), refund_amount, new_deposit),
    );
}

/// Emitted when the contract is paused during an emergency.
pub fn contract_paused(env: &Env, admin: &Address, timestamp: u64) {
    env.events().publish(
        (Symbol::new(env, "ContractPaused"), admin.clone()),
        timestamp,
    );
}

/// Emitted when the contract is resumed after an emergency pause.
pub fn contract_resumed(env: &Env, admin: &Address, timestamp: u64) {
    env.events().publish(
        (Symbol::new(env, "ContractResumed"), admin.clone()),
        timestamp,
    );
}

/// Emitted when a stream is paused by the sender.
pub fn stream_paused(env: &Env, stream_id: u64, sender: &Address) {
    env.events().publish(
        (Symbol::new(env, "StreamPaused"), stream_id),
        sender.clone(),
    );
}

/// Emitted when a stream is resumed by the sender.
pub fn stream_resumed(env: &Env, stream_id: u64, sender: &Address) {
    env.events().publish(
        (Symbol::new(env, "StreamResumed"), stream_id),
        sender.clone(),
    );
}

/// Emitted when a protocol fee is collected on withdrawal.
pub fn fee_collected(
    env: &Env,
    stream_id: u64,
    amount: i128,
    treasury: &Address,
) {
    env.events().publish(
        (Symbol::new(env, "FeeCollected"), stream_id),
        (amount, treasury.clone()),
    );
}

/// Emitted when a fee change is proposed.
pub fn fee_change_proposed(env: &Env, new_fee: u32, unlock_time: u64) {
    env.events().publish(
        (Symbol::new(env, "FeeChangeProposed"),),
        (new_fee, unlock_time),
    );
}

/// Emitted when a fee change is executed.
pub fn fee_change_executed(env: &Env, new_fee: u32) {
    env.events().publish(
        (Symbol::new(env, "FeeChangeExecuted"),),
        (new_fee,),
    );
}

/// Emitted when a recipient terminates a stream early.
pub fn stream_terminated_by_recipient(
    env: &Env,
    stream_id: u64,
    recipient: &Address,
    recipient_amount: i128,
    refund_amount: i128,
) {
    env.events().publish(
        (Symbol::new(env, "StreamTerminatedByRecipient"), stream_id),
        (recipient.clone(), recipient_amount, refund_amount),
    );
}

/// Emitted when a stream recipient transfers their rights to a new recipient.
pub fn recipient_transferred(
    env: &Env,
    stream_id: u64,
    old_recipient: &Address,
    new_recipient: &Address,
) {
    env.events().publish(
        (Symbol::new(env, "RecipientTransferred"), stream_id),
        (old_recipient.clone(), new_recipient.clone()),
    );
}

/// Emitted when a migration is successfully applied.
pub fn contract_migrated(env: &Env, from_version: &String, to_version: &String, admin: &Address) {
    env.events().publish(
        (Symbol::new(env, "ContractMigrated"),),
        (from_version.clone(), to_version.clone(), admin.clone()),
    );
}

/// Emitted when an admin action is logged.
pub fn admin_action(env: &Env, instruction: &String, admin: &Address, timestamp: u64) {
    env.events().publish(
        (Symbol::new(env, "AdminAction"),),
        (instruction.clone(), admin.clone(), timestamp),
    );
}

/// Emitted when a stream is archived after full settlement.
pub fn stream_archived(
    env: &Env,
    stream_id: u64,
    sender: &Address,
    recipient: &Address,
    total_amount: i128,
) {
    env.events().publish(
        (Symbol::new(env, "StreamArchived"), stream_id),
        (sender.clone(), recipient.clone(), total_amount),
    );
}

/// Emitted when metadata is updated for a stream.
pub fn metadata_updated(env: &Env, stream_id: u64, metadata: &Bytes) {
    env.events().publish(
        (Symbol::new(env, "MetadataUpdated"), stream_id),
        metadata.clone(),
    );
}

/// Emitted when a stream's metadata URI is updated.
pub fn metadata_uri_updated(env: &Env, stream_id: u64, metadata_uri: &Option<String>) {
    let uri_str = if let Some(uri) = metadata_uri {
        uri.clone()
    } else {
        String::from_slice(env, "")
    };
    env.events().publish(
        (Symbol::new(env, "MetadataUriUpdated"), stream_id),
        uri_str,
    );
}

/// Emitted when an expired stream is swept from storage.
pub fn stream_swept(env: &Env, stream_id: u64, caller: &Address) {
    env.events().publish(
        (Symbol::new(env, "StreamSwept"), stream_id),
        caller.clone(),
    );
}

/// Emitted when a milestone is released by the sender.
pub fn milestone_released(env: &Env, stream_id: u64, milestone_index: u32) {
    env.events().publish(
        (Symbol::new(env, "MilestoneReleased"), stream_id),
        milestone_index,
    );
}

/// Emitted when an auto-renewal is cancelled for a stream.
pub fn auto_renew_cancelled(env: &Env, stream_id: u64) {
    env.events().publish(
        (Symbol::new(env, "AutoRenewCancelled"), stream_id),
        (),
    );
}

/// Emitted when a stream is renewed.
#[allow(dead_code)]
pub fn stream_renewed(env: &Env, old_stream_id: u64, new_stream_id: u64) {
    env.events().publish(
        (Symbol::new(env, "StreamRenewed"), old_stream_id),
        new_stream_id,
    );
}

/// Emitted when a creation fee is collected in XLM at stream creation time.
pub fn creation_fee_collected(env: &Env, fee_amount: i128, treasury: &Address) {
    env.events().publish(
        (Symbol::new(env, "CreationFeeCollected"),),
        (fee_amount, treasury.clone()),
    );
}

/// Emitted when accumulated protocol fees are swept from the contract to a destination.
/// Emitted when the sender releases the holdback escrow to the recipient.
pub fn holdback_released(env: &Env, stream_id: u64, amount: i128, recipient: &Address) {
    env.events().publish(
        (Symbol::new(env, "HoldbackReleased"), stream_id),
        (amount, recipient.clone()),
    );
}

/// Emitted when the sender claws back the holdback escrow before the recipient claims it.
pub fn holdback_clawed_back(env: &Env, stream_id: u64, amount: i128, sender: &Address) {
    env.events().publish(
        (Symbol::new(env, "HoldbackClawedBack"), stream_id),
        (amount, sender.clone()),
// ---------------------------------------------------------------------------
// Step-vesting tranche events
// ---------------------------------------------------------------------------

/// Emitted when a step-vesting stream is created with a tranche schedule.
pub fn tranche_stream_created(env: &Env, stream_id: u64, sender: &Address, tranche_count: u32, total_amount: i128) {
    env.events().publish(
        (Symbol::new(env, "TrancheStreamCreated"), stream_id),
        (sender.clone(), tranche_count, total_amount),
    );
}

/// Emitted when one or more tranches are claimed during a withdrawal.
pub fn tranches_withdrawn(
    env: &Env,
    stream_id: u64,
    recipient: &Address,
    tranches_claimed: u32,
    amount: i128,
) {
    env.events().publish(
        (Symbol::new(env, "TranchesWithdrawn"), stream_id),
        (recipient.clone(), tranches_claimed, amount),
    );
}

/// Emitted when a step-vesting stream is cancelled and unclaimed tranches are refunded.
pub fn tranche_stream_cancelled(
    env: &Env,
    stream_id: u64,
    sender: &Address,
    unclaimed_tranche_refund: i128,
    recipient_amount: i128,
) {
    env.events().publish(
        (Symbol::new(env, "TrancheStreamCancelled"), stream_id),
        (sender.clone(), unclaimed_tranche_refund, recipient_amount),
    );
}

// ---------------------------------------------------------------------------
// Oracle price-check event
// ---------------------------------------------------------------------------

/// Emitted when an oracle price check passes successfully.
pub fn price_check_passed(
    env: &Env,
    stream_id: u64,
    token: &Address,
    price: i128,
    deviation_bps: u32,
) {
    env.events().publish(
        (Symbol::new(env, "PriceCheckPassed"), stream_id),
        (token.clone(), price, deviation_bps),
/// Emitted when a stream transitions to the Expired state via mark_expired.
pub fn stream_expired(env: &Env, stream_id: u64) {
    env.events().publish(
        (Symbol::new(env, "StreamExpired"), stream_id),
        (),
/// Emitted when a stream's TTL is bumped to extend its ledger lifetime.
pub fn ttl_bumped(env: &Env, stream_id: u64, new_expiry_ledger: u32) {
    env.events().publish(
        (Symbol::new(env, "TtlBumped"), stream_id),
        new_expiry_ledger,
/// Emitted when a delegate is set for a stream.
pub fn delegate_set(env: &Env, stream_id: u64, sender: &Address, delegate: &Address) {
    env.events().publish(
        (Symbol::new(env, "DelegateSet"), stream_id),
        (sender.clone(), delegate.clone()),
    );
}

/// Emitted when a delegate is revoked from a stream.
pub fn delegate_revoked(env: &Env, stream_id: u64, sender: &Address) {
    env.events().publish(
        (Symbol::new(env, "DelegateRevoked"), stream_id),
        sender.clone(),
    );
}
/// Emitted when fees are swept from the contract.
pub fn fee_swept(env: &Env, token: &Address, amount: i128, destination: &Address) {
    env.events().publish(
        (Symbol::new(env, "FeeSwept"),),
        (token.clone(), amount, destination.clone()),
    );
}

/// Emitted when slippage threshold is exceeded.
pub fn slippage_exceeded(env: &Env, stream_id: u64, current_price: i128, max_slippage_bps: u32) {
    env.events().publish(
        (Symbol::new(env, "SlippageExceeded"), stream_id),
        (current_price, max_slippage_bps),
    );
}

/// Emitted when slippage is within 80% of the limit (warning).
pub fn slippage_warning(env: &Env, stream_id: u64, current_deviation_bps: u32, max_slippage_bps: u32) {
    env.events().publish(
        (Symbol::new(env, "SlippageWarning"), stream_id),
        (current_deviation_bps, max_slippage_bps),
    );
}

/// Emitted when an address hits the rate limit.
pub fn rate_limit_exceeded(env: &Env, sender: &Address) {
    env.events().publish(
        (Symbol::new(env, "RateLimitExceeded"),),
        sender.clone(),
    );
}

/// Emitted when rate limit parameters are updated.
pub fn rate_limit_updated(env: &Env, window_seconds: u64, max_creations: u32) {
    env.events().publish(
        (Symbol::new(env, "RateLimitUpdated"),),
        (window_seconds, max_creations),
    );
}

/// Emitted when a token is added to the whitelist.
pub fn token_whitelisted(env: &Env, token: &Address) {
    env.events().publish(
        (Symbol::new(env, "TokenWhitelisted"),),
        token.clone(),
    );
}

/// Emitted when a token is removed from the whitelist.
pub fn token_dwhitelisted(env: &Env, token: &Address) {
    env.events().publish(
        (Symbol::new(env, "TokenDewhitelisted"),),
        token.clone(),
    );
}

/// Emitted when token whitelist is toggled.
pub fn token_whitelist_toggled(env: &Env, enabled: bool) {
    env.events().publish(
        (Symbol::new(env, "TokenWhitelistToggled"),),
        enabled,
    );
}

/// Emitted when a federation name is registered (Issue #238).
pub fn federation_registered(env: &Env, federation_name: &String, stellar_address: &Address) {
    env.events().publish(
        (Symbol::new(env, "FederationRegistered"),),
        (federation_name.clone(), stellar_address.clone()),
    );
}

/// Emitted when a federation name is unregistered (Issue #238).
pub fn federation_unregistered(env: &Env, federation_name: &String) {
    env.events().publish(
        (Symbol::new(env, "FederationUnregistered"),),
        federation_name.clone(),
    );
}
