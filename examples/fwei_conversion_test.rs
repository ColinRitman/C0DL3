// Fwei to HEAT conversion demonstration
// Adjusted for HEAT's larger supply (69 trillion vs ETH's billions)
// 1 fwei = 10^12 wei (more practical for HEAT ecosystem)

fn main() {
    println!("=== Fwei to HEAT Conversion Test (HEAT-Optimized) ===");
    println!("1 fwei = 10^12 wei (optimized for HEAT's 69T supply)");
    println!("20 Gwei = 0.02 fwei (more practical scale)");
    
    // Test 0.02 fwei conversion (equivalent to 20 Gwei)
    let gas_price_fwei = 0.02f64;
    
    // Convert fwei to wei (10^12 wei per fwei)
    let gas_price_wei = (gas_price_fwei * 1_000_000_000_000.0) as u64;
    
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
    println!("0.02 fwei to wei: {}", (0.02 * 1_000_000_000_000.0) as u64);
    println!("0.02 fwei to HEAT: {}", 0.02 / 1_000_000.0);
    println!("0.00000002 HEAT to fwei: {}", 0.00000002 * 1_000_000.0);
    
    // Test different gas prices
    println!("\n=== Different Gas Prices ===");
    let gas_prices = vec![0.01f64, 0.02f64, 0.05f64, 0.1f64, 0.2f64, 0.5f64];
    
    for price in &gas_prices {
        let heat_amount = *price / 1_000_000.0;
        println!("{} fwei = {} HEAT", price, heat_amount);
    }
    
    // Test transaction costs with different gas prices
    println!("\n=== Transaction Costs ===");
    for price in &gas_prices {
        let gas_price_wei = (*price * 1_000_000_000_000.0) as u64;
        let total_cost_wei = gas_price_wei * gas_limit;
        let total_cost_heat = total_cost_wei as f64 / 1_000_000_000_000_000_000.0;
        println!("{} fwei gas price = {} HEAT per transaction", price, total_cost_heat);
    }
    
    // Compare with Gwei and show HEAT supply context
    println!("\n=== Gwei vs Fwei Comparison ===");
    println!("20 Gwei = {} wei", 20u64 * 1_000_000_000u64);
    println!("0.02 fwei = {} wei", (0.02 * 1_000_000_000_000.0) as u64);
    println!("Ratio: 1 fwei = {} Gwei", 1_000_000_000_000u64 / 1_000_000_000u64);
    
    println!("\n=== HEAT Supply Context ===");
    println!("HEAT Supply: ~69 trillion tokens");
    println!("ETH Supply: ~120 million tokens");
    println!("HEAT/ETH Ratio: ~575,000x");
    println!("This justifies fwei being 1000x larger than Gwei");
    
    println!("\n=== Test Completed Successfully ===");
}
