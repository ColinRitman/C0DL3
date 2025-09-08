// Fwei to HEAT conversion demonstration
// Shows how 20 fwei converts to HEAT tokens

fn main() {
    println!("=== Fwei to HEAT Conversion Test ===");
    
    // Test 20 fwei conversion
    let gas_price_fwei = 20u64;
    
    // Convert fwei to wei (same as Gwei conversion)
    let gas_price_wei = gas_price_fwei * 1_000_000_000u64;
    
    // Convert wei to HEAT (18 decimal places)
    let gas_price_heat = gas_price_wei as f64 / 1_000_000_000_000_000_000.0;
    
    println!("Gas Price: {} fwei", gas_price_fwei);
    println!("Gas Price in wei: {} wei", gas_price_wei);
    println!("Gas Price in HEAT: {} HEAT", gas_price_heat);
    
    // Test gas configuration
    let gas_limit = 21000u64;
    let transaction_cost_wei = gas_price_wei * gas_limit;
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
    println!("20 fwei to wei: {}", 20u64 * 1_000_000_000u64);
    println!("20 fwei to HEAT: {}", 20.0 / 1_000_000_000.0);
    println!("0.00000002 HEAT to fwei: {}", (0.00000002 * 1_000_000_000.0) as u64);
    
    // Test different gas prices
    println!("\n=== Different Gas Prices ===");
    let gas_prices = vec![1u64, 5u64, 10u64, 20u64, 50u64, 100u64];
    
    for price in &gas_prices {
        let heat_amount = *price as f64 / 1_000_000_000.0;
        println!("{} fwei = {} HEAT", price, heat_amount);
    }
    
    // Test transaction costs with different gas prices
    println!("\n=== Transaction Costs ===");
    for price in &gas_prices {
        let cost_wei = *price * 1_000_000_000u64 * gas_limit;
        let cost_heat = cost_wei as f64 / 1_000_000_000_000_000_000.0;
        println!("{} fwei gas price = {} HEAT per transaction", price, cost_heat);
    }
    
    println!("\n=== Test Completed Successfully ===");
}
