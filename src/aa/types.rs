// src/aa/types.rs
// Account Abstraction types for COLDL3.
//
// Every account is a smart contract wallet. No EOAs.
// Wallets store balance commitments, not plaintext balances.
// Transactions are UserOperations validated by the wallet contract.

use serde::{Deserialize, Serialize};

/// Wallet type — distinguishes legacy EOA accounts from AA wallets.
/// During migration, both coexist. New accounts are always AA.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WalletType {
    /// Legacy EOA account (pre-AA migration). Plaintext balance, ECDSA auth.
    LegacyEOA,
    /// Native AA wallet. Commitment-based balance, Schnorr auth.
    PrivateWallet,
}

impl Default for WalletType {
    fn default() -> Self {
        WalletType::PrivateWallet // New accounts default to AA
    }
}

/// A user operation — the AA equivalent of a transaction.
///
/// Instead of (from, to, value, nonce, signature), a UserOp contains:
/// - sender: the wallet contract address
/// - call_data: what the wallet should execute
/// - commitment_updates: new balance commitments for sender + recipient
/// - conservation_proof: proves Σ inputs = Σ outputs (no value created/destroyed)
/// - auth_proof: Schnorr proof that the sender owns the wallet
/// - paymaster: optional address that pays gas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserOperation {
    /// Sender wallet contract address.
    pub sender: String,
    /// Nonce (anti-replay, sequential per wallet).
    pub nonce: u64,
    /// Recipient address (wallet or contract).
    pub to: String,
    /// New sender balance commitment after transfer.
    pub sender_new_commitment: [u8; 32],
    /// New recipient balance commitment after transfer.
    pub recipient_new_commitment: [u8; 32],
    /// Pedersen commitment to the transfer amount: C = amount*G + r*H
    pub amount_commitment: [u8; 32],
    /// Schnorr knowledge proof: sender knows (amount, blinding) for amount_commitment.
    pub knowledge_proof: KnowledgeProofBytes,
    /// Range proof: amount in [0, 2^64). Serialized Bulletproofs.
    pub range_proof: Vec<u8>,
    /// Conservation proof: old_sender_commit - new_sender_commit = amount_commit
    /// AND new_recipient_commit - old_recipient_commit = amount_commit
    /// Encoded as the kernel excess point (should be identity if valid).
    pub conservation_excess: [u8; 32],
    /// Optional paymaster address. If set, paymaster pays gas.
    pub paymaster: Option<String>,
    /// Gas limit for this operation.
    pub gas_limit: u64,
    /// Gas price in fwei.
    pub gas_price: u64,
    /// Auth signature — Schnorr signature over operation hash.
    pub auth_signature: SchnorrSignature,
    /// Optional encrypted memo for recipient (ElGamal encrypted amount + blinding).
    pub encrypted_memo: Option<Vec<u8>>,
    /// Arbitrary calldata for contract interactions (empty for simple transfers).
    pub call_data: Vec<u8>,
}

/// Schnorr signature (R, s) over a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchnorrSignature {
    /// R = k*G (nonce point, compressed Ristretto)
    pub r_point: [u8; 32],
    /// s = k + e*privkey where e = H(R || pubkey || message)
    pub s_scalar: [u8; 32],
}

/// Knowledge proof bytes — matches CommitmentKnowledgeProof layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeProofBytes {
    pub commitment: [u8; 32],
    pub announcement: [u8; 32],
    pub response_v: [u8; 32],
    pub response_r: [u8; 32],
}

/// Default wallet type for deserialization of legacy data.
fn default_wallet_type() -> WalletType {
    WalletType::LegacyEOA
}

/// On-chain account state for every wallet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountState {
    /// Pedersen commitment to balance: C = balance*G + r*H
    /// Published on-chain — hides balance from external observers.
    pub balance_commitment: [u8; 32],
    /// Plaintext balance kept for sequencer-internal execution.
    /// Trusted testnet model: sequencer knows amounts, observers don't.
    pub balance: u64,  // In fwei (1 fwei = 0.001 HEAT = 1,000,000 gwei)
    pub nonce: u64,
    #[serde(default = "default_wallet_type")]
    pub wallet_type: WalletType,
    /// Owner public key (Ristretto255 compressed). Only set for PrivateWallet accounts.
    #[serde(default)]
    pub owner_pubkey: Option<[u8; 32]>,
}

/// Paymaster approval — a signed statement that a paymaster will cover gas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymasterApproval {
    /// Paymaster address (must have sufficient public balance for gas).
    pub paymaster: String,
    /// Maximum gas the paymaster will cover for this UserOp.
    pub max_gas: u64,
    /// Schnorr signature from paymaster authorizing this gas payment.
    pub signature: SchnorrSignature,
    /// Expiry block height (approval invalid after this).
    pub valid_until_block: u64,
}

/// Configuration for a PrivateWallet contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletConfig {
    /// Owner's public key (Ristretto255 compressed).
    pub owner_pubkey: [u8; 32],
    /// Optional guardian public keys for social recovery.
    pub guardians: Vec<[u8; 32]>,
    /// Number of guardian signatures required for recovery.
    pub recovery_threshold: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wallet_type_default_is_private() {
        assert_eq!(WalletType::default(), WalletType::PrivateWallet);
    }

    #[test]
    fn test_wallet_type_serialization() {
        let wt = WalletType::PrivateWallet;
        let json = serde_json::to_string(&wt).unwrap();
        let decoded: WalletType = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, wt);
    }

    #[test]
    fn test_user_operation_serialization() {
        let op = UserOperation {
            sender: "0x1234".to_string(),
            nonce: 1,
            to: "0x5678".to_string(),
            sender_new_commitment: [0xAA; 32],
            recipient_new_commitment: [0xBB; 32],
            amount_commitment: [0xCC; 32],
            knowledge_proof: KnowledgeProofBytes {
                commitment: [0xCC; 32],
                announcement: [0xDD; 32],
                response_v: [0x01; 32],
                response_r: [0x02; 32],
            },
            range_proof: vec![0u8; 64],
            conservation_excess: [0; 32],
            paymaster: None,
            gas_limit: 100_000,
            gas_price: 1,
            auth_signature: SchnorrSignature {
                r_point: [0xEE; 32],
                s_scalar: [0xFF; 32],
            },
            encrypted_memo: None,
            call_data: vec![],
        };
        let json = serde_json::to_string(&op).unwrap();
        let decoded: UserOperation = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.sender, op.sender);
        assert_eq!(decoded.nonce, op.nonce);
    }
}
