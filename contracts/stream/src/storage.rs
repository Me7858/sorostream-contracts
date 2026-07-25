use soroban_sdk::{Address, BytesN, Env, Symbol, Vec};
use crate::types::Stream;

// ---------------------------------------------------------------------------
// Storage key constants
// ---------------------------------------------------------------------------
const STREAM_ID_KEY: &str = "next_id";

// Sender index prefix  – (Symbol("si"), Address)
const SENDER_IDX: &str = "si";
// Recipient index prefix – (Symbol("ri"), Address)
const RECIPIENT_IDX: &str = "ri";
// Token index prefix – (Symbol("ti"), Address)
const TOKEN_IDX: &str = "ti";
// SEP-0010 used-nonce marker prefix – (Symbol("nc"), BytesN<32>)
const NONCE_KEY: &str = "nc";

// ---------------------------------------------------------------------------
// Stream ID counter
// ---------------------------------------------------------------------------

/// Returns the next stream ID and increments the global counter.
pub fn next_stream_id(env: &Env) -> u64 {
    let key = Symbol::new(env, STREAM_ID_KEY);
    let id: u64 = env.storage().instance().get(&key).unwrap_or(0u64);
    env.storage().instance().set(&key, &(id + 1));
    id
}

// ---------------------------------------------------------------------------
// Stream CRUD
// ---------------------------------------------------------------------------

/// Persists a stream to persistent storage.
pub fn save_stream(env: &Env, stream: &Stream) {
    env.storage().persistent().set(&stream.id, stream);
}

/// Loads a stream from persistent storage. Returns `None` if not found.
pub fn load_stream(env: &Env, stream_id: u64) -> Option<Stream> {
    env.storage().persistent().get(&stream_id)
}

// ---------------------------------------------------------------------------
// Sender index  (persistent so it survives ledger expiry)
// ---------------------------------------------------------------------------

/// Appends `stream_id` to the persistent sender index.
pub fn index_by_sender(env: &Env, sender: &Address, stream_id: u64) {
    let key = (Symbol::new(env, SENDER_IDX), sender.clone());
    let mut ids: Vec<u64> = env.storage().persistent().get(&key).unwrap_or(Vec::new(env));
    ids.push_back(stream_id);
    env.storage().persistent().set(&key, &ids);
}

/// Returns all stream IDs indexed under a sender address.
pub fn get_ids_by_sender(env: &Env, sender: &Address) -> Vec<u64> {
    let key = (Symbol::new(env, SENDER_IDX), sender.clone());
    env.storage().persistent().get(&key).unwrap_or(Vec::new(env))
}

// ---------------------------------------------------------------------------
// Recipient index  (persistent)
// ---------------------------------------------------------------------------

/// Appends `stream_id` to the persistent recipient index.
pub fn index_by_recipient(env: &Env, recipient: &Address, stream_id: u64) {
    let key = (Symbol::new(env, RECIPIENT_IDX), recipient.clone());
    let mut ids: Vec<u64> = env.storage().persistent().get(&key).unwrap_or(Vec::new(env));
    ids.push_back(stream_id);
    env.storage().persistent().set(&key, &ids);
}

/// Returns all stream IDs indexed under a recipient address.
pub fn get_ids_by_recipient(env: &Env, recipient: &Address) -> Vec<u64> {
    let key = (Symbol::new(env, RECIPIENT_IDX), recipient.clone());
    env.storage().persistent().get(&key).unwrap_or(Vec::new(env))
}

// ---------------------------------------------------------------------------
// Token cross-index  (#234 — persistent)
// ---------------------------------------------------------------------------

/// Appends `stream_id` to the persistent token index.
pub fn index_by_token(env: &Env, token: &Address, stream_id: u64) {
    let key = (Symbol::new(env, TOKEN_IDX), token.clone());
    let mut ids: Vec<u64> = env.storage().persistent().get(&key).unwrap_or(Vec::new(env));
    ids.push_back(stream_id);
    env.storage().persistent().set(&key, &ids);
}

/// Returns all stream IDs indexed under a token address.
pub fn get_ids_by_token(env: &Env, token: &Address) -> Vec<u64> {
    let key = (Symbol::new(env, TOKEN_IDX), token.clone());
    env.storage().persistent().get(&key).unwrap_or(Vec::new(env))
}

// ---------------------------------------------------------------------------
// SEP-0010 nonce tracking  (#235)
// ---------------------------------------------------------------------------

/// Returns `true` when the nonce has already been consumed (replay protection).
pub fn nonce_used(env: &Env, nonce: &BytesN<32>) -> bool {
    let key = (Symbol::new(env, NONCE_KEY), nonce.clone());
    env.storage().persistent().has(&key)
}

/// Marks a nonce as consumed so it can never be reused.
pub fn mark_nonce_used(env: &Env, nonce: &BytesN<32>) {
    let key = (Symbol::new(env, NONCE_KEY), nonce.clone());
    env.storage().persistent().set(&key, &true);
}
