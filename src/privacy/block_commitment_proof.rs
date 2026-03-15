// Block Commitment Proof — STARK-COMMITMENT-V1
//
// Proves that a block's commitment tree root is a valid SHA-256 Merkle root
// over all transaction commitment hashes in the block.
//
// SCOPE: This is a commitment integrity proof, NOT a full execution proof.
// It proves: given {tx_commitment_hashes}, the Merkle root is correct.
// It does NOT prove: EVM state transitions, balance updates, or tx validity.
//
// Full execution proofs (Boojum/Airbender) require dedicated GPU prover
// infrastructure and are planned for mainnet via zkSync Era integration.
//
// This proof is deterministic and publicly verifiable with only sha2.

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

/// Public inputs committed to by the proof
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlockProofPublicInputs {
    /// Block height
    pub block_height: u64,
    /// Number of transactions in the block
    pub tx_count: u64,
    /// Merkle root over all transaction commitment hashes (32 bytes, hex)
    pub commitment_tree_root: [u8; 32],
}

/// Block commitment proof — verifiable Merkle root over tx commitment hashes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockCommitmentProof {
    /// Proof type identifier
    pub proof_type: String,
    /// Block height this proof covers
    pub block_height: u64,
    /// Number of transactions
    pub tx_count: u64,
    /// Commitment tree root (SHA-256 Merkle root)
    pub commitment_tree_root: [u8; 32],
    /// All leaf hashes (one per transaction), in order
    /// Included so verifier can recompute root without trusting prover
    pub tx_commitment_hashes: Vec<[u8; 32]>,
    /// SHA-256 of (block_height || tx_count || commitment_tree_root)
    /// Binds the proof to its public inputs
    pub proof_binding: [u8; 32],
}

impl BlockCommitmentProof {
    /// Serialize to bytes for embedding in ZkProof.proof_data
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

/// Build a SHA-256 Merkle tree from leaf hashes and return the root.
/// For an empty list, returns the hash of an empty input.
/// For a single leaf, the root is the leaf itself.
fn merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    if leaves.is_empty() {
        let mut h = Sha256::new();
        h.update(b"empty_block");
        return h.finalize().into();
    }
    if leaves.len() == 1 {
        return leaves[0];
    }

    // Build tree level by level
    let mut current = leaves.to_vec();
    while current.len() > 1 {
        let mut next = Vec::new();
        let mut i = 0;
        while i < current.len() {
            let left = current[i];
            // Duplicate last node if odd count
            let right = if i + 1 < current.len() {
                current[i + 1]
            } else {
                current[i]
            };
            let mut h = Sha256::new();
            h.update(left);
            h.update(right);
            next.push(h.finalize().into());
            i += 2;
        }
        current = next;
    }
    current[0]
}

/// Hash a transaction commitment to a 32-byte leaf.
/// Input: the raw commitment bytes (e.g., a serialised Pedersen point).
/// Output: SHA-256(b"tx_commitment" || commitment_bytes)
fn hash_tx_commitment(commitment_bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"tx_commitment");
    h.update(commitment_bytes);
    h.finalize().into()
}

/// Compute the proof binding: SHA-256(height_le || tx_count_le || root)
fn compute_proof_binding(block_height: u64, tx_count: u64, root: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"COLDL3:commitment:v1");
    h.update(block_height.to_le_bytes());
    h.update(tx_count.to_le_bytes());
    h.update(root);
    h.finalize().into()
}

/// Generate a block commitment proof from a list of transaction commitment byte slices.
///
/// `tx_commitments`: one entry per transaction — the raw commitment bytes
///   (e.g., a compressed Pedersen point from AmountCommitment, or a tx hash
///    if no CT is used for that tx).
pub fn generate_block_commitment_proof(
    block_height: u64,
    tx_commitments: &[Vec<u8>],
) -> Result<BlockCommitmentProof> {
    let tx_count = tx_commitments.len() as u64;

    // Hash each commitment to a Merkle leaf
    let leaves: Vec<[u8; 32]> = tx_commitments
        .iter()
        .map(|c| hash_tx_commitment(c))
        .collect();

    // Build Merkle root
    let commitment_tree_root = merkle_root(&leaves);

    // Compute binding digest
    let proof_binding = compute_proof_binding(block_height, tx_count, &commitment_tree_root);

    Ok(BlockCommitmentProof {
        proof_type: "STARK-COMMITMENT-V1".to_string(),
        block_height,
        tx_count,
        commitment_tree_root,
        tx_commitment_hashes: leaves,
        proof_binding,
    })
}

