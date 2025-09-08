// C0DL3 Transaction Fees in USD
// Based on HEAT pricing: 1 XFG = $100, so 10M HEAT = $100

fn main() {
    println!("=== C0DL3 Transaction Fees in USD ===");
    println!("HEAT Pricing Model:");
    println!("1 XFG = $100");
    println!("10M HEAT = $100");
    println!("1 HEAT = $0.00001");
    println!();
    
    // Current C0DL3 configuration
    let default_gas_price_fwei = 1u64;
    let standard_gas_limit = 21000u64;
    
    // Calculate transaction cost in HEAT
    let txn_cost_heat = default_gas_price_fwei as f64 * standard_gas_limit as f64 / 1000.0;
    
    // Convert to USD
    let heat_price_usd = 0.00001; // $0.00001 per HEAT
    let txn_cost_usd = txn_cost_heat * heat_price_usd;
    
    println!("Current C0DL3 Configuration:");
    println!("Gas Price: {} fwei", default_gas_price_fwei);
    println!("Gas Limit: {} gas", standard_gas_limit);
    println!("Transaction Cost: {} HEAT", txn_cost_heat);
    println!("Transaction Cost: ${:.6}", txn_cost_usd);
    println!();
    
    // Different gas price scenarios
    println!("=== Different Gas Price Scenarios ===");
    let gas_prices = vec![0.01f64, 0.05f64, 0.1f64, 0.5f64, 1.0f64, 2.0f64, 5.0f64, 10.0f64];
    
    for price in &gas_prices {
        let txn_cost_heat = *price * standard_gas_limit as f64 / 1000.0;
        let txn_cost_usd = txn_cost_heat * heat_price_usd;
        
        println!("{:.2} fwei gas price = {:.2} HEAT = ${:.6} per transaction", 
                price, txn_cost_heat, txn_cost_usd);
    }
    
    println!();
    
    // Compare with ETH
    println!("=== Comparison with ETH ===");
    let eth_gas_price_gwei = 20u64;
    let eth_txn_cost_eth = (eth_gas_price_gwei * standard_gas_limit) as f64 / 1_000_000_000_000_000_000.0;
    let eth_price_usd = 3000.0; // Assuming $3000 per ETH
    let eth_txn_cost_usd = eth_txn_cost_eth * eth_price_usd;
    
    println!("ETH (20 Gwei): {} ETH = ${:.6} per transaction", eth_txn_cost_eth, eth_txn_cost_usd);
    println!("C0DL3 (1 fwei): {} HEAT = ${:.6} per transaction", txn_cost_heat, txn_cost_usd);
    println!("Ratio: C0DL3 is {:.1}x more expensive than ETH", txn_cost_usd / eth_txn_cost_usd);
    
    println!();
    
    // Practical recommendations
    println!("=== Practical Recommendations ===");
    println!("For reasonable transaction fees:");
    
    let recommendations = vec![
        (0.01, "Very Low"),
        (0.05, "Low"),
        (0.1, "Medium"),
        (0.5, "High"),
        (1.0, "Premium"),
        (2.0, "Ultra Premium")
    ];
    
    for (price, label) in &recommendations {
        let txn_cost_heat = *price * standard_gas_limit as f64 / 1000.0;
        let txn_cost_usd = txn_cost_heat * heat_price_usd;
        
        println!("{}: {:.2} fwei = {:.2} HEAT = ${:.6} per transaction", 
                label, price, txn_cost_heat, txn_cost_usd);
    }
    
    println!();
    
    // HEAT pricing reference
    println!("=== HEAT Pricing Reference ===");
    let heat_amounts = vec![
        10000, 50000, 100000, 500000, 1000000, 5000000, 10000000
    ];
    
    for amount in &heat_amounts {
        let usd_value = *amount as f64 * heat_price_usd;
        println!("{} HEAT = ${:.2}", amount, usd_value);
    }
    
    println!();
    println!("=== Test Completed Successfully ===");
}
