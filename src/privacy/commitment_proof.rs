// Commitment Knowledge Proof — Schnorr-style sigma protocol
//
// Proves knowledge of a Pedersen commitment's preimage (v, r)
// such that C = v*G + r*H, without revealing v or r.
//
// Uses the Fiat-Shamir heuristic for non-interactive proofs.

use serde::{Deserialize, Serialize};
use curve25519_dalek_ng::{
    ristretto::{CompressedRistretto, RistrettoPoint},
    scalar::Scalar,
};
use sha2::{Sha256, Digest};
use bulletproofs::PedersenGens;
use once_cell::sync::Lazy;

static PEDERSEN_GENS: Lazy<PedersenGens> = Lazy::new(|| PedersenGens::default());

/// Schnorr-style proof of knowledge for a Pedersen commitment.
///
/// Proves: "I know (v, r) such that C = v*G + r*H"
/// without revealing v or r.
///
/// Protocol (Fiat-Shamir heuristic):
///   1. Prover picks random k_v, k_r
///   2. Computes R = k_v*G + k_r*H (announcement)
///   3. Challenge e = SHA-256("C0DL3:ck_proof:" || C || R)
///   4. Response s_v = k_v + e*v, s_r = k_r + e*r
///   5. Verifier checks: s_v*G + s_r*H == R + e*C
///
/// Proof size: 96 bytes (R: 32, s_v: 32, s_r: 32)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitmentKnowledgeProof {
    /// The Pedersen commitment C = v*G + r*H
    pub commitment: [u8; 32],
    /// Announcement R = k_v*G + k_r*H
    pub announcement: [u8; 32],
    /// Response for value: s_v = k_v + e*v
    pub response_v: [u8; 32],
    /// Response for blinding: s_r = k_r + e*r
    pub response_r: [u8; 32],
}

/// Encode a u64 value as a Scalar using the same pattern as shielded_pool.rs.
fn value_to_scalar(value: u64) -> Scalar {
    let mut amount_padded = [0u8; 32];
    amount_padded[..8].copy_from_slice(&value.to_le_bytes());
    Scalar::from_bytes_mod_order(amount_padded)
}

/// Compute the Fiat-Shamir challenge: e = SHA-256("C0DL3:ck_proof:" || C || R)
fn compute_challenge(commitment: &CompressedRistretto, announcement: &CompressedRistretto) -> Scalar {
    let mut hasher = Sha256::new();
    hasher.update(b"C0DL3:ck_proof:");
    hasher.update(commitment.as_bytes());
    hasher.update(announcement.as_bytes());
    let hash = hasher.finalize();
    let mut hash_bytes = [0u8; 32];
    hash_bytes.copy_from_slice(&hash);
    Scalar::from_bytes_mod_order(hash_bytes)
}

/// Generate random nonces k_v, k_r.
fn random_scalars() -> (Scalar, Scalar) {
    let mut rng = rand::thread_rng();
    let mut k_v_bytes = [0u8; 64];
    let mut k_r_bytes = [0u8; 64];
    rand::RngCore::fill_bytes(&mut rng, &mut k_v_bytes);
    rand::RngCore::fill_bytes(&mut rng, &mut k_r_bytes);
    (
        Scalar::from_bytes_mod_order_wide(&k_v_bytes),
        Scalar::from_bytes_mod_order_wide(&k_r_bytes),
    )
}

/// Prove knowledge of the preimage (value, blinding) for a Pedersen commitment.
///
/// Computes C = value*G + blinding*H internally, then produces a
/// Schnorr-style sigma proof that the prover knows (value, blinding).
pub fn prove_commitment_knowledge(value: u64, blinding: &Scalar) -> CommitmentKnowledgeProof {
    let gens = &*PEDERSEN_GENS;
    let v = value_to_scalar(value);

    // C = v*G + r*H
    let c_point: RistrettoPoint = v * gens.B + *blinding * gens.B_blinding;
    let c_compressed = c_point.compress();

    prove_inner(v, blinding, c_compressed, gens)
}

/// Prove knowledge of the preimage for a pre-computed Pedersen commitment.
///
/// Same as [`prove_commitment_knowledge`] but accepts a commitment that the
/// caller already has, avoiding a redundant point multiplication.
pub fn prove_commitment_knowledge_with_commitment(
    value: u64,
    blinding: &Scalar,
    commitment: [u8; 32],
) -> CommitmentKnowledgeProof {
    let gens = &*PEDERSEN_GENS;
    let v = value_to_scalar(value);
    let c_compressed = CompressedRistretto::from_slice(&commitment);

    prove_inner(v, blinding, c_compressed, gens)
}

