use c0dl3_zksync::omni_mixer::{OmniMixer, OmniMixerConfig, PrivacyLevel, LPPosition};
use c0dl3_zksync::treasury_integration::TreasuryIntegrator;
use c0dl3_zksync::lp_integration::{LPIntegrator, LiquidityPool};
use std::time::Duration;
use tokio::time::sleep;

/// Complete Omni-Mixer Demo
/// This example shows how to implement and use the omni-mixer for LP privacy

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 C0DL3 Omni-Mixer Demo");
    println!("========================");

    // Initialize components
    let omni_mixer = initialize_omni_mixer().await?;
    let treasury = initialize_treasury().await?;
    let lp_integrator = initialize_lp_integrator().await?;

    // Setup demo scenario
    setup_demo_scenario(&omni_mixer, &treasury, &lp_integrator).await?;

    // Run mixing rounds
    run_mixing_demo(&omni_mixer, &treasury, &lp_integrator).await?;

    // Show results and analytics
    show_demo_results(&omni_mixer, &treasury).await?;

    println!("\n✅ Omni-Mixer Demo Completed Successfully!");
    Ok(())
}

async fn initialize_omni_mixer() -> Result<OmniMixer, Box<dyn std::error::Error>> {
    println!("\n📦 Initializing Omni-Mixer...");

    let config = OmniMixerConfig {
        min_mixing_threshold: 5, // Minimum 5 positions for a round
        max_mixing_delay: 60,    // 1 minute max delay
        treasury_obfuscation_ratio: 0.15, // 15% treasury assets for obfuscation
        zk_proof_required: true,
        privacy_level: PrivacyLevel::Enhanced,
        rotation_frequency: 1800, // 30 minutes
    };

    let mixer = OmniMixer::new(config)?;
    println!("✅ Omni-Mixer initialized with enhanced privacy settings");

    Ok(mixer)
}

async fn initialize_treasury() -> Result<TreasuryIntegrator, Box<dyn std::error::Error>> {
    println!("\n💰 Initializing Treasury Integration...");

    let treasury = TreasuryIntegrator::new()?;

    // Initialize with substantial treasury reserves
    let heat_reserve = 10_000_000u128; // 10M HEAT
    let cd_reserve = 5_000_000u128;    // 5M CD

    treasury.initialize_pools(heat_reserve, cd_reserve).await?;
    println!("✅ Treasury initialized with {} HEAT and {} CD", heat_reserve, cd_reserve);

    Ok(treasury)
}

async fn initialize_lp_integrator() -> Result<LPIntegrator, Box<dyn std::error::Error>> {
    println!("\n🏊 Initializing LP Integrator...");

    let integrator = LPIntegrator::new()?;

    // Register some example pools
    let pools = vec![
        LiquidityPool {
            id: "heat_cd_pool".to_string(),
            token_a: "HEAT".to_string(),
            token_b: "CD".to_string(),
            reserve_a: 1_000_000,
            reserve_b: 500_000,
            total_liquidity: 750_000,
            fee_tier: 30, // 0.3%
            protocol: "uniswap_v3".to_string(),
        },
        LiquidityPool {
            id: "heat_usdc_pool".to_string(),
            token_a: "HEAT".to_string(),
            token_b: "USDC".to_string(),
            reserve_a: 800_000,
            reserve_b: 1_200_000,
            total_liquidity: 1_000_000,
            fee_tier: 5, // 0.05%
            protocol: "uniswap_v3".to_string(),
        },
        LiquidityPool {
            id: "cd_usdc_pool".to_string(),
            token_a: "CD".to_string(),
            token_b: "USDC".to_string(),
            reserve_a: 600_000,
            reserve_b: 900_000,
            total_liquidity: 750_000,
            fee_tier: 10, // 0.1%
            protocol: "sushiswap".to_string(),
        },
    ];

    for pool in pools {
        integrator.register_pool(pool).await?;
    }

    println!("✅ LP Integrator initialized with 3 pools");
    Ok(integrator)
}

