use anyhow::Result;
use clap::Parser;
use tracing::{info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use node::{CODL3ZkSyncNode, NodeConfig};

#[derive(Parser)]
#[command(name = "codl3-zksync-node")]
#[command(about = "CODL3 zkSync Hyperchains Node")]
#[command(version)]
struct Cli {
    /// Log level (debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
    
    /// Data directory
    #[arg(long, default_value = "./data")]
    data_dir: String,
    
    /// zkSync L1 RPC URL
    #[arg(long, default_value = "http://localhost:8545")]
    l1_rpc_url: String,
    
    /// zkSync L2 RPC URL
    #[arg(long, default_value = "http://localhost:3050")]
    l2_rpc_url: String,
    
    /// Fuego RPC URL
    #[arg(long, default_value = "http://localhost:8080")]
    fuego_rpc_url: String,
    
    /// P2P port
    #[arg(long, default_value = "30333")]
    p2p_port: u16,
    
    /// RPC port
    #[arg(long, default_value = "9944")]
    rpc_port: u16,
    
    /// Validator address
    #[arg(long)]
    validator_address: Option<String>,
    
    /// Gas token address (HEAT)
    #[arg(long)]
    gas_token_address: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments
    let cli = Cli::parse();
    
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| cli.log_level.clone()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();
    
    info!("Starting CODL3 zkSync Hyperchains node");
    info!("Log level: {}", cli.log_level);
    info!("Data directory: {}", cli.data_dir);
    info!("L1 RPC URL: {}", cli.l1_rpc_url);
    info!("L2 RPC URL: {}", cli.l2_rpc_url);
    info!("Fuego RPC URL: {}", cli.fuego_rpc_url);
    
    // Create node configuration
    let config = create_node_config(&cli)?;
    
    // Create and start the node
    let node = CODL3ZkSyncNode::new(config).await?;
    
    // Set up signal handling for graceful shutdown
    let node_clone = node.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();
        info!("Received shutdown signal");
        if let Err(e) = node_clone.stop().await {
            error!("Error stopping node: {}", e);
        }
    });
    
    // Start the node
    if let Err(e) = node.start().await {
        error!("Node error: {}", e);
        return Err(e);
    }
    
    info!("CODL3 zkSync node stopped");
    Ok(())
}

fn create_node_config(cli: &Cli) -> Result<NodeConfig> {
    let mut config = NodeConfig::default();
    
    // Network configuration
    config.network.data_dir = cli.data_dir.clone();
    config.network.p2p_port = cli.p2p_port;
    config.network.listen_addr = "0.0.0.0".to_string();
    
    // RPC configuration
    config.rpc.bind_address = format!("0.0.0.0:{}", cli.rpc_port);
    
    // zkSync configuration
    config.zksync.l1_rpc_url = cli.l1_rpc_url.clone();
    config.zksync.l2_rpc_url = cli.l2_rpc_url.clone();
    config.zksync.hyperchain_id = 324; // zkSync Era mainnet
    
    if let Some(addr) = &cli.gas_token_address {
        config.zksync.gas_token_address = addr.parse()?;
    }
    
    if let Some(addr) = &cli.validator_address {
        config.zksync.validator_address = addr.parse()?;
    }
    
    // Fuego configuration
    config.fuego.rpc_url = cli.fuego_rpc_url.clone();
    config.fuego.wallet_address = "".to_string(); // Will be configured later
    config.fuego.mining_threads = 4;
    config.fuego.block_time = 30;
    
    // Bridge configuration
    config.bridge.l1_rpc_url = cli.l1_rpc_url.clone();
    config.bridge.l2_rpc_url = cli.l2_rpc_url.clone();
    
    // Consensus configuration
    config.consensus.validator_count = 21;
    config.consensus.min_stake = 800_000_000_000_000_000_000_000_000u128; // 800B HEAT
    
    // Logging configuration
    config.logging.level = cli.log_level.clone();
    
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cli_parsing() {
        let args = vec![
            "codl3-zksync-node",
            "--log-level", "debug",
            "--data-dir", "/tmp/test",
            "--l1-rpc-url", "http://test:8545",
            "--l2-rpc-url", "http://test:3050",
        ];
        
        let cli = Cli::try_parse_from(args).unwrap();
        assert_eq!(cli.log_level, "debug");
        assert_eq!(cli.data_dir, "/tmp/test");
        assert_eq!(cli.l1_rpc_url, "http://test:8545");
        assert_eq!(cli.l2_rpc_url, "http://test:3050");
    }
    
    #[test]
    fn test_config_creation() {
        let cli = Cli {
            log_level: "info".to_string(),
            data_dir: "/tmp/test".to_string(),
            l1_rpc_url: "http://test:8545".to_string(),
            l2_rpc_url: "http://test:3050".to_string(),
            fuego_rpc_url: "http://test:8080".to_string(),
            p2p_port: 30333,
            rpc_port: 9944,
            validator_address: None,
            gas_token_address: None,
        };
        
        let config = create_node_config(&cli).unwrap();
        assert_eq!(config.network.data_dir, "/tmp/test");
        assert_eq!(config.zksync.l1_rpc_url, "http://test:8545");
        assert_eq!(config.zksync.l2_rpc_url, "http://test:3050");
        assert_eq!(config.fuego.rpc_url, "http://test:8080");
    }
}
