// Schnorr-style proof of knowledge for Pedersen commitments.
//
// Proves: "I know (v, r) such that C = v*G + r*H"
// without revealing v or r.
//
// Protocol (Fiat-Shamir heuristic, non-interactive):
//   1. Prover picks random k_v, k_r
//   2. Computes R = k_v*G + k_r*H (announcement)
//   3. Challenge e = SHA-256("C0DL3:ck_proof:" || C || R) → Scalar
//   4. Response s_v = k_v + e*v, s_r = k_r + e*r
//   5. Verifier checks: s_v*G + s_r*H == R + e*C
//
// Proof size: 96 bytes (R: 32, s_v: 32, s_r: 32)
// Generation time: <1ms on any device

use bulletproofs::PedersenGens;
use curve25519_dalek_ng::{
    ristretto::CompressedRistretto,
    scalar::Scalar,
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

static PEDERSEN_GENS: Lazy<PedersenGens> = Lazy::new(PedersenGens::default);

/// Proof of knowledge for a Pedersen commitment C = v*G + r*H.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitmentKnowledgeProof {
    /// The Pedersen commitment C = v*G + r*H (compressed Ristretto)
    pub commitment: [u8; 32],
    /// Announcement R = k_v*G + k_r*H (compressed Ristretto)
    pub announcement: [u8; 32],
    /// Response for value: s_v = k_v + e*v
    pub response_v: [u8; 32],
    /// Response for blinding: s_r = k_r + e*r
    pub response_r: [u8; 32],
}

/// Encode a u64 value as a Scalar (matching host/guest convention).
fn value_to_scalar(value: u64) -> Scalar {
    let mut padded = [0u8; 32];
    padded[..8].copy_from_slice(&value.to_le_bytes());
    Scalar::from_bytes_mod_order(padded)
}

/// Compute the Fiat-Shamir challenge.
fn compute_challenge(commitment: &[u8; 32], announcement: &[u8; 32]) -> Scalar {
    let mut hasher = Sha256::new();
    hasher.update(b"C0DL3:ck_proof:");
    hasher.update(commitment);
    hasher.update(announcement);
    let hash: [u8; 32] = hasher.finalize().into();
    Scalar::from_bytes_mod_order(hash)
}

/// Generate a proof of knowledge for a Pedersen commitment.
///
/// The caller knows `value` and `blinding` such that `C = value*G + blinding*H`.
/// Returns a proof that can be verified without knowing these secrets.
pub fn prove_commitment_knowledge(value: u64, blinding: &Scalar) -> CommitmentKnowledgeProof {
    let v = value_to_scalar(value);
    let gens = &*PEDERSEN_GENS;

    // Compute commitment
    let c_point = gens.commit(v, *blinding);
    let c_bytes = c_point.compress().to_bytes();

    prove_commitment_knowledge_with_commitment(value, blinding, c_bytes)
}

/// Generate a proof for a pre-computed commitment.
///
/// Use when the commitment is already known (e.g., from an existing note).
pub fn prove_commitment_knowledge_with_commitment(
    value: u64,
    blinding: &Scalar,
    commitment: [u8; 32],
) -> CommitmentKnowledgeProof {
    let v = value_to_scalar(value);
    let gens = &*PEDERSEN_GENS;

    // Step 1: Pick random nonces
    let mut rng = rand::thread_rng();
    let mut k_v_bytes = [0u8; 32];
    let mut k_r_bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rng, &mut k_v_bytes);
    rand::RngCore::fill_bytes(&mut rng, &mut k_r_bytes);
    let k_v = Scalar::from_bytes_mod_order(k_v_bytes);
    let k_r = Scalar::from_bytes_mod_order(k_r_bytes);

    // Step 2: Compute announcement R = k_v*G + k_r*H
    let r_point = gens.commit(k_v, k_r);
    let r_bytes = r_point.compress().to_bytes();

    // Step 3: Fiat-Shamir challenge
    let e = compute_challenge(&commitment, &r_bytes);

    // Step 4: Responses
    let s_v = k_v + e * v;
    let s_r = k_r + e * *blinding;

    CommitmentKnowledgeProof {
        commitment,
        announcement: r_bytes,
        response_v: s_v.to_bytes(),
        response_r: s_r.to_bytes(),
    }
}

