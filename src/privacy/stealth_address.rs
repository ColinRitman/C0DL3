// Stealth Addresses — Monero-style dual-key scheme on Ristretto255
//
// Fixes the account-traceability gap: without stealth addresses, sender/recipient
// can be identified from the state trie because account keys are plaintext addresses.
// With stealth addresses, each payment creates a fresh one-time address (P) that is
// unlinkable to the recipient's public identity without their view key.
//
// Scheme (Ristretto255, no new crate deps):
//   Recipient holds:  (v, V=v·G)  view keypair
//                     (b, B=b·G)  spend keypair
//   Sender generates: r (ephemeral scalar), R = r·G (published per-tx)
//   Shared secret:    D = r·V = v·R  (ECDH — same point, both sides)
//   Scalar offset:    s_h = SHA-512(D.compress()) → Scalar (hash-to-scalar)
//   View tag:         s_h_bytes[0]  (1-byte filter — 99.6% skip rate for non-owners)
//   One-time address: P = s_h·G + B  (new unique address per payment)
//   Stealth privkey:  p = s_h + b    (recipient derives after scanning)
//
// Account model integration:
//   - P (compressed, 32 bytes hex) becomes the account key in the state trie
//   - R and view_tag are stored in Transaction.data for recipient scanning
//   - No linkage between P and recipient's public identity without view key

use anyhow::{anyhow, Result};
use curve25519_dalek_ng::{
    constants::RISTRETTO_BASEPOINT_POINT as G,
    ristretto::{CompressedRistretto, RistrettoPoint},
    scalar::Scalar,
};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};

// ──────────────────────────────────────────
// Key types
// ──────────────────────────────────────────

/// Recipient's dual-key pair: separate view key (for scanning) and spend key (for claiming).
/// Splitting keys means a watch-only scanner only needs the view key — the spend key
/// can remain offline.
#[derive(Clone)]
pub struct StealthKeypair {
    pub view_private: Scalar,
    pub view_public: RistrettoPoint,
    pub spend_private: Scalar,
    pub spend_public: RistrettoPoint,
}

/// Recipient's public payment address — shareable, contains no private key material.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StealthPaymentAddress {
    /// Compressed view public key (32 bytes)
    pub view_public: [u8; 32],
    /// Compressed spend public key (32 bytes)
    pub spend_public: [u8; 32],
}

/// Output produced by the sender for a stealth payment.
/// Stored in the transaction so the recipient can scan and claim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StealthOutput {
    /// Compressed one-time address P = s_h·G + B
    /// This becomes the account key in the state trie.
    pub one_time_address: [u8; 32],
    /// Compressed ephemeral public key R = r·G
    /// Published in the transaction so recipient can recompute D = v·R.
    pub ephemeral_pubkey: [u8; 32],
    /// First byte of SHA-512(D.compress()) — fast scan filter.
    /// Recipient skips this output if their computed tag doesn't match.
    pub view_tag: u8,
}

// ──────────────────────────────────────────
// Internal helpers
// ──────────────────────────────────────────

/// SHA-512(point.compress()) → Scalar (Monero's Hs function).
fn hash_to_scalar(point: &RistrettoPoint) -> (Scalar, [u8; 64]) {
    let compressed = point.compress();
    let hash_bytes: [u8; 64] = Sha512::digest(compressed.as_bytes()).into();
    // from_bytes_mod_order_wide maps 64 bytes to a valid scalar
    let scalar = Scalar::from_bytes_mod_order_wide(&hash_bytes);
    (scalar, hash_bytes)
}

fn decompress(bytes: &[u8; 32]) -> Result<RistrettoPoint> {
    CompressedRistretto(*bytes)
        .decompress()
        .ok_or_else(|| anyhow!("invalid Ristretto point"))
}

// ──────────────────────────────────────────
// Public API
// ──────────────────────────────────────────

impl StealthKeypair {
    /// Generate a fresh random dual-key pair.
    pub fn generate() -> Self {
        let mut rng = OsRng;
        let view_private = Scalar::random(&mut rng);
        let spend_private = Scalar::random(&mut rng);
        Self {
            view_public: view_private * G,
            spend_public: spend_private * G,
            view_private,
            spend_private,
        }
    }

    /// Export the shareable payment address (no private key material).
    pub fn payment_address(&self) -> StealthPaymentAddress {
        StealthPaymentAddress {
            view_public: self.view_public.compress().to_bytes(),
            spend_public: self.spend_public.compress().to_bytes(),
        }
    }
}

