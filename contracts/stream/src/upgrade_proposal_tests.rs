#![cfg(test)]
use crate::storage::{
    propose_upgrade, get_pending_upgrade_proposal, approve_upgrade_proposal,
    clear_upgrade_proposal, get_upgrade_proposal_expiry
};
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env};

#[test]
fn test_propose_upgrade_success() {
    let env = Env::default();
    let admin = Address::random(&env);

    let wasm_hash = BytesN::<32>::from_array(&env, &[1u8; 32]);
    let result = propose_upgrade(&env, wasm_hash.clone(), &admin, 1000);

    assert!(result.is_ok());
    let proposal = get_pending_upgrade_proposal(&env);
    assert!(proposal.is_some());
    let (hash, proposer, _) = proposal.unwrap();
    assert_eq!(hash, wasm_hash);
    assert_eq!(proposer, admin);
}

#[test]
fn test_propose_upgrade_already_pending() {
    let env = Env::default();
    let admin = Address::random(&env);
    let admin2 = Address::random(&env);

    let wasm_hash1 = BytesN::<32>::from_array(&env, &[1u8; 32]);
    let wasm_hash2 = BytesN::<32>::from_array(&env, &[2u8; 32]);

    let result1 = propose_upgrade(&env, wasm_hash1, &admin, 1000);
    assert!(result1.is_ok());

    let result2 = propose_upgrade(&env, wasm_hash2, &admin2, 2000);
    assert!(result2.is_err());
}

#[test]
fn test_approve_upgrade_proposal_success() {
    let env = Env::default();
    let admin = Address::random(&env);
    let approver = Address::random(&env);

    let wasm_hash = BytesN::<32>::from_array(&env, &[1u8; 32]);
    propose_upgrade(&env, wasm_hash.clone(), &admin, 10000).unwrap();

    let result = approve_upgrade_proposal(&env, &approver);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), wasm_hash);

    assert!(get_pending_upgrade_proposal(&env).is_none());
}

#[test]
fn test_approve_upgrade_proposal_expired() {
    let env = Env::default();
    let admin = Address::random(&env);
    let approver = Address::random(&env);

    let wasm_hash = BytesN::<32>::from_array(&env, &[1u8; 32]);
    propose_upgrade(&env, wasm_hash.clone(), &admin, 1).unwrap();

    let result = approve_upgrade_proposal(&env, &approver);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Upgrade proposal expired");

    assert!(get_pending_upgrade_proposal(&env).is_none());
}

#[test]
fn test_approve_upgrade_no_proposal() {
    let env = Env::default();
    let approver = Address::random(&env);

    let result = approve_upgrade_proposal(&env, &approver);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "No pending upgrade proposal");
}

#[test]
fn test_clear_upgrade_proposal() {
    let env = Env::default();
    let admin = Address::random(&env);

    let wasm_hash = BytesN::<32>::from_array(&env, &[1u8; 32]);
    propose_upgrade(&env, wasm_hash, &admin, 1000).unwrap();

    assert!(get_pending_upgrade_proposal(&env).is_some());
    clear_upgrade_proposal(&env);
    assert!(get_pending_upgrade_proposal(&env).is_none());
    assert!(get_upgrade_proposal_expiry(&env).is_none());
}

#[test]
fn test_upgrade_proposal_expiry_window() {
    let env = Env::default();
    let admin = Address::random(&env);
    let wasm_hash = BytesN::<32>::from_array(&env, &[1u8; 32]);

    let expiry_ledger = 5000u64;
    propose_upgrade(&env, wasm_hash, &admin, expiry_ledger).unwrap();

    let stored_expiry = get_upgrade_proposal_expiry(&env);
    assert!(stored_expiry.is_some());
    assert_eq!(stored_expiry.unwrap(), expiry_ledger);
}

#[test]
fn test_multiple_proposals_sequential() {
    let env = Env::default();
    let admin1 = Address::random(&env);
    let admin2 = Address::random(&env);

    let wasm_hash1 = BytesN::<32>::from_array(&env, &[1u8; 32]);
    let wasm_hash2 = BytesN::<32>::from_array(&env, &[2u8; 32]);

    propose_upgrade(&env, wasm_hash1.clone(), &admin1, 10000).unwrap();
    let proposal1 = get_pending_upgrade_proposal(&env).unwrap();
    assert_eq!(proposal1.0, wasm_hash1);

    clear_upgrade_proposal(&env);

    propose_upgrade(&env, wasm_hash2.clone(), &admin2, 20000).unwrap();
    let proposal2 = get_pending_upgrade_proposal(&env).unwrap();
    assert_eq!(proposal2.0, wasm_hash2);
}
