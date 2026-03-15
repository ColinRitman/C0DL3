// zkC0DL3 Prover Service — Standalone SP1 Block Prover
//
// This binary is what GPU holders run to earn HEAT rewards.
// It connects to a COLDL3 node, downloads pending blocks,
// generates SP1 execution proofs, and submits them back.
//
// Architecture:
//   1. Poll node for pending (unproven) blocks
//   2. Download block data + pre-state witness for each
//   3. Run SP1 guest program to generate execution proof
//   4. Submit proof to node via POST /proof/submit
//   5. Repeat
//
// The guest program (program/src/main.rs) runs inside SP1's RISC-V zkVM
// and proves: EVM execution (revm) + privacy validation (Pedersen conservation,
// nullifier freshness) + state root transition.
//
// Usage:
//   # First, build the guest ELF:
//   cd program && cargo prove build
//
//   # Then run the prover:
//   cargo run -p coldl3-prover -- \
//     --node-url http://localhost:8545 \
//     --prover-address 0xYOUR_ADDRESS_HERE \
//     --elf-path program/elf/riscv32im-succinct-zkvm-elf \
//     --mode groth16
//
//   # For testing (no real proof, just execute):
//   cargo run -p coldl3-prover -- \
//     --node-url http://localhost:8545 \
//     --prover-address 0xYOUR_ADDRESS_HERE \
//     --mode execute

mod node_client;
mod prover;
mod types;

use anyhow::{Result, Context};
use clap::Parser;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{info, warn, error};

use node_client::NodeClient;
use prover::{ProofMode, Sp1Prover};
use types::ProofSubmission;

// ── CLI ──────────────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(
    name = "coldl3-prover",
    about = "zkC0DL3 Prover Service — generates SP1 execution proofs for HEAT rewards",
    version
)]
struct Cli {
    /// COLDL3 node RPC URL.
    #[arg(long, default_value = "http://localhost:8545")]
    node_url: String,

    /// Prover address (receives HEAT rewards on proof acceptance).
    #[arg(long)]
    prover_address: String,

    /// Path to the SP1 guest program ELF binary.
    /// Build with: cd program && cargo prove build
    #[arg(long, default_value = "program/elf/riscv32im-succinct-zkvm-elf")]
    elf_path: PathBuf,

    /// Proof mode: execute (test only), compressed, groth16, plonk.
    #[arg(long, default_value = "groth16")]
    mode: String,

    /// Poll interval in seconds (how often to check for pending blocks).
    #[arg(long, default_value = "5")]
    poll_interval: u64,

    /// Maximum concurrent proofs (limited by GPU memory).
    #[arg(long, default_value = "1")]
    max_concurrent: usize,

    /// Run once (prove one block and exit). Useful for testing.
    #[arg(long, default_value = "false")]
    once: bool,

    /// Specific block height to prove (skips pending block discovery).
    #[arg(long)]
    block_height: Option<u64>,

    /// Export verification key to file and exit.
    #[arg(long)]
    export_vkey: Option<PathBuf>,
}

// ── Main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    info!("zkC0DL3 Prover Service starting");
    info!("  Node URL:    {}", cli.node_url);
    info!("  Prover:      {}", cli.prover_address);
    info!("  ELF path:    {}", cli.elf_path.display());
    info!("  Proof mode:  {}", cli.mode);

    // Validate prover address
    if cli.prover_address.len() < 20 {
        anyhow::bail!(
            "Prover address too short ({}). Use a full hex address (>= 20 chars).",
            cli.prover_address.len()
        );
    }

    // Parse proof mode
    let mode = ProofMode::from_str(&cli.mode)?;

    // Load guest ELF
    let elf_bytes = std::fs::read(&cli.elf_path)
        .context(format!(
            "Failed to read guest ELF from '{}'. \
             Build it with: cd program && cargo prove build",
            cli.elf_path.display()
        ))?;
    info!("Loaded guest ELF: {} bytes", elf_bytes.len());

    // Initialize SP1 prover
    let sp1 = Sp1Prover::new(elf_bytes, mode)?;

    // Export vkey if requested
    if let Some(vkey_path) = cli.export_vkey {
        let vkey_bytes = sp1.verification_key_bytes()?;
        std::fs::write(&vkey_path, &vkey_bytes)
            .context(format!("failed to write vkey to {}", vkey_path.display()))?;
        info!("Verification key exported to {} ({} bytes)", vkey_path.display(), vkey_bytes.len());
        return Ok(());
    }

    // Initialize node client
    let node = NodeClient::new(&cli.node_url);

    // Check node connectivity
    if !node.health_check().await? {
        anyhow::bail!(
            "Cannot reach COLDL3 node at {}. Is the node running?",
            cli.node_url
        );
    }
    info!("Connected to COLDL3 node at {}", cli.node_url);

    // If a specific block height is requested, prove just that block
    if let Some(height) = cli.block_height {
        return prove_single_block(&node, &sp1, &cli.prover_address, height).await;
    }

    // Main proving loop
    if cli.once {
        prove_next_pending(&node, &sp1, &cli.prover_address).await?;
    } else {
        proving_loop(&node, &sp1, &cli.prover_address, cli.poll_interval).await?;
    }

    Ok(())
}