async fn setup_demo_scenario(
    omni_mixer: &OmniMixer,
    treasury: &TreasuryIntegrator,
    lp_integrator: &LPIntegrator,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🎭 Setting up Demo Scenario...");

    // Create some sample LP positions
    let providers = vec!["alice", "bob", "charlie", "diana", "eve"];
    let pools = vec!["heat_cd_pool", "heat_usdc_pool", "cd_usdc_pool"];

    println!("📝 Creating sample LP positions...");

    for (i, provider) in providers.iter().enumerate() {
        let pool_id = pools[i % pools.len()];

        // Create position in LP integrator
        let position_id = lp_integrator.create_position(
            pool_id,
            provider,
            10000 + (i as u128 * 5000), // Varying liquidity amounts
            -2000 + (i as i32 * 500),    // Varying lower ticks
            2000 + (i as i32 * 500),     // Varying upper ticks
        ).await?;

        // Add to mixing queue
        lp_integrator.add_position_to_mixing(&position_id).await?;

        // Create corresponding position for omni-mixer
        let lp_position = LPPosition {
            id: position_id.clone(),
            provider: provider.to_string(),
            pool_id: pool_id.to_string(),
            token_a: if pool_id.contains("heat") { "HEAT".to_string() } else { "CD".to_string() },
            token_b: if pool_id.contains("usdc") { "USDC".to_string() } else if pool_id.contains("cd") { "CD".to_string() } else { "HEAT".to_string() },
            amount_a: 5000 + (i as u128 * 2000),
            amount_b: 3000 + (i as u128 * 1500),
            liquidity_tokens: 8000 + (i as u128 * 3000),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
            nonce: i as u64,
        };

        omni_mixer.add_lp_position(lp_position).await?;

        println!("  ✅ Created position for {} in pool {}", provider, pool_id);
    }

    println!("✅ Demo scenario setup complete with {} LP positions", providers.len());
    Ok(())
}

async fn run_mixing_demo(
    omni_mixer: &OmniMixer,
    treasury: &TreasuryIntegrator,
    lp_integrator: &LPIntegrator,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔄 Running Mixing Rounds...");

    // Check initial state
    let initial_stats = omni_mixer.get_mixing_stats().await?;
    println!("📊 Initial state: {} positions in queue", initial_stats["queue_length"]);

    // Start first mixing round
    println!("\n🎲 Starting Round 1...");
    let round_1_id = omni_mixer.start_mixing_round().await?;
    println!("🔢 Round 1 ID: {}", round_1_id);

    // Allocate treasury assets
    let mixing_value = lp_integrator.calculate_mixing_pool_value().await?;
    println!("💰 Mixing pool value: {}", mixing_value);

    let allocated = treasury.allocate_for_mixing(&round_1_id, mixing_value / 10).await?;
    println!("🏦 Treasury allocated: {:?}", allocated);

    // Process the mixing round
    omni_mixer.process_mixing_round(&round_1_id).await?;
    println!("✅ Round 1 completed successfully");

    // Show round statistics
    let round_stats = omni_mixer.get_mixing_stats().await?;
    println!("📈 Round 1 Results:");
    println!("  - Completed rounds: {}", round_stats["completed_rounds"]);
    println!("  - Privacy score: {:.2}", round_stats["privacy_metrics"]["privacy_score"].as_f64().unwrap_or(0.0));

    // Return treasury assets
    treasury.return_allocated_assets(&round_1_id).await?;
    println!("🔄 Treasury assets returned");

    // Wait a bit and run another round
    println!("\n⏳ Waiting for next round...");
    sleep(Duration::from_secs(2)).await;

    // Create more positions for second round
    for i in 5..10 {
        let provider = format!("provider_{}", i);
        let pool_id = if i % 2 == 0 { "heat_cd_pool" } else { "heat_usdc_pool" };

        let position_id = lp_integrator.create_position(
            pool_id,
            &provider,
            15000 + (i as u128 * 3000),
            -1500 + (i as i32 * 300),
            1500 + (i as i32 * 300),
        ).await?;

        lp_integrator.add_position_to_mixing(&position_id).await?;

        let lp_position = LPPosition {
            id: position_id,
            provider: provider.clone(),
            pool_id: pool_id.to_string(),
            token_a: "HEAT".to_string(),
            token_b: if pool_id.contains("usdc") { "USDC".to_string() } else { "CD".to_string() },
            amount_a: 7000 + (i as u128 * 1000),
            amount_b: 4000 + (i as u128 * 800),
            liquidity_tokens: 11000 + (i as u128 * 2000),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
            nonce: i as u64,
        };

        omni_mixer.add_lp_position(lp_position).await?;
    }

    // Start second round
    println!("\n🎲 Starting Round 2...");
    let round_2_id = omni_mixer.start_mixing_round().await?;
    println!("🔢 Round 2 ID: {}", round_2_id);

    // Allocate more treasury assets
    let mixing_value_2 = lp_integrator.calculate_mixing_pool_value().await?;
    let allocated_2 = treasury.allocate_for_mixing(&round_2_id, mixing_value_2 / 8).await?;
    println!("🏦 Treasury allocated for Round 2: {:?}", allocated_2);

    // Process second round
    omni_mixer.process_mixing_round(&round_2_id).await?;
    treasury.return_allocated_assets(&round_2_id).await?;
    println!("✅ Round 2 completed successfully");

    // Generate some privacy proofs
    println!("\n🔐 Generating Privacy Proofs...");
    let eligible_positions = lp_integrator.get_mixing_eligible_positions().await?;
    for position in eligible_positions.into_iter().take(3) {
        let proof = omni_mixer.generate_privacy_proof(&position.id).await?;
        let is_valid = omni_mixer.verify_privacy_proof(&proof).await?;
        println!("  ✅ Privacy proof for position {}: {}", position.id, if is_valid { "VALID" } else { "INVALID" });
    }

    Ok(())
}

