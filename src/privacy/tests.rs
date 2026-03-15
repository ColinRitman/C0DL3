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
        let _manager = UserPrivacyManager::new().unwrap();
    }

    #[test]
    fn test_private_transaction_creation() {
        let mut manager = UserPrivacyManager::new().unwrap();
        let tx = manager.create_private_transaction(
            "0xsender_address_0123",
            "recipient_address_456",
            1000,
            1234567890,
            5000,
            None,
        ).unwrap();

        assert!(!tx.hash.is_empty(), "Transaction hash should not be empty");
        assert!(!tx.encrypted_sender.ciphertext.is_empty(), "Encrypted sender should not be empty");
        assert!(!tx.encrypted_recipient.ciphertext.is_empty(), "Encrypted recipient should not be empty");
        assert!(!tx.amount_commitment.commitment.is_empty(), "Amount commitment should not be empty");
        assert!(!tx.encrypted_timestamp.ciphertext.is_empty(), "Encrypted timestamp should not be empty");
        assert!(!tx.range_proof_bytes.is_empty(), "Range proof should not be empty");
    }

    #[test]
    fn test_transaction_verification() {
        let mut manager = UserPrivacyManager::new().unwrap();
        let tx = manager.create_private_transaction(
            "0xsender_addr_000000",
            "0xrecipient_addr_0000",
            1000,
            1234567890,
            5000,
            None,
        ).unwrap();

        let is_valid = manager.verify_private_transaction(&tx).unwrap();
        assert!(is_valid, "Valid transaction should pass verification");
    }

    #[test]
    fn test_transaction_decryption() {
        let mut manager = UserPrivacyManager::new().unwrap();
        let tx = manager.create_private_transaction(
            "0xtest_sender_0000000",
            "0xtest_recipient_0000",
            1000,
            1234567890,
            5000,
            None,
        ).unwrap();

        let decrypted = manager.decrypt_transaction_details(&tx).unwrap();
        assert_eq!(decrypted.sender, "0xtest_sender_0000000");
        assert_eq!(decrypted.recipient, "0xtest_recipient_0000");
        assert_eq!(decrypted.timestamp, 1234567890);
    }

    #[test]
    fn test_amount_commitments() {
        let commitment = AmountCommitment::new(1000).unwrap();

        // Commitment bytes should be non-empty (Pedersen commitment)
        assert!(!commitment.commitment.is_empty());
        assert_eq!(commitment.commitment.len(), 32); // CompressedRistretto

        // Range proof
        let (proof_bytes, _) = commitment.prove_range(1000, 64).unwrap();
        assert!(!proof_bytes.is_empty());
    }

    #[test]
    fn test_address_encryption() {
        let key = [1u8; 32];
        let mut encryption = AddressEncryption::new(&key).unwrap();

        // Test sender encryption
        let encrypted_sender = encryption.encrypt_sender("0xsender_addr_000000").unwrap();
        assert!(!encrypted_sender.ciphertext.is_empty());
        assert_ne!(encrypted_sender.nonce, [0u8; 12]);
        assert_ne!(encrypted_sender.tag, [0u8; 16]);
        assert_eq!(encrypted_sender.metadata.address_type, "sender");

        // Test recipient encryption
        let encrypted_recipient = encryption.encrypt_recipient("0xrecipient_addr_0000").unwrap();
        assert!(!encrypted_recipient.ciphertext.is_empty());
        assert_eq!(encrypted_recipient.metadata.address_type, "recipient");

        // Test decryption
        let decrypted_sender = encryption.decrypt_address(&encrypted_sender).unwrap();
        assert_eq!(decrypted_sender, "0xsender_addr_000000");

        let decrypted_recipient = encryption.decrypt_address(&encrypted_recipient).unwrap();
        assert_eq!(decrypted_recipient, "0xrecipient_addr_0000");

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
    }

    #[test]
    fn test_cryptographic_security() {
        let mut manager1 = UserPrivacyManager::new().unwrap();
        let mut manager2 = UserPrivacyManager::new().unwrap();

        let tx1 = manager1.create_private_transaction("0xsender_addr_000000", "0xrecipient_addr_0000", 1000, 1234567890, 5000, None).unwrap();
        let tx2 = manager2.create_private_transaction("0xsender_addr_000000", "0xrecipient_addr_0000", 1000, 1234567890, 5000, None).unwrap();

        // Different managers (different keys) produce different encrypted data
        assert_ne!(tx1.encrypted_sender.ciphertext, tx2.encrypted_sender.ciphertext);
        assert_ne!(tx1.encrypted_recipient.ciphertext, tx2.encrypted_recipient.ciphertext);
        assert_ne!(tx1.amount_commitment.commitment, tx2.amount_commitment.commitment);
        assert_ne!(tx1.encrypted_timestamp.ciphertext, tx2.encrypted_timestamp.ciphertext);

        // Decryption should work for each manager
        let decrypted1 = manager1.decrypt_transaction_details(&tx1).unwrap();
        let decrypted2 = manager2.decrypt_transaction_details(&tx2).unwrap();

        assert_eq!(decrypted1.sender, "0xsender_addr_000000");
        assert_eq!(decrypted1.recipient, "0xrecipient_addr_0000");
        assert_eq!(decrypted2.sender, "0xsender_addr_000000");
        assert_eq!(decrypted2.recipient, "0xrecipient_addr_0000");
    }

    #[test]
    fn test_performance_benchmarks() {
        use std::time::Instant;

        let mut manager = UserPrivacyManager::new().unwrap();

        let start = Instant::now();
        let tx = manager.create_private_transaction(
            "0xsender_addr_000000", "0xrecipient_addr_0000", 1000, 1234567890, 5000, None,
        ).unwrap();
        let creation_time = start.elapsed();

        let start = Instant::now();
        let is_valid = manager.verify_private_transaction(&tx).unwrap();
        let verification_time = start.elapsed();

        let start = Instant::now();
        let _decrypted = manager.decrypt_transaction_details(&tx).unwrap();
        let decryption_time = start.elapsed();

        assert!(is_valid);

        println!("Transaction creation time: {:?}", creation_time);
        println!("Transaction verification time: {:?}", verification_time);
        println!("Transaction decryption time: {:?}", decryption_time);

        assert!(creation_time.as_millis() < 5000, "Transaction creation should complete in 5s");
        assert!(verification_time.as_millis() < 1000, "Verification should be fast");
        assert!(decryption_time.as_millis() < 100, "Decryption should be very fast");
    }
}

