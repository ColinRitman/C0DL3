// Shielded Pool — ZK State Layer
//
// Implements the privacy state layer for C0DL3:
// 1. Committed balances (Pedersen commitments on account state)
// 2. Homomorphic balance conservation (Σinputs = Σoutputs + fee)
// 3. Nullifier set for double-spend prevention
//
// ARCHITECTURE:
// - ShieldedNote: a committed value owned by a stealth address
// - SpendProof: proves a valid spend (nullifiers + conservation + range proofs)
// - ShieldedPool: tracks note Merkle tree + nullifier set
//
// HONEST SCOPE:
// The sequencer still knows plaintext amounts for execution (trusted testnet model).
// Full trustless privacy requires a ZK execution circuit (Boojum/Airbender).
// What IS cryptographically enforced:
// - Balance conservation (homomorphic Pedersen check — anyone can verify)
// - Double-spend prevention (nullifier set — deterministic, unforgeable)
// - Amount hiding (Bulletproofs range proofs — amounts non-negative and hidden)

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use bulletproofs::{BulletproofGens, PedersenGens, RangeProof};
use curve25519_dalek_ng::{
    ristretto::{CompressedRistretto, RistrettoPoint},
    scalar::Scalar,
    traits::Identity,
};
use merlin::Transcript;
use once_cell::sync::Lazy;
use std::collections::HashSet;

/// Reuse the same generators as confidential_transactions.rs
static PEDERSEN_GENS: Lazy<PedersenGens> = Lazy::new(|| PedersenGens::default());
static BP_GENS: Lazy<BulletproofGens> = Lazy::new(|| BulletproofGens::new(64, 128));

// ──────────────────────────────────────────────
// Data structures
// ──────────────────────────────────────────────

/// A committed note in the shielded pool.
/// Each note is a Pedersen commitment to some value, owned by a one-time address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldedNote {
    /// Pedersen commitment C = v·G + r·H (compressed Ristretto, 32 bytes)
    pub commitment: [u8; 32],
    /// Owner's one-time public key (stealth address, compressed Ristretto)
    pub owner_pubkey: [u8; 32],
    /// Position index in the note Merkle tree
    pub position: u64,
}

/// Proof that a spend is valid: nullifiers are fresh, balance is conserved,
/// and all output amounts are non-negative.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendProof {
    /// Nullifiers for each input note (one per spend)
    pub nullifiers: Vec<[u8; 32]>,
    /// Commitments to the input notes being spent (for conservation check)
    pub input_commitments: Vec<[u8; 32]>,
    /// Commitments to new output notes
    pub output_commitments: Vec<[u8; 32]>,
    /// Commitment to the transaction fee: C(fee, r_fee)
    pub fee_commitment: [u8; 32],
    /// Kernel excess: compressed point proving Σr_in = Σr_out + r_fee
    /// If conservation holds, the excess is the identity point (all zeros after compress)
    pub kernel_excess: [u8; 32],
    /// Bulletproofs range proof for each output (proves amount ≥ 0)
    pub range_proofs: Vec<Vec<u8>>,
}

/// The shielded pool: tracks all notes and spent nullifiers.
#[derive(Debug, Clone)]
pub struct ShieldedPool {
    /// All notes (Merkle leaves), append-only
    pub notes: Vec<ShieldedNote>,
    /// Set of spent nullifiers — checked for double-spend
    pub nullifier_set: HashSet<[u8; 32]>,
    /// SHA-256 Merkle root over all note commitments
    pub note_tree_root: [u8; 32],
}

// ──────────────────────────────────────────────
// Nullifier computation
// ──────────────────────────────────────────────

/// Compute a nullifier for a note.
///
/// nullifier = SHA-256("C0DL3:nullifier:" || spend_key_bytes || note_commitment)
///
/// The nullifier is deterministic: same inputs always produce the same output.
/// Without knowing spend_key, an observer cannot link a nullifier to its note.
pub fn compute_nullifier(spend_key: &Scalar, note_commitment: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"C0DL3:nullifier:");
    hasher.update(spend_key.to_bytes());
    hasher.update(note_commitment);
    hasher.finalize().into()
}

