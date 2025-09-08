// C0DL3 Standard Transaction Fees in HEAT
// Based on current fwei configuration

fn main() {
    println!("=== C0DL3 Standard Transaction Fees in HEAT ===");
    
    // Current C0DL3 configuration
    let default_gas_price_fwei = 1u64;  // From FuegoGasConfig default
    let standard_gas_limit = 21000u64;  // Standard transaction
    
    // Convert to wei and HEAT
    let gas_price_wei = default_gas_price_fwei as u128 * 1_000_000_000_000_000u128;
    let gas_price_heat = gas_price_wei as f64 / 1_000_000_000_000_000_000.0;
    
    // Calculate transaction cost
    let txn_cost_wei = gas_price_wei * standard_gas_limit as u128;
    let txn_cost_fwei = default_gas_price_fwei * standard_gas_limit;
    let txn_cost_heat = txn_cost_wei as f64 / 1_000_000_000_000_000_000.0;
    
    println!("Current C0DL3 Configuration:");
    println!("Gas Price: {} fwei", default_gas_price_fwei);
    println!("Gas Price: {} HEAT per gas unit", gas_price_heat);
    println!("Gas Limit: {} gas", standard_gas_limit);
    println!();
    
    println!("Standard Transaction Cost:");
    println!("Total Cost: {} fwei", txn_cost_fwei);
    println!("Total Cost: {} HEAT", txn_cost_heat);
    println!();
    
    // Different gas price scenarios
    println!("=== Different Gas Price Scenarios ===");
    let gas_prices = vec![1u64, 5u64, 10u64, 20u64, 50u64, 100u64];
    
    for price in &gas_prices {
        let price_wei = *price as u128 * 1_000_000_000_000_000u128;
        let price_heat = price_wei as f64 / 1_000_000_000_000_000_000.0;
        let txn_cost = price_wei * standard_gas_limit as u128;
        let txn_cost_heat = txn_cost as f64 / 1_000_000_000_000_000_000.0;
        
        println!("{} fwei gas price = {} HEAT per transaction", price, txn_cost_heat);
    }
    
    println!();
    
    // Compare with ETH
    println!("=== Comparison with ETH ===");
    let eth_gas_price_gwei = 20u64;
    let eth_gas_price_wei = eth_gas_price_gwei * 1_000_000_000u64;
    let eth_txn_cost_wei = eth_gas_price_wei * standard_gas_limit;
    let eth_txn_cost_eth = eth_txn_cost_wei as f64 / 1_000_000_000_000_000_000.0;
    
    println!("ETH (20 Gwei): {} ETH per transaction", eth_txn_cost_eth);
    println!("C0DL3 (1 fwei): {} HEAT per transaction", txn_cost_heat);
    println!("Ratio: C0DL3 is {}x more expensive than ETH", txn_cost_heat / eth_txn_cost_eth);
    
    println!();
    
    // Practical recommendations
    println!("=== Practical Recommendations ===");
    println!("For reasonable transaction fees:");
    println!("- Low: 0.1 fwei = {} HEAT per transaction", 0.1 * standard_gas_limit as f64 / 1000.0);
    println!("- Medium: 0.5 fwei = {} HEAT per transaction", 0.5 * standard_gas_limit as f64 / 1000.0);
    println!("- High: 1 fwei = {} HEAT per transaction", 1.0 * standard_gas_limit as f64 / 1000.0);
    println!("- Premium: 2 fwei = {} HEAT per transaction", 2.0 * standard_gas_limit as f64 / 1000.0);
    
    println!();
    println!("=== Test Completed Successfully ===");
}
