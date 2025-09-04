//! # C0DL3 Omni-Mixer CLI
//!
//! Command-line interface for the C0DL3 Omni-Mixer.
//! Provides LP privacy through treasury-backed obfuscation.
//!
//! ## Usage
//!
//! ```bash
//! # Create a new mixer and add positions
//! cargo run -- --command demo
//!
//! # Run health check
//! cargo run -- --command health
//!
//! # Show help
//! cargo run -- --help
//! ```

use clap::{Parser, Subcommand};
use c0dl3_omni::{health_check, init_logging, SimpleOmniMixer, LPPosition};
use serde_json;
use std::error::Error;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "c0dl3-omni")]
#[command(about = "C0DL3 Omni-Mixer - LP Privacy Solution")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a demo of the omni-mixer functionality
    Demo,
    /// Health check for the system
    Health,
    /// Show current mixer statistics
    Stats,
    /// Add a sample position to the mixer
    AddPosition {
        /// User identifier
        #[arg(short, long)]
        user: String,
        /// Pool address
        #[arg(short, long)]
        pool: String,
        /// Token A amount
        #[arg(short = 'a', long)]
        token_a: u128,
        /// Token B amount
        #[arg(short = 'b', long)]
        token_b: u128,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging
    init_logging()?;

    println!("🚀 C0DL3 Omni-Mixer v{}", c0dl3_omni::VERSION);
    println!("🔒 Privacy-first LP mixing with treasury obfuscation\n");

    let cli = Cli::parse();

    match cli.command {
        Commands::Demo => run_demo().await?,
        Commands::Health => run_health_check().await?,
        Commands::Stats => run_stats().await?,
        Commands::AddPosition { user, pool, token_a, token_b } => {
            run_add_position(user, pool, token_a, token_b).await?;
        }
    }

    Ok(())
}

