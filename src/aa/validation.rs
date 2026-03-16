// src/aa/validation.rs
// AA wallet transaction validation.
//
// Validates UserOperations before inclusion in a block:
//   1. Schnorr auth: sender proves ownership
//   2. Conservation: old_sender - new_sender = new_recipient - old_recipient
//   3. Knowledge proof: sender knows (amount, blinding) for amount_commitment
//
// This is "early rejection" — the guest re-verifies everything inside SP1.

use curve25519_dalek_ng::ristretto::{CompressedRistretto, RistrettoPoint};
use curve25519_dalek_ng::traits::Identity;

use crate::aa::schnorr::schnorr_verify;
use crate::aa::types::UserOperation;
use crate::privacy::{verify_commitment_knowledge, CommitmentKnowledgeProof};

/// Validate a UserOperation for block inclusion.
pub fn validate_user_operation(
    op: &UserOperation,
    sender_pubkey: &[u8; 32],
    sender_current_commitment: &[u8; 32],
    recipient_current_commitment: &[u8; 32],
) -> Result<(), String> {
    // 1. Verify Schnorr auth signature
    let op_hash = compute_user_op_hash(op);
    if !schnorr_verify(sender_pubkey, &op_hash, &op.auth_signature) {
        return Err("invalid auth signature".to_string());
    }

    // 2. Verify conservation
    if !verify_conservation(
        sender_current_commitment,
        &op.sender_new_commitment,
        recipient_current_commitment,
        &op.recipient_new_commitment,
    ) {
        return Err("conservation check failed".to_string());
    }

    // 3. Verify knowledge proof for amount commitment
    let knowledge_proof = CommitmentKnowledgeProof {
        commitment: op.knowledge_proof.commitment,
        announcement: op.knowledge_proof.announcement,
        response_v: op.knowledge_proof.response_v,
        response_r: op.knowledge_proof.response_r,
    };
    if !verify_commitment_knowledge(&knowledge_proof) {
        return Err("invalid knowledge proof".to_string());
    }

    // 4. Check knowledge proof commitment matches amount commitment
    if op.knowledge_proof.commitment != op.amount_commitment {
        return Err("knowledge proof commitment mismatch".to_string());
    }

    Ok(())
}

/// Verify Pedersen commitment conservation.
/// Checks: (old_sender - new_sender) == (new_recipient - old_recipient)
pub fn verify_conservation(
    old_sender: &[u8; 32],
    new_sender: &[u8; 32],
    old_recipient: &[u8; 32],
    new_recipient: &[u8; 32],
) -> bool {
    let os = match CompressedRistretto(*old_sender).decompress() {
        Some(p) => p,
        None => return false,
    };
    let ns = match CompressedRistretto(*new_sender).decompress() {
        Some(p) => p,
        None => return false,
    };
    let or_point = match CompressedRistretto(*old_recipient).decompress() {
        Some(p) => p,
        None => return false,
    };
    let nr = match CompressedRistretto(*new_recipient).decompress() {
        Some(p) => p,
        None => return false,
    };

    let sender_delta = os - ns;
    let recipient_delta = nr - or_point;
    let excess = sender_delta - recipient_delta;
    excess == RistrettoPoint::identity()
}

