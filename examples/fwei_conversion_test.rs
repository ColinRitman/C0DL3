// Fwei to HEAT conversion demonstration
// Optimized for HEAT's 69T supply with ~1 HEAT per transaction
// 1 fwei = 10^15 wei (optimized for ~1 HEAT per transaction)

fn main() {
    println!("=== Fwei to HEAT Conversion Test (1 HEAT per Transaction) ===");
    println!("1 fwei = 10^15 wei (optimized for ~1 HEAT per transaction)");
    println!("20 Gwei = 0.00002 fwei (more practical scale)");
    
    // Test 0.00002 fwei conversion (equivalent to 20 Gwei)
    let gas_price_fwei = 0.00002f64;
    
    // Convert fwei to wei (10^15 wei per fwei)
    let gas_price_wei = (gas_price_fwei * 1_000_000_000_000_000.0) as u64;
    
    // Convert wei to HEAT (18 decimal places)
    let gas_price_heat = gas_price_wei as f64 / 1_000_000_000_000_000_000.0;
    
    println!("\nGas Price: {} fwei", gas_price_fwei);
    println!("Gas Price in wei: {} wei", gas_price_wei);
    println!("Gas Price in HEAT: {} HEAT", gas_price_heat);
    
    // Test gas configuration
    let gas_limit = 21000u64;
    let transaction_cost_wei = gas_price_wei * gas_limit;
    let transaction_cost_fwei = gas_price_fwei * gas_limit as f64;
    let transaction_cost_heat = transaction_cost_wei as f64 / 1_000_000_000_000_000_000.0;
    
    println!("\n=== Gas Configuration ===");
    println!("Gas Price: {} fwei", gas_price_fwei);
    println!("Gas Limit: {} gas", gas_limit);
    println!("Transaction Cost: {} wei", transaction_cost_wei);
    println!("Transaction Cost: {} fwei", transaction_cost_fwei);
    println!("Transaction Cost: {} HEAT", transaction_cost_heat);
    
    // Test utility functions
    println!("\n=== Utility Functions ===");
    println!("0.00002 fwei to wei: {}", (0.00002 * 1_000_000_000_000_000.0) as u64);
    println!("0.00002 fwei to HEAT: {}", 0.00002 / 1_000.0);
    println!("0.00000002 HEAT to fwei: {}", 0.00000002 * 1_000.0);
    
    // Test different gas prices to find ~1 HEAT per transaction
    println!("\n=== Gas Prices for ~1 HEAT per Transaction ===");
    let gas_prices = vec![0.00001f64, 0.00002f64, 0.00005f64, 0.0001f64, 0.0002f64, 0.0005f64];
    
    for price in &gas_prices {
        let heat_amount = *price / 1_000.0;
        println!("{} fwei = {} HEAT", price, heat_amount);
    }
    
    // Test transaction costs with different gas prices
    println!("\n=== Transaction Costs ===");
    for price in &gas_prices {
        let gas_price_wei = (*price * 1_000_000_000_000_000.0) as u64;
        let total_cost_wei = gas_price_wei * gas_limit;
        let total_cost_heat = total_cost_wei as f64 / 1_000_000_000_000_000_000.0;
        println!("{} fwei gas price = {} HEAT per transaction", price, total_cost_heat);
    }
    
    // Find the gas price that gives ~1 HEAT per transaction
    println!("\n=== Finding ~1 HEAT per Transaction ===");
    let target_heat_per_tx = 1.0;
    let required_gas_price_wei = (target_heat_per_tx * 1_000_000_000_000_000_000.0) / gas_limit as f64;
    let required_gas_price_fwei = required_gas_price_wei / 1_000_000_000_000_000.0;
    
    println!("Target: {} HEAT per transaction", target_heat_per_tx);
    println!("Required gas price: {} fwei", required_gas_price_fwei);
    println!("Required gas price: {} wei", required_gas_price_wei as u64);
    
    // Test the calculated gas price
    let test_gas_price_wei = (required_gas_price_fwei * 1_000_000_000_000_000.0) as u64;
    let test_total_cost_wei = test_gas_price_wei * gas_limit;
    let test_total_cost_heat = test_total_cost_wei as f64 / 1_000_000_000_000_000_000.0;
    
    println!("Test: {} fwei gas price = {} HEAT per transaction", required_gas_price_fwei, test_total_cost_heat);
    
    // Compare with Gwei and show HEAT supply context
    println!("\n=== Gwei vs Fwei Comparison ===");
    println!("20 Gwei = {} wei", 20u64 * 1_000_000_000u64);
    println!("0.00002 fwei = {} wei", (0.00002 * 1_000_000_000_000_000.0) as u64);
    println!("Ratio: 1 fwei = {} Gwei", 1_000_000_000_000_000u64 / 1_000_000_000u64);
    
    println!("\n=== HEAT Supply Context ===");
    println!("HEAT Supply: ~69 trillion tokens");
    println!("ETH Supply: ~120 million tokens");
    println!("HEAT/ETH Ratio: ~575,000x");
    println!("This justifies fwei being 1,000,000x larger than Gwei");
    println!("Target: ~1 HEAT per transaction for practical usage");
    
    println!("\n=== Test Completed Successfully ===");
}
