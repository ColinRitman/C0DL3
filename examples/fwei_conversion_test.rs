// Fwei to HEAT conversion demonstration
// fwei = 3rd decimal atomic unit of HEAT (1 fwei = 0.001 HEAT = 10^15 wei)

fn main() {
    println!("=== Fwei to HEAT Conversion Test (3rd Decimal Atomic Unit) ===");
    println!("1 fwei = 0.001 HEAT = 10^15 wei (3rd decimal atomic unit)");
    println!("420 fwei = 0.420 HEAT");
    
    // Test 420 fwei conversion
    let gas_price_fwei = 420u64;
    
    // Convert fwei to wei (10^15 wei per fwei) - use u128 to avoid overflow
    let gas_price_wei = gas_price_fwei as u128 * 1_000_000_000_000_000u128;
    
    // Convert wei to HEAT (18 decimal places)
    let gas_price_heat = gas_price_wei as f64 / 1_000_000_000_000_000_000.0;
    
    println!("\nGas Price: {} fwei", gas_price_fwei);
    println!("Gas Price in wei: {} wei", gas_price_wei);
    println!("Gas Price in HEAT: {} HEAT", gas_price_heat);
    
    // Test gas configuration
    let gas_limit = 21000u64;
    let transaction_cost_wei = gas_price_wei * gas_limit as u128;
    let transaction_cost_fwei = gas_price_fwei * gas_limit;
    let transaction_cost_heat = transaction_cost_wei as f64 / 1_000_000_000_000_000_000.0;
    
    println!("\n=== Gas Configuration ===");
    println!("Gas Price: {} fwei", gas_price_fwei);
    println!("Gas Limit: {} gas", gas_limit);
    println!("Transaction Cost: {} wei", transaction_cost_wei);
    println!("Transaction Cost: {} fwei", transaction_cost_fwei);
    println!("Transaction Cost: {} HEAT", transaction_cost_heat);
    
    // Test utility functions
    println!("\n=== Utility Functions ===");
    println!("420 fwei to wei: {}", 420u128 * 1_000_000_000_000_000u128);
    println!("420 fwei to HEAT: {}", 420.0 / 1_000.0);
    println!("0.420 HEAT to fwei: {}", (0.420 * 1_000.0) as u64);
    
    // Test different gas prices
    println!("\n=== Different Gas Prices ===");
    let gas_prices = vec![1u64, 10u64, 100u64, 420u64, 1000u64, 2000u64];
    
    for price in &gas_prices {
        let heat_amount = *price as f64 / 1_000.0;
        println!("{} fwei = {} HEAT", price, heat_amount);
    }
    
    // Test transaction costs with different gas prices
    println!("\n=== Transaction Costs ===");
    for price in &gas_prices {
        let gas_price_wei = *price as u128 * 1_000_000_000_000_000u128;
        let total_cost_wei = gas_price_wei * gas_limit as u128;
        let total_cost_heat = total_cost_wei as f64 / 1_000_000_000_000_000_000.0;
        println!("{} fwei gas price = {} HEAT per transaction", price, total_cost_heat);
    }
    
    // Compare with Gwei
    println!("\n=== Gwei vs Fwei Comparison ===");
    println!("20 Gwei = {} wei", 20u64 * 1_000_000_000u64);
    println!("420 fwei = {} wei", 420u128 * 1_000_000_000_000_000u128);
    println!("Ratio: 1 fwei = {} Gwei", 1_000_000_000_000_000u128 / 1_000_000_000u128);
    
    println!("\n=== HEAT Supply Context ===");
    println!("HEAT Supply: ~69 trillion tokens");
    println!("ETH Supply: ~120 million tokens");
    println!("HEAT/ETH Ratio: ~575,000x");
    println!("fwei = 3rd decimal atomic unit of HEAT (1 fwei = 0.001 HEAT)");
    
    println!("\n=== Test Completed Successfully ===");
}