/// Compute the hash of a UserOperation for signing.
pub fn compute_user_op_hash(op: &UserOperation) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"C0DL3:userop:");
    hasher.update(op.sender.as_bytes());
    hasher.update(op.nonce.to_le_bytes());
    hasher.update(op.to.as_bytes());
    hasher.update(&op.amount_commitment);
    hasher.update(&op.sender_new_commitment);
    hasher.update(&op.recipient_new_commitment);
    hasher.update(op.gas_limit.to_le_bytes());
    hasher.update(op.gas_price.to_le_bytes());
    if let Some(ref pm) = op.paymaster {
        hasher.update(pm.as_bytes());
    }
    hasher.finalize().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use curve25519_dalek_ng::constants::RISTRETTO_BASEPOINT_POINT as G;
    use curve25519_dalek_ng::scalar::Scalar;
    use rand::RngCore;

    use crate::aa::schnorr::schnorr_sign;
    use crate::aa::types::{KnowledgeProofBytes, SchnorrSignature};
    use crate::privacy::prove_commitment_knowledge_with_commitment;
    use crate::privacy::shielded_pool::commit_with_blinding;

    fn random_scalar() -> Scalar {
        let mut bytes = [0u8; 64];
        rand::thread_rng().fill_bytes(&mut bytes);
        Scalar::from_bytes_mod_order_wide(&bytes)
    }

    #[test]
    fn test_conservation_valid() {
        // Sender has 100, sends 30, keeps 70.
        // Recipient has 50, receives 30, now has 80.
        let r_sender_old = random_scalar();
        let r_sender_new = random_scalar();
        let r_recipient_old = random_scalar();
        // Coordinate blinding so conservation holds:
        let r_recipient_new = r_recipient_old + (r_sender_old - r_sender_new);

        let old_sender = commit_with_blinding(100, &r_sender_old);
        let new_sender = commit_with_blinding(70, &r_sender_new);
        let old_recipient = commit_with_blinding(50, &r_recipient_old);
        let new_recipient = commit_with_blinding(80, &r_recipient_new);

        assert!(verify_conservation(
            &old_sender,
            &new_sender,
            &old_recipient,
            &new_recipient,
        ));
    }

    #[test]
    fn test_conservation_invalid_amount() {
        // Sender loses 30 but recipient gains 40 — should fail.
        let r_sender_old = random_scalar();
        let r_sender_new = random_scalar();
        let r_recipient_old = random_scalar();
        let r_recipient_new = r_recipient_old + (r_sender_old - r_sender_new);

        let old_sender = commit_with_blinding(100, &r_sender_old);
        let new_sender = commit_with_blinding(70, &r_sender_new);   // lost 30
        let old_recipient = commit_with_blinding(50, &r_recipient_old);
        let new_recipient = commit_with_blinding(90, &r_recipient_new); // gained 40 (mismatch!)

        assert!(!verify_conservation(
            &old_sender,
            &new_sender,
            &old_recipient,
            &new_recipient,
        ));
    }

    #[test]
    fn test_conservation_invalid_point() {
        // All 0xFF bytes are not valid Ristretto points.
        let bad = [0xFF; 32];
        let good = commit_with_blinding(10, &random_scalar());
        assert!(!verify_conservation(&bad, &good, &good, &good));
    }

    #[test]
    fn test_full_user_op_validation() {
        // Setup keys
        let privkey = random_scalar();
        let pubkey = (privkey * G).compress().to_bytes();

        // Balances: sender 100 -> 70, recipient 50 -> 80, amount = 30
        let r_sender_old = random_scalar();
        let r_sender_new = random_scalar();
        let r_recipient_old = random_scalar();
        let r_recipient_new = r_recipient_old + (r_sender_old - r_sender_new);

        let old_sender_commit = commit_with_blinding(100, &r_sender_old);
        let new_sender_commit = commit_with_blinding(70, &r_sender_new);
        let old_recipient_commit = commit_with_blinding(50, &r_recipient_old);
        let new_recipient_commit = commit_with_blinding(80, &r_recipient_new);

        // Amount commitment with blinding = r_sender_old - r_sender_new
        let amount_blinding = r_sender_old - r_sender_new;
        let amount_commit = commit_with_blinding(30, &amount_blinding);

        // Knowledge proof for the amount commitment
        let kp = prove_commitment_knowledge_with_commitment(30, &amount_blinding, amount_commit);

        let knowledge_proof = KnowledgeProofBytes {
            commitment: kp.commitment,
            announcement: kp.announcement,
            response_v: kp.response_v,
            response_r: kp.response_r,
        };

        // Build the UserOperation (without signature first, to compute hash)
        let mut op = UserOperation {
            sender: "0xAlice".to_string(),
            nonce: 1,
            to: "0xBob".to_string(),
            sender_new_commitment: new_sender_commit,
            recipient_new_commitment: new_recipient_commit,
            amount_commitment: amount_commit,
            knowledge_proof,
            range_proof: vec![],
            conservation_excess: [0; 32],
            paymaster: None,
            gas_limit: 100_000,
            gas_price: 1,
            auth_signature: SchnorrSignature {
                r_point: [0; 32],
                s_scalar: [0; 32],
            },
            encrypted_memo: None,
            call_data: vec![],
        };

        // Sign the operation
        let op_hash = compute_user_op_hash(&op);
        op.auth_signature = schnorr_sign(&privkey, &op_hash);

        // Validate
        let result = validate_user_operation(
            &op,
            &pubkey,
            &old_sender_commit,
            &old_recipient_commit,
        );
        assert!(result.is_ok(), "validation failed: {:?}", result.err());
    }
}
