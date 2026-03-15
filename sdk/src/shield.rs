// Shield and Unshield request builders.
//
// These are the opaque request types that wallets send to the sequencer.
// The sequencer never sees plaintext amounts — only commitments and proofs.

use anyhow::Result;
use bulletproofs::{BulletproofGens, PedersenGens, RangeProof};
use curve25519_dalek_ng::scalar::Scalar;
use merlin::Transcript;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::commitment_proof::{
    compute_commitment, prove_commitment_knowledge_with_commitment, CommitmentKnowledgeProof,
};

static PEDERSEN_GENS: Lazy<PedersenGens> = Lazy::new(PedersenGens::default);
static BP_GENS: Lazy<BulletproofGens> = Lazy::new(|| BulletproofGens::new(64, 128));

/// Shield request — deposit from EVM into the shielded pool.
///
/// The sequencer sees: note_commitment, value_commitment, recipient_pubkey, and proofs.
/// The sequencer does NOT see: the amount or blinding factor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldRequest {
    /// Note commitment = SHA-256("C0DL3:note:" || value_commitment || recipient_pubkey)
    pub note_commitment: [u8; 32],
    /// Pedersen commitment to the shielded amount: C = amount*G + blinding*H
    pub value_commitment: [u8; 32],
    /// Recipient's one-time public key (stealth address)
    pub recipient_pubkey: [u8; 32],
    /// Proof that the creator knows (amount, blinding) for value_commitment
    pub knowledge_proof: CommitmentKnowledgeProof,
    /// Bulletproofs range proof: amount in [0, 2^64)
    pub range_proof: Vec<u8>,
}

/// Unshield request — withdraw from shielded pool back to EVM.
///
/// The sequencer sees: nullifier, note_commitment, value_commitment, and proofs.
/// The sequencer does NOT see: the amount, blinding factor, or spend key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnshieldRequest {
    /// Nullifier proving this note is being spent (prevents double-spend)
    pub nullifier: [u8; 32],
    /// The note commitment being spent (must exist in the note tree)
    pub note_commitment: [u8; 32],
    /// Pedersen commitment to the withdrawn amount
    pub value_commitment: [u8; 32],
    /// Proof that the creator knows (amount, blinding) for value_commitment
    pub knowledge_proof: CommitmentKnowledgeProof,
    /// Merkle proof that note_commitment exists in the note tree
    pub merkle_proof: Vec<([u8; 32], bool)>,
}

/// Build a shield request (deposit into shielded pool).
///
/// Called by the wallet. The amount and blinding factor stay on the user's device.
///
/// # Arguments
/// * `amount` - The amount to shield (plaintext, stays local)
/// * `blinding` - Random blinding factor (stays local)
/// * `recipient_pubkey` - Recipient's stealth address public key
///
/// # Returns
/// A `ShieldRequest` containing only commitments and proofs — no plaintext.
pub fn create_shield_request(
    amount: u64,
    blinding: &Scalar,
    recipient_pubkey: [u8; 32],
) -> Result<ShieldRequest> {
    // Compute value commitment: C = amount*G + blinding*H
    let value_commitment = compute_commitment(amount, blinding);

    // Compute note commitment: SHA-256("C0DL3:note:" || value_commitment || pubkey)
    let mut hasher = Sha256::new();
    hasher.update(b"C0DL3:note:");
    hasher.update(&value_commitment);
    hasher.update(&recipient_pubkey);
    let note_commitment: [u8; 32] = hasher.finalize().into();

    // Generate knowledge proof
    let knowledge_proof =
        prove_commitment_knowledge_with_commitment(amount, blinding, value_commitment);

    // Generate range proof: amount in [0, 2^64)
    let mut transcript = Transcript::new(b"C0DL3-ShieldedPool-RangeProof");
    let (rp, _committed) = RangeProof::prove_single(
        &BP_GENS,
        &PEDERSEN_GENS,
        &mut transcript,
        amount,
        blinding,
        64,
    )
    .map_err(|e| anyhow::anyhow!("range proof generation failed: {:?}", e))?;

    Ok(ShieldRequest {
        note_commitment,
        value_commitment,
        recipient_pubkey,
        knowledge_proof,
        range_proof: rp.to_bytes(),
    })
}

/// Build an unshield request (withdraw from shielded pool to EVM).
///
/// Called by the wallet. The spend key and amount stay on the user's device.
///
/// # Arguments
/// * `nullifier` - Pre-computed nullifier for this note
/// * `note_commitment` - The note being spent
/// * `amount` - The amount being withdrawn (plaintext, stays local)
/// * `blinding` - The blinding factor for the value commitment
/// * `merkle_proof` - Proof that note_commitment is in the note tree
pub fn create_unshield_request(
    nullifier: [u8; 32],
    note_commitment: [u8; 32],
    amount: u64,
    blinding: &Scalar,
    merkle_proof: Vec<([u8; 32], bool)>,
) -> Result<UnshieldRequest> {
    // Compute value commitment
    let value_commitment = compute_commitment(amount, blinding);

    // Generate knowledge proof
    let knowledge_proof =
        prove_commitment_knowledge_with_commitment(amount, blinding, value_commitment);

    Ok(UnshieldRequest {
        nullifier,
        note_commitment,
        value_commitment,
        knowledge_proof,
        merkle_proof,
    })
}