// ── Proving Functions ────────────────────────────────────────────────────────

/// Prove a specific block by height.
async fn prove_single_block(
    node: &NodeClient,
    sp1: &Sp1Prover,
    prover_address: &str,
    height: u64,
) -> Result<()> {
    info!("Proving specific block {}", height);

    // Fetch block input
    let input_resp = node.get_block_input(height).await?;
    let input = input_resp.block_input;

    // Generate proof
    let result = sp1.prove_block(&input)?;

    info!(
        "Block {} proof generated in {:.2}s",
        height, result.duration_secs,
    );

    if result.has_proof() {
        // Submit to node
        let submission = ProofSubmission {
            block_height: height,
            prover_address: prover_address.to_string(),
            proof_bytes: result.proof_bytes,
            submitted_at: now_unix(),
        };

        let resp = node.submit_proof(&submission).await?;
        info!("Block {} proof submitted: {:?}", height, resp);
    } else {
        info!(
            "Block {} executed successfully (no proof in execute mode). \
             Claim: {} txs, {} gas",
            height, result.claim.tx_count, result.claim.total_gas_used,
        );
    }

    Ok(())
}

/// Find and prove the next pending block.
async fn prove_next_pending(
    node: &NodeClient,
    sp1: &Sp1Prover,
    prover_address: &str,
) -> Result<()> {
    let pending = node.get_pending_blocks().await?;

    let target = pending.iter()
        .filter(|b| b.status == "pending")
        .min_by_key(|b| b.block_height);

    match target {
        Some(block) => {
            info!(
                "Found pending block {} ({} txs, {} existing submissions)",
                block.block_height, block.tx_count, block.submissions,
            );
            prove_single_block(node, sp1, prover_address, block.block_height).await
        }
        None => {
            info!("No pending blocks found");
            Ok(())
        }
    }
}

/// Continuous proving loop — poll for pending blocks and prove them.
async fn proving_loop(
    node: &NodeClient,
    sp1: &Sp1Prover,
    prover_address: &str,
    poll_interval_secs: u64,
) -> Result<()> {
    info!(
        "Starting proving loop (poll every {}s). Press Ctrl+C to stop.",
        poll_interval_secs,
    );

    let mut blocks_proven: u64 = 0;
    let mut total_proof_time: f64 = 0.0;

    loop {
        // Check node is still alive
        if !node.health_check().await.unwrap_or(false) {
            warn!("Node unreachable, retrying in {}s...", poll_interval_secs);
            tokio::time::sleep(Duration::from_secs(poll_interval_secs)).await;
            continue;
        }

        // Find pending blocks
        match node.get_pending_blocks().await {
            Ok(pending) => {
                let actionable: Vec<_> = pending.iter()
                    .filter(|b| b.status == "pending")
                    .collect();

                if actionable.is_empty() {
                    // No work to do — wait and poll again
                    tokio::time::sleep(Duration::from_secs(poll_interval_secs)).await;
                    continue;
                }

                // Prove the oldest pending block first
                let target = actionable.iter()
                    .min_by_key(|b| b.block_height)
                    .unwrap();

                info!(
                    "Proving block {} ({} pending blocks total)",
                    target.block_height,
                    actionable.len(),
                );

                match prove_single_block(node, sp1, prover_address, target.block_height).await {
                    Ok(()) => {
                        blocks_proven += 1;
                        info!(
                            "Blocks proven this session: {} (avg {:.1}s/block)",
                            blocks_proven,
                            if blocks_proven > 0 {
                                total_proof_time / blocks_proven as f64
                            } else {
                                0.0
                            },
                        );
                    }
                    Err(e) => {
                        error!("Failed to prove block {}: {}", target.block_height, e);
                        // Don't bail — continue trying other blocks
                    }
                }
            }
            Err(e) => {
                warn!("Failed to fetch pending blocks: {}", e);
            }
        }

        // Brief pause before next poll
        tokio::time::sleep(Duration::from_secs(poll_interval_secs)).await;
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
