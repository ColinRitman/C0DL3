// Stealth addresses for C0DL3.
//
// Stealth addresses let a sender create a one-time address for the recipient.
// Only the recipient can identify and spend from it.

use curve25519_dalek_ng::{
    constants::RISTRETTO_BASEPOINT_POINT as G,
    ristretto::CompressedRistretto,
    scalar::Scalar,
};
use sha2::{Digest, Sha256};

/// Generate a stealth address for a recipient.
///
/// Returns: `(stealth_pub, ephemeral_pub, offset)` or `None` if the
/// recipient pubkey is not a valid compressed Ristretto point.
pub fn generate_stealth_address(
    recipient_pubkey: &[u8; 32],
) -> Option<([u8; 32], [u8; 32], [u8; 32])> {
    let recipient = CompressedRistretto(*recipient_pubkey).decompress()?;

    let mut k_bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut k_bytes);
    let k = Scalar::from_bytes_mod_order(k_bytes);

    let ephemeral_pub = (k * G).compress().to_bytes();
    let shared = (k * recipient).compress().to_bytes();

    let mut hasher = Sha256::new();
    hasher.update(b"C0DL3:stealth:");
    hasher.update(&shared);
    let offset: [u8; 32] = hasher.finalize().into();
    let offset_scalar = Scalar::from_bytes_mod_order(offset);

    let stealth_pub = (recipient + offset_scalar * G).compress().to_bytes();

    Some((stealth_pub, ephemeral_pub, offset))
}

/// Scan for stealth addresses — recipient checks if an ephemeral pubkey targets them.
///
/// Returns `Some(stealth_privkey)` if this ephemeral key was meant for the
/// given recipient, or `None` otherwise.
pub fn scan_stealth_address(
    recipient_privkey: &Scalar,
    ephemeral_pubkey: &[u8; 32],
    expected_stealth_pubkey: &[u8; 32],
) -> Option<Scalar> {
    let ephemeral = CompressedRistretto(*ephemeral_pubkey).decompress()?;
    let shared = (recipient_privkey * ephemeral).compress().to_bytes();

    let mut hasher = Sha256::new();
    hasher.update(b"C0DL3:stealth:");
    hasher.update(&shared);
    let offset: [u8; 32] = hasher.finalize().into();
    let offset_scalar = Scalar::from_bytes_mod_order(offset);

    let stealth_privkey = recipient_privkey + offset_scalar;
    let derived_pub = (stealth_privkey * G).compress().to_bytes();
    if derived_pub == *expected_stealth_pubkey {
        Some(stealth_privkey)
    } else {
        None
    }
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
    fn test_stealth_address_roundtrip() {
        let privkey = random_scalar();
        let pubkey = (privkey * G).compress().to_bytes();

        let (stealth_pub, ephemeral_pub, _offset) =
            generate_stealth_address(&pubkey).expect("generate must succeed");

        let stealth_priv = scan_stealth_address(&privkey, &ephemeral_pub, &stealth_pub)
            .expect("scan must succeed for correct recipient");

        // stealth_priv * G must equal stealth_pub
        let derived = (stealth_priv * G).compress().to_bytes();
        assert_eq!(derived, stealth_pub);
    }

    #[test]
    fn test_stealth_wrong_recipient_fails() {
        let privkey = random_scalar();
        let pubkey = (privkey * G).compress().to_bytes();

        let (stealth_pub, ephemeral_pub, _) =
            generate_stealth_address(&pubkey).expect("generate must succeed");

        // Wrong private key
        let wrong_privkey = random_scalar();
        let result = scan_stealth_address(&wrong_privkey, &ephemeral_pub, &stealth_pub);
        assert!(result.is_none(), "wrong recipient must fail scan");
    }

    #[test]
    fn test_stealth_addresses_are_unique() {
        let privkey = random_scalar();
        let pubkey = (privkey * G).compress().to_bytes();

        let (stealth1, _, _) = generate_stealth_address(&pubkey).unwrap();
        let (stealth2, _, _) = generate_stealth_address(&pubkey).unwrap();

        assert_ne!(stealth1, stealth2, "two stealth addresses must differ");
    }
}