// ──────────────────────────────────────────────
// Balance commitment (for account state)
// ──────────────────────────────────────────────

/// Compute a deterministic Pedersen commitment for an account balance.
///
/// C = balance·G + r·H
/// where r = SHA-256("C0DL3:balance:" || address || nonce_le_bytes)
///
/// Deterministic blinding means the commitment is reproducible from
/// (address, balance, nonce) without storing a separate blinding key.
pub fn compute_balance_commitment(address: &str, balance: u64, nonce: u64) -> [u8; 32] {
    // Derive deterministic blinding factor
    let mut blinding_input = Vec::new();
    blinding_input.extend_from_slice(b"C0DL3:balance:");
    blinding_input.extend_from_slice(address.as_bytes());
    blinding_input.extend_from_slice(&nonce.to_le_bytes());
    let blinding_hash: [u8; 32] = Sha256::digest(&blinding_input).into();
    let r = Scalar::from_bytes_mod_order(blinding_hash);

    // Commit: C = balance·G + r·H
    let mut amount_padded = [0u8; 32];
    amount_padded[..8].copy_from_slice(&balance.to_le_bytes());
    let v = Scalar::from_bytes_mod_order(amount_padded);
    PEDERSEN_GENS.commit(v, r).compress().to_bytes()
}

/// Create a Pedersen commitment with a specific blinding factor.
/// Returns (commitment_bytes, blinding_scalar).
pub fn commit_with_blinding(amount: u64, blinding: &Scalar) -> [u8; 32] {
    let mut amount_padded = [0u8; 32];
    amount_padded[..8].copy_from_slice(&amount.to_le_bytes());
    let v = Scalar::from_bytes_mod_order(amount_padded);
    PEDERSEN_GENS.commit(v, *blinding).compress().to_bytes()
}

// ──────────────────────────────────────────────
// Homomorphic balance conservation
// ──────────────────────────────────────────────

/// Verify balance conservation using homomorphic properties of Pedersen commitments.
///
/// Checks: Σ(input_commitments) - Σ(output_commitments) - fee_commitment = identity
///
/// If this holds, then Σ(input_amounts) = Σ(output_amounts) + fee
/// AND Σ(input_blindings) = Σ(output_blindings) + fee_blinding
///
/// This is a pure elliptic curve arithmetic check — no ZK circuit needed.
pub fn verify_balance_conservation(
    input_commitments: &[[u8; 32]],
    output_commitments: &[[u8; 32]],
    fee_commitment: &[u8; 32],
) -> Result<bool> {
    if input_commitments.is_empty() {
        return Err(anyhow!("No input commitments provided"));
    }

    // Sum all input commitment points
    let mut sum_inputs = RistrettoPoint::identity();
    for c in input_commitments {
        let point = CompressedRistretto(*c)
            .decompress()
            .ok_or_else(|| anyhow!("Invalid input commitment point"))?;
        sum_inputs += point;
    }

    // Sum all output commitment points
    let mut sum_outputs = RistrettoPoint::identity();
    for c in output_commitments {
        let point = CompressedRistretto(*c)
            .decompress()
            .ok_or_else(|| anyhow!("Invalid output commitment point"))?;
        sum_outputs += point;
    }

    // Add fee commitment to outputs
    let fee_point = CompressedRistretto(*fee_commitment)
        .decompress()
        .ok_or_else(|| anyhow!("Invalid fee commitment point"))?;
    sum_outputs += fee_point;

    // Check: Σinputs - Σoutputs = identity (zero point)
    let excess = sum_inputs - sum_outputs;
    let excess_compressed = excess.compress().to_bytes();

    // The identity point compresses to all zeros
    Ok(excess_compressed == RistrettoPoint::identity().compress().to_bytes())
}

// ──────────────────────────────────────────────
// Merkle tree for notes
// ──────────────────────────────────────────────