/// Verify a block commitment proof.
///
/// Recomputes the Merkle root from the embedded leaf hashes, checks it matches
/// the claimed root, and verifies the proof binding.
pub fn verify_block_commitment_proof(
    proof: &BlockCommitmentProof,
    expected: &BlockProofPublicInputs,
) -> Result<bool> {
    // Check public input consistency
    if proof.block_height != expected.block_height {
        return Ok(false);
    }
    if proof.tx_count != expected.tx_count {
        return Ok(false);
    }
    if proof.commitment_tree_root != expected.commitment_tree_root {
        return Ok(false);
    }
    if proof.tx_commitment_hashes.len() as u64 != proof.tx_count {
        return Ok(false);
    }

    // Recompute Merkle root from leaves
    let recomputed_root = merkle_root(&proof.tx_commitment_hashes);
    if recomputed_root != proof.commitment_tree_root {
        return Ok(false);
    }

    // Recompute and check binding
    let expected_binding = compute_proof_binding(
        proof.block_height,
        proof.tx_count,
        &proof.commitment_tree_root,
    );
    if proof.proof_binding != expected_binding {
        return Ok(false);
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_block_proof() {
        let proof = generate_block_commitment_proof(1, &[]).unwrap();
        assert_eq!(proof.tx_count, 0);
        assert_eq!(proof.proof_type, "STARK-COMMITMENT-V1");

        let inputs = BlockProofPublicInputs {
            block_height: 1,
            tx_count: 0,
            commitment_tree_root: proof.commitment_tree_root,
        };
        assert!(verify_block_commitment_proof(&proof, &inputs).unwrap());
    }

    #[test]
    fn test_single_tx_proof() {
        let tx_commitment = b"pedersen_commitment_bytes_here".to_vec();
        let proof = generate_block_commitment_proof(42, &[tx_commitment]).unwrap();
        assert_eq!(proof.tx_count, 1);

        let inputs = BlockProofPublicInputs {
            block_height: 42,
            tx_count: 1,
            commitment_tree_root: proof.commitment_tree_root,
        };
        assert!(verify_block_commitment_proof(&proof, &inputs).unwrap());
    }

    #[test]
    fn test_multi_tx_proof() {
        let txs: Vec<Vec<u8>> = (0u8..8)
            .map(|i| vec![i; 32])
            .collect();
        let proof = generate_block_commitment_proof(100, &txs).unwrap();
        assert_eq!(proof.tx_count, 8);
        assert_eq!(proof.tx_commitment_hashes.len(), 8);

        let inputs = BlockProofPublicInputs {
            block_height: 100,
            tx_count: 8,
            commitment_tree_root: proof.commitment_tree_root,
        };
        assert!(verify_block_commitment_proof(&proof, &inputs).unwrap());
    }

    #[test]
    fn test_tampered_proof_fails() {
        let txs = vec![b"tx1".to_vec(), b"tx2".to_vec()];
        let mut proof = generate_block_commitment_proof(5, &txs).unwrap();

        // Tamper with one leaf hash
        proof.tx_commitment_hashes[0][0] ^= 0xFF;

        let inputs = BlockProofPublicInputs {
            block_height: 5,
            tx_count: 2,
            commitment_tree_root: proof.commitment_tree_root,
        };
        assert!(!verify_block_commitment_proof(&proof, &inputs).unwrap());
    }

    #[test]
    fn test_wrong_block_height_fails() {
        let proof = generate_block_commitment_proof(10, &[b"tx".to_vec()]).unwrap();
        let inputs = BlockProofPublicInputs {
            block_height: 99, // wrong
            tx_count: 1,
            commitment_tree_root: proof.commitment_tree_root,
        };
        assert!(!verify_block_commitment_proof(&proof, &inputs).unwrap());
    }

    #[test]
    fn test_serialization_roundtrip() {
        let txs = vec![b"hello".to_vec(), b"world".to_vec()];
        let proof = generate_block_commitment_proof(7, &txs).unwrap();
        let bytes = proof.to_bytes().unwrap();
        let proof2 = BlockCommitmentProof::from_bytes(&bytes).unwrap();
        assert_eq!(proof.commitment_tree_root, proof2.commitment_tree_root);
        assert_eq!(proof.proof_binding, proof2.proof_binding);
    }

    #[test]
    fn test_odd_tx_count_merkle() {
        // 3 txs — Merkle tree should still work (duplicates last node)
        let txs: Vec<Vec<u8>> = (0u8..3).map(|i| vec![i; 16]).collect();
        let proof = generate_block_commitment_proof(3, &txs).unwrap();
        let inputs = BlockProofPublicInputs {
            block_height: 3,
            tx_count: 3,
            commitment_tree_root: proof.commitment_tree_root,
        };
        assert!(verify_block_commitment_proof(&proof, &inputs).unwrap());
    }
}
