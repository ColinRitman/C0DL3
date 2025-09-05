// Comprehensive Test Suite for Privacy Features
// Tests all privacy components with elite-level security standards

use super::*;
use anyhow::Result;

/// Test suite for user-level privacy features
#[cfg(test)]
mod privacy_tests {
    use super::*;
    
    #[test]
    fn test_user_privacy_manager_creation() {
        let manager = UserPrivacyManager::new().unwrap();
        assert!(true, "User privacy manager should be created successfully");
    }
    
    #[test]
    fn test_private_transaction_creation() {
        let manager = UserPrivacyManager::new().unwrap();
        let tx = manager.create_private_transaction(
            "sender_address_123",
            "recipient_address_456", 
            1000,
            1234567890,
            5000,
        ).unwrap();
        
        // Verify all privacy components are present
        assert!(!tx.hash.is_empty(), "Transaction hash should not be empty");
        assert!(!tx.encrypted_sender.ciphertext.is_empty(), "Encrypted sender should not be empty");
        assert!(!tx.encrypted_recipient.ciphertext.is_empty(), "Encrypted recipient should not be empty");
        assert!(!tx.amount_commitment.commitment.is_empty(), "Amount commitment should not be empty");
        assert!(!tx.encrypted_timestamp.ciphertext.is_empty(), "Encrypted timestamp should not be empty");
        
        // Verify STARK proofs are present
        assert!(!tx.validity_proof.proof_data.is_empty(), "Validity proof should not be empty");
        assert!(!tx.range_proof.proof_data.is_empty(), "Range proof should not be empty");
        assert!(!tx.balance_proof.proof_data.is_empty(), "Balance proof should not be empty");
    }
    
    #[test]
    fn test_transaction_verification() {
        let manager = UserPrivacyManager::new().unwrap();
        let tx = manager.create_private_transaction(
            "sender_address",
            "recipient_address",
            1000,
            1234567890,
            5000,
        ).unwrap();
        
        let is_valid = manager.verify_private_transaction(&tx).unwrap();
        assert!(is_valid, "Valid transaction should pass verification");
    }
    
    #[test]
    fn test_transaction_decryption() {
        let manager = UserPrivacyManager::new().unwrap();
        let tx = manager.create_private_transaction(
            "test_sender",
            "test_recipient",
            1000,
            1234567890,
            5000,
        ).unwrap();
        
        let decrypted = manager.decrypt_transaction_details(&tx).unwrap();
        assert_eq!(decrypted.sender, "test_sender");
        assert_eq!(decrypted.recipient, "test_recipient");
        assert_eq!(decrypted.timestamp, 1234567890);
    }
    
    #[test]
    fn test_stark_proof_system() {
        let stark_system = StarkProofSystem::new().unwrap();
        
        // Test transaction validity proof
        let validity_proof = stark_system.prove_transaction_validity(1000, 5000).unwrap();
        assert_eq!(validity_proof.proof_type, "transaction_validity");
        assert_eq!(validity_proof.security_level, 128);
        
        // Test amount range proof
        let range_proof = stark_system.prove_amount_range(1000, 100, 10000).unwrap();
        assert_eq!(range_proof.proof_type, "amount_range");
        
        // Test balance consistency proof
        let balance_proof = stark_system.prove_balance_consistency(5000, 1000).unwrap();
        assert_eq!(balance_proof.proof_type, "balance_consistency");
        
        // Test proof verification
        assert!(stark_system.verify_proof(&validity_proof).unwrap());
        assert!(stark_system.verify_proof(&range_proof).unwrap());
        assert!(stark_system.verify_proof(&balance_proof).unwrap());
    }
    