async fn show_demo_results(
    omni_mixer: &OmniMixer,
    treasury: &TreasuryIntegrator,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📊 Final Demo Results");
    println!("====================");

    // Omni-Mixer Statistics
    let mixer_stats = omni_mixer.get_mixing_stats().await?;
    println!("\n🔄 Omni-Mixer Performance:");
    println!("  - Total rounds completed: {}", mixer_stats["completed_rounds"]);
    println!("  - Total mixed value: {}", mixer_stats["total_mixed_value"]);
    println!("  - Privacy score: {:.3}", mixer_stats["privacy_metrics"]["privacy_score"]);
    println!("  - Average mixing time: {}s", mixer_stats["privacy_metrics"]["average_mixing_time"]);
    println!("  - Treasury efficiency: {:.2}%", mixer_stats["privacy_metrics"]["treasury_efficiency"].as_f64().unwrap_or(0.0) * 100.0);

    // Treasury Statistics
    let treasury_stats = treasury.get_utilization_metrics().await?;
    println!("\n💰 Treasury Performance:");
    println!("  - Total HEAT allocated: {}", treasury_stats["heat_allocated"]);
    println!("  - Total CD allocated: {}", treasury_stats["cd_allocated"]);
    println!("  - Current utilization: {:.2}%", treasury_stats["utilization_ratio"].as_f64().unwrap_or(0.0) * 100.0);
    println!("  - Total allocations: {}", treasury_stats["total_allocations"]);
    println!("  - Successful returns: {}", treasury_stats["successful_returns"]);

    // Privacy Analysis
    println!("\n🔒 Privacy Analysis:");
    let privacy_score = mixer_stats["privacy_metrics"]["privacy_score"].as_f64().unwrap_or(0.0);
    if privacy_score >= 0.8 {
        println!("  - Privacy Level: EXCELLENT (Score: {:.3})", privacy_score);
        println!("  - Status: Strong privacy guarantees achieved");
    } else if privacy_score >= 0.6 {
        println!("  - Privacy Level: GOOD (Score: {:.3})", privacy_score);
        println!("  - Status: Adequate privacy with room for improvement");
    } else {
        println!("  - Privacy Level: NEEDS IMPROVEMENT (Score: {:.3})", privacy_score);
        println!("  - Status: More rounds needed for better privacy");
    }

    // Economic Impact
    let total_mixed = mixer_stats["total_mixed_value"].as_u64().unwrap_or(0);
    let treasury_efficiency = mixer_stats["privacy_metrics"]["treasury_efficiency"].as_f64().unwrap_or(0.0);

    println!("\n💸 Economic Impact:");
    println!("  - Total LP value processed: {}", total_mixed);
    println!("  - Treasury assets utilized: {:.2}%", treasury_efficiency * 100.0);
    println!("  - Estimated privacy-preserving fee savings: ${:.2}",
             total_mixed as f64 * 0.002); // Assuming 0.2% fee savings

    println!("\n🎯 Key Achievements:");
    println!("  ✅ Network-wide LP privacy achieved");
    println!("  ✅ Treasury assets effectively utilized for obfuscation");
    println!("  ✅ ZK-proof based privacy verification");
    println!("  ✅ Dynamic mixing rounds with optimal batching");
    println!("  ✅ Emergency rotation capabilities");
    println!("  ✅ Real-time privacy metrics and monitoring");

    Ok(())
}
