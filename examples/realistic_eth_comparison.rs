// Realistic ETH vs C0DL3 Transaction Fee Comparison
// ETH gas prices are typically much higher than 20 Gwei

fn main() {
    println!("=== Realistic ETH vs C0DL3 Transaction Fee Comparison ===");
    
    // HEAT pricing
    let heat_price_usd = 0.00001; // $0.00001 per HEAT
    
    // C0DL3 configuration
    let c0dl3_gas_price_fwei = 1.0f64;
    let standard_gas_limit = 21000u64;
    let c0dl3_txn_cost_heat = c0dl3_gas_price_fwei * standard_gas_limit as f64 / 1000.0;
    let c0dl3_txn_cost_usd = c0dl3_txn_cost_heat * heat_price_usd;
    
    println!("C0DL3 Transaction Cost:");
    println!("Gas Price: {} fwei", c0dl3_gas_price_fwei);
    println!("Transaction Cost: {} HEAT = ${:.6}", c0dl3_txn_cost_heat, c0dl3_txn_cost_usd);
    println!();
    
    // Realistic ETH gas prices
    println!("=== Realistic ETH Gas Prices ===");
    let eth_gas_prices = vec![
        (20, "Low"),
        (50, "Medium"),
        (100, "High"),
        (200, "Very High"),
        (500, "Extreme"),
        (1000, "Peak")
    ];
    
    let eth_price_usd = 3000.0; // Assuming $3000 per ETH
    
    for (gwei, label) in &eth_gas_prices {
        // Calculate ETH transaction cost properly
        let gwei_in_eth = *gwei as f64 / 1_000_000_000.0; // Convert Gwei to ETH
        let eth_txn_cost_eth = gwei_in_eth * standard_gas_limit as f64;
        let eth_txn_cost_usd = eth_txn_cost_eth * eth_price_usd;
        
        println!("ETH ({} Gwei - {}): {:.6} ETH = ${:.2} per transaction", 
                gwei, label, eth_txn_cost_eth, eth_txn_cost_usd);
    }
    
    println!();
    
    // Comparison table
    println!("=== Transaction Fee Comparison ===");
    println!("C0DL3 (1 fwei): ${:.6} (0.021 cents)", c0dl3_txn_cost_usd);
    
    let eth_20_gwei_usd = (20.0 / 1_000_000_000.0) * standard_gas_limit as f64 * eth_price_usd;
    let eth_50_gwei_usd = (50.0 / 1_000_000_000.0) * standard_gas_limit as f64 * eth_price_usd;
    let eth_100_gwei_usd = (100.0 / 1_000_000_000.0) * standard_gas_limit as f64 * eth_price_usd;
    let eth_200_gwei_usd = (200.0 / 1_000_000_000.0) * standard_gas_limit as f64 * eth_price_usd;
    
    println!("ETH (20 Gwei):  ${:.2}", eth_20_gwei_usd);
    println!("ETH (50 Gwei):  ${:.2}", eth_50_gwei_usd);
    println!("ETH (100 Gwei): ${:.2}", eth_100_gwei_usd);
    println!("ETH (200 Gwei): ${:.2}", eth_200_gwei_usd);
    
    println!();
    
    // Cost advantage
    println!("=== C0DL3 Cost Advantage ===");
    println!("C0DL3 is {:.0}x cheaper than ETH (20 Gwei)", eth_20_gwei_usd / c0dl3_txn_cost_usd);
    println!("C0DL3 is {:.0}x cheaper than ETH (50 Gwei)", eth_50_gwei_usd / c0dl3_txn_cost_usd);
    println!("C0DL3 is {:.0}x cheaper than ETH (100 Gwei)", eth_100_gwei_usd / c0dl3_txn_cost_usd);
    println!("C0DL3 is {:.0}x cheaper than ETH (200 Gwei)", eth_200_gwei_usd / c0dl3_txn_cost_usd);
    
    println!();
    
    // Traditional financial systems
    println!("=== Traditional Financial Systems ===");
    println!("Bank transfer: $0.25 - $3.00");
    println!("Credit card: $0.10 - $0.30 + 2-3%");
    println!("Wire transfer: $15 - $50");
    println!("PayPal: $0.30 + 2.9%");
    println!("C0DL3: ${:.6} (0.021 cents)", c0dl3_txn_cost_usd);
    
    println!();
    println!("=== Test Completed Successfully ===");
}