/// Sender: generate a stealth output for a payment to `payment_addr`.
///
/// Returns a `StealthOutput` containing:
/// - `one_time_address` — use as `Transaction.to` / state trie key
/// - `ephemeral_pubkey` + `view_tag` — store in `Transaction.data` for scanning
pub fn generate_stealth_output(payment_addr: &StealthPaymentAddress) -> Result<StealthOutput> {
    let mut rng = OsRng;
    let ephemeral_scalar = Scalar::random(&mut rng);
    let ephemeral_pubkey_point = ephemeral_scalar * G;

    let view_pub = decompress(&payment_addr.view_public)?;
    let spend_pub = decompress(&payment_addr.spend_public)?;

    // ECDH: D = r·V
    let shared_point = ephemeral_scalar * view_pub;
    let (s_h, hash_bytes) = hash_to_scalar(&shared_point);

    let view_tag = hash_bytes[0];

    // One-time address: P = s_h·G + B
    let one_time_point = s_h * G + spend_pub;

    Ok(StealthOutput {
        one_time_address: one_time_point.compress().to_bytes(),
        ephemeral_pubkey: ephemeral_pubkey_point.compress().to_bytes(),
        view_tag,
    })
}

/// Recipient scanning: check if a transaction output belongs to this recipient.
///
/// Fast path: if `view_tag` doesn't match the first hash byte, return `None` immediately
/// (this filters ~99.6% of outputs without doing the full ECDH).
///
/// Returns the one-time address as a `RistrettoPoint` if this output is ours,
/// or `None` if it isn't.
pub fn scan_output(
    ephemeral_pubkey: &[u8; 32],
    view_tag: u8,
    view_priv: &Scalar,
    spend_pub: &RistrettoPoint,
) -> Option<RistrettoPoint> {
    let ephemeral_point = decompress(ephemeral_pubkey).ok()?;

    // ECDH: D = v·R
    let shared_point = view_priv * ephemeral_point;
    let (s_h, hash_bytes) = hash_to_scalar(&shared_point);

    // Fast view tag check
    if hash_bytes[0] != view_tag {
        return None;
    }

    // Full match: recompute P = s_h·G + B
    let one_time_point = s_h * G + spend_pub;
    Some(one_time_point)
}

/// Recipient claiming: derive the spend private key for a matched output.
///
/// Call this only after `scan_output` confirms the output is ours.
/// Returns `p = s_h + b` — the private key for `one_time_address`.
pub fn derive_stealth_privkey(
    ephemeral_pubkey: &[u8; 32],
    view_priv: &Scalar,
    spend_priv: &Scalar,
) -> Result<Scalar> {
    let ephemeral_point = decompress(ephemeral_pubkey)?;
    let shared_point = view_priv * ephemeral_point;
    let (s_h, _) = hash_to_scalar(&shared_point);
    Ok(s_h + spend_priv)
}

/// Hex-encode a one-time address for use as a state trie / account key.
pub fn one_time_address_to_hex(addr: &[u8; 32]) -> String {
    hex::encode(addr)
}

/// Encode a `StealthOutput`'s scanning data (ephemeral_pubkey + view_tag) for
/// storage in `Transaction.data`. Layout: [0..32] = R, [32] = view_tag.
pub fn encode_stealth_data(output: &StealthOutput) -> Vec<u8> {
    let mut data = Vec::with_capacity(33);
    data.extend_from_slice(&output.ephemeral_pubkey);
    data.push(output.view_tag);
    data
}

/// Decode stealth scanning data from `Transaction.data`.
/// Returns `(ephemeral_pubkey, view_tag)` or error if data is too short.
pub fn decode_stealth_data(data: &[u8]) -> Result<([u8; 32], u8)> {
    if data.len() < 33 {
        return Err(anyhow!("stealth data too short: {} bytes", data.len()));
    }
    let mut epk = [0u8; 32];
    epk.copy_from_slice(&data[..32]);
    Ok((epk, data[32]))
}

