// Merkle tree utilities for the COLDL3 note tree.
//
// Must match host-side `note_merkle_root()` in `src/privacy/shielded_pool.rs`
// and guest-side `compute_note_tree_root()` in `program/src/privacy.rs` exactly.
//
// Tree properties:
//   - SHA-256 with domain separator "C0DL3:note_tree:" per internal node
//   - Empty tree: SHA-256("C0DL3:empty_pool")
//   - Single leaf: the leaf commitment itself (no hashing)
//   - Odd-length levels: duplicate last element

use sha2::{Digest, Sha256};

/// Compute the note Merkle root from a list of note commitments.
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

/// Generate a Merkle membership proof for a leaf at `index`.
///
/// Returns a vector of `(sibling_hash, is_left)` pairs, where `is_left` means
/// the sibling is on the left side of the hash.
///
/// Returns None if index is out of bounds or commitments is empty.
pub fn compute_merkle_proof(
    commitments: &[[u8; 32]],
    index: usize,
) -> Option<Vec<([u8; 32], bool)>> {
    if commitments.is_empty() || index >= commitments.len() {
        return None;
    }
    if commitments.len() == 1 {
        return Some(vec![]); // leaf IS the root
    }

    let mut proof = Vec::new();
    let mut current_level = commitments.to_vec();
    let mut idx = index;

    while current_level.len() > 1 {
        // Handle odd-length: duplicate last
        if current_level.len() % 2 == 1 {
            let last = *current_level.last().unwrap();
            current_level.push(last);
        }

        // Find sibling
        let sibling_idx = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
        let is_left = idx % 2 == 1; // sibling is on the left if we're on the right
        proof.push((current_level[sibling_idx], is_left));

        // Build next level
        let mut next_level = Vec::new();
        let mut i = 0;
        while i < current_level.len() {
            let mut h = Sha256::new();
            h.update(b"C0DL3:note_tree:");
            h.update(current_level[i]);
            h.update(current_level[i + 1]);
            next_level.push(h.finalize().into());
            i += 2;
        }

        idx /= 2;
        current_level = next_level;
    }

    Some(proof)
}

/// Verify a Merkle membership proof.
///
/// Reconstructs the root from the leaf and proof path, then compares against
/// the expected root.
pub fn verify_merkle_proof(
    root: &[u8; 32],
    leaf: &[u8; 32],
    proof: &[([u8; 32], bool)],
) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_commitment(n: u8) -> [u8; 32] {
        let mut c = [0u8; 32];
        c[0] = n;
        c
    }

    #[test]
    fn test_empty_tree() {
        let root = compute_note_tree_root(&[]);
        let expected: [u8; 32] = Sha256::digest(b"C0DL3:empty_pool").into();
        assert_eq!(root, expected);
    }

    #[test]
    fn test_single_leaf() {
        let leaf = mock_commitment(0x42);
        let root = compute_note_tree_root(&[leaf]);
        assert_eq!(root, leaf, "single leaf IS the root");
    }

    #[test]
    fn test_merkle_proof_single_element() {
        let leaf = mock_commitment(1);
        let root = compute_note_tree_root(&[leaf]);
        let proof = compute_merkle_proof(&[leaf], 0).unwrap();
        assert!(proof.is_empty());
        assert!(verify_merkle_proof(&root, &leaf, &proof));
    }

    #[test]
    fn test_merkle_proof_two_elements() {
        let commitments = [mock_commitment(1), mock_commitment(2)];
        let root = compute_note_tree_root(&commitments);

        for idx in 0..2 {
            let proof = compute_merkle_proof(&commitments, idx).unwrap();
            assert_eq!(proof.len(), 1);
            assert!(
                verify_merkle_proof(&root, &commitments[idx], &proof),
                "proof for index {} must verify",
                idx
            );
        }
    }

    #[test]
    fn test_merkle_proof_four_elements() {
        let commitments: Vec<[u8; 32]> = (0..4).map(|i| mock_commitment(i + 1)).collect();
        let root = compute_note_tree_root(&commitments);

        for idx in 0..4 {
            let proof = compute_merkle_proof(&commitments, idx).unwrap();
            assert_eq!(proof.len(), 2, "depth-2 tree needs 2 proof elements");
            assert!(
                verify_merkle_proof(&root, &commitments[idx], &proof),
                "proof for index {} must verify",
                idx
            );
        }
    }

    #[test]
    fn test_merkle_proof_five_elements() {
        let commitments: Vec<[u8; 32]> = (0..5).map(|i| mock_commitment(i + 1)).collect();
        let root = compute_note_tree_root(&commitments);

        for idx in 0..5 {
            let proof = compute_merkle_proof(&commitments, idx).unwrap();
            assert!(
                verify_merkle_proof(&root, &commitments[idx], &proof),
                "proof for index {} must verify (odd tree)",
                idx
            );
        }
    }

    #[test]
    fn test_merkle_proof_invalid_leaf() {
        let commitments = [mock_commitment(1), mock_commitment(2)];
        let root = compute_note_tree_root(&commitments);
        let proof = compute_merkle_proof(&commitments, 0).unwrap();

        let wrong_leaf = mock_commitment(99);
        assert!(
            !verify_merkle_proof(&root, &wrong_leaf, &proof),
            "wrong leaf must fail"
        );
    }

    #[test]
    fn test_merkle_proof_invalid_root() {
        let commitments = [mock_commitment(1), mock_commitment(2)];
        let proof = compute_merkle_proof(&commitments, 0).unwrap();

        let wrong_root = [0xFFu8; 32];
        assert!(
            !verify_merkle_proof(&wrong_root, &commitments[0], &proof),
            "wrong root must fail"
        );
    }

    #[test]
    fn test_merkle_proof_out_of_bounds() {
        let commitments = [mock_commitment(1), mock_commitment(2)];
        assert!(compute_merkle_proof(&commitments, 2).is_none());
        assert!(compute_merkle_proof(&commitments, 100).is_none());
        assert!(compute_merkle_proof(&[], 0).is_none());
    }

    #[test]
    fn test_merkle_proof_large_tree() {
        let commitments: Vec<[u8; 32]> = (0..100u8)
            .map(|i| {
                let mut c = [0u8; 32];
                c[0] = i;
                c[31] = 0xFF - i;
                c
            })
            .collect();
        let root = compute_note_tree_root(&commitments);

        // Spot-check several indices
        for &idx in &[0, 1, 49, 50, 99] {
            let proof = compute_merkle_proof(&commitments, idx).unwrap();
            assert!(
                verify_merkle_proof(&root, &commitments[idx], &proof),
                "proof for index {} in 100-element tree must verify",
                idx
            );
        }
    }
}
