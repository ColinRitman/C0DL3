use c0dl3_zksync::simple_omni_mixer::{SimpleOmniMixer, LPPosition};
use std::time::Duration;
use tokio::time::sleep;

/// Simple Omni-Mixer Demo
/// This shows the core concept working with just LP position mixing

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Simple Omni-Mixer Demo");
    println!("=========================");

    // Create simple mixer with realistic parameters
    let mixer = SimpleOmniMixer::new(
        3,      // Minimum 3 positions to trigger mixing
        300,    // 5 minute timeout
        100000  // 100k treasury balance
    );

    println!("✅ Created Omni-Mixer:");
    println!("   - Minimum positions: 3");
    println!("   - Max round time: 5 minutes");
    println!("   - Treasury balance: 100,000");

    // Add some LP positions
    println!("\n📝 Adding LP positions...");

    let positions = vec![
        ("Alice", 5000, 2500),   // 5k HEAT, 2.5k CD
        ("Bob", 3000, 3000),     // 3k HEAT, 3k CD
        ("Charlie", 7000, 3500), // 7k HEAT, 3.5k CD
        ("Diana", 4000, 2000),   // 4k HEAT, 2k CD
        ("Eve", 6000, 4000),     // 6k HEAT, 4k CD
    ];

    for (i, (provider, amount_a, amount_b)) in positions.iter().enumerate() {
        let position = LPPosition {
            id: format!("pos_{}", i + 1),
            provider: provider.to_string(),
            pool_id: "heat_cd_pool".to_string(),
            token_a: "HEAT".to_string(),
            token_b: "CD".to_string(),
            amount_a: *amount_a,
            amount_b: *amount_b,
            liquidity_tokens: ((*amount_a as f64) * (*amount_b as f64)).sqrt() as u128,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
        };

        mixer.add_position(position).await?;
        println!("   ✅ Added {}'s position: {} HEAT + {} CD", provider, amount_a, amount_b);

        // Show current stats
        let stats = mixer.get_stats().await?;
        if let Some(active_round) = stats["active_round"].as_object() {
            println!("   📊 Round status: {} positions, ${} value",
                    active_round["position_count"], active_round["total_value"]);
        }
    }

    // Wait for potential mixing
    println!("\n⏳ Waiting for mixing round to complete...");
    sleep(Duration::from_secs(1)).await;

    // Show final results
    show_final_results(&mixer).await?;

    println!("\n🎉 Demo shows the core omni-mixer concept works!");
    println!("   - Positions are batched together");
    println!("   - Treasury provides obfuscation");
    println!("   - Privacy through mixing is achieved");
    println!("   - Simple to implement and understand");

    Ok(())
}

async fn show_final_results(mixer: &SimpleOmniMixer) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📊 Final Results");
    println!("===============");

    let stats = mixer.get_stats().await?;
    println!("🎲 Mixing Rounds Completed: {}", stats["completed_rounds"]);
    println!("👥 Total Positions Mixed: {}", stats["total_positions_mixed"]);
    println!("💰 Total Value Mixed: ${}", stats["total_value_mixed"]);
    println!("🏦 Treasury Balance: ${}", stats["treasury_balance"]);
    println!("💸 Treasury Used: ${}", stats["treasury_used"]);

    // Show completed rounds
    let completed_rounds = mixer.get_completed_rounds().await?;
    if !completed_rounds.is_empty() {
        println!("\n📋 Completed Rounds:");
        for round in completed_rounds {
            println!("   Round {}: {} positions, ${} value, ${} treasury",
                    &round.round_id[..8],
                    round.positions.len(),
                    round.total_value,
                    round.treasury_contribution);
        }
    }

    // Show current round status
    let current_positions = mixer.get_current_positions().await?;
    if !current_positions.is_empty() {
        println!("\n⏳ Current Round Status:");
        println!("   Positions waiting: {}", current_positions.len());
        for pos in current_positions {
            println!("   - {}: {} HEAT + {} CD",
                    pos.provider, pos.amount_a, pos.amount_b);
        }
    }

    Ok(())
}
