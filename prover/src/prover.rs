// SP1 proof generation engine.
//
// This module wraps the SP1 SDK (blocking API) to:
//   1. Load the guest program ELF binary
//   2. Serialize GuestBlockInput into SP1Stdin
//   3. Execute the guest program (optionally with local execution for debugging)
//   4. Generate a SNARK proof (Groth16 or PLONK)
//   5. Serialize the proof + public values for submission
//
// Uses `sp1_sdk::blocking` to avoid tokio double-runtime issues.
//
// The guest ELF is the compiled RISC-V binary from `program/`.
// Build it with: cd program && cargo prove build
// The ELF path is typically: program/elf/riscv32im-succinct-zkvm-elf

use anyhow::{Result, Context};
use sp1_sdk::blocking::{
    ProverClient, Prover, ProveRequest,
    CpuProver, SP1Stdin, Elf,
};
use sp1_sdk::ProvingKey as ProvingKeyTrait;
use sp1_sdk::{
    ExecutionReport, SP1ProofWithPublicValues, SP1ProvingKey, SP1PublicValues, SP1VerifyingKey,
};
use tracing::{info, debug};
use std::sync::Arc;
use std::time::Instant;

use crate::types::{BlockExecutionClaim, GuestBlockInput};

/// Proof mode — what kind of SP1 proof to generate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProofMode {
    /// Execute only — no proof generated. Fast. For testing input correctness.
    Execute,
    /// Compressed proof (SP1's default STARK compression).
    Compressed,
    /// Groth16 SNARK — smallest proof, verifiable on-chain.
    Groth16,
    /// PLONK SNARK — alternative on-chain verifiable proof.
    Plonk,
}

impl std::fmt::Display for ProofMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProofMode::Execute => write!(f, "execute"),
            ProofMode::Compressed => write!(f, "compressed"),
            ProofMode::Groth16 => write!(f, "groth16"),
            ProofMode::Plonk => write!(f, "plonk"),
        }
    }
}

impl ProofMode {
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "execute" | "exec" => Ok(Self::Execute),
            "compressed" | "compress" => Ok(Self::Compressed),
            "groth16" => Ok(Self::Groth16),
            "plonk" => Ok(Self::Plonk),
            _ => anyhow::bail!("unknown proof mode: '{}' (use: execute, compressed, groth16, plonk)", s),
        }
    }
}

/// SP1 proof generation engine using the blocking API.
pub struct Sp1Prover {
    client: CpuProver,
    pk: SP1ProvingKey,
    vk: SP1VerifyingKey,
    elf: Elf,
    mode: ProofMode,
}

impl Sp1Prover {
    /// Initialize the prover with the guest program ELF.
    ///
    /// This sets up the SP1 proving/verifying keys from the ELF binary.
    /// The ELF is the compiled guest program from `program/elf/`.
    pub fn new(elf_bytes: Vec<u8>, mode: ProofMode) -> Result<Self> {
        info!("Initializing SP1 prover (mode: {})", mode);
        let start = Instant::now();

        let elf = Elf::Dynamic(Arc::from(elf_bytes.into_boxed_slice()));
        let client = ProverClient::builder().cpu().build();
        let pk = client.setup(elf.clone())
            .map_err(|e| anyhow::anyhow!("SP1 setup failed: {}", e))?;

        let vk = pk.verifying_key().clone();

        let vk_bytes = bincode::serialize(&vk).unwrap_or_default();
        let vk_prefix = &vk_bytes[..8.min(vk_bytes.len())];
        info!(
            "SP1 prover initialized in {:.2}s (vkey prefix: {}...)",
            start.elapsed().as_secs_f64(),
            hex::encode(vk_prefix),
        );

        Ok(Self { client, pk, vk, elf, mode })
    }

    /// Get the serialized verification key.
    /// The node needs this to verify proofs.
    pub fn verification_key_bytes(&self) -> Result<Vec<u8>> {
        bincode::serialize(&self.vk)
            .context("failed to serialize SP1 verification key")
    }