/// Verify a shield request (sequencer-side early rejection).
///
/// The SP1 guest re-verifies everything, so this is just for fast rejection
/// of obviously invalid requests.
pub fn verify_shield_request(request: &ShieldRequest) -> Result<bool> {
    // 1. Verify knowledge proof
    if !crate::verify_commitment_knowledge(&request.knowledge_proof) {
        return Ok(false);
    }

    // 2. Verify the knowledge proof's commitment matches the request's value_commitment
    if request.knowledge_proof.commitment != request.value_commitment {
        return Ok(false);
    }

    // 3. Verify note commitment derivation
    let mut hasher = Sha256::new();
    hasher.update(b"C0DL3:note:");
    hasher.update(&request.value_commitment);
    hasher.update(&request.recipient_pubkey);
    let expected_note: [u8; 32] = hasher.finalize().into();
    if expected_note != request.note_commitment {
        return Ok(false);
    }

    // 4. Verify range proof
    let rp = RangeProof::from_bytes(&request.range_proof)
        .map_err(|e| anyhow::anyhow!("invalid range proof: {:?}", e))?;
    let mut transcript = Transcript::new(b"C0DL3-ShieldedPool-RangeProof");
    let committed =
        curve25519_dalek_ng::ristretto::CompressedRistretto(request.value_commitment);
    rp.verify_single(&BP_GENS, &PEDERSEN_GENS, &mut transcript, &committed, 64)
        .map_err(|e| anyhow::anyhow!("range proof verification failed: {:?}", e))?;

    Ok(true)
}

/// Verify an unshield request (sequencer-side early rejection).
///
/// Does NOT check Merkle membership (needs the current tree root from the node).
/// The SP1 guest does the full check.
pub fn verify_unshield_request(request: &UnshieldRequest) -> Result<bool> {
    // 1. Verify knowledge proof
    if !crate::verify_commitment_knowledge(&request.knowledge_proof) {
        return Ok(false);
    }

    // 2. Verify the knowledge proof's commitment matches
    if request.knowledge_proof.commitment != request.value_commitment {
        return Ok(false);
    }

    Ok(true)
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

    fn random_pubkey() -> [u8; 32] {
        let s = random_scalar();
        (s * PEDERSEN_GENS.B).compress().to_bytes()
    }

    #[test]
    fn test_shield_request_roundtrip() {
        let amount = 1000u64;
        let blinding = random_scalar();
        let pubkey = random_pubkey();

        let request = create_shield_request(amount, &blinding, pubkey).unwrap();

        // Verify all fields are populated
        assert_ne!(request.note_commitment, [0u8; 32]);
        assert_ne!(request.value_commitment, [0u8; 32]);
        assert_eq!(request.recipient_pubkey, pubkey);
        assert!(!request.range_proof.is_empty());

        // Verify the request
        assert!(verify_shield_request(&request).unwrap());
    }

    #[test]
    fn test_shield_request_various_amounts() {
        for &amount in &[0u64, 1, 1000, 1_000_000_000] {
            let blinding = random_scalar();
            let pubkey = random_pubkey();
            let request = create_shield_request(amount, &blinding, pubkey).unwrap();
            assert!(
                verify_shield_request(&request).unwrap(),
                "shield request for amount {} must verify",
                amount
            );
        }
    }

    #[test]
    fn test_shield_request_tampered_commitment() {
        let amount = 500u64;
        let blinding = random_scalar();
        let pubkey = random_pubkey();

        let mut request = create_shield_request(amount, &blinding, pubkey).unwrap();

        // Tamper with value commitment
        request.value_commitment = compute_commitment(999, &random_scalar());

        assert!(
            !verify_shield_request(&request).unwrap(),
            "tampered commitment must fail"
        );
    }

    #[test]
    fn test_unshield_request_roundtrip() {
        let amount = 500u64;
        let blinding = random_scalar();
        let nullifier = [0xABu8; 32];
        let note_commitment = [0xCDu8; 32];

        let request = create_unshield_request(
            nullifier,
            note_commitment,
            amount,
            &blinding,
            vec![], // empty merkle proof for this test
        )
        .unwrap();

        assert_eq!(request.nullifier, nullifier);
        assert_eq!(request.note_commitment, note_commitment);
        assert!(verify_unshield_request(&request).unwrap());
    }

    #[test]
    fn test_unshield_request_tampered_knowledge_proof() {
        let amount = 500u64;
        let blinding = random_scalar();

        let mut request = create_unshield_request(
            [0xAB; 32],
            [0xCD; 32],
            amount,
            &blinding,
            vec![],
        )
        .unwrap();

        // Tamper with knowledge proof announcement
        request.knowledge_proof.announcement = [0xFF; 32];

        assert!(
            !verify_unshield_request(&request).unwrap(),
            "tampered knowledge proof must fail"
        );
    }

    #[test]
    fn test_shield_hides_amount() {
        // Two shield requests with different amounts should have different commitments
        // but the request itself never contains the plaintext amount
        let blinding1 = random_scalar();
        let blinding2 = random_scalar();
        let pubkey = random_pubkey();

        let req1 = create_shield_request(100, &blinding1, pubkey).unwrap();
        let req2 = create_shield_request(200, &blinding2, pubkey).unwrap();

        // Different value commitments
        assert_ne!(req1.value_commitment, req2.value_commitment);
        // Different note commitments
        assert_ne!(req1.note_commitment, req2.note_commitment);

        // Both verify
        assert!(verify_shield_request(&req1).unwrap());
        assert!(verify_shield_request(&req2).unwrap());

        // The ShieldRequest struct has no `amount` field — privacy by construction
    }
}