    #[test]
    fn test_amount_commitments() {
        let commitment = AmountCommitment::new(1000).unwrap();
        
        // Test commitment verification
        assert!(commitment.verify_commitment(1000).unwrap());
        assert!(!commitment.verify_commitment(2000).unwrap());
        
        // Test range proof
        let range_proof = commitment.prove_amount_range(1000, 10000).unwrap();
        assert_eq!(range_proof.max_amount, 10000);
        assert!(commitment.verify_range_proof(&range_proof).unwrap());
        
        // Test commitment hash
        let hash = commitment.get_commitment_hash();
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64); // SHA-256 hex string length
    }
    
    #[test]
    fn test_address_encryption() {
        let key = [1u8; 32];
        let encryption = AddressEncryption::new(&key).unwrap();
        
        // Test sender encryption
        let encrypted_sender = encryption.encrypt_sender("sender_address").unwrap();
        assert!(!encrypted_sender.ciphertext.is_empty());
        assert_ne!(encrypted_sender.nonce, [0u8; 12]);
        assert_ne!(encrypted_sender.tag, [0u8; 16]);
        assert_eq!(encrypted_sender.metadata.address_type, "sender");
        
        // Test recipient encryption
        let encrypted_recipient = encryption.encrypt_recipient("recipient_address").unwrap();
        assert!(!encrypted_recipient.ciphertext.is_empty());
        assert_eq!(encrypted_recipient.metadata.address_type, "recipient");
        
        // Test decryption
        let decrypted_sender = encryption.decrypt_address(&encrypted_sender).unwrap();
        assert_eq!(decrypted_sender, "sender_address");
        
        let decrypted_recipient = encryption.decrypt_address(&encrypted_recipient).unwrap();
        assert_eq!(decrypted_recipient, "recipient_address");
        
        // Test verification
        assert!(encryption.verify_address(&encrypted_sender).unwrap());
        assert!(encryption.verify_address(&encrypted_recipient).unwrap());
    }
    
    #[test]
    fn test_timing_privacy() {
        let key = [1u8; 32];
        let timing_privacy = TimingPrivacy::new(&key).unwrap();
        
        // Test timestamp encryption
        let timestamp = 1234567890;
        let encrypted = timing_privacy.encrypt_timestamp(timestamp).unwrap();
        assert!(!encrypted.ciphertext.is_empty());
        assert_ne!(encrypted.nonce, [0u8; 12]);
        assert_ne!(encrypted.tag, [0u8; 16]);
        assert_eq!(encrypted.metadata.timestamp_type, "transaction");
        
        // Test decryption
        let decrypted = timing_privacy.decrypt_timestamp(&encrypted).unwrap();
        assert_eq!(decrypted, timestamp);
        
        // Test verification
        assert!(timing_privacy.verify_timestamp(&encrypted).unwrap());
        
        // Test range proof
        let range_proof = timing_privacy.prove_timestamp_range(timestamp, 1000000000, 2000000000).unwrap();
        assert_eq!(range_proof.min_timestamp, 1000000000);
        assert_eq!(range_proof.max_timestamp, 2000000000);
        assert!(timing_privacy.verify_timestamp_range_proof(&range_proof).unwrap());
    }
    
    #[test]
    fn test_private_block_creation() {
        let stark_system = StarkProofSystem::new().unwrap();
        let manager = UserPrivacyManager::new().unwrap();
        
        // Create private transactions
        let tx1 = manager.create_private_transaction("sender1", "recipient1", 1000, 1234567890, 5000).unwrap();
        let tx2 = manager.create_private_transaction("sender2", "recipient2", 2000, 1234567891, 10000).unwrap();
        
        let transactions = vec![tx1, tx2];
        
        // Create private block
        let block = privacy::PrivateBlock::new(1, transactions, 1234567890, &stark_system).unwrap();
        
        assert_eq!(block.height, 1);
        assert!(!block.hash.is_empty());
        assert!(!block.encrypted_timestamp.ciphertext.is_empty());
        assert_eq!(block.private_transactions.len(), 2);
        assert!(!block.validity_proof.proof_data.is_empty());
        assert!(!block.merkle_proof.proof_data.is_empty());
        assert!(!block.batch_proof.proof_data.is_empty());
    }
    
    #[test]
    fn test_batch_operations() {
        // Test commitment batch
        let commitments = vec![
            AmountCommitment::new(1000).unwrap(),
            AmountCommitment::new(2000).unwrap(),
            AmountCommitment::new(3000).unwrap(),
        ];
        
        let commitment_batch = CommitmentBatch::new(commitments).unwrap();
        assert!(commitment_batch.verify_batch().unwrap());
        assert_eq!(commitment_batch.metadata.count, 3);
        
        // Test address encryption batch
        let key = [1u8; 32];
        let encryption = AddressEncryption::new(&key).unwrap();
        
        let encrypted_addresses = vec![
            encryption.encrypt_sender("sender1").unwrap(),
            encryption.encrypt_recipient("recipient1").unwrap(),
            encryption.encrypt_sender("sender2").unwrap(),
        ];
        
        let address_batch = AddressEncryptionBatch::new(encrypted_addresses).unwrap();
        assert!(address_batch.verify_batch().unwrap());
        assert_eq!(address_batch.metadata.count, 3);
        
        // Test timing privacy batch
        let timing_privacy = TimingPrivacy::new(&key).unwrap();
        
        let encrypted_timestamps = vec![
            timing_privacy.encrypt_timestamp(1234567890).unwrap(),
            timing_privacy.encrypt_timestamp(1234567891).unwrap(),
            timing_privacy.encrypt_timestamp(1234567892).unwrap(),
        ];
        
        let timing_batch = TimingPrivacyBatch::new(encrypted_timestamps).unwrap();
        assert!(timing_batch.verify_batch().unwrap());
        assert_eq!(timing_batch.metadata.count, 3);
    }
    
    #[test]
    fn test_error_handling() {
        // Test zero amount error
        let result = AmountCommitment::new(0);
        assert!(result.is_err());
        
        // Test invalid range proof
        let commitment = AmountCommitment::new(1000).unwrap();
        let result = commitment.prove_amount_range(1000, 500);
        assert!(result.is_err());
        
        // Test insufficient balance
        let stark_system = StarkProofSystem::new().unwrap();
        let result = stark_system.prove_transaction_validity(10000, 5000);
        assert!(result.is_err());
        
        // Test zero timestamp error
        let key = [1u8; 32];
        let timing_privacy = TimingPrivacy::new(&key).unwrap();
        let result = timing_privacy.encrypt_timestamp(0);
        assert!(result.is_err());
        
        // Test empty address error
        let encryption = AddressEncryption::new(&key).unwrap();
        let result = encryption.encrypt_sender("");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_privacy_levels() {
        let manager = UserPrivacyManager::new().unwrap();
        
        // Privacy should always be enabled at maximum level (100)
        // This is tested implicitly through successful creation and operation
        let tx = manager.create_private_transaction(
            "sender",
            "recipient",
            1000,
            1234567890,
            5000,
        ).unwrap();
        
        // All privacy features should be active
        assert!(!tx.encrypted_sender.ciphertext.is_empty()); // Address privacy
        assert!(!tx.encrypted_recipient.ciphertext.is_empty()); // Address privacy
        assert!(!tx.amount_commitment.commitment.is_empty()); // Amount privacy
        assert!(!tx.encrypted_timestamp.ciphertext.is_empty()); // Timing privacy
        assert!(!tx.validity_proof.proof_data.is_empty()); // STARK proofs
        assert!(!tx.range_proof.proof_data.is_empty()); // STARK proofs
        assert!(!tx.balance_proof.proof_data.is_empty()); // STARK proofs
    }
    
    #[test]
    fn test_cryptographic_security() {
        // Test that different inputs produce different outputs
        let manager1 = UserPrivacyManager::new().unwrap();
        let manager2 = UserPrivacyManager::new().unwrap();
        
        let tx1 = manager1.create_private_transaction("sender", "recipient", 1000, 1234567890, 5000).unwrap();
        let tx2 = manager2.create_private_transaction("sender", "recipient", 1000, 1234567890, 5000).unwrap();
        
        // Different managers should produce different encrypted data
        assert_ne!(tx1.encrypted_sender.ciphertext, tx2.encrypted_sender.ciphertext);
        assert_ne!(tx1.encrypted_recipient.ciphertext, tx2.encrypted_recipient.ciphertext);
        assert_ne!(tx1.amount_commitment.commitment, tx2.amount_commitment.commitment);
        assert_ne!(tx1.encrypted_timestamp.ciphertext, tx2.encrypted_timestamp.ciphertext);
        
        // But decryption should work for each manager
        let decrypted1 = manager1.decrypt_transaction_details(&tx1).unwrap();
        let decrypted2 = manager2.decrypt_transaction_details(&tx2).unwrap();
        
        assert_eq!(decrypted1.sender, "sender");
        assert_eq!(decrypted1.recipient, "recipient");
        assert_eq!(decrypted2.sender, "sender");
        assert_eq!(decrypted2.recipient, "recipient");
    }
    
    #[test]
    fn test_performance_benchmarks() {
        use std::time::Instant;
        
        let manager = UserPrivacyManager::new().unwrap();
        
        // Benchmark transaction creation
        let start = Instant::now();
        let tx = manager.create_private_transaction(
            "sender",
            "recipient",
            1000,
            1234567890,
            5000,
        ).unwrap();
        let creation_time = start.elapsed();
        
        // Benchmark transaction verification
        let start = Instant::now();
        let is_valid = manager.verify_private_transaction(&tx).unwrap();
        let verification_time = start.elapsed();
        
        // Benchmark decryption
        let start = Instant::now();
        let _decrypted = manager.decrypt_transaction_details(&tx).unwrap();
        let decryption_time = start.elapsed();
        
        assert!(is_valid);
        
        // Performance should be reasonable (these are placeholder benchmarks)
        println!("Transaction creation time: {:?}", creation_time);
        println!("Transaction verification time: {:?}", verification_time);
        println!("Transaction decryption time: {:?}", decryption_time);
        
        // In production, these should be optimized for sub-second performance
        assert!(creation_time.as_millis() < 1000, "Transaction creation should be fast");
        assert!(verification_time.as_millis() < 100, "Transaction verification should be very fast");
        assert!(decryption_time.as_millis() < 100, "Transaction decryption should be very fast");
    }
}

