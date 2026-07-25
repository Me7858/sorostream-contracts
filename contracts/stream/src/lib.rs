#![no_std]

mod errors;
mod events;
mod storage;
mod types;

#[cfg(test)]
mod test;

use errors::StreamError;
use soroban_sdk::{
    contract, contractimpl, token, xdr::ToXdr, Address, Bytes, BytesN, Env, Vec,
};
use storage::{
    get_ids_by_recipient, get_ids_by_sender, get_ids_by_token,
    index_by_recipient, index_by_sender, index_by_token,
    load_stream, mark_nonce_used, next_stream_id, nonce_used, save_stream,
};
use types::{Stream, StellarAuth, StreamStatus};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Verify an optimistic concurrency `expected_version` against the stream's
/// current `version`.  If provided and mismatched, returns `VersionConflict`.
/// Always increments `stream.version` before returning.
fn check_and_bump_version(
    stream: &mut Stream,
    expected_version: Option<u32>,
) -> Result<(), StreamError> {
    if let Some(ev) = expected_version {
        if ev != stream.version {
            return Err(StreamError::VersionConflict);
        }
    }
    stream.version += 1;
    Ok(())
}

/// Build the canonical SEP-0010 signing message:
///   sha256(account_xdr_bytes || nonce_bytes || expires_at_big_endian_8_bytes)
///
/// Returns the 32-byte digest that the classic account must sign with its
/// Ed25519 key to obtain `StellarAuth::signature`.
pub fn sep0010_message(env: &Env, auth: &StellarAuth) -> BytesN<32> {
    let mut msg = Bytes::new(env);

    // Include account XDR bytes so the message is bound to this account.
    let account_xdr: Bytes = auth.account.clone().to_xdr(env);
    msg.append(&account_xdr);

    // Append 32-byte nonce.
    let nonce_bytes: Bytes = auth.nonce.clone().into();
    msg.append(&nonce_bytes);

    // Append expires_at as 8 big-endian bytes.
    let exp = auth.expires_at;
    let exp_bytes: [u8; 8] = [
        ((exp >> 56) & 0xff) as u8,
        ((exp >> 48) & 0xff) as u8,
        ((exp >> 40) & 0xff) as u8,
        ((exp >> 32) & 0xff) as u8,
        ((exp >> 24) & 0xff) as u8,
        ((exp >> 16) & 0xff) as u8,
        ((exp >>  8) & 0xff) as u8,
        ( exp        & 0xff) as u8,
    ];
    for b in exp_bytes {
        msg.push_back(b);
    }

    env.crypto().sha256(&msg).into()
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct SoroStreamContract;

#[contractimpl]
impl SoroStreamContract {
    // -----------------------------------------------------------------------
    // Stream lifecycle
    // -----------------------------------------------------------------------

    /// Creates a new payment stream locking `amount` tokens for `recipient`
    /// over `duration_seconds`.
    ///
    /// # Arguments
    /// * `sender` - The payer who funds the stream.
    /// * `recipient` - The beneficiary of the stream.
    /// * `token` - The SAC token contract address (e.g. USDC).
    /// * `amount` - Total tokens to stream (in stroops).
    /// * `duration_seconds` - Stream duration in seconds.
    /// * `auto_renew` - Whether the stream restarts automatically on completion.
    ///
    /// # Returns
    /// The unique stream ID.
    pub fn create_stream(
        env: Env,
        sender: Address,
        recipient: Address,
        token: Address,
        amount: i128,
        duration_seconds: u64,
        auto_renew: bool,
    ) -> Result<u64, StreamError> {
        sender.require_auth();

        if amount <= 0 {
            return Err(StreamError::ZeroAmount);
        }
        if duration_seconds == 0 {
            return Err(StreamError::InvalidDuration);
        }

        let flow_rate = amount / duration_seconds as i128;
        let now = env.ledger().timestamp();
        let end_time = now + duration_seconds;
        let stream_id = next_stream_id(&env);

        token::Client::new(&env, &token)
            .transfer(&sender, &env.current_contract_address(), &amount);

        let stream = Stream {
            id: stream_id,
            sender: sender.clone(),
            recipient: recipient.clone(),
            token: token.clone(),
            deposit: amount,
            flow_rate,
            start_time: now,
            end_time,
            last_withdraw_time: now,
            status: StreamStatus::Active,
            auto_renew,
            version: 1,
        };

        save_stream(&env, &stream);
        index_by_sender(&env, &sender, stream_id);
        index_by_recipient(&env, &recipient, stream_id);
        index_by_token(&env, &token, stream_id);

        events::stream_created(&env, stream_id, &sender, &recipient, amount, flow_rate, end_time);

        Ok(stream_id)
    }

    // -----------------------------------------------------------------------
    // Issue #235: SEP-0010 classic account streaming
    // -----------------------------------------------------------------------

    /// Creates a payment stream on behalf of a classic Stellar keypair account
    /// using a SEP-0010 signed authentication payload.
    ///
    /// Classic Stellar accounts cannot directly invoke Soroban contracts.
    /// This entry point accepts a [`StellarAuth`] struct containing an Ed25519
    /// signature over `sha256(account_xdr || nonce || expires_at_be_8_bytes)`.
    /// The contract enforces:
    ///
    /// 1. **Expiry check** — `auth.expires_at` must be in the future.
    /// 2. **Replay protection** — `auth.nonce` must not have been seen before.
    /// 3. **Signature verification** — the Ed25519 signature must be valid for
    ///    the canonical message and the account's public key.
    ///
    /// The nonce is consumed atomically with stream creation so it can never
    /// be reused.
    ///
    /// # Arguments
    /// * `auth` - SEP-0010 authentication payload.
    /// * `public_key` - Raw 32-byte Ed25519 public key of the classic account.
    /// * `recipient` - Beneficiary address.
    /// * `token` - SAC token address.
    /// * `amount` - Total tokens (stroops).
    /// * `duration_seconds` - Stream duration.
    /// * `auto_renew` - Auto-renewal flag.
    ///
    /// # Errors
    /// * `AuthTokenExpired` — `auth.expires_at` is in the past.
    /// * `AuthNonceReplayed` — nonce was already used.
    /// * `AuthInvalidSignature` — Ed25519 signature verification failed.
    pub fn create_stream_classic(
        env: Env,
        auth: StellarAuth,
        public_key: BytesN<32>,
        recipient: Address,
        token: Address,
        amount: i128,
        duration_seconds: u64,
        auto_renew: bool,
    ) -> Result<u64, StreamError> {
        let now = env.ledger().timestamp();

        // 1. Expiry check.
        if auth.expires_at <= now {
            return Err(StreamError::AuthTokenExpired);
        }

        // 2. Replay protection: reject reused nonces.
        if nonce_used(&env, &auth.nonce) {
            return Err(StreamError::AuthNonceReplayed);
        }

        // 3. Ed25519 signature verification.
        //    Message = sha256(account_xdr || nonce || expires_at_be_8_bytes)
        let message_hash: BytesN<32> = sep0010_message(&env, &auth);

        env.crypto().ed25519_verify(
            &public_key,
            &Bytes::from(message_hash),
            &auth.signature,
        );

        // 4. Consume nonce (replay protection write — performed after signature
        //    verification but before state changes so a bad sig never burns a nonce).
        mark_nonce_used(&env, &auth.nonce);

        // 5. Require Soroban authorisation from the classic account address.
        //    In practice this is satisfied by attaching the proper
        //    `SorobanAuthorizationEntry` to the transaction envelope.
        auth.account.require_auth();

        // Validate stream parameters.
        if amount <= 0 {
            return Err(StreamError::ZeroAmount);
        }
        if duration_seconds == 0 {
            return Err(StreamError::InvalidDuration);
        }

        let flow_rate = amount / duration_seconds as i128;
        let end_time = now + duration_seconds;
        let stream_id = next_stream_id(&env);

        token::Client::new(&env, &token)
            .transfer(&auth.account, &env.current_contract_address(), &amount);

        let stream = Stream {
            id: stream_id,
            sender: auth.account.clone(),
            recipient: recipient.clone(),
            token: token.clone(),
            deposit: amount,
            flow_rate,
            start_time: now,
            end_time,
            last_withdraw_time: now,
            status: StreamStatus::Active,
            auto_renew,
            version: 1,
        };

        save_stream(&env, &stream);
        index_by_sender(&env, &auth.account, stream_id);
        index_by_recipient(&env, &recipient, stream_id);
        index_by_token(&env, &token, stream_id);

        events::stream_classic_created(
            &env,
            stream_id,
            &auth.account,
            &recipient,
            amount,
            flow_rate,
            end_time,
        );

        Ok(stream_id)
    }

    // -----------------------------------------------------------------------
    // Withdraw
    // -----------------------------------------------------------------------

    /// Allows the recipient to withdraw all tokens earned since last withdrawal.
    ///
    /// If the stream has reached its end time and `auto_renew` is true, the
    /// stream is automatically restarted.
    ///
    /// # Arguments
    /// * `stream_id` - The stream to withdraw from.
    /// * `recipient` - Must match the stream's recipient (auth required).
    /// * `expected_version` - Optional optimistic concurrency guard (#236).
    pub fn withdraw(
        env: Env,
        stream_id: u64,
        recipient: Address,
        expected_version: Option<u32>,
    ) -> Result<(), StreamError> {
        recipient.require_auth();

        let mut stream = load_stream(&env, stream_id).ok_or(StreamError::StreamNotFound)?;

        if stream.recipient != recipient {
            return Err(StreamError::NotRecipient);
        }
        if stream.status != StreamStatus::Active {
            return Err(StreamError::StreamNotActive);
        }

        // Optimistic concurrency check (#236).
        check_and_bump_version(&mut stream, expected_version)?;

        let now = env.ledger().timestamp();
        let effective_now = now.min(stream.end_time);
        let elapsed = effective_now.saturating_sub(stream.last_withdraw_time);
        let claimable = stream.flow_rate * elapsed as i128;

        if claimable > 0 {
            token::Client::new(&env, &stream.token)
                .transfer(&env.current_contract_address(), &recipient, &claimable);
        }

        stream.last_withdraw_time = effective_now;

        // Handle natural completion.
        if now >= stream.end_time {
            if stream.auto_renew {
                let duration = stream.end_time - stream.start_time;
                // Pull fresh deposit from sender for the new cycle.
                stream.sender.require_auth();
                token::Client::new(&env, &stream.token).transfer(
                    &stream.sender,
                    &env.current_contract_address(),
                    &stream.deposit,
                );
                stream.start_time = stream.end_time;
                stream.end_time = stream.start_time + duration;
                stream.last_withdraw_time = stream.start_time;
            } else {
                stream.status = StreamStatus::Completed;
                events::stream_completed(&env, stream_id);
            }
        }

        save_stream(&env, &stream);
        events::stream_withdrawn(&env, stream_id, &recipient, claimable, now);

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Cancel
    // -----------------------------------------------------------------------

    /// Cancels an active stream. The recipient receives all earned tokens so
    /// far; the sender receives the unstreamed remainder.
    ///
    /// # Arguments
    /// * `stream_id` - The stream to cancel.
    /// * `sender` - Must match the stream's sender (auth required).
    /// * `expected_version` - Optional optimistic concurrency guard (#236).
    pub fn cancel_stream(
        env: Env,
        stream_id: u64,
        sender: Address,
        expected_version: Option<u32>,
    ) -> Result<(), StreamError> {
        sender.require_auth();

        let mut stream = load_stream(&env, stream_id).ok_or(StreamError::StreamNotFound)?;

        if stream.sender != sender {
            return Err(StreamError::NotSender);
        }
        if stream.status != StreamStatus::Active {
            return Err(StreamError::StreamNotActive);
        }

        // Optimistic concurrency check (#236).
        check_and_bump_version(&mut stream, expected_version)?;

        let now = env.ledger().timestamp();
        let effective_now = now.min(stream.end_time);
        let elapsed = effective_now.saturating_sub(stream.last_withdraw_time);
        let recipient_amount = stream.flow_rate * elapsed as i128;
        let refund_amount = stream.deposit.saturating_sub(
            stream.flow_rate * effective_now.saturating_sub(stream.start_time) as i128,
        );

        let token_client = token::Client::new(&env, &stream.token);

        if recipient_amount > 0 {
            token_client.transfer(
                &env.current_contract_address(),
                &stream.recipient,
                &recipient_amount,
            );
        }
        if refund_amount > 0 {
            token_client.transfer(&env.current_contract_address(), &sender, &refund_amount);
        }

        stream.status = StreamStatus::Cancelled;
        save_stream(&env, &stream);

        events::stream_cancelled(&env, stream_id, &sender, refund_amount, recipient_amount);

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Top-up
    // -----------------------------------------------------------------------

    /// Adds more tokens to an existing stream, extending its end time
    /// proportionally.
    ///
    /// # Arguments
    /// * `stream_id` - The stream to top up.
    /// * `sender` - Must match the stream's sender (auth required).
    /// * `amount` - Additional tokens to add (in stroops).
    /// * `expected_version` - Optional optimistic concurrency guard (#236).
    pub fn top_up(
        env: Env,
        stream_id: u64,
        sender: Address,
        amount: i128,
        expected_version: Option<u32>,
    ) -> Result<(), StreamError> {
        sender.require_auth();

        let mut stream = load_stream(&env, stream_id).ok_or(StreamError::StreamNotFound)?;

        if stream.sender != sender {
            return Err(StreamError::NotSender);
        }
        if stream.status != StreamStatus::Active {
            return Err(StreamError::StreamNotActive);
        }
        if amount <= 0 {
            return Err(StreamError::ZeroAmount);
        }

        // Optimistic concurrency check (#236).
        check_and_bump_version(&mut stream, expected_version)?;

        token::Client::new(&env, &stream.token)
            .transfer(&sender, &env.current_contract_address(), &amount);

        let extra_seconds = (amount / stream.flow_rate) as u64;
        stream.end_time += extra_seconds;
        stream.deposit += amount;

        let new_end_time = stream.end_time;
        save_stream(&env, &stream);

        events::stream_topped_up(&env, stream_id, amount, new_end_time);

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Views
    // -----------------------------------------------------------------------

    /// Returns the full stream struct for a given stream ID.
    pub fn get_stream(env: Env, stream_id: u64) -> Result<Stream, StreamError> {
        load_stream(&env, stream_id).ok_or(StreamError::StreamNotFound)
    }

    /// Returns the amount of tokens currently claimable by the recipient.
    pub fn get_claimable(env: Env, stream_id: u64) -> Result<i128, StreamError> {
        let stream = load_stream(&env, stream_id).ok_or(StreamError::StreamNotFound)?;

        if stream.status != StreamStatus::Active {
            return Ok(0);
        }

        let now = env.ledger().timestamp();
        let effective_now = now.min(stream.end_time);
        let elapsed = effective_now.saturating_sub(stream.last_withdraw_time);
        Ok(stream.flow_rate * elapsed as i128)
    }

    /// Returns all streams created by a sender address.
    pub fn get_streams_by_sender(env: Env, sender: Address) -> Vec<Stream> {
        let ids = get_ids_by_sender(&env, &sender);
        let mut streams = Vec::new(&env);
        for id in ids.iter() {
            if let Some(s) = load_stream(&env, id) {
                streams.push_back(s);
            }
        }
        streams
    }

    /// Returns all streams targeting a recipient address.
    pub fn get_streams_by_recipient(env: Env, recipient: Address) -> Vec<Stream> {
        let ids = get_ids_by_recipient(&env, &recipient);
        let mut streams = Vec::new(&env);
        for id in ids.iter() {
            if let Some(s) = load_stream(&env, id) {
                streams.push_back(s);
            }
        }
        streams
    }

    /// Returns only active streams created by a sender address.
    pub fn get_active_streams_by_sender(env: Env, sender: Address) -> Vec<Stream> {
        let ids = get_ids_by_sender(&env, &sender);
        let mut streams = Vec::new(&env);
        for id in ids.iter() {
            if let Some(s) = load_stream(&env, id) {
                if s.status == StreamStatus::Active {
                    streams.push_back(s);
                }
            }
        }
        streams
    }

    /// Returns only active streams targeting a recipient address.
    pub fn get_active_streams_by_recipient(env: Env, recipient: Address) -> Vec<Stream> {
        let ids = get_ids_by_recipient(&env, &recipient);
        let mut streams = Vec::new(&env);
        for id in ids.iter() {
            if let Some(s) = load_stream(&env, id) {
                if s.status == StreamStatus::Active {
                    streams.push_back(s);
                }
            }
        }
        streams
    }

    // -----------------------------------------------------------------------
    // Issue #234: Token cross-index + pagination queries
    // -----------------------------------------------------------------------

    /// Returns a paginated list of stream IDs associated with a token address.
    ///
    /// # Arguments
    /// * `token` - The token contract address to query.
    /// * `start` - Zero-based offset into the full list (pagination cursor).
    /// * `limit` - Maximum number of IDs to return.
    ///
    /// # Returns
    /// A `Vec<u64>` containing up to `limit` stream IDs beginning at `start`.
    pub fn get_streams_by_token(env: Env, token: Address, start: u32, limit: u32) -> Vec<u64> {
        let all_ids = get_ids_by_token(&env, &token);
        let mut result = Vec::new(&env);
        let total = all_ids.len();

        if start >= total || limit == 0 {
            return result;
        }

        let end = (start + limit).min(total);
        for i in start..end {
            result.push_back(all_ids.get(i).unwrap());
        }
        result
    }

    /// Returns all stream IDs created by `sender` for streams funded with `token`.
    ///
    /// Performs an in-contract set-intersection so callers don't need to
    /// download and filter large index lists client-side.
    ///
    /// # Arguments
    /// * `token` - Token contract address.
    /// * `sender` - Sender address.
    ///
    /// # Returns
    /// A `Vec<u64>` of stream IDs matching both token and sender.
    pub fn get_streams_by_token_and_sender(
        env: Env,
        token: Address,
        sender: Address,
    ) -> Vec<u64> {
        let token_ids = get_ids_by_token(&env, &token);
        let sender_ids = get_ids_by_sender(&env, &sender);
        let mut result = Vec::new(&env);

        // O(n*m) intersection — acceptable for contract-level index sizes.
        for tid in token_ids.iter() {
            for sid in sender_ids.iter() {
                if tid == sid {
                    result.push_back(tid);
                    break;
                }
            }
        }
        result
    }
}
