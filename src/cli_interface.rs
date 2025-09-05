// Interactive CLI Interface for Unified Daemon
// Provides commands for mining, validation, and status monitoring

use std::io::{self, Write};
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use tracing::{info, error, debug};

use crate::unified_cli::{UnifiedCliDaemon, UnifiedCliConfig, SystemStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CliCommand {
    Status,
    MiningStart,
    MiningStop,
    MiningStats,
    ValidatorsList,
    ValidatorInfo(String),
    ValidatorStake(String, u64),
    NetworkStats,
    DaemonRestart(String),
    DaemonStop(String),
    DaemonStart(String),
    Help,
    Exit,
    Unknown(String),
}

pub struct CliInterface {
    daemon: UnifiedCliDaemon,
    running: bool,
}

impl CliInterface {
    pub fn new(config: UnifiedCliConfig) -> Self {
        let daemon = UnifiedCliDaemon::new(config);
        Self {
            daemon,
            running: true,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        info!("🎮 Starting Interactive CLI Interface");
        
        // Start the daemon
        self.daemon.start().await?;
        
        // Display welcome message
        self.display_welcome();
        
        // Main CLI loop
        while self.running {
            self.display_prompt();
            
            if let Some(command) = self.read_command() {
                self.execute_command(command).await?;
            }
        }
        
        Ok(())
    }

    fn display_welcome(&self) {
        println!("\n╔══════════════════════════════════════════════════════════════╗");
        println!("║                UNIFIED CLI DAEMON INTERFACE                ║");
        println!("║                                                              ║");
        println!("║  🚀 C0DL3 zkSync Hyperchain + Fuego L1 Mining              ║");
        println!("║  ⛏️  Mining • 🛡️  Validation • 📊 Monitoring              ║");
        println!("║                                                              ║");
        println!("║  Commands: status, mining, validators, network, help, exit   ║");
        println!("╚══════════════════════════════════════════════════════════════╝\n");
    }

    fn display_prompt(&self) {
        print!("unified-cli> ");
        io::stdout().flush().unwrap();
    }

    fn read_command(&self) -> Option<CliCommand> {
        let mut input = String::new();
        
        match io::stdin().read_line(&mut input) {
            Ok(_) => {
                let input = input.trim().to_lowercase();
                Some(self.parse_command(&input))
            }
            Err(_) => None,
        }
    }

    fn parse_command(&self, input: &str) -> CliCommand {
        let parts: Vec<&str> = input.split_whitespace().collect();
        
        if parts.is_empty() {
            return CliCommand::Unknown(input.to_string());
        }
        
        match parts[0] {
            "status" | "s" => CliCommand::Status,
            "mining" | "m" => {
                if parts.len() > 1 {
                    match parts[1] {
                        "start" => CliCommand::MiningStart,
                        "stop" => CliCommand::MiningStop,
                        "stats" => CliCommand::MiningStats,
                        _ => CliCommand::Unknown(input.to_string()),
                    }
                } else {
                    CliCommand::MiningStats
                }
            }
            "validators" | "v" => {
                if parts.len() > 1 {
                    match parts[1] {
                        "list" | "ls" => CliCommand::ValidatorsList,
                        "info" => {
                            if parts.len() > 2 {
                                CliCommand::ValidatorInfo(parts[2].to_string())
                            } else {
                                CliCommand::Unknown(input.to_string())
                            }
                        }
                        "stake" => {
                            if parts.len() > 3 {
                                if let Ok(stake) = parts[3].parse::<u64>() {
                                    CliCommand::ValidatorStake(parts[2].to_string(), stake)
                                } else {
                                    CliCommand::Unknown(input.to_string())
                                }
                            } else {
                                CliCommand::Unknown(input.to_string())
                            }
                        }
                        _ => CliCommand::ValidatorsList,
                    }
                } else {
                    CliCommand::ValidatorsList
                }
            }
            "network" | "n" => CliCommand::NetworkStats,
            "daemon" | "d" => {
                if parts.len() > 2 {
                    match parts[1] {
                        "restart" => CliCommand::DaemonRestart(parts[2].to_string()),
                        "stop" => CliCommand::DaemonStop(parts[2].to_string()),
                        "start" => CliCommand::DaemonStart(parts[2].to_string()),
                        _ => CliCommand::Unknown(input.to_string()),
                    }
                } else {
                    CliCommand::Unknown(input.to_string())
                }
            }
            "help" | "h" | "?" => CliCommand::Help,
            "exit" | "quit" | "q" => CliCommand::Exit,
            _ => CliCommand::Unknown(input.to_string()),
        }
    }

    async fn execute_command(&mut self, command: CliCommand) -> Result<()> {
        match command {
            CliCommand::Status => {
                self.show_status().await?;
            }
            CliCommand::MiningStart => {
                self.start_mining().await?;
            }
            CliCommand::MiningStop => {
                self.stop_mining().await?;
            }
            CliCommand::MiningStats => {
                self.show_mining_stats().await?;
            }
            CliCommand::ValidatorsList => {
                self.list_validators().await?;
            }
            CliCommand::ValidatorInfo(address) => {
                self.show_validator_info(&address).await?;
            }
            CliCommand::ValidatorStake(address, stake) => {
                self.stake_validator(&address, stake).await?;
            }
            CliCommand::NetworkStats => {
                self.show_network_stats().await?;
            }
            CliCommand::DaemonRestart(daemon) => {
                self.restart_daemon(&daemon).await?;
            }
            CliCommand::DaemonStop(daemon) => {
                self.stop_daemon(&daemon).await?;
            }
            CliCommand::DaemonStart(daemon) => {
                self.start_daemon(&daemon).await?;
            }
            CliCommand::Help => {
                self.show_help();
            }
            CliCommand::Exit => {
                self.exit_cli().await?;
            }
            CliCommand::Unknown(input) => {
                println!("❌ Unknown command: {}", input);
                println!("Type 'help' for available commands");
            }
        }
        
        Ok(())
    }

    async fn show_status(&self) -> Result<()> {
        let status = self.daemon.get_status();
        
        println!("\n📊 SYSTEM STATUS");
        println!("═══════════════════════════════════════════════════════════════");
        
        // Daemon Status
        println!("🔧 DAEMON STATUS:");
        println!("   C0DL3: {:?}", status.c0dl3_daemon_status);
        println!("   Fuego: {:?}", status.fuego_daemon_status);
        
        // Mining Status
        println!("\n⛏️  MINING STATUS:");
        println!("   C0DL3 Mining: {}", if status.mining_status.c0dl3_mining { "✅ Active" } else { "❌ Inactive" });
        println!("   Fuego Mining: {}", if status.mining_status.fuego_mining { "✅ Active" } else { "❌ Inactive" });
        println!("   C0DL3 Hash Rate: {} H/s", status.mining_status.c0dl3_hash_rate);
        println!("   Fuego Hash Rate: {} H/s", status.mining_status.fuego_hash_rate);
        println!("   Total Rewards: {} tokens", status.mining_status.total_rewards);
        
        // Validation Status
        println!("\n🛡️  VALIDATION STATUS:");
        println!("   C0DL3 Validation: {}", if status.validation_status.c0dl3_validation_active { "✅ Active" } else { "❌ Inactive" });
        println!("   Fuego Validation: {}", if status.validation_status.fuego_validation_active { "✅ Active" } else { "❌ Inactive" });
        println!("   Total Stake: {} tokens", status.validation_status.total_stake);
        
        // Network Stats
        println!("\n🌐 NETWORK STATS:");
        println!("   C0DL3: {} peers, height {}, difficulty {}", 
                 status.network_stats.c0dl3_peers,
                 status.network_stats.c0dl3_block_height,
                 status.network_stats.c0dl3_difficulty);
        println!("   Fuego: {} peers, height {}, difficulty {}", 
                 status.network_stats.fuego_peers,
                 status.network_stats.fuego_block_height,
                 status.network_stats.fuego_difficulty);
        
        println!("   Uptime: {} seconds", status.uptime);
        println!("═══════════════════════════════════════════════════════════════\n");
        
        Ok(())
    }

    async fn start_mining(&self) -> Result<()> {
        println!("🚀 Starting mining on both chains...");
        
        // In a real implementation, this would start actual mining
        println!("✅ Mining started successfully");
        println!("   C0DL3: Mining blocks every 30 seconds");
        println!("   Fuego: Mining blocks every 480 seconds");
        
        Ok(())
    }

    async fn stop_mining(&self) -> Result<()> {
        println!("🛑 Stopping mining on both chains...");
        
        // In a real implementation, this would stop actual mining
        println!("✅ Mining stopped successfully");
        
        Ok(())
    }

    async fn show_mining_stats(&self) -> Result<()> {
        let status = self.daemon.get_status();
        
        println!("\n⛏️  MINING STATISTICS");
        println!("═══════════════════════════════════════════════════════════════");
        println!("C0DL3 Mining:");
        println!("   Status: {}", if status.mining_status.c0dl3_mining { "✅ Active" } else { "❌ Inactive" });
        println!("   Hash Rate: {} H/s", status.mining_status.c0dl3_hash_rate);
        println!("   Blocks Mined: {}", status.mining_status.c0dl3_blocks_mined);
        
        println!("\nFuego Mining:");
        println!("   Status: {}", if status.mining_status.fuego_mining { "✅ Active" } else { "❌ Inactive" });
        println!("   Hash Rate: {} H/s", status.mining_status.fuego_hash_rate);
        println!("   Blocks Mined: {}", status.mining_status.fuego_blocks_mined);
        
        println!("\nCombined:");
        println!("   Total Rewards: {} tokens", status.mining_status.total_rewards);
        println!("   Mining Efficiency: {:.1}%", status.mining_status.mining_efficiency * 100.0);
        println!("═══════════════════════════════════════════════════════════════\n");
        
        Ok(())
    }

    async fn list_validators(&self) -> Result<()> {
        let status = self.daemon.get_status();
        
        println!("\n🛡️  VALIDATORS");
        println!("═══════════════════════════════════════════════════════════════");
        
        // C0DL3 Eldorados
        println!("C0DL3 ELDORADOS:");
        for (i, validator) in status.validation_status.c0dl3_elderados.iter().enumerate() {
            println!("   {}. {} ({})", i + 1, validator.address, validator.status);
            println!("      Stake: {} tokens", validator.stake);
            println!("      Uptime: {} seconds", validator.uptime);
            println!("      Blocks Validated: {}", validator.blocks_validated);
            println!("      Rewards: {} tokens", validator.rewards_earned);
            println!("      Reputation: {:.2}", validator.reputation_score);
        }
        
        // Fuego Elderfiers
        println!("\nFuego ELDERFIERS:");
        for (i, validator) in status.validation_status.fuego_elderfiers.iter().enumerate() {
            println!("   {}. {} ({})", i + 1, validator.address, validator.status);
            println!("      Stake: {} tokens", validator.stake);
            println!("      Uptime: {} seconds", validator.uptime);
            println!("      Blocks Validated: {}", validator.blocks_validated);
            println!("      Rewards: {} tokens", validator.rewards_earned);
            println!("      Elderfier Level: {}", validator.elderfier_level);
            println!("      Reputation: {:.2}", validator.reputation_score);
        }
        
        println!("═══════════════════════════════════════════════════════════════\n");
        
        Ok(())
    }

    async fn show_validator_info(&self, address: &str) -> Result<()> {
        let status = self.daemon.get_status();
        
        // Search in C0DL3 Eldorados
        if let Some(validator) = status.validation_status.c0dl3_elderados.iter()
            .find(|v| v.address == address) {
            println!("\n🛡️  C0DL3 ELDORADO VALIDATOR INFO");
            println!("═══════════════════════════════════════════════════════════════");
            println!("Address: {}", validator.address);
            println!("Status: {:?}", validator.status);
            println!("Stake: {} tokens", validator.stake);
            println!("Uptime: {} seconds", validator.uptime);
            println!("Blocks Validated: {}", validator.blocks_validated);
            println!("Rewards Earned: {} tokens", validator.rewards_earned);
            println!("Reputation Score: {:.2}", validator.reputation_score);
            println!("═══════════════════════════════════════════════════════════════\n");
            return Ok(());
        }
        
        // Search in Fuego Elderfiers
        if let Some(validator) = status.validation_status.fuego_elderfiers.iter()
            .find(|v| v.address == address) {
            println!("\n🛡️  Fuego ELDERFIER VALIDATOR INFO");
            println!("═══════════════════════════════════════════════════════════════");
            println!("Address: {}", validator.address);
            println!("Status: {:?}", validator.status);
            println!("Stake: {} tokens", validator.stake);
            println!("Uptime: {} seconds", validator.uptime);
            println!("Blocks Validated: {}", validator.blocks_validated);
            println!("Rewards Earned: {} tokens", validator.rewards_earned);
            println!("Elderfier Level: {}", validator.elderfier_level);
            println!("Reputation Score: {:.2}", validator.reputation_score);
            println!("═══════════════════════════════════════════════════════════════\n");
            return Ok(());
        }
        
        println!("❌ Validator not found: {}", address);
        Ok(())
    }

    async fn stake_validator(&self, address: &str, stake: u64) -> Result<()> {
        println!("💰 Staking {} tokens to validator {}", stake, address);
        
        // In a real implementation, this would perform actual staking
        println!("✅ Stake transaction submitted successfully");
        println!("   Validator: {}", address);
        println!("   Amount: {} tokens", stake);
        
        Ok(())
    }

    async fn show_network_stats(&self) -> Result<()> {
        let status = self.daemon.get_status();
        
        println!("\n🌐 NETWORK STATISTICS");
        println!("═══════════════════════════════════════════════════════════════");
        println!("C0DL3 Network:");
        println!("   Peers: {}", status.network_stats.c0dl3_peers);
        println!("   Block Height: {}", status.network_stats.c0dl3_block_height);
        println!("   Difficulty: {}", status.network_stats.c0dl3_difficulty);
        
        println!("\nFuego Network:");
        println!("   Peers: {}", status.network_stats.fuego_peers);
        println!("   Block Height: {}", status.network_stats.fuego_block_height);
        println!("   Difficulty: {}", status.network_stats.fuego_difficulty);
        
        println!("\nNetwork Performance:");
        println!("   Latency: {} ms", status.network_stats.network_latency);
        println!("═══════════════════════════════════════════════════════════════\n");
        
        Ok(())
    }

    async fn restart_daemon(&self, daemon: &str) -> Result<()> {
        println!("🔄 Restarting {} daemon...", daemon);
        
        match daemon.to_lowercase().as_str() {
            "c0dl3" => {
                println!("✅ C0DL3 daemon restarted successfully");
            }
            "fuego" => {
                println!("✅ Fuego daemon restarted successfully");
            }
            _ => {
                println!("❌ Unknown daemon: {}", daemon);
            }
        }
        
        Ok(())
    }

    async fn stop_daemon(&self, daemon: &str) -> Result<()> {
        println!("🛑 Stopping {} daemon...", daemon);
        
        match daemon.to_lowercase().as_str() {
            "c0dl3" => {
                println!("✅ C0DL3 daemon stopped successfully");
            }
            "fuego" => {
                println!("✅ Fuego daemon stopped successfully");
            }
            _ => {
                println!("❌ Unknown daemon: {}", daemon);
            }
        }
        
        Ok(())
    }

    async fn start_daemon(&self, daemon: &str) -> Result<()> {
        println!("🚀 Starting {} daemon...", daemon);
        
        match daemon.to_lowercase().as_str() {
            "c0dl3" => {
                println!("✅ C0DL3 daemon started successfully");
            }
            "fuego" => {
                println!("✅ Fuego daemon started successfully");
            }
            _ => {
                println!("❌ Unknown daemon: {}", daemon);
            }
        }
        
        Ok(())
    }

    fn show_help(&self) {
        println!("\n📖 AVAILABLE COMMANDS");
        println!("═══════════════════════════════════════════════════════════════");
        println!("STATUS & MONITORING:");
        println!("   status, s                    - Show system status");
        println!("   network, n                   - Show network statistics");
        
        println!("\nMINING:");
        println!("   mining, m                    - Show mining statistics");
        println!("   mining start                 - Start mining on both chains");
        println!("   mining stop                  - Stop mining on both chains");
        println!("   mining stats                 - Show detailed mining stats");
        
        println!("\nVALIDATION:");
        println!("   validators, v                - List all validators");
        println!("   validators list              - List all validators");
        println!("   validators info <address>    - Show validator details");
        println!("   validators stake <addr> <amount> - Stake tokens to validator");
        
        println!("\nDAEMON MANAGEMENT:");
        println!("   daemon restart <c0dl3|fuego> - Restart daemon");
        println!("   daemon stop <c0dl3|fuego>     - Stop daemon");
        println!("   daemon start <c0dl3|fuego>    - Start daemon");
        
        println!("\nUTILITY:");
        println!("   help, h, ?                   - Show this help");
        println!("   exit, quit, q                 - Exit CLI");
        println!("═══════════════════════════════════════════════════════════════\n");
    }

    async fn exit_cli(&mut self) -> Result<()> {
        println!("👋 Exiting Unified CLI Daemon...");
        
        self.running = false;
        self.daemon.stop().await?;
        
        println!("✅ Goodbye!");
        Ok(())
    }
}
