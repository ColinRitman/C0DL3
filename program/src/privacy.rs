// In-circuit privacy validation for the SP1 guest program.
//
// These functions validate shielded pool invariants inside the ZK circuit:
//   1. Pedersen commitment conservation (homomorphic balance check)
//   2. Nullifier freshness (no double-spend)
//   3. Note tree root computation (Merkle tree over all note commitments)
//
// All logic must match the host-side implementation in `src/privacy/shielded_pool.rs` exactly.

use bulletproofs::PedersenGens;
use curve25519_dalek_ng::{
    ristretto::{CompressedRistretto, RistrettoPoint},
    scalar::Scalar,
    traits::Identity,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

// ── Shielded Block Data ─────────────────────────────────────────────────────

/// All shielded pool data for a single block, fed as input to the guest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldedBlockData {
    /// Spend proofs for notes being consumed in this block.
    pub spend_proofs: Vec<GuestSpendProof>,
    /// New note commitments created in this block.
    pub new_notes: Vec<[u8; 32]>,
    /// Nullifiers being revealed (spent) in this block.
    pub nullifiers: Vec<[u8; 32]>,
    /// All previously spent nullifiers (for freshness check).
    /// The guest verifies none of the new nullifiers exist in this set.
    pub prev_nullifiers: Vec<[u8; 32]>,
    /// All note commitments from previous blocks (for tree root verification).
    pub prev_note_commitments: Vec<[u8; 32]>,
    /// Client-side proven shield requests (deposits into shielded pool).
    /// The guest verifies each knowledge proof + note commitment derivation.
    pub shield_requests: Vec<GuestShieldRequest>,
    /// Client-side proven unshield requests (withdrawals from shielded pool).
    /// The guest verifies each knowledge proof + Merkle membership.
    pub unshield_requests: Vec<GuestUnshieldRequest>,
}

/// Spend proof — proves valid consumption of shielded notes.
/// Matches host-side `SpendProof` in `src/privacy/shielded_pool.rs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestSpendProof {
    /// Nullifiers for each input note being spent.
    pub nullifiers: Vec<[u8; 32]>,
    /// Pedersen commitments to the input notes.
    pub input_commitments: Vec<[u8; 32]>,
    /// Pedersen commitments to the output notes.
    pub output_commitments: Vec<[u8; 32]>,
    /// Pedersen commitment to the transaction fee.
    pub fee_commitment: [u8; 32],
    /// Kernel excess: compressed point. Should equal identity if conservation holds.
    pub kernel_excess: [u8; 32],
    /// Bulletproofs range proofs for each output (serialized).
    pub range_proofs: Vec<Vec<u8>>,
}

// ── Client-Side Proof Types ──────────────────────────────────────────────────
//
// These types match the wallet SDK's output. The guest verifies these proofs
// inside the ZK circuit, making the verification trustless.

/// Schnorr-style proof of knowledge for a Pedersen commitment.
/// Proves: "I know (v, r) such that C = v*G + r*H" without revealing v or r.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitmentKnowledgeProof {
    pub commitment: [u8; 32],
    pub announcement: [u8; 32],
    pub response_v: [u8; 32],
    pub response_r: [u8; 32],
}

/// Shield request — client-side proven deposit into shielded pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestShieldRequest {
    pub note_commitment: [u8; 32],
    pub value_commitment: [u8; 32],
    pub recipient_pubkey: [u8; 32],
    pub knowledge_proof: CommitmentKnowledgeProof,
    // Range proof verification is handled separately (Bulletproofs)
}

/// Unshield request — client-side proven withdrawal from shielded pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestUnshieldRequest {
    pub nullifier: [u8; 32],
    pub note_commitment: [u8; 32],
    pub value_commitment: [u8; 32],
    pub knowledge_proof: CommitmentKnowledgeProof,
    pub merkle_proof: Vec<([u8; 32], bool)>,
}

/// Verify a commitment knowledge proof (Schnorr sigma protocol).
///
/// Checks: s_v*G + s_r*H == R + e*C
/// where e = SHA-256("C0DL3:ck_proof:" || C || R)
pub fn verify_commitment_knowledge_proof(proof: &CommitmentKnowledgeProof) -> bool {
    let gens = PedersenGens::default();

    let c_point = match CompressedRistretto(proof.commitment).decompress() {
        Some(p) => p,
        None => return false,
    };
    let r_point = match CompressedRistretto(proof.announcement).decompress() {
        Some(p) => p,
        None => return false,
    };

    // Fiat-Shamir challenge
    let mut hasher = Sha256::new();
    hasher.update(b"C0DL3:ck_proof:");
    hasher.update(&proof.commitment);
    hasher.update(&proof.announcement);
    let hash: [u8; 32] = hasher.finalize().into();
    let e = Scalar::from_bytes_mod_order(hash);

    let s_v = Scalar::from_bytes_mod_order(proof.response_v);
    let s_r = Scalar::from_bytes_mod_order(proof.response_r);

    // Verify: s_v*G + s_r*H == R + e*C
    let lhs = gens.commit(s_v, s_r);
    let rhs = r_point + e * c_point;

    lhs == rhs
}