/// Integration tests for privacy features
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_end_to_end_privacy_flow() {
        let mut manager = UserPrivacyManager::new().unwrap();

        let tx = manager.create_private_transaction(
            "0xalice_address_00000", "0xbob_address_0000000", 5000, 1234567890, 10000, None,
        ).unwrap();

        let is_valid = manager.verify_private_transaction(&tx).unwrap();
        assert!(is_valid);

        let decrypted = manager.decrypt_transaction_details(&tx).unwrap();
        assert_eq!(decrypted.sender, "0xalice_address_00000");
        assert_eq!(decrypted.recipient, "0xbob_address_0000000");
        assert_eq!(decrypted.timestamp, 1234567890);

        assert!(!tx.encrypted_sender.ciphertext.is_empty());
        assert!(!tx.encrypted_recipient.ciphertext.is_empty());
        assert!(!tx.amount_commitment.commitment.is_empty());
        assert!(!tx.encrypted_timestamp.ciphertext.is_empty());
        assert!(!tx.range_proof_bytes.is_empty());
    }

    #[test]
    fn test_privacy_with_multiple_transactions() {
        let mut manager = UserPrivacyManager::new().unwrap();

        let transactions = vec![
            manager.create_private_transaction("0xalice_addr_0000000", "0xbob_addr_000000000", 1000, 1234567890, 5000, None).unwrap(),
            manager.create_private_transaction("0xbob_addr_000000000", "0xcharlie_addr_00000", 2000, 1234567891, 8000, None).unwrap(),
            manager.create_private_transaction("0xcharlie_addr_00000", "0xalice_addr_0000000", 1500, 1234567892, 6000, None).unwrap(),
        ];

        for tx in &transactions {
            assert!(manager.verify_private_transaction(tx).unwrap());
        }

        for tx in &transactions {
            let decrypted = manager.decrypt_transaction_details(tx).unwrap();
            assert!(!decrypted.sender.is_empty());
            assert!(!decrypted.recipient.is_empty());
            assert!(decrypted.timestamp > 0);
        }

        assert_ne!(transactions[0].encrypted_sender.ciphertext, transactions[1].encrypted_sender.ciphertext);
        assert_ne!(transactions[0].amount_commitment.commitment, transactions[1].amount_commitment.commitment);
        assert_ne!(transactions[0].encrypted_timestamp.ciphertext, transactions[1].encrypted_timestamp.ciphertext);
    }

    #[test]
    fn test_privacy_with_different_amounts() {
        let mut manager = UserPrivacyManager::new().unwrap();
        let amounts = vec![1u64, 100, 1000, 10000, 100000, 1000000];

        for amount in amounts {
            let tx = manager.create_private_transaction(
                "0xsender_addr_000000", "0xrecipient_addr_0000", amount, 1234567890, amount * 2, None,
            ).unwrap();

            assert!(manager.verify_private_transaction(&tx).unwrap());
            assert!(!tx.amount_commitment.commitment.is_empty());
            assert!(!tx.range_proof_bytes.is_empty());
        }
    }
}
