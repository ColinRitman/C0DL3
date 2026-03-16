//! Poseidon-like algebraic hash for C0DL3.
//!
//! Uses Ristretto255 scalar field operations from curve25519-dalek-ng
//! to build a ZK-friendly permutation-based hash. This is a simplified
//! Poseidon-style construction suitable for Merkle trees and nullifiers.

use curve25519_dalek_ng::scalar::Scalar;
use sha2::{Digest, Sha256};

/// Number of full rounds in the permutation.
const FULL_ROUNDS: usize = 8;
/// Number of partial rounds in the permutation.
const PARTIAL_ROUNDS: usize = 57;
/// Width of the state (rate=2, capacity=1).
const WIDTH: usize = 3;

/// Domain separator for C0DL3 Poseidon.
const DOMAIN: &[u8] = b"C0DL3:poseidon:";

/// Generate a round constant from index via domain-separated SHA-256.
fn round_constant(round: usize, pos: usize) -> Scalar {
    let mut hasher = Sha256::new();
    hasher.update(DOMAIN);
    hasher.update(b"rc:");
    hasher.update((round as u64).to_le_bytes());
    hasher.update((pos as u64).to_le_bytes());
    let hash: [u8; 32] = hasher.finalize().into();
    Scalar::from_bytes_mod_order(hash)
}

/// S-box: cube in scalar field (x^3). Provides algebraic degree needed for security.
#[inline]
fn sbox(x: Scalar) -> Scalar {
    let x2 = x * x;
    x2 * x
}

/// MDS mix: a simple 3x3 Cauchy matrix mix over the state.
/// Uses the matrix:
///   [[2, 1, 1],
///    [1, 2, 1],
///    [1, 1, 2]]
/// This is MDS over the scalar field.
fn mds_mix(state: &mut [Scalar; WIDTH]) {
    let s0 = state[0];
    let s1 = state[1];
    let s2 = state[2];
    let two = Scalar::from(2u64);
    state[0] = two * s0 + s1 + s2;
    state[1] = s0 + two * s1 + s2;
    state[2] = s0 + s1 + two * s2;
}

/// Poseidon-like hash: H(left, right) -> 32-byte digest.
///
/// State = [left, right, capacity]. Applies full rounds with S-box on all elements,
/// partial rounds with S-box on only the first element, then full rounds again.
pub fn poseidon_hash(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut state: [Scalar; WIDTH] = [
        Scalar::from_bytes_mod_order(*left),
        Scalar::from_bytes_mod_order(*right),
        // Capacity element: domain separation
        {
            let mut hasher = Sha256::new();
            hasher.update(DOMAIN);
            hasher.update(b"capacity");
            let hash: [u8; 32] = hasher.finalize().into();
            Scalar::from_bytes_mod_order(hash)
        },
    ];

    let half_full = FULL_ROUNDS / 2;
    let mut round_idx = 0usize;

    // First half full rounds
    for _ in 0..half_full {
        for j in 0..WIDTH {
            state[j] += round_constant(round_idx, j);
        }
        for j in 0..WIDTH {
            state[j] = sbox(state[j]);
        }
        mds_mix(&mut state);
        round_idx += 1;
    }

    // Partial rounds (S-box only on first element)
    for _ in 0..PARTIAL_ROUNDS {
        for j in 0..WIDTH {
            state[j] += round_constant(round_idx, j);
        }
        state[0] = sbox(state[0]);
        mds_mix(&mut state);
        round_idx += 1;
    }

    // Second half full rounds
    for _ in 0..half_full {
        for j in 0..WIDTH {
            state[j] += round_constant(round_idx, j);
        }
        for j in 0..WIDTH {
            state[j] = sbox(state[j]);
        }
        mds_mix(&mut state);
        round_idx += 1;
    }

    state[0].to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poseidon_deterministic() {
        let a = [0x01u8; 32];
        let b = [0x02u8; 32];
        let h1 = poseidon_hash(&a, &b);
        let h2 = poseidon_hash(&a, &b);
        assert_eq!(h1, h2, "Poseidon hash must be deterministic");
        // Must not be all zeros
        assert_ne!(h1, [0u8; 32], "Hash output should not be zero");
    }

    #[test]
    fn test_poseidon_different_inputs() {
        let a = [0x01u8; 32];
        let b = [0x02u8; 32];
        let c = [0x03u8; 32];
        let h1 = poseidon_hash(&a, &b);
        let h2 = poseidon_hash(&a, &c);
        assert_ne!(h1, h2, "Different inputs must produce different outputs");
    }

    #[test]
    fn test_poseidon_commutative_check() {
        let a = [0x01u8; 32];
        let b = [0x02u8; 32];
        let h_ab = poseidon_hash(&a, &b);
        let h_ba = poseidon_hash(&b, &a);
        assert_ne!(h_ab, h_ba, "hash(a,b) != hash(b,a) — not commutative");
    }
}