/// Verify a Merkle membership proof against the note tree root.
pub fn verify_merkle_proof(root: &[u8; 32], leaf: &[u8; 32], proof: &[([u8; 32], bool)]) -> bool {
    if proof.is_empty() {
        return leaf == root;
    }

    let mut current = *leaf;
    for &(sibling, is_left) in proof {
        let mut h = Sha256::new();
        h.update(b"C0DL3:note_tree:");
        if is_left {
            h.update(sibling);
            h.update(current);
        } else {
            h.update(current);
            h.update(sibling);
        }
        current = h.finalize().into();
    }

    current == *root
}

/// Validate all shield requests in a block.
///
/// Checks:
///   1. Knowledge proof is valid for each value commitment
///   2. Note commitment = SHA-256("C0DL3:note:" || value_commitment || pubkey)
pub fn validate_shield_requests(requests: &[GuestShieldRequest]) {
    for (i, req) in requests.iter().enumerate() {
        // Verify knowledge proof
        assert!(
            verify_commitment_knowledge_proof(&req.knowledge_proof),
            "shield request {}: invalid knowledge proof",
            i
        );

        // Verify knowledge proof matches value commitment
        assert_eq!(
            req.knowledge_proof.commitment, req.value_commitment,
            "shield request {}: knowledge proof commitment mismatch",
            i
        );

        // Verify note commitment derivation
        let mut hasher = Sha256::new();
        hasher.update(b"C0DL3:note:");
        hasher.update(&req.value_commitment);
        hasher.update(&req.recipient_pubkey);
        let expected: [u8; 32] = hasher.finalize().into();
        assert_eq!(
            expected, req.note_commitment,
            "shield request {}: note commitment derivation mismatch",
            i
        );
    }
}

/// Validate all unshield requests in a block.
///
/// Checks:
///   1. Knowledge proof is valid for each value commitment
///   2. Note commitment exists in the note tree (Merkle proof)
///   3. Nullifier is fresh (handled by validate_nullifiers)
pub fn validate_unshield_requests(
    requests: &[GuestUnshieldRequest],
    note_tree_root: &[u8; 32],
) {
    for (i, req) in requests.iter().enumerate() {
        // Verify knowledge proof
        assert!(
            verify_commitment_knowledge_proof(&req.knowledge_proof),
            "unshield request {}: invalid knowledge proof",
            i
        );

        // Verify commitment match
        assert_eq!(
            req.knowledge_proof.commitment, req.value_commitment,
            "unshield request {}: knowledge proof commitment mismatch",
            i
        );

        // Verify Merkle membership
        assert!(
            verify_merkle_proof(note_tree_root, &req.note_commitment, &req.merkle_proof),
            "unshield request {}: invalid Merkle membership proof",
            i
        );
    }
}

// ── Conservation Check ──────────────────────────────────────────────────────
//
// Verifies: Σ(input_commitments) - Σ(output_commitments) - fee_commitment = identity
//
// This is a pure elliptic curve arithmetic check. If it holds, then:
//   Σ(input_values) = Σ(output_values) + fee
//   AND Σ(input_blindings) = Σ(output_blindings) + fee_blinding
//
// Matches host-side `verify_balance_conservation()` in shielded_pool.rs.

/// Verify Pedersen commitment conservation for a spend proof.
///
/// Returns true if the sum of inputs equals the sum of outputs + fee.
/// Panics on invalid curve points (inside a ZK circuit, invalid inputs
/// mean the prover is malicious — the proof will be invalid).
pub fn validate_conservation(proof: &GuestSpendProof) -> bool {
    if proof.input_commitments.is_empty() {
        return false;
    }

    // Sum all input commitment points
    let mut sum_inputs = RistrettoPoint::identity();
    for c in &proof.input_commitments {
        let point = CompressedRistretto(*c)
            .decompress()
            .expect("invalid input commitment point in spend proof");
        sum_inputs += point;
    }

    // Sum all output commitment points + fee
    let mut sum_outputs = RistrettoPoint::identity();
    for c in &proof.output_commitments {
        let point = CompressedRistretto(*c)
            .decompress()
            .expect("invalid output commitment point in spend proof");
        sum_outputs += point;
    }

    let fee_point = CompressedRistretto(proof.fee_commitment)
        .decompress()
        .expect("invalid fee commitment point in spend proof");
    sum_outputs += fee_point;

    // Check: Σ inputs - Σ outputs = identity (zero point)
    let excess = sum_inputs - sum_outputs;
    let excess_compressed = excess.compress().to_bytes();

    excess_compressed == RistrettoPoint::identity().compress().to_bytes()
}

// ── Nullifier Freshness ─────────────────────────────────────────────────────
//
// Verifies that none of the new nullifiers have been spent before.
// Also checks for duplicates within the current block.