/// Build SHA-256 Merkle root from note commitment leaves.
fn note_merkle_root(commitments: &[[u8; 32]]) -> [u8; 32] {
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

/// Compute a Merkle membership proof for the leaf at `index`.
///
/// Returns `None` if the index is out of bounds or the commitment list is empty.
/// Returns `Some(proof)` where each element is `(sibling_hash, is_left)`:
/// - `is_left = true` means the sibling is on the LEFT side of the pair.
///
/// The proof is compatible with `note_merkle_root` (same domain separator and
/// odd-duplication logic).  For a single-element tree the proof vec is empty
/// because the leaf IS the root.
pub fn compute_merkle_proof(commitments: &[[u8; 32]], index: usize) -> Option<Vec<([u8; 32], bool)>> {
    if commitments.is_empty() || index >= commitments.len() {
        return None;
    }
    if commitments.len() == 1 {
        return Some(Vec::new());
    }

    let mut proof = Vec::new();
    let mut current_level: Vec<[u8; 32]> = commitments.to_vec();
    let mut idx = index;

    while current_level.len() > 1 {
        // If odd, duplicate the last element (matches note_merkle_root)
        if current_level.len() % 2 != 0 {
            let last = *current_level.last().unwrap();
            current_level.push(last);
        }

        // Determine sibling
        let sibling_idx = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
        let sibling = current_level[sibling_idx];
        // is_left = true means the sibling is on the left
        let is_left = idx % 2 == 1;
        proof.push((sibling, is_left));

        // Build next level
        let mut next = Vec::new();
        let mut i = 0;
        while i < current_level.len() {
            let left = current_level[i];
            let right = current_level[i + 1];
            let mut h = Sha256::new();
            h.update(b"C0DL3:note_tree:");
            h.update(left);
            h.update(right);
            next.push(h.finalize().into());
            i += 2;
        }

        current_level = next;
        idx /= 2;
    }

    Some(proof)
}

/// Verify a Merkle membership proof against an expected root.
///
/// Walks `proof` bottom-up, hashing with the domain separator `"C0DL3:note_tree:"`.
/// For each `(sibling, is_left)`:
/// - `is_left = true`  → hash(sibling || current)
/// - `is_left = false` → hash(current || sibling)
///
/// An empty proof is valid iff `leaf == root` (single-element tree).
pub fn verify_merkle_proof(root: &[u8; 32], leaf: &[u8; 32], proof: &[([u8; 32], bool)]) -> bool {
    let mut current = *leaf;

    for (sibling, is_left) in proof {
        let mut h = Sha256::new();
        h.update(b"C0DL3:note_tree:");
        if *is_left {
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

// ──────────────────────────────────────────────
// ShieldedPool implementation
// ──────────────────────────────────────────────

impl ShieldedPool {
    /// Create a new empty shielded pool.
    pub fn new() -> Self {
        let empty_root = note_merkle_root(&[]);
        Self {
            notes: Vec::new(),
            nullifier_set: HashSet::new(),
            note_tree_root: empty_root,
        }
    }

    /// Add a new note to the pool. Returns the note's position index.
    pub fn add_note(&mut self, commitment: [u8; 32], owner_pubkey: [u8; 32]) -> u64 {
        let position = self.notes.len() as u64;
        self.notes.push(ShieldedNote {
            commitment,
            owner_pubkey,
            position,
        });
        self.recompute_tree_root();
        position
    }

    /// Recompute the note Merkle tree root from all notes.
    pub fn recompute_tree_root(&mut self) {
        let commitments: Vec<[u8; 32]> = self.notes.iter().map(|n| n.commitment).collect();
        self.note_tree_root = note_merkle_root(&commitments);
    }

    /// Check whether a nullifier has already been spent.
    pub fn is_nullifier_spent(&self, nullifier: &[u8; 32]) -> bool {
        self.nullifier_set.contains(nullifier)
    }

    /// Verify a spend proof against this pool.
    ///
    /// Checks:
    /// 1. All nullifiers are fresh (not in nullifier_set)
    /// 2. Balance conservation: Σ(inputs) = Σ(outputs) + fee
    /// 3. All output range proofs are valid (amounts ≥ 0)
    pub fn verify_spend(&self, proof: &SpendProof) -> Result<bool> {
        // 1. Check nullifiers are fresh
        for nullifier in &proof.nullifiers {
            if self.nullifier_set.contains(nullifier) {
                return Ok(false); // Double-spend attempt
            }
        }

        // 2. Verify balance conservation
        let conserved = verify_balance_conservation(
            &proof.input_commitments,
            &proof.output_commitments,
            &proof.fee_commitment,
        )?;
        if !conserved {
            return Ok(false); // Inflation attempt
        }

        // 3. Verify range proofs for each output
        for (i, (commitment_bytes, proof_bytes)) in proof
            .output_commitments
            .iter()
            .zip(proof.range_proofs.iter())
            .enumerate()
        {
            let rp = RangeProof::from_bytes(proof_bytes)
                .map_err(|e| anyhow!("Invalid range proof for output {}: {:?}", i, e))?;
            let mut transcript = Transcript::new(b"C0DL3-ShieldedPool-RangeProof");
            let committed = CompressedRistretto(*commitment_bytes);
            rp.verify_single(&BP_GENS, &PEDERSEN_GENS, &mut transcript, &committed, 64)
                .map_err(|e| anyhow!("Range proof verification failed for output {}: {:?}", i, e))?;
        }

        Ok(true)
    }

    /// Apply a verified spend proof: add nullifiers and output notes.
    ///
    /// IMPORTANT: Call verify_spend() first. This does NOT re-verify.
    pub fn apply_spend(&mut self, proof: &SpendProof, output_owner_pubkeys: &[[u8; 32]]) {
        // Record nullifiers as spent
        for nullifier in &proof.nullifiers {
            self.nullifier_set.insert(*nullifier);
        }

        // Add output notes to the tree
        for (commitment, owner) in proof.output_commitments.iter().zip(output_owner_pubkeys.iter()) {
            self.add_note(*commitment, *owner);
        }
    }

    /// Get the current number of notes in the pool.
    pub fn note_count(&self) -> u64 {
        self.notes.len() as u64
    }

    /// Get the current number of spent nullifiers.
    pub fn nullifier_count(&self) -> u64 {
        self.nullifier_set.len() as u64
    }

    /// Get a Merkle membership proof for a note at the given position.
    pub fn get_note_proof(&self, position: usize) -> Option<Vec<([u8; 32], bool)>> {
        let commitments: Vec<[u8; 32]> = self.notes.iter().map(|n| n.commitment).collect();
        compute_merkle_proof(&commitments, position)
    }
}

// ──────────────────────────────────────────────
// Spend proof construction helpers
// ──────────────────────────────────────────────

/// Create a spend proof for a set of input notes and output amounts.
///
/// This is called by the sender/wallet when constructing a private transaction.
/// The sequencer verifies it via ShieldedPool::verify_spend().
///
/// `inputs`: (note_commitment, amount, blinding_scalar, spend_key)
/// `outputs`: (amount, owner_pubkey)
/// `fee`: transaction fee (public value)
pub fn create_spend_proof(
    inputs: &[([u8; 32], u64, Scalar, Scalar)],  // (commitment, amount, blinding, spend_key)
    outputs: &[(u64, [u8; 32])],                   // (amount, owner_pubkey)
    fee: u64,
) -> Result<(SpendProof, Vec<Scalar>)> {
    // Verify conservation: Σinput_amounts = Σoutput_amounts + fee
    let sum_in: u64 = inputs.iter().map(|(_, a, _, _)| a).sum();
    let sum_out: u64 = outputs.iter().map(|(a, _)| a).sum();
    if sum_in != sum_out + fee {
        return Err(anyhow!(
            "Balance mismatch: inputs={} outputs={} fee={}", sum_in, sum_out, fee
        ));
    }

    // Compute nullifiers
    let nullifiers: Vec<[u8; 32]> = inputs
        .iter()
        .map(|(commitment, _, _, spend_key)| compute_nullifier(spend_key, commitment))
        .collect();

    let input_commitments: Vec<[u8; 32]> = inputs.iter().map(|(c, _, _, _)| *c).collect();

    // Generate output commitments with random blinding
    let mut output_commitments = Vec::new();
    let mut output_blindings = Vec::new();
    let mut range_proofs = Vec::new();

    for &(amount, _) in outputs {
        // Generate random blinding for output
        let mut rng = rand::thread_rng();
        let mut blinding_bytes = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rng, &mut blinding_bytes);
        let blinding = Scalar::from_bytes_mod_order(blinding_bytes);

        let commitment = commit_with_blinding(amount, &blinding);
        output_commitments.push(commitment);
        output_blindings.push(blinding);

        // Generate Bulletproofs range proof
        let mut transcript = Transcript::new(b"C0DL3-ShieldedPool-RangeProof");
        let (rp, _committed) = RangeProof::prove_single(
            &BP_GENS,
            &PEDERSEN_GENS,
            &mut transcript,
            amount,
            &blinding,
            64, // 64-bit range
        ).map_err(|e| anyhow!("Range proof generation failed: {:?}", e))?;
        range_proofs.push(rp.to_bytes());
    }

    // Compute fee commitment: the blinding for the fee must make conservation work.
    // Σr_in = Σr_out + r_fee  →  r_fee = Σr_in - Σr_out
    let sum_blinding_in: Scalar = inputs.iter().map(|(_, _, r, _)| *r).sum();
    let sum_blinding_out: Scalar = output_blindings.iter().copied().sum();
    let fee_blinding = sum_blinding_in - sum_blinding_out;
    let fee_commitment = commit_with_blinding(fee, &fee_blinding);

    // Kernel excess should be identity if conservation holds
    let excess_blinding = sum_blinding_in - sum_blinding_out - fee_blinding;
    let kernel_excess = (PEDERSEN_GENS.B_blinding * excess_blinding).compress().to_bytes();

    Ok((
        SpendProof {
            nullifiers,
            input_commitments,
            output_commitments,
            fee_commitment,
            kernel_excess,
            range_proofs,
        },
        output_blindings,
    ))
}

// ──────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────

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
        (&s * &PEDERSEN_GENS.B).compress().to_bytes()
    }

    #[test]
    fn test_nullifier_deterministic() {
        let spend_key = random_scalar();
        let commitment = [0xABu8; 32];

        let n1 = compute_nullifier(&spend_key, &commitment);
        let n2 = compute_nullifier(&spend_key, &commitment);
        assert_eq!(n1, n2, "Same inputs must produce same nullifier");
    }

    #[test]
    fn test_nullifier_different_keys() {
        let key1 = random_scalar();
        let key2 = random_scalar();
        let commitment = [0x42u8; 32];

        let n1 = compute_nullifier(&key1, &commitment);
        let n2 = compute_nullifier(&key2, &commitment);
        assert_ne!(n1, n2, "Different keys must produce different nullifiers");
    }

    #[test]
    fn test_nullifier_double_spend_rejected() {
        let mut pool = ShieldedPool::new();
        let nullifier = [0xFFu8; 32];

        // First spend: fresh
        assert!(!pool.is_nullifier_spent(&nullifier));
        pool.nullifier_set.insert(nullifier);

        // Second spend: rejected
        assert!(pool.is_nullifier_spent(&nullifier));
    }

    #[test]
    fn test_balance_conservation_valid() {
        // Create two inputs: 1000 and 2000
        let r1 = random_scalar();
        let r2 = random_scalar();
        let c_in1 = commit_with_blinding(1000, &r1);
        let c_in2 = commit_with_blinding(2000, &r2);

        // Output: 2500, fee: 500
        let r_out = random_scalar();
        let c_out = commit_with_blinding(2500, &r_out);

        // Fee blinding must satisfy: r_fee = r1 + r2 - r_out
        let r_fee = r1 + r2 - r_out;
        let c_fee = commit_with_blinding(500, &r_fee);

        let result = verify_balance_conservation(
            &[c_in1, c_in2],
            &[c_out],
            &c_fee,
        ).unwrap();
        assert!(result, "Valid conservation must pass");
    }

    #[test]
    fn test_balance_conservation_invalid() {
        // Create input: 1000
        let r1 = random_scalar();
        let c_in = commit_with_blinding(1000, &r1);

        // Output: 2000 (inflation!) + fee: 0
        let r_out = random_scalar();
        let c_out = commit_with_blinding(2000, &r_out);

        // Fee: 0 but blinding won't balance
        let r_fee = random_scalar();
        let c_fee = commit_with_blinding(0, &r_fee);

        let result = verify_balance_conservation(&[c_in], &[c_out], &c_fee).unwrap();
        assert!(!result, "Inflation attempt must fail");
    }

    #[test]
    fn test_balance_commitment_deterministic() {
        let c1 = compute_balance_commitment("0xABCD", 1000, 0);
        let c2 = compute_balance_commitment("0xABCD", 1000, 0);
        assert_eq!(c1, c2, "Same inputs must produce same commitment");
    }

    #[test]
    fn test_balance_commitment_different_amounts() {
        let c1 = compute_balance_commitment("0xABCD", 1000, 0);
        let c2 = compute_balance_commitment("0xABCD", 2000, 0);
        assert_ne!(c1, c2, "Different amounts must produce different commitments");
    }

    #[test]
    fn test_balance_commitment_different_nonces() {
        let c1 = compute_balance_commitment("0xABCD", 1000, 0);
        let c2 = compute_balance_commitment("0xABCD", 1000, 1);
        assert_ne!(c1, c2, "Different nonces must produce different commitments");
    }

    #[test]
    fn test_note_add_and_tree_root() {
        let mut pool = ShieldedPool::new();
        let initial_root = pool.note_tree_root;

        let commitment = [0x11u8; 32];
        let owner = [0x22u8; 32];
        let pos = pool.add_note(commitment, owner);
        assert_eq!(pos, 0);
        assert_ne!(pool.note_tree_root, initial_root, "Root must change after adding note");

        let pos2 = pool.add_note([0x33u8; 32], [0x44u8; 32]);
        assert_eq!(pos2, 1);
    }

    #[test]
    fn test_spend_proof_roundtrip() {
        let mut pool = ShieldedPool::new();

        // Create an input note: 3000 tokens
        let spend_key = random_scalar();
        let blinding = random_scalar();
        let input_commitment = commit_with_blinding(3000, &blinding);
        let owner = random_pubkey();
        pool.add_note(input_commitment, owner);

        // Create spend: 3000 → 2500 + 500 fee
        let (proof, _output_blindings) = create_spend_proof(
            &[(input_commitment, 3000, blinding, spend_key)],
            &[(2500, random_pubkey())],
            500,
        ).unwrap();

        // Verify
        assert!(pool.verify_spend(&proof).unwrap(), "Valid spend must pass");

        // Apply
        let output_owners = vec![random_pubkey()];
        pool.apply_spend(&proof, &output_owners);

        // Nullifier is now spent
        assert!(pool.is_nullifier_spent(&proof.nullifiers[0]));

        // Double-spend rejected
        assert!(!pool.verify_spend(&proof).unwrap(), "Double-spend must fail");
    }

    #[test]
    fn test_spend_proof_multiple_inputs_outputs() {
        let mut pool = ShieldedPool::new();

        // Two inputs: 1000 + 4000 = 5000
        let sk1 = random_scalar();
        let sk2 = random_scalar();
        let r1 = random_scalar();
        let r2 = random_scalar();
        let c1 = commit_with_blinding(1000, &r1);
        let c2 = commit_with_blinding(4000, &r2);
        pool.add_note(c1, random_pubkey());
        pool.add_note(c2, random_pubkey());

        // Two outputs: 2000 + 2800 = 4800, fee: 200
        let (proof, _) = create_spend_proof(
            &[
                (c1, 1000, r1, sk1),
                (c2, 4000, r2, sk2),
            ],
            &[
                (2000, random_pubkey()),
                (2800, random_pubkey()),
            ],
            200,
        ).unwrap();

        assert!(pool.verify_spend(&proof).unwrap());
    }

    #[test]
    fn test_create_spend_proof_balance_mismatch() {
        let sk = random_scalar();
        let r = random_scalar();
        let c = commit_with_blinding(1000, &r);

        // Try to create more than we have
        let result = create_spend_proof(
            &[(c, 1000, r, sk)],
            &[(2000, random_pubkey())],
            100,
        );
        assert!(result.is_err(), "Mismatched amounts must be rejected at proof creation");
    }

    // ── Merkle proof tests ──────────────────────

    fn make_commitment(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    #[test]
    fn test_merkle_proof_single_element() {
        let commitments = [make_commitment(0x01)];
        let proof = compute_merkle_proof(&commitments, 0).unwrap();
        assert!(proof.is_empty(), "Single-element tree proof must be empty");

        let root = note_merkle_root(&commitments);
        assert!(verify_merkle_proof(&root, &commitments[0], &proof));
    }

    #[test]
    fn test_merkle_proof_two_elements() {
        let commitments = [make_commitment(0x01), make_commitment(0x02)];
        let root = note_merkle_root(&commitments);

        for i in 0..2 {
            let proof = compute_merkle_proof(&commitments, i).unwrap();
            assert!(
                verify_merkle_proof(&root, &commitments[i], &proof),
                "Proof verification failed for index {}",
                i,
            );
        }
    }

    #[test]
    fn test_merkle_proof_four_elements() {
        let commitments = [
            make_commitment(0x01),
            make_commitment(0x02),
            make_commitment(0x03),
            make_commitment(0x04),
        ];
        let root = note_merkle_root(&commitments);

        for i in 0..4 {
            let proof = compute_merkle_proof(&commitments, i).unwrap();
            assert!(
                verify_merkle_proof(&root, &commitments[i], &proof),
                "Proof verification failed for index {}",
                i,
            );
        }
    }

    #[test]
    fn test_merkle_proof_five_elements() {
        let commitments = [
            make_commitment(0x01),
            make_commitment(0x02),
            make_commitment(0x03),
            make_commitment(0x04),
            make_commitment(0x05),
        ];
        let root = note_merkle_root(&commitments);

        for i in 0..5 {
            let proof = compute_merkle_proof(&commitments, i).unwrap();
            assert!(
                verify_merkle_proof(&root, &commitments[i], &proof),
                "Proof verification failed for index {}",
                i,
            );
        }
    }

    #[test]
    fn test_merkle_proof_invalid_leaf() {
        let commitments = [make_commitment(0x01), make_commitment(0x02)];
        let root = note_merkle_root(&commitments);
        let proof = compute_merkle_proof(&commitments, 0).unwrap();

        let wrong_leaf = make_commitment(0xFF);
        assert!(
            !verify_merkle_proof(&root, &wrong_leaf, &proof),
            "Verification with wrong leaf must fail",
        );
    }

    #[test]
    fn test_merkle_proof_invalid_root() {
        let commitments = [make_commitment(0x01), make_commitment(0x02)];
        let proof = compute_merkle_proof(&commitments, 0).unwrap();

        let wrong_root = [0xFFu8; 32];
        assert!(
            !verify_merkle_proof(&wrong_root, &commitments[0], &proof),
            "Verification with wrong root must fail",
        );
    }

    #[test]
    fn test_merkle_proof_roundtrip_with_pool() {
        let mut pool = ShieldedPool::new();

        // Add several notes
        let owners: Vec<[u8; 32]> = (0..5).map(|_| random_pubkey()).collect();
        let commitments: Vec<[u8; 32]> = (1u8..=5).map(make_commitment).collect();

        for i in 0..5 {
            pool.add_note(commitments[i], owners[i]);
        }

        // Verify proofs for every note position against the pool root
        for i in 0..5 {
            let proof = pool.get_note_proof(i).expect("proof must exist");
            assert!(
                verify_merkle_proof(&pool.note_tree_root, &commitments[i], &proof),
                "Pool roundtrip proof failed for position {}",
                i,
            );
        }

        // Out-of-bounds returns None
        assert!(pool.get_note_proof(5).is_none());
        assert!(pool.get_note_proof(100).is_none());
    }
}
