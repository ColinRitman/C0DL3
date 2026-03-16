//! Threshold proof: verify that a committed value is >= a given threshold.
//!
//! Uses Bulletproofs range proofs on the shifted commitment C' = C - threshold*B.
//! If the original value v >= threshold, then v - threshold >= 0 and fits in a 64-bit range.

use bulletproofs::{BulletproofGens, PedersenGens, RangeProof};
use curve25519_dalek_ng::{ristretto::CompressedRistretto, scalar::Scalar};
use merlin::Transcript;
use once_cell::sync::Lazy;

static PEDERSEN_GENS: Lazy<PedersenGens> = Lazy::new(PedersenGens::default);
static BP_GENS: Lazy<BulletproofGens> = Lazy::new(|| BulletproofGens::new(64, 128));

/// Verify that the value inside `commitment` is >= `threshold`.
///
/// The prover must supply a range proof over C' = C - threshold*B,
/// proving that (value - threshold) is in [0, 2^64).
pub fn verify_threshold_proof(
    commitment: &[u8; 32],
    threshold: u64,
    range_proof_bytes: &[u8],
) -> bool {
    let c_point = match CompressedRistretto(*commitment).decompress() {
        Some(p) => p,
        None => return false,
    };
    let mut threshold_padded = [0u8; 32];
    threshold_padded[..8].copy_from_slice(&threshold.to_le_bytes());
    let threshold_scalar = Scalar::from_bytes_mod_order(threshold_padded);
    let c_prime = c_point - threshold_scalar * PEDERSEN_GENS.B;
    let c_prime_compressed = c_prime.compress();

    let rp = match RangeProof::from_bytes(range_proof_bytes) {
        Ok(rp) => rp,
        Err(_) => return false,
    };

    let mut transcript = Transcript::new(b"C0DL3-ThresholdProof");
    rp.verify_single(
        &BP_GENS,
        &PEDERSEN_GENS,
        &mut transcript,
        &c_prime_compressed,
        64,
    )
    .is_ok()
}

/// Prove that `value` >= `threshold` by creating a range proof over (value - threshold).
///
/// Returns `None` if value < threshold.
pub fn prove_threshold(value: u64, blinding: &Scalar, threshold: u64) -> Option<Vec<u8>> {
    if value < threshold {
        return None;
    }
    let diff = value - threshold;
    let mut transcript = Transcript::new(b"C0DL3-ThresholdProof");
    let (rp, _) = RangeProof::prove_single(
        &BP_GENS,
        &PEDERSEN_GENS,
        &mut transcript,
        diff,
        blinding,
        64,
    )
    .ok()?;
    Some(rp.to_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_commitment(value: u64, blinding: &Scalar) -> [u8; 32] {
        let gens = PedersenGens::default();
        let mut v_padded = [0u8; 32];
        v_padded[..8].copy_from_slice(&value.to_le_bytes());
        let v_scalar = Scalar::from_bytes_mod_order(v_padded);
        gens.commit(v_scalar, *blinding).compress().to_bytes()
    }

    #[test]
    fn test_threshold_proof_valid() {
        let blinding = Scalar::from_bytes_mod_order([0x42u8; 32]);
        let value = 1000u64;
        let threshold = 500u64;

        let commitment = make_commitment(value, &blinding);
        let proof_bytes = prove_threshold(value, &blinding, threshold)
            .expect("should produce proof for value >= threshold");

        assert!(
            verify_threshold_proof(&commitment, threshold, &proof_bytes),
            "valid threshold proof should verify"
        );
    }

    #[test]
    fn test_threshold_proof_below_threshold() {
        let blinding = Scalar::from_bytes_mod_order([0x42u8; 32]);
        let value = 100u64;
        let threshold = 500u64;

        let result = prove_threshold(value, &blinding, threshold);
        assert!(
            result.is_none(),
            "should not produce proof when value < threshold"
        );
    }

    #[test]
    fn test_threshold_proof_exactly_at_threshold() {
        let blinding = Scalar::from_bytes_mod_order([0x42u8; 32]);
        let value = 500u64;
        let threshold = 500u64;

        let commitment = make_commitment(value, &blinding);
        let proof_bytes = prove_threshold(value, &blinding, threshold)
            .expect("should produce proof for value == threshold (diff=0)");

        assert!(
            verify_threshold_proof(&commitment, threshold, &proof_bytes),
            "threshold proof at exact threshold should verify"
        );
    }
}