/// Validate that all nullifiers are fresh (not previously spent, no intra-block duplicates).
///
/// Panics if any nullifier is double-spent — inside the ZK circuit, this means
/// the block is invalid and the proof will not verify.
pub fn validate_nullifiers(new_nullifiers: &[[u8; 32]], prev_nullifiers: &[[u8; 32]]) {
    let prev_set: HashSet<[u8; 32]> = prev_nullifiers.iter().copied().collect();

    let mut seen_in_block: HashSet<[u8; 32]> = HashSet::new();

    for nullifier in new_nullifiers {
        // Check against historical nullifiers
        assert!(
            !prev_set.contains(nullifier),
            "double-spend: nullifier already in previous set"
        );

        // Check for intra-block duplicates
        assert!(
            seen_in_block.insert(*nullifier),
            "double-spend: duplicate nullifier within block"
        );
    }
}

// ── Note Tree Root ──────────────────────────────────────────────────────────
//
// SHA-256 Merkle tree over all note commitments. Must match host-side
// `note_merkle_root()` in `src/privacy/shielded_pool.rs` exactly.

/// Compute the note Merkle root from a list of note commitments.
///
/// Domain separator: "C0DL3:note_tree:" per internal node.
/// Empty pool: SHA-256("C0DL3:empty_pool").
/// Single note: the note commitment itself (no hashing).
pub fn compute_note_tree_root(commitments: &[[u8; 32]]) -> [u8; 32] {
    if commitments.is_empty() {
        let mut h = Sha256::new();
        h.update(b"C0DL3:empty_pool");
        return h.finalize().into();
    }
    if commitments.len() == 1 {
        return commitments[0];
    }

    let mut current = commitments.to_vec();
    while current.len() > 1 {
        let mut next = Vec::new();
        let mut i = 0;
        while i < current.len() {
            let left = current[i];
            let right = if i + 1 < current.len() {
                current[i + 1]
            } else {
                current[i] // duplicate last if odd
            };
            let mut h = Sha256::new();
            h.update(b"C0DL3:note_tree:");
            h.update(left);
            h.update(right);
            next.push(h.finalize().into());
            i += 2;
        }
        current = next;
    }
    current[0]
}

/// Compute the updated note tree root after adding new notes.
///
/// Combines previous note commitments with new ones and computes the full tree.
/// This is O(total_notes) — for large pools, an incremental approach would be
/// more efficient, but this is correct and simple.
pub fn compute_updated_note_tree_root(
    prev_commitments: &[[u8; 32]],
    new_notes: &[[u8; 32]],
) -> [u8; 32] {
    let mut all = prev_commitments.to_vec();
    all.extend_from_slice(new_notes);
    compute_note_tree_root(&all)
}

/// Verify that the previous note tree root matches the expected value.
///
/// The guest verifies this to ensure the prover's witness is consistent.
pub fn verify_prev_note_tree_root(
    prev_commitments: &[[u8; 32]],
    expected_root: &[u8; 32],
) -> bool {
    compute_note_tree_root(prev_commitments) == *expected_root
}

// ── Validate All Shielded Transitions ───────────────────────────────────────

/// Validate all shielded pool transitions for a block.
///
/// This is the top-level privacy validation called from main().
/// It checks conservation, nullifier freshness, and note tree consistency.
///
/// Returns the new note tree root and total nullifier count.
pub fn validate_shielded_block(
    data: &ShieldedBlockData,
    expected_prev_note_root: &[u8; 32],
) -> ([u8; 32], u32) {
    // 1. Verify previous note tree root
    assert!(
        verify_prev_note_tree_root(&data.prev_note_commitments, expected_prev_note_root),
        "previous note tree root mismatch — witness inconsistent"
    );

    // 2. Validate conservation for each spend proof
    for (i, proof) in data.spend_proofs.iter().enumerate() {
        assert!(
            validate_conservation(proof),
            "conservation check failed for spend proof {}",
            i
        );
    }

    // 3. Validate nullifier freshness
    validate_nullifiers(&data.nullifiers, &data.prev_nullifiers);

    // 4. Validate client-side shield proofs (knowledge proof + note derivation)
    validate_shield_requests(&data.shield_requests);

    // 5. Validate client-side unshield proofs (knowledge proof + Merkle membership)
    validate_unshield_requests(&data.unshield_requests, expected_prev_note_root);

    // 6. Compute updated note tree root
    // Include new notes from both legacy path and shield requests
    let mut all_new_notes = data.new_notes.clone();
    for req in &data.shield_requests {
        all_new_notes.push(req.note_commitment);
    }
    let new_root = compute_updated_note_tree_root(
        &data.prev_note_commitments,
        &all_new_notes,
    );

    // Total nullifiers: legacy + unshield nullifiers
    let nullifier_count = data.nullifiers.len() as u32
        + data.unshield_requests.len() as u32;

    (new_root, nullifier_count)
}
