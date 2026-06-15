//! Security integration tests: replay, tamper, wrong key, nonce uniqueness.

use justdrop_security::crypto::{verify_signature, KeyExchangeInitiator};
use ring::aead::NONCE_LEN;
use ring::rand::SystemRandom;
use ring::signature::{Ed25519KeyPair, KeyPair};

fn create_session_pair() -> (
    justdrop_security::crypto::SessionKeys,
    justdrop_security::crypto::SessionKeys,
) {
    let initiator = KeyExchangeInitiator::new().unwrap();
    let responder = KeyExchangeInitiator::new().unwrap();

    let init_pub = initiator.public_key_bytes.clone();
    let resp_pub = responder.public_key_bytes.clone();

    let init_keys = initiator.complete(&resp_pub, true).unwrap();
    let resp_keys = responder.complete(&init_pub, false).unwrap();

    (init_keys, resp_keys)
}

#[test]
fn replay_attack_detected() {
    let (init_keys, resp_keys) = create_session_pair();

    let mut encryptor = init_keys.encryptor().unwrap();
    let decryptor = resp_keys.decryptor().unwrap();

    let mut data = b"transfer manifest".to_vec();
    let nonce = encryptor.encrypt(&mut data).unwrap();

    // First decrypt succeeds
    let mut data_copy = data.clone();
    decryptor.decrypt(&nonce, &mut data_copy).unwrap();
    assert_eq!(&data_copy, b"transfer manifest");

    // Same ciphertext with wrong nonce fails (simulates replay with wrong counter)
    let wrong_nonce = [0xFF; NONCE_LEN];
    let mut replay_data = data.clone();
    let result = decryptor.decrypt(&wrong_nonce, &mut replay_data);
    assert!(result.is_err(), "Replay with wrong nonce must fail");
}

#[test]
fn tampered_ciphertext_rejected() {
    let (init_keys, resp_keys) = create_session_pair();

    let mut encryptor = init_keys.encryptor().unwrap();
    let decryptor = resp_keys.decryptor().unwrap();

    let mut data = b"sensitive file data".to_vec();
    let nonce = encryptor.encrypt(&mut data).unwrap();

    // Tamper with ciphertext
    data[0] ^= 0xFF;

    let result = decryptor.decrypt(&nonce, &mut data);
    assert!(
        result.is_err(),
        "Tampered ciphertext must fail AEAD verification"
    );
}

#[test]
fn wrong_key_rejected() {
    let init_a = KeyExchangeInitiator::new().unwrap();
    let resp_a = KeyExchangeInitiator::new().unwrap();
    let resp_b = KeyExchangeInitiator::new().unwrap();

    let _init_a_pub = init_a.public_key_bytes.clone();
    let resp_a_pub = resp_a.public_key_bytes.clone();
    let resp_b_pub = resp_b.public_key_bytes.clone();

    let keys_ab = init_a.complete(&resp_a_pub, true).unwrap();

    // Different responder
    let init_c = KeyExchangeInitiator::new().unwrap();
    let keys_cb = init_c.complete(&resp_b_pub, true).unwrap();

    let mut encryptor = keys_ab.encryptor().unwrap();
    let wrong_decryptor = keys_cb.decryptor().unwrap();

    let mut data = b"authenticated data".to_vec();
    let nonce = encryptor.encrypt(&mut data).unwrap();

    let result = wrong_decryptor.decrypt(&nonce, &mut data);
    assert!(result.is_err(), "Wrong key must fail decryption");
}

#[test]
fn nonce_uniqueness_across_messages() {
    let (init_keys, _resp_keys) = create_session_pair();
    let mut encryptor = init_keys.encryptor().unwrap();

    let mut d1 = b"same message".to_vec();
    let n1 = encryptor.encrypt(&mut d1).unwrap();

    let mut d2 = b"same message".to_vec();
    let n2 = encryptor.encrypt(&mut d2).unwrap();

    assert_ne!(n1, n2, "Sequential nonces must differ");
    assert_ne!(
        d1, d2,
        "Same plaintext must produce different ciphertext with different nonces"
    );
}

#[test]
fn signature_forgery_rejected() {
    let rng = SystemRandom::new();
    let pkcs8_a = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
    let kp_a = Ed25519KeyPair::from_pkcs8(pkcs8_a.as_ref()).unwrap();

    let pkcs8_b = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
    let kp_b = Ed25519KeyPair::from_pkcs8(pkcs8_b.as_ref()).unwrap();

    let message = b"handshake payload";
    let sig_a = kp_a.sign(message);

    // Verify with correct key
    verify_signature(kp_a.public_key().as_ref(), message, sig_a.as_ref()).unwrap();

    // Verify with wrong key (forgery)
    let result = verify_signature(kp_b.public_key().as_ref(), message, sig_a.as_ref());
    assert!(result.is_err(), "Signature from different key must fail");
}
