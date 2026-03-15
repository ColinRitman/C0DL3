// src/aa/schnorr.rs
// Schnorr signature scheme over Ristretto255.
//
// Sign: pick k → R = k*G → e = H("C0DL3:schnorr:" || R || pubkey || msg) → s = k + e*privkey
// Verify: s*G == R + e*pubkey
//
// Used for AA wallet auth — proving ownership without revealing the private key.

use curve25519_dalek_ng::{
    constants::RISTRETTO_BASEPOINT_POINT as G,
    ristretto::{CompressedRistretto, RistrettoPoint},
    scalar::Scalar,
};
use sha2::{Digest, Sha256};

use crate::aa::types::SchnorrSignature;

/// Sign a message with a Ristretto255 private key.
///
/// Deterministic nonce (RFC 6979-style) prevents nonce reuse.
pub fn schnorr_sign(privkey: &Scalar, message: &[u8]) -> SchnorrSignature {
    let pubkey = (privkey * G).compress().to_bytes();

    // Deterministic nonce: k = H("C0DL3:schnorr_nonce:" || privkey || message)
    let mut nonce_input = Vec::new();
    nonce_input.extend_from_slice(b"C0DL3:schnorr_nonce:");
    nonce_input.extend_from_slice(&privkey.to_bytes());
    nonce_input.extend_from_slice(message);
    let k_bytes: [u8; 32] = Sha256::digest(&nonce_input).into();
    let k = Scalar::from_bytes_mod_order(k_bytes);

    let r_point = (k * G).compress();

    let e = schnorr_challenge(&r_point.to_bytes(), &pubkey, message);
    let s = k + e * privkey;

    SchnorrSignature {
        r_point: r_point.to_bytes(),
        s_scalar: s.to_bytes(),
    }
}

/// Verify a Schnorr signature against a public key.
///
/// Checks: s*G == R + e*pubkey
pub fn schnorr_verify(pubkey: &[u8; 32], message: &[u8], sig: &SchnorrSignature) -> bool {
    let pk_point = match CompressedRistretto(*pubkey).decompress() {
        Some(p) => p,
        None => return false,
    };
    let r_point = match CompressedRistretto(sig.r_point).decompress() {
        Some(p) => p,
        None => return false,
    };

    let e = schnorr_challenge(&sig.r_point, pubkey, message);
    let s = Scalar::from_bytes_mod_order(sig.s_scalar);

    let lhs = s * G;
    let rhs = r_point + e * pk_point;

    lhs == rhs
}

/// Compute Schnorr challenge scalar.
fn schnorr_challenge(r_bytes: &[u8; 32], pubkey: &[u8; 32], message: &[u8]) -> Scalar {
    let mut hasher = Sha256::new();
    hasher.update(b"C0DL3:schnorr:");
    hasher.update(r_bytes);
    hasher.update(pubkey);
    hasher.update(message);
    let hash: [u8; 32] = hasher.finalize().into();
    Scalar::from_bytes_mod_order(hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;

    fn random_scalar() -> Scalar {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        Scalar::from_bytes_mod_order(bytes)
    }

    #[test]
    fn test_schnorr_sign_verify_roundtrip() {
        let privkey = random_scalar();
        let pubkey = (privkey * G).compress().to_bytes();
        let message = b"transfer 100 to Bob";

        let sig = schnorr_sign(&privkey, message);
        assert!(schnorr_verify(&pubkey, message, &sig));
    }

    #[test]
    fn test_schnorr_wrong_message_fails() {
        let privkey = random_scalar();
        let pubkey = (privkey * G).compress().to_bytes();

        let sig = schnorr_sign(&privkey, b"original message");
        assert!(!schnorr_verify(&pubkey, b"tampered message", &sig));
    }

    #[test]
    fn test_schnorr_wrong_pubkey_fails() {
        let privkey = random_scalar();
        let wrong_pubkey = (random_scalar() * G).compress().to_bytes();
        let message = b"test";

        let sig = schnorr_sign(&privkey, message);
        assert!(!schnorr_verify(&wrong_pubkey, message, &sig));
    }

    #[test]
    fn test_schnorr_deterministic_nonce() {
        let privkey = random_scalar();
        let message = b"same message";

        let sig1 = schnorr_sign(&privkey, message);
        let sig2 = schnorr_sign(&privkey, message);
        assert_eq!(sig1.r_point, sig2.r_point);
        assert_eq!(sig1.s_scalar, sig2.s_scalar);
    }

    #[test]
    fn test_schnorr_different_messages_different_sigs() {
        let privkey = random_scalar();
        let sig1 = schnorr_sign(&privkey, b"message A");
        let sig2 = schnorr_sign(&privkey, b"message B");
        assert_ne!(sig1.r_point, sig2.r_point);
    }

    #[test]
    fn test_schnorr_invalid_points() {
        let sig = SchnorrSignature {
            r_point: [0xFF; 32], // not a valid Ristretto point
            s_scalar: [0; 32],
        };
        assert!(!schnorr_verify(&[0xFF; 32], b"test", &sig));
    }
}