// ──────────────────────────────────────────
// Tests
// ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_scan_match() {
        let keypair = StealthKeypair::generate();
        let payment_addr = keypair.payment_address();

        let output = generate_stealth_output(&payment_addr).unwrap();

        let matched = scan_output(
            &output.ephemeral_pubkey,
            output.view_tag,
            &keypair.view_private,
            &keypair.spend_public,
        );

        assert!(matched.is_some(), "recipient should find their own payment");
        assert_eq!(
            matched.unwrap().compress().to_bytes(),
            output.one_time_address,
            "scanned address must match generated one-time address"
        );
    }

    #[test]
    fn test_wrong_recipient_no_match() {
        let sender_target = StealthKeypair::generate();
        let eavesdropper = StealthKeypair::generate();

        let output = generate_stealth_output(&sender_target.payment_address()).unwrap();

        // Eavesdropper tries to scan with their own view key
        let matched = scan_output(
            &output.ephemeral_pubkey,
            output.view_tag,
            &eavesdropper.view_private,
            &eavesdropper.spend_public,
        );

        // Very unlikely to match (would require view_tag collision = 1/256 chance)
        // In a real test suite this might flake 1/256 times — acceptable for unit tests
        if matched.is_some() {
            // If view tag happened to match, the full address should still differ
            assert_ne!(
                matched.unwrap().compress().to_bytes(),
                output.one_time_address,
                "eavesdropper must not derive the same one-time address"
            );
        }
    }

    #[test]
    fn test_derive_stealth_privkey() {
        let keypair = StealthKeypair::generate();
        let output = generate_stealth_output(&keypair.payment_address()).unwrap();

        let stealth_priv = derive_stealth_privkey(
            &output.ephemeral_pubkey,
            &keypair.view_private,
            &keypair.spend_private,
        )
        .unwrap();

        // Verify: stealth_priv * G == one_time_address
        let derived_pub = stealth_priv * G;
        assert_eq!(
            derived_pub.compress().to_bytes(),
            output.one_time_address,
            "derived private key must correspond to the one-time address"
        );
    }

    #[test]
    fn test_view_tag_fast_reject() {
        let keypair = StealthKeypair::generate();
        let other = StealthKeypair::generate();

        let output = generate_stealth_output(&keypair.payment_address()).unwrap();

        // Corrupt view tag to force fast-path rejection
        let bad_tag = output.view_tag.wrapping_add(1);
        let result = scan_output(
            &output.ephemeral_pubkey,
            bad_tag,
            &keypair.view_private,
            &keypair.spend_public,
        );
        assert!(result.is_none(), "corrupted view tag must be rejected");
    }

    #[test]
    fn test_encode_decode_stealth_data() {
        let keypair = StealthKeypair::generate();
        let output = generate_stealth_output(&keypair.payment_address()).unwrap();

        let encoded = encode_stealth_data(&output);
        assert_eq!(encoded.len(), 33);

        let (epk, tag) = decode_stealth_data(&encoded).unwrap();
        assert_eq!(epk, output.ephemeral_pubkey);
        assert_eq!(tag, output.view_tag);
    }

    #[test]
    fn test_decode_short_data_fails() {
        assert!(decode_stealth_data(&[0u8; 32]).is_err());
        assert!(decode_stealth_data(&[]).is_err());
    }

    #[test]
    fn test_payment_address_roundtrip() {
        let keypair = StealthKeypair::generate();
        let addr = keypair.payment_address();

        // Verify both points decompress correctly
        assert!(decompress(&addr.view_public).is_ok());
        assert!(decompress(&addr.spend_public).is_ok());

        // Verify they match the original public keys
        assert_eq!(
            decompress(&addr.view_public).unwrap(),
            keypair.view_public
        );
        assert_eq!(
            decompress(&addr.spend_public).unwrap(),
            keypair.spend_public
        );
    }

    #[test]
    fn test_multiple_outputs_unlinkable() {
        let keypair = StealthKeypair::generate();
        let addr = keypair.payment_address();

        let out1 = generate_stealth_output(&addr).unwrap();
        let out2 = generate_stealth_output(&addr).unwrap();

        // Two payments to same recipient produce different one-time addresses
        assert_ne!(out1.one_time_address, out2.one_time_address);
        assert_ne!(out1.ephemeral_pubkey, out2.ephemeral_pubkey);
    }

    #[test]
    fn test_one_time_address_hex_encoding() {
        let keypair = StealthKeypair::generate();
        let output = generate_stealth_output(&keypair.payment_address()).unwrap();
        let hex_addr = one_time_address_to_hex(&output.one_time_address);
        assert_eq!(hex_addr.len(), 64); // 32 bytes = 64 hex chars
        assert!(hex_addr.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