/// Verify a proof of knowledge for a Pedersen commitment.
///
/// Returns true if the prover knows (v, r) such that C = v*G + r*H.
/// Returns false on any invalid input (bad points, failed check).
pub fn verify_commitment_knowledge(proof: &CommitmentKnowledgeProof) -> bool {
    let gens = &*PEDERSEN_GENS;

    // Decompress commitment and announcement
    let c_point = match CompressedRistretto(proof.commitment).decompress() {
        Some(p) => p,
        None => return false,
    };
    let r_point = match CompressedRistretto(proof.announcement).decompress() {
        Some(p) => p,
        None => return false,
    };

    // Recompute challenge
    let e = compute_challenge(&proof.commitment, &proof.announcement);

    // Deserialize responses
    let s_v = Scalar::from_canonical_bytes(proof.response_v);
    let s_r = Scalar::from_canonical_bytes(proof.response_r);

    // from_canonical_bytes returns None if not in [0, L)
    // Fall back to from_bytes_mod_order which always succeeds
    let s_v = match s_v {
        Some(s) => s,
        None => Scalar::from_bytes_mod_order(proof.response_v),
    };
    let s_r = match s_r {
        Some(s) => s,
        None => Scalar::from_bytes_mod_order(proof.response_r),
    };

    // Verify: s_v*G + s_r*H == R + e*C
    let lhs = gens.commit(s_v, s_r);
    let rhs = r_point + e * c_point;

    lhs == rhs
}

/// Compute a Pedersen commitment for a value and blinding factor.
///
/// Convenience function matching the host's commitment scheme.
/// C = value*G + blinding*H
pub fn compute_commitment(value: u64, blinding: &Scalar) -> [u8; 32] {
    let v = value_to_scalar(value);
    PEDERSEN_GENS.commit(v, *blinding).compress().to_bytes()
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
    fn test_knowledge_proof_roundtrip() {
        let value = 1000u64;
        let blinding = random_scalar();

        let proof = prove_commitment_knowledge(value, &blinding);
        assert!(
            verify_commitment_knowledge(&proof),
            "valid proof must verify"
        );
    }

    #[test]
    fn test_knowledge_proof_various_values() {
        for &value in &[0u64, 1, 100, 1_000_000, u64::MAX] {
            let blinding = random_scalar();
            let proof = prove_commitment_knowledge(value, &blinding);
            assert!(
                verify_commitment_knowledge(&proof),
                "proof for value {} must verify",
                value
            );
        }
    }

    #[test]
    fn test_knowledge_proof_wrong_commitment() {
        let value = 500u64;
        let blinding = random_scalar();

        let mut proof = prove_commitment_knowledge(value, &blinding);

        // Tamper with commitment — replace with commitment to different value
        let wrong_blinding = random_scalar();
        proof.commitment = compute_commitment(999, &wrong_blinding);

        assert!(
            !verify_commitment_knowledge(&proof),
            "tampered commitment must fail"
        );
    }

    #[test]
    fn test_knowledge_proof_wrong_announcement() {
        let value = 500u64;
        let blinding = random_scalar();

        let mut proof = prove_commitment_knowledge(value, &blinding);

        // Tamper with announcement
        let k = random_scalar();
        proof.announcement = (k * PEDERSEN_GENS.B).compress().to_bytes();

        assert!(
            !verify_commitment_knowledge(&proof),
            "tampered announcement must fail"
        );
    }

    #[test]
    fn test_knowledge_proof_invalid_point() {
        let proof = CommitmentKnowledgeProof {
            commitment: [0xFF; 32], // not a valid point
            announcement: [0xFF; 32],
            response_v: [0; 32],
            response_r: [0; 32],
        };

        assert!(
            !verify_commitment_knowledge(&proof),
            "garbage points must fail"
        );
    }

    #[test]
    fn test_knowledge_proof_with_precomputed_commitment() {
        let value = 42u64;
        let blinding = random_scalar();
        let commitment = compute_commitment(value, &blinding);

        let proof = prove_commitment_knowledge_with_commitment(value, &blinding, commitment);
        assert_eq!(proof.commitment, commitment);
        assert!(verify_commitment_knowledge(&proof));
    }

    #[test]
    fn test_proofs_are_unique() {
        // Two proofs for the same (value, blinding) should differ (random nonces)
        let value = 100u64;
        let blinding = random_scalar();

        let proof1 = prove_commitment_knowledge(value, &blinding);
        let proof2 = prove_commitment_knowledge(value, &blinding);

        // Same commitment
        assert_eq!(proof1.commitment, proof2.commitment);
        // Different announcements (overwhelmingly likely)
        assert_ne!(proof1.announcement, proof2.announcement);
        // Both verify
        assert!(verify_commitment_knowledge(&proof1));
        assert!(verify_commitment_knowledge(&proof2));
    }
}
