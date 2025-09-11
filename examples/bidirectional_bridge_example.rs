//! Example demonstrating bidirectional bridge between C0DL3 and Fuego
//!
//! This example shows how C0DL3 events can be sent to Fuego and Fuego events
//! can be read on C0DL3, enabling full cross-chain interoperability.

use anyhow::Result;
use std::time::Duration;
use tokio::time;

use c0dl3_zksync::privacy::bidirectional_bridge::{
    BidirectionalBridgeManager, C0dl3Event, C0dl3EventType
};

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 C0DL3 ↔ Fuego Bidirectional Bridge Example");
    println!("==============================================\n");

    // Initialize bidirectional bridge
    let bridge_manager = BidirectionalBridgeManager::new("http://localhost:8546");

    println!("✅ Bridge initialized successfully");

    // Example 1: Send C0DL3 event to Fuego
    println!("\n📤 Sending C0DL3 event to Fuego...");

    let c0dl3_event = C0dl3Event {
        event_type: C0dl3EventType::HeatTokenMint,
        c0dl3_tx_hash: "0xc0dl3_tx_1234567890abcdef".to_string(),
        c0dl3_block_height: 1000,
        event_data: vec![1, 2, 3, 4, 5], // Encrypted event data
        stark_proof: vec![0xAA; 64], // Mock STARK proof
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs(),
    };

    match bridge_manager.send_c0dl3_event_to_fuego(c0dl3_event).await {
        Ok(result) => println!("✅ {}", result),
        Err(e) => println!("❌ Failed to send event: {}", e),
    }

    // Example 2: Read Fuego events on C0DL3
    println!("\n📥 Reading Fuego events on C0DL3...");

    match bridge_manager.read_fuego_events_on_c0dl3(12345).await {
        Ok(events) => {
            println!("✅ Successfully read {} events from Fuego block 12345", events.len());
            for (i, _event) in events.iter().enumerate() {
                println!("  Event {}: Fuego → C0DL3", i + 1);
            }
        }
        Err(e) => println!("❌ Failed to read Fuego events: {}", e),
    }

    // Example 3: Monitor bridge status
    println!("\n📊 Bridge Status Monitoring...");

    let mut interval = time::interval(Duration::from_secs(5));

    for i in 0..3 {
        interval.tick().await;

        match bridge_manager.get_sync_status() {
            Ok(status) => {
                println!("🔄 Sync Status #{}:", i + 1);
                println!("  Last C0DL3 block on Fuego: {}", status.last_c0dl3_block_on_fuego);
                println!("  Last Fuego block on C0DL3: {}", status.last_fuego_block_on_c0dl3);
                println!("  Sync lag: {} blocks", status.sync_lag_blocks);
            }
            Err(e) => println!("❌ Failed to get sync status: {}", e),
        }

        match bridge_manager.get_metrics() {
            Ok(metrics) => {
                println!("📈 Bridge Metrics #{}:", i + 1);
                println!("  C0DL3 → Fuego events: {}", metrics.c0dl3_to_fuego_events_sent);
                println!("  Fuego → C0DL3 events: {}", metrics.fuego_to_c0dl3_events_received);
                println!("  Failed transmissions: {}", metrics.failed_transmissions);
                println!("  Bridge uptime: {:.1}%", metrics.bridge_uptime_percentage);
            }
            Err(e) => println!("❌ Failed to get metrics: {}", e),
        }

        println!();
    }

    println!("🎯 Bidirectional Bridge Demo Complete!");
    println!("\nKey Benefits:");
    println!("• ✅ C0DL3 events can be read on Fuego");
    println!("• ✅ Fuego events can be read on C0DL3");
    println!("• ✅ Full cross-chain interoperability");
    println!("• ✅ Privacy-preserving event transmission");
    println!("• ✅ STARK proof verification for authenticity");
    println!("• ✅ Real-time synchronization monitoring");

    Ok(())
}