/// Shared proving logic.
fn prove_inner(
    v: Scalar,
    blinding: &Scalar,
    c_compressed: CompressedRistretto,
    gens: &PedersenGens,
) -> CommitmentKnowledgeProof {
    // Step 1: random nonces
    let (k_v, k_r) = random_scalars();

    // Step 2: announcement R = k_v*G + k_r*H
    let r_point: RistrettoPoint = k_v * gens.B + k_r * gens.B_blinding;
    let r_compressed = r_point.compress();

    // Step 3: challenge
    let e = compute_challenge(&c_compressed, &r_compressed);

    // Step 4: responses
    let s_v = k_v + e * v;
    let s_r = k_r + e * *blinding;

    CommitmentKnowledgeProof {
        commitment: *c_compressed.as_bytes(),
        announcement: *r_compressed.as_bytes(),
        response_v: s_v.to_bytes(),
        response_r: s_r.to_bytes(),
    }
}

/// Verify a commitment knowledge proof.
///
/// Checks that `s_v*G + s_r*H == R + e*C` where e is recomputed from
/// the proof's commitment and announcement.
///
/// Returns `false` on any invalid input (bad points, failed equation).
/// Never panics.
pub fn verify_commitment_knowledge(proof: &CommitmentKnowledgeProof) -> bool {
    let gens = &*PEDERSEN_GENS;

    // Decompress C
    let c_compressed = CompressedRistretto::from_slice(&proof.commitment);
    let c_point = match c_compressed.decompress() {
        Some(p) => p,
        None => return false,
    };

    // Decompress R
    let r_compressed = CompressedRistretto::from_slice(&proof.announcement);
    let r_point = match r_compressed.decompress() {
        Some(p) => p,
        None => return false,
    };

    // Recompute challenge
    let e = compute_challenge(&c_compressed, &r_compressed);

    // Deserialize responses
    let s_v = Scalar::from_canonical_bytes(proof.response_v);
    let s_r = Scalar::from_canonical_bytes(proof.response_r);

    let s_v = match s_v {
        Some(s) => s,
        None => {
            // Fall back to mod-order interpretation (responses may exceed canonical range
            // after addition with challenge * witness).
            Scalar::from_bytes_mod_order(proof.response_v)
        }
    };
    let s_r = match s_r {
        Some(s) => s,
        None => {
            Scalar::from_bytes_mod_order(proof.response_r)
        }
    };

    // Verify: s_v*G + s_r*H == R + e*C
    let lhs: RistrettoPoint = s_v * gens.B + s_r * gens.B_blinding;
    let rhs: RistrettoPoint = r_point + e * c_point;

    lhs == rhs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_blinding() -> Scalar {
        let mut bytes = [0u8; 64];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
        Scalar::from_bytes_mod_order_wide(&bytes)
    }

    #[test]
    fn test_knowledge_proof_roundtrip() {
        let value = 42u64;
        let blinding = test_blinding();

        let proof = prove_commitment_knowledge(value, &blinding);
        assert!(verify_commitment_knowledge(&proof));
    }

    #[test]
    fn test_knowledge_proof_wrong_value() {
        let blinding = test_blinding();

        // Prove with value = 100
        let mut proof = prove_commitment_knowledge(100, &blinding);

        // Tamper: replace commitment with one for value = 999
        let gens = &*PEDERSEN_GENS;
        let v_bad = value_to_scalar(999);
        let bad_commit = (v_bad * gens.B + blinding * gens.B_blinding).compress();
        proof.commitment = *bad_commit.as_bytes();

        assert!(!verify_commitment_knowledge(&proof));
    }

    #[test]
    fn test_knowledge_proof_wrong_blinding() {
        let blinding_a = test_blinding();
        let blinding_b = test_blinding();
        let value = 50u64;

        // Prove with blinding_a
        let mut proof = prove_commitment_knowledge(value, &blinding_a);

        // Tamper: replace commitment with one using blinding_b
        let gens = &*PEDERSEN_GENS;
        let v = value_to_scalar(value);
        let bad_commit = (v * gens.B + blinding_b * gens.B_blinding).compress();
        proof.commitment = *bad_commit.as_bytes();

        assert!(!verify_commitment_knowledge(&proof));
    }

    #[test]
    fn test_knowledge_proof_deterministic_challenge() {
        let value = 77u64;
        let blinding = test_blinding();
        let gens = &*PEDERSEN_GENS;

        let v = value_to_scalar(value);
        let c = (v * gens.B + blinding * gens.B_blinding).compress();

        // Use a fixed announcement point for determinism
        let k = Scalar::from(123u64);
        let r = (k * gens.B + k * gens.B_blinding).compress();

        let e1 = compute_challenge(&c, &r);
        let e2 = compute_challenge(&c, &r);

        assert_eq!(e1, e2, "Same inputs must produce the same challenge");
    }

    #[test]
    fn test_knowledge_proof_invalid_point() {
        let blinding = test_blinding();
        let mut proof = prove_commitment_knowledge(10, &blinding);

        // Write garbage bytes into the announcement field
        proof.announcement = [0xFFu8; 32];

        assert!(!verify_commitment_knowledge(&proof));
    }
}
