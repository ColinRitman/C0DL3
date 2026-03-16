// src/aa/paymaster.rs
// Gas payment abstraction.
//
// Paymasters are accounts that agree to pay gas for UserOperations.
// The sequencer verifies the paymaster has sufficient balance and a valid
// signature authorizing the gas payment.
//
// Privacy benefit: the gas payer (paymaster) is NOT the sender.

use crate::aa::schnorr::schnorr_verify;
use crate::aa::types::{PaymasterApproval, UserOperation};

/// Verify that a paymaster approval is valid for a given UserOperation.
pub fn verify_paymaster_approval(
    approval: &PaymasterApproval,
    paymaster_pubkey: &[u8; 32],
    op: &UserOperation,
    current_block: u64,
) -> Result<(), String> {
    // Check expiry
    if current_block > approval.valid_until_block {
        return Err(format!(
            "paymaster approval expired: current block {} > valid_until {}",
            current_block, approval.valid_until_block
        ));
    }

    // Check gas limit
    if op.gas_limit > approval.max_gas {
        return Err(format!(
            "gas limit {} exceeds paymaster max {}",
            op.gas_limit, approval.max_gas
        ));
    }

    // Verify signature: paymaster signs H("C0DL3:paymaster:" || op_hash || max_gas || valid_until)
    let op_hash = crate::aa::validation::compute_user_op_hash(op);
    let mut msg = Vec::new();
    msg.extend_from_slice(b"C0DL3:paymaster:");
    msg.extend_from_slice(&op_hash);
    msg.extend_from_slice(&approval.max_gas.to_le_bytes());
    msg.extend_from_slice(&approval.valid_until_block.to_le_bytes());

    if !schnorr_verify(paymaster_pubkey, &msg, &approval.signature) {
        return Err("invalid paymaster signature".to_string());
    }

    Ok(())
}

/// Compute the gas cost in fwei for a UserOperation.
pub fn compute_gas_cost(op: &UserOperation) -> u64 {
    let base_gas: u64 = 21_000;
    let data_gas: u64 = (op.call_data.len() as u64) * 68;
    let proof_gas: u64 = if op.range_proof.is_empty() { 0 } else { 50_000 };
    let conservation_gas: u64 = 4_000;
    let auth_gas: u64 = 3_000;
    let total_gas = base_gas + data_gas + proof_gas + conservation_gas + auth_gas;
    op.gas_price * total_gas
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aa::schnorr::schnorr_sign;
    use crate::aa::types::{KnowledgeProofBytes, SchnorrSignature};
    use curve25519_dalek_ng::constants::RISTRETTO_BASEPOINT_POINT as G;
    use curve25519_dalek_ng::scalar::Scalar;
    use rand::RngCore;

    fn random_scalar() -> Scalar {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        Scalar::from_bytes_mod_order(bytes)
    }

    fn dummy_user_op() -> UserOperation {
        UserOperation {
            sender: "0xAlice".to_string(),
            nonce: 0,
            to: "0xBob".to_string(),
            sender_new_commitment: [0xAA; 32],
            recipient_new_commitment: [0xBB; 32],
            amount_commitment: [0xCC; 32],
            knowledge_proof: KnowledgeProofBytes {
                commitment: [0; 32],
                announcement: [0; 32],
                response_v: [0; 32],
                response_r: [0; 32],
            },
            range_proof: vec![],
            conservation_excess: [0; 32],
            paymaster: Some("0xPaymaster".to_string()),
            gas_limit: 100_000,
            gas_price: 1,
            auth_signature: SchnorrSignature {
                r_point: [0; 32],
                s_scalar: [0; 32],
            },
            encrypted_memo: None,
            call_data: vec![],
        }
    }

    #[test]
    fn test_paymaster_approval_valid() {
        let pm_privkey = random_scalar();
        let pm_pubkey = (pm_privkey * G).compress().to_bytes();
        let op = dummy_user_op();

        let op_hash = crate::aa::validation::compute_user_op_hash(&op);
        let mut msg = Vec::new();
        msg.extend_from_slice(b"C0DL3:paymaster:");
        msg.extend_from_slice(&op_hash);
        msg.extend_from_slice(&200_000u64.to_le_bytes());
        msg.extend_from_slice(&100u64.to_le_bytes());
        let sig = schnorr_sign(&pm_privkey, &msg);

        let approval = PaymasterApproval {
            paymaster: "0xPaymaster".to_string(),
            max_gas: 200_000,
            signature: sig,
            valid_until_block: 100,
        };

        let result = verify_paymaster_approval(&approval, &pm_pubkey, &op, 50);
        assert!(result.is_ok(), "expected ok, got: {:?}", result.err());
    }

    #[test]
    fn test_paymaster_approval_expired() {
        let pm_privkey = random_scalar();
        let pm_pubkey = (pm_privkey * G).compress().to_bytes();
        let op = dummy_user_op();

        let op_hash = crate::aa::validation::compute_user_op_hash(&op);
        let mut msg = Vec::new();
        msg.extend_from_slice(b"C0DL3:paymaster:");
        msg.extend_from_slice(&op_hash);
        msg.extend_from_slice(&200_000u64.to_le_bytes());
        msg.extend_from_slice(&100u64.to_le_bytes());
        let sig = schnorr_sign(&pm_privkey, &msg);

        let approval = PaymasterApproval {
            paymaster: "0xPaymaster".to_string(),
            max_gas: 200_000,
            signature: sig,
            valid_until_block: 100,
        };

        let result = verify_paymaster_approval(&approval, &pm_pubkey, &op, 101);
        assert!(result.is_err());
        assert!(
            result.unwrap_err().contains("expired"),
            "error should mention 'expired'"
        );
    }

    #[test]
    fn test_paymaster_gas_exceeded() {
        let pm_privkey = random_scalar();
        let pm_pubkey = (pm_privkey * G).compress().to_bytes();
        let op = dummy_user_op(); // gas_limit = 100_000

        let approval = PaymasterApproval {
            paymaster: "0xPaymaster".to_string(),
            max_gas: 50_000, // less than op.gas_limit
            signature: SchnorrSignature {
                r_point: [0; 32],
                s_scalar: [0; 32],
            },
            valid_until_block: 100,
        };

        let result = verify_paymaster_approval(&approval, &pm_pubkey, &op, 50);
        assert!(result.is_err());
        assert!(
            result.unwrap_err().contains("exceeds"),
            "error should mention 'exceeds'"
        );
    }

    #[test]
    fn test_gas_cost_computation() {
        let op = dummy_user_op(); // call_data empty, range_proof empty, gas_price = 1
        let cost = compute_gas_cost(&op);
        // base(21_000) + data(0) + range(0) + conservation(4_000) + auth(3_000) = 28_000
        assert_eq!(cost, 28_000);
    }
}
