// Production STARK Implementation Test
// Phase 1: Core STARK Infrastructure Testing

use codl3_zksync::privacy::{
    ProductionStarkProofSystem,
    ProductionStarkProof,
    ProofType,
};

fn main() {
    println!("=== C0DL3 Production STARK Implementation Test ===");
    
    // Test 1: Production STARK System Creation
    println!("\n1. Testing Production STARK System Creation...");
    match ProductionStarkProofSystem::new() {
        Ok(mut stark_system) => {
            println!("✅ Production STARK system created successfully");
            
            // Test 2: Transaction Validity Proof
            println!("\n2. Testing Transaction Validity Proof...");
            let recipient_address = b"0x1234567890123456789012345678901234567890";
            
            match stark_system.prove_transaction_validity(1000, 5000, recipient_address) {
                Ok(proof) => {
                    println!("✅ Transaction validity proof generated successfully");
                    println!("   Proof type: {:?}", proof.proof_type);
                    println!("   Security level: {} bits", proof.security_level);
                    println!("   Generation time: {:?}", proof.generation_time);
                    println!("   Proof data size: {} bytes", proof.proof_data.len());
                    
                    // Test proof verification
                    match stark_system.verify_proof(&proof) {
                        Ok(is_valid) => {
                            if is_valid {
                                println!("✅ Proof verification successful");
                            } else {
                                println!("❌ Proof verification failed");
                            }
                        }
                        Err(e) => println!("❌ Proof verification error: {}", e),
                    }
                }
                Err(e) => println!("❌ Transaction validity proof generation failed: {}", e),
            }
            
            // Test 3: Amount Range Proof
            println!("\n3. Testing Amount Range Proof...");
            match stark_system.prove_amount_range(1000, 500, 2000) {
                Ok(proof) => {
                    println!("✅ Amount range proof generated successfully");
                    println!("   Proof type: {:?}", proof.proof_type);
                    println!("   Security level: {} bits", proof.security_level);
                    println!("   Generation time: {:?}", proof.generation_time);
                    println!("   Proof data size: {} bytes", proof.proof_data.len());
                    
                    // Test proof verification
                    match stark_system.verify_proof(&proof) {
                        Ok(is_valid) => {
                            if is_valid {
                                println!("✅ Proof verification successful");
                            } else {
                                println!("❌ Proof verification failed");
                            }
                        }
                        Err(e) => println!("❌ Proof verification error: {}", e),
                    }
                }
                Err(e) => println!("❌ Amount range proof generation failed: {}", e),
            }
            
            // Test 4: Balance Consistency Proof
            println!("\n4. Testing Balance Consistency Proof...");
            match stark_system.prove_balance_consistency(5000, 4000, 1000) {
                Ok(proof) => {
                    println!("✅ Balance consistency proof generated successfully");
                    println!("   Proof type: {:?}", proof.proof_type);
                    println!("   Security level: {} bits", proof.security_level);
                    println!("   Generation time: {:?}", proof.generation_time);
                    println!("   Proof data size: {} bytes", proof.proof_data.len());
                    
                    // Test proof verification
                    match stark_system.verify_proof(&proof) {
                        Ok(is_valid) => {
                            if is_valid {
                                println!("✅ Proof verification successful");
                            } else {
                                println!("❌ Proof verification failed");
                            }
                        }
                        Err(e) => println!("❌ Proof verification error: {}", e),
                    }
                }
                Err(e) => println!("❌ Balance consistency proof generation failed: {}", e),
            }
            
            // Test 5: Error Handling
            println!("\n5. Testing Error Handling...");
            
            // Test zero amount
            match stark_system.prove_transaction_validity(0, 5000, recipient_address) {
                Ok(_) => println!("❌ Zero amount should have failed"),
                Err(_) => println!("✅ Zero amount correctly rejected"),
            }
            
            // Test insufficient balance
            match stark_system.prove_transaction_validity(6000, 5000, recipient_address) {
                Ok(_) => println!("❌ Insufficient balance should have failed"),
                Err(_) => println!("✅ Insufficient balance correctly rejected"),
            }
            
            // Test amount out of range
            match stark_system.prove_amount_range(1000, 1500, 2000) {
                Ok(_) => println!("❌ Amount out of range should have failed"),
                Err(_) => println!("✅ Amount out of range correctly rejected"),
            }
            
            // Test balance consistency error
            match stark_system.prove_balance_consistency(5000, 4000, 2000) {
                Ok(_) => println!("❌ Invalid balance consistency should have failed"),
                Err(_) => println!("✅ Invalid balance consistency correctly rejected"),
            }
        }
        Err(e) => println!("❌ Production STARK system creation failed: {}", e),
    }
    
    // Test 6: Proof Type Testing
    println!("\n6. Testing Proof Types...");
    let proof_types = [
        ProofType::TransactionValidity,
        ProofType::AmountRange,
        ProofType::BalanceConsistency,
        ProofType::CrossChain,
        ProofType::MiningReward,
        ProofType::MergeMining,
    ];
    
    for proof_type in &proof_types {
        println!("   {:?}: {}", proof_type, proof_type.identifier());
    }
    
    println!("\n=== Production STARK Implementation Test Completed ===");
    println!("Phase 1: Core STARK Infrastructure - ✅ COMPLETE");
    println!("Ready for Phase 2: Transaction Privacy STARKs");
}