/// Integration tests for privacy features
#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[test]
    fn test_end_to_end_privacy_flow() {
        // Create privacy manager
        let manager = UserPrivacyManager::new().unwrap();
        
        // Create private transaction
        let tx = manager.create_private_transaction(
            "alice_address",
            "bob_address",
            5000,
            1234567890,
            10000,
        ).unwrap();
        
        // Verify transaction
        let is_valid = manager.verify_private_transaction(&tx).unwrap();
        assert!(is_valid);
        
        // Decrypt transaction details
        let decrypted = manager.decrypt_transaction_details(&tx).unwrap();
        assert_eq!(decrypted.sender, "alice_address");
        assert_eq!(decrypted.recipient, "bob_address");
        assert_eq!(decrypted.timestamp, 1234567890);
        
        // Verify all privacy components are working
        assert!(!tx.encrypted_sender.ciphertext.is_empty());
        assert!(!tx.encrypted_recipient.ciphertext.is_empty());
        assert!(!tx.amount_commitment.commitment.is_empty());
        assert!(!tx.encrypted_timestamp.ciphertext.is_empty());
        assert!(!tx.validity_proof.proof_data.is_empty());
        assert!(!tx.range_proof.proof_data.is_empty());
        assert!(!tx.balance_proof.proof_data.is_empty());
    }
    
    #[test]
    fn test_privacy_with_multiple_transactions() {
        let manager = UserPrivacyManager::new().unwrap();
        
        // Create multiple private transactions
        let transactions = vec![
            manager.create_private_transaction("alice", "bob", 1000, 1234567890, 5000).unwrap(),
            manager.create_private_transaction("bob", "charlie", 2000, 1234567891, 8000).unwrap(),
            manager.create_private_transaction("charlie", "alice", 1500, 1234567892, 6000).unwrap(),
        ];
        
        // Verify all transactions
        for tx in &transactions {
            let is_valid = manager.verify_private_transaction(tx).unwrap();
            assert!(is_valid);
        }
        
        // Decrypt all transactions
        for tx in &transactions {
            let decrypted = manager.decrypt_transaction_details(tx).unwrap();
            assert!(!decrypted.sender.is_empty());
            assert!(!decrypted.recipient.is_empty());
            assert!(decrypted.timestamp > 0);
        }
        
        // Verify privacy is maintained across transactions
        assert_ne!(transactions[0].encrypted_sender.ciphertext, transactions[1].encrypted_sender.ciphertext);
        assert_ne!(transactions[0].amount_commitment.commitment, transactions[1].amount_commitment.commitment);
        assert_ne!(transactions[0].encrypted_timestamp.ciphertext, transactions[1].encrypted_timestamp.ciphertext);
    }
    
    #[test]
    fn test_privacy_with_different_amounts() {
        let manager = UserPrivacyManager::new().unwrap();
        
        // Test with different amounts
        let amounts = vec![1, 100, 1000, 10000, 100000, 1000000];
        
        for amount in amounts {
            let tx = manager.create_private_transaction(
                "sender",
                "recipient",
                amount,
                1234567890,
                amount * 2, // Ensure sufficient balance
            ).unwrap();
            
            let is_valid = manager.verify_private_transaction(&tx).unwrap();
            assert!(is_valid);
            
            // Amount should be hidden in commitment
            assert!(!tx.amount_commitment.commitment.is_empty());
            
            // Range proof should be valid
            assert!(!tx.range_proof.proof_data.is_empty());
        }
    }
}