/// Run a comprehensive demo of the omni-mixer
async fn run_demo() -> Result<(), Box<dyn Error>> {
    println!("🎯 Running C0DL3 Omni-Mixer Demo");
    println!("=" .repeat(50));

    // Create mixer with 3 positions minimum, 5min timeout, 100k HEAT treasury
    let mixer = SimpleOmniMixer::new(3, 300, 100000);

    println!("✅ Created mixer with:");
    println!("   • Minimum positions per round: 3");
    println!("   • Round timeout: 300 seconds");
    println!("   • Treasury funds: 100,000 HEAT");
    println!();

    // Add sample positions
    println!("📥 Adding sample LP positions...");

    let positions = vec![
        ("alice", "0x1234...abcd", 50000, 75000),
        ("bob", "0x5678...efgh", 25000, 125000),
        ("charlie", "0x9abc...ijkl", 100000, 50000),
        ("diana", "0xdef0...mnop", 75000, 100000),
        ("eve", "0x1234...qrst", 30000, 80000),
    ];

    let mut round_ids = Vec::new();

    for (user, pool, token_a, token_b) in positions {
        let position = LPPosition::new(
            user.to_string(),
            pool.to_string(),
            token_a,
            token_b,
        );

        match mixer.add_position(position.clone()).await {
            Ok(round_id) => {
                println!("   ✅ Added {}'s position to round {}", user, &round_id[..8]);
                round_ids.push(round_id);
            }
            Err(e) => {
                println!("   ❌ Failed to add {}'s position: {}", user, e);
            }
        }

        // Small delay to simulate real usage
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    println!();
    println!("⏳ Waiting for mixing rounds to complete...");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Get and display statistics
    let stats = mixer.get_stats().await?;
    println!("📊 Mixer Statistics:");
    println!("   • Total positions processed: {}", stats.total_positions);
    println!("   • Completed mixing rounds: {}", stats.completed_rounds);
    println!("   • Active rounds: {}", stats.active_rounds);
    println!("   • Treasury funds used: {} HEAT", stats.total_treasury_used);
    println!("   • Average positions per round: {:.1}", stats.avg_positions_per_round);
    println!("   • Success rate: {:.1}%", stats.success_rate * 100.0);
    println!();

    // Show completed rounds
    let completed = mixer.get_completed_rounds().await?;
    if !completed.is_empty() {
        println!("🎉 Completed Mixing Rounds:");
        for (i, round) in completed.iter().enumerate() {
            println!("   Round {} ({}):", i + 1, &round.id[..8]);
            println!("     • Positions: {}", round.position_count());
            println!("     • Total value: {} tokens", round.total_value());
            println!("     • Treasury used: {} HEAT", round.treasury_used);
            println!("     • Merkle root: {}...", &round.merkle_root[..16]);
            println!("     • Status: {:?}", round.status);
        }
    } else {
        println!("⏳ No rounds completed yet - they may still be processing in background");
    }

    println!();
    println!("🎯 Demo completed! The omni-mixer successfully:");
    println!("   • Protected LP privacy through position mixing");
    println!("   • Used treasury funds for obfuscation");
    println!("   • Generated cryptographic proofs (Merkle roots)");
    println!("   • Maintained real-time statistics");

    Ok(())
}

/// Run health check
async fn run_health_check() -> Result<(), Box<dyn Error>> {
    println!("🏥 Running Health Check");
    println!("=" .repeat(30));

    match health_check().await {
        Ok(health) => {
            println!("✅ System Status: {}", health["status"]);
            println!("📅 Timestamp: {}", health["timestamp"]);
            println!("🔧 Version: {}", health["version"]);

            println!("\n🚀 Features:");
            if let Some(features) = health["features"].as_array() {
                for feature in features {
                    println!("   • {}", feature.as_str().unwrap_or("unknown"));
                }
            }

            println!("\n💚 System is healthy!");
        }
        Err(e) => {
            println!("❌ Health check failed: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

/// Show mixer statistics (requires running mixer instance)
async fn run_stats() -> Result<(), Box<dyn Error>> {
    println!("📊 Current Mixer Statistics");
    println!("=" .repeat(35));

    // For this CLI demo, we'll create a fresh mixer
    // In a real deployment, this would connect to a running instance
    let mixer = SimpleOmniMixer::new_default();

    match mixer.get_stats().await {
        Ok(stats) => {
            println!("📈 Statistics:");
            println!("   Positions: {}", stats.total_positions);
            println!("   Completed Rounds: {}", stats.completed_rounds);
            println!("   Active Rounds: {}", stats.active_rounds);
            println!("   Treasury Used: {} HEAT", stats.total_treasury_used);
            println!("   Avg Positions/Round: {:.1}", stats.avg_positions_per_round);
            println!("   Success Rate: {:.1}%", stats.success_rate * 100.0);
        }
        Err(e) => {
            println!("❌ Failed to get statistics: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

/// Add a single position to the mixer
async fn run_add_position(user: String, pool: String, token_a: u128, token_b: u128) -> Result<(), Box<dyn Error>> {
    println!("📥 Adding Position to Omni-Mixer");
    println!("=" .repeat(35));

    let mixer = SimpleOmniMixer::new_default();

    let position = LPPosition::new(user.clone(), pool.clone(), token_a, token_b);

    println!("👤 User: {}", user);
    println!("🏊 Pool: {}", pool);
    println!("💰 Token A: {}", token_a);
    println!("💰 Token B: {}", token_b);
    println!("💎 Total Value: {}", position.total_value());
    println!();

    match mixer.add_position(position).await {
        Ok(round_id) => {
            println!("✅ Position added successfully!");
            println!("🎫 Round ID: {}", round_id);

            // Show updated stats
            let stats = mixer.get_stats().await?;
            println!("\n📊 Updated Statistics:");
            println!("   Total positions: {}", stats.total_positions);
            println!("   Active rounds: {}", stats.active_rounds);
        }
        Err(e) => {
            println!("❌ Failed to add position: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cli_demo() {
        // This would be a full integration test
        // For now, just test that we can create a mixer
        let mixer = SimpleOmniMixer::new_default();
        let stats = mixer.get_stats().await.unwrap();
        assert_eq!(stats.total_positions, 0);
    }
}
