// sdk/src/encrypted_memo.rs
// Encrypted memos — sender encrypts (amount, blinding) to recipient's pubkey.
//
// Uses ElGamal encryption over Ristretto255:
//   Encrypt(pubkey, amount, blinding):
//     k = random ephemeral scalar
//     C1 = k*G (ephemeral pubkey, 32 bytes)
//     shared = k * recipient_pubkey
//     mask = derive_memo_mask(shared) — 40 bytes
//     plaintext = amount(8 LE) || blinding(32)
//     C2 = plaintext XOR mask
//     memo = C1(32) || C2(40) = 72 bytes
//
//   Decrypt(privkey, memo):
//     shared = privkey * C1
//     mask = derive_memo_mask(shared)
//     plaintext = C2 XOR mask
//     returns (amount, blinding)

use curve25519_dalek_ng::{
    constants::RISTRETTO_BASEPOINT_POINT as G,
    ristretto::CompressedRistretto,
    scalar::Scalar,
};
use sha2::{Digest, Sha256};

/// Encrypt amount + blinding to recipient's public key.
///
/// Returns 72-byte memo: ephemeral_pubkey(32) || encrypted_data(40)
pub fn encrypt_memo(
    recipient_pubkey: &[u8; 32],
    amount: u64,
    blinding: &[u8; 32],
) -> Option<Vec<u8>> {
    let pk = CompressedRistretto(*recipient_pubkey).decompress()?;

    // Random ephemeral key
    let mut k_bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut k_bytes);
    let k = Scalar::from_bytes_mod_order(k_bytes);

    // C1 = k*G
    let c1 = (k * G).compress().to_bytes();

    // Shared secret = k * recipient_pubkey
    let shared = (k * pk).compress().to_bytes();

    // Derive mask (40 bytes: 8 for amount + 32 for blinding)
    let mask = derive_memo_mask(&shared);

    // Plaintext: amount(8) || blinding(32)
    let mut plaintext = [0u8; 40];
    plaintext[..8].copy_from_slice(&amount.to_le_bytes());
    plaintext[8..40].copy_from_slice(blinding);

    // XOR
    let mut encrypted = [0u8; 40];
    for i in 0..40 {
        encrypted[i] = plaintext[i] ^ mask[i];
    }

    let mut memo = Vec::with_capacity(72);
    memo.extend_from_slice(&c1);
    memo.extend_from_slice(&encrypted);
    Some(memo)
}

/// Decrypt a memo using the recipient's private key.
///
/// Returns (amount, blinding) if decryption succeeds.
pub fn decrypt_memo(privkey: &Scalar, memo: &[u8]) -> Option<(u64, [u8; 32])> {
    if memo.len() != 72 {
        return None;
    }

    let mut c1_bytes = [0u8; 32];
    c1_bytes.copy_from_slice(&memo[..32]);
    let c1 = CompressedRistretto(c1_bytes).decompress()?;

    // Shared secret = privkey * C1
    let shared = (privkey * c1).compress().to_bytes();

    let mask = derive_memo_mask(&shared);

    let mut plaintext = [0u8; 40];
    for i in 0..40 {
        plaintext[i] = memo[32 + i] ^ mask[i];
    }

    let amount = u64::from_le_bytes(plaintext[..8].try_into().ok()?);
    let mut blinding = [0u8; 32];
    blinding.copy_from_slice(&plaintext[8..40]);

    Some((amount, blinding))
}

/// Derive 40-byte mask from shared secret.
fn derive_memo_mask(shared_secret: &[u8; 32]) -> [u8; 40] {
    let h1: [u8; 32] = Sha256::digest(
        [b"C0DL3:memo:".as_slice(), shared_secret].concat()
    ).into();
    let h2: [u8; 32] = Sha256::digest(
        [b"C0DL3:memo:1:".as_slice(), shared_secret].concat()
    ).into();

    let mut mask = [0u8; 40];
    mask[..32].copy_from_slice(&h1);
    mask[32..40].copy_from_slice(&h2[..8]);
    mask
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random_scalar() -> Scalar {
        let mut bytes = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
        Scalar::from_bytes_mod_order(bytes)
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let privkey = random_scalar();
        let pubkey = (privkey * G).compress().to_bytes();
        let amount = 1000u64;
        let blinding = random_scalar().to_bytes();

        let memo = encrypt_memo(&pubkey, amount, &blinding).unwrap();
        assert_eq!(memo.len(), 72);

        let (dec_amount, dec_blinding) = decrypt_memo(&privkey, &memo).unwrap();
        assert_eq!(dec_amount, amount);
        assert_eq!(dec_blinding, blinding);
    }

    #[test]
    fn test_wrong_key_fails() {
        let privkey = random_scalar();
        let pubkey = (privkey * G).compress().to_bytes();
        let wrong_key = random_scalar();

        let memo = encrypt_memo(&pubkey, 500, &[0xAB; 32]).unwrap();
        let (dec_amount, _) = decrypt_memo(&wrong_key, &memo).unwrap();
        // With wrong key, decrypted amount is garbage (overwhelmingly != 500)
        assert_ne!(dec_amount, 500);
    }

    #[test]
    fn test_various_amounts() {
        let privkey = random_scalar();
        let pubkey = (privkey * G).compress().to_bytes();

        for &amount in &[0u64, 1, u64::MAX, 1_000_000_000] {
            let blinding = random_scalar().to_bytes();
            let memo = encrypt_memo(&pubkey, amount, &blinding).unwrap();
            let (dec_amount, dec_blinding) = decrypt_memo(&privkey, &memo).unwrap();
            assert_eq!(dec_amount, amount);
            assert_eq!(dec_blinding, blinding);
        }
    }

    #[test]
    fn test_invalid_memo_length() {
        let privkey = random_scalar();
        assert!(decrypt_memo(&privkey, &[0u8; 10]).is_none());
        assert!(decrypt_memo(&privkey, &[0u8; 100]).is_none());
    }
}