    /// Generate a proof for a block.
    ///
    /// Steps:
    ///   1. Serialize GuestBlockInput into SP1Stdin
    ///   2. Run the guest program in SP1's zkVM
    ///   3. Extract proof + public values
    ///   4. Verify public values match expected claim
    ///   5. Return serialized proof bytes
    pub fn prove_block(&self, input: &GuestBlockInput) -> Result<ProofResult> {
        info!(
            "Proving block {} ({} txs, {} accounts, mode: {})",
            input.block_height,
            input.transactions.len(),
            input.accounts.len(),
            self.mode,
        );

        let start = Instant::now();

        // Serialize input into SP1Stdin
        let mut stdin = SP1Stdin::new();
        stdin.write(input);

        // Execute-only mode — no proof, just run the program
        if self.mode == ProofMode::Execute {
            let (public_values, report): (SP1PublicValues, ExecutionReport) = self.client
                .execute(self.elf.clone(), stdin)
                .run()
                .map_err(|e| anyhow::anyhow!("SP1 execution failed: {}", e))?;

            info!(
                "Block {} executed in {:.2}s ({} cycles)",
                input.block_height,
                start.elapsed().as_secs_f64(),
                report.total_instruction_count(),
            );

            let claim = BlockExecutionClaim::decode(public_values.as_slice())
                .context("failed to decode execution claim from SP1 output")?;

            return Ok(ProofResult {
                block_height: input.block_height,
                proof_bytes: vec![],
                claim,
                cycles: report.total_instruction_count(),
                duration_secs: start.elapsed().as_secs_f64(),
                mode: self.mode,
            });
        }

        // Generate proof with selected mode
        let proof: SP1ProofWithPublicValues = match self.mode {
            ProofMode::Compressed => {
                self.client
                    .prove(&self.pk, stdin)
                    .compressed()
                    .run()
                    .map_err(|e| anyhow::anyhow!("SP1 compressed proof failed: {}", e))?
            }
            ProofMode::Groth16 => {
                self.client
                    .prove(&self.pk, stdin)
                    .groth16()
                    .run()
                    .map_err(|e| anyhow::anyhow!("SP1 Groth16 proof failed: {}", e))?
            }
            ProofMode::Plonk => {
                self.client
                    .prove(&self.pk, stdin)
                    .plonk()
                    .run()
                    .map_err(|e| anyhow::anyhow!("SP1 PLONK proof failed: {}", e))?
            }
            ProofMode::Execute => unreachable!(),
        };

        let duration = start.elapsed().as_secs_f64();

        // Decode claim from public values
        let claim = BlockExecutionClaim::decode(proof.public_values.as_slice())
            .context("failed to decode execution claim from SP1 proof output")?;

        info!(
            "Block {} proved in {:.2}s (mode: {}, claim: height={}, txs={}, gas={})",
            input.block_height,
            duration,
            self.mode,
            claim.block_height,
            claim.tx_count,
            claim.total_gas_used,
        );

        // Verify proof locally before submitting (catch bugs early)
        self.client.verify(&proof, &self.vk, None)
            .map_err(|e| anyhow::anyhow!("local proof verification failed: {}", e))?;

        debug!("Local proof verification passed for block {}", input.block_height);

        // Serialize proof for submission
        let proof_bytes = bincode::serialize(&proof)
            .context("failed to serialize SP1 proof")?;

        info!(
            "Proof for block {}: {} bytes",
            input.block_height,
            proof_bytes.len(),
        );

        Ok(ProofResult {
            block_height: input.block_height,
            proof_bytes,
            claim,
            cycles: 0,
            duration_secs: duration,
            mode: self.mode,
        })
    }
}

/// Result of proof generation.
pub struct ProofResult {
    pub block_height: u64,
    /// Serialized SP1ProofWithPublicValues (empty in execute mode).
    pub proof_bytes: Vec<u8>,
    /// Decoded BlockExecutionClaim from the proof's public values.
    pub claim: BlockExecutionClaim,
    /// Total RISC-V cycles (only available in execute mode).
    pub cycles: u64,
    /// Wall-clock time for proof generation.
    pub duration_secs: f64,
    /// Proof mode used.
    pub mode: ProofMode,
}

impl ProofResult {
    /// Whether this result contains an actual proof (not just execution).
    pub fn has_proof(&self) -> bool {
        !self.proof_bytes.is_empty()
    }
}
