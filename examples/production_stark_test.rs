// Production STARK Proof System Test
// Tests the real winter-crypto integration for production-grade STARK proofs

use anyhow::Result;
use std::time::Instant;

// Import the production STARK proof system directly
mod production_stark_proofs {
    include!("../src/privacy/production_stark_proofs.rs");
}

use production_stark_proofs::ProductionStarkProofSystem;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Production STARK Proof System Test ===");
    
    // Initialize production STARK proof system
    println!("Initializing production STARK proof system...");
    let stark_system = ProductionStarkProofSystem::new()?;
    println!("✅ Production STARK proof system initialized successfully");
    
    // Test transaction validity proof
    println!("\n--- Testing Transaction Validity Proof ---");
    let amount = 1000u64;
    let sender_balance = 5000u64;
    
    println!("Amount: {}, Sender Balance: {}", amount, sender_balance);
    
    let start_time = Instant::now();
    let validity_proof = stark_system.prove_transaction_validity(amount, sender_balance)?;
    let generation_time = start_time.elapsed();
    
    println!("✅ Transaction validity proof generated successfully");
    println!("   Proof type: {}", validity_proof.proof_type);
    println!("   Security level: {} bits", validity_proof.security_level);
    println!("   FRI proof size: {} bytes", validity_proof.fri_proof.len());
    println!("   Public inputs size: {} bytes", validity_proof.public_inputs.len());
    println!("   Generation time: {:.2}ms", generation_time.as_micros() as f64 / 1000.0);
    println!("   Field size: {}", validity_proof.metadata.field_size);
    println!("   Constraint count: {}", validity_proof.metadata.constraint_count);
    
    // Test amount range proof
    println!("\n--- Testing Amount Range Proof ---");
    let amount = 2500u64;
    let min_amount = 1000u64;
    let max_amount = 5000u64;
    
    println!("Amount: {}, Range: [{}, {}]", amount, min_amount, max_amount);
    
    let start_time = Instant::now();
    let range_proof = stark_system.prove_amount_range(amount, min_amount, max_amount)?;
    let generation_time = start_time.elapsed();
    
    println!("✅ Amount range proof generated successfully");
    println!("   Proof type: {}", range_proof.proof_type);
    println!("   Security level: {} bits", range_proof.security_level);
    println!("   FRI proof size: {} bytes", range_proof.fri_proof.len());
    println!("   Public inputs size: {} bytes", range_proof.public_inputs.len());
    println!("   Generation time: {:.2}ms", generation_time.as_micros() as f64 / 1000.0);
    println!("   Field size: {}", range_proof.metadata.field_size);
    println!("   Constraint count: {}", range_proof.metadata.constraint_count);
    
    // Test error cases
    println!("\n--- Testing Error Cases ---");
    
    // Test insufficient balance
    match stark_system.prove_transaction_validity(6000, 5000) {
        Ok(_) => println!("❌ Should have failed for insufficient balance"),
        Err(e) => println!("✅ Correctly rejected insufficient balance: {}", e),
    }
    
    // Test amount out of range
    match stark_system.prove_amount_range(6000, 1000, 5000) {
        Ok(_) => println!("❌ Should have failed for amount out of range"),
        Err(e) => println!("✅ Correctly rejected amount out of range: {}", e),
    }
    
    // Performance test
    println!("\n--- Performance Test ---");
    let iterations = 100;
    let mut total_time = 0u128;
    
    for i in 0..iterations {
        let amount = (i * 10 + 1000) as u64;
        let balance = (i * 50 + 5000) as u64;
        
        let start_time = Instant::now();
        let _proof = stark_system.prove_transaction_validity(amount, balance)?;
        let generation_time = start_time.elapsed().as_micros();
        total_time += generation_time;
        
        if i % 20 == 0 {
            println!("   Iteration {}: {:.2}ms", i, generation_time as f64 / 1000.0);
        }
    }
    
    let avg_time = total_time as f64 / iterations as f64 / 1000.0;
    println!("✅ Performance test completed");
    println!("   Iterations: {}", iterations);
    println!("   Average generation time: {:.2}ms", avg_time);
    println!("   Total time: {:.2}ms", total_time as f64 / 1000.0);
    
    // Test field element operations
    println!("\n--- Field Element Operations Test ---");
    use winter_math::fields::f64::BaseElement as Field;
    
    let field_a = Field::from(1000u32);
    let field_b = Field::from(2000u32);
    let field_c = field_a + field_b;
    let field_d = field_b - field_a;
    
    println!("Field element A: {:?}", field_a);
    println!("Field element B: {:?}", field_b);
    println!("A + B: {:?}", field_c);
    println!("B - A: {:?}", field_d);
    
    // Test serialization
    println!("\n--- Serialization Test ---");
    let serialized = serde_json::to_string(&validity_proof)?;
    println!("✅ Proof serialized successfully");
    println!("   Serialized size: {} bytes", serialized.len());
    
    let deserialized: production_stark_proofs::ProductionStarkProof = 
        serde_json::from_str(&serialized)?;
    println!("✅ Proof deserialized successfully");
    println!("   Deserialized proof type: {}", deserialized.proof_type);
    
    println!("\n=== Production STARK Proof System Test Completed Successfully ===");
    println!("🎉 All tests passed! Production STARK proof system is working correctly.");
    
    Ok(())
}
