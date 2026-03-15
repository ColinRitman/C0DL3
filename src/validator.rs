// Validator Module for C0DL3 zkSync Era Hyperchain (L3)
//
// C0DL3 validators (Elderados) can qualify via two paths:
//
// Path 1 - Elderfier Bridge-Stake: Prove 4000 XFG Elderfier stake on Fuego L1
//   via STARK proof of 5x 800 XFG deposits (0xEF tx_extra tags). Infrastructure operator path.
//
// Path 2 - HEAT Native Stake: Stake 40B HEAT directly on L3. Capital provider path.
//
// Both paths can be combined for higher validator weight.
//
// Cascading slash:
//   - Fuego Elderfier slashed → L3 Elderado auto-suspended
//   - L3 Elderado slashed → HEAT burned + Elderfier flagged for Elder Council review
//
// Settlement goes to Ethereum via zkSync Era, NOT to Fuego.
// Fuego connection is for proof verification and validator qualification only.

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{info, warn, debug, error};

use crate::fuego_l1_client::FuegoL1Client;

// xfg-stark STARK proof verification (Winterfell 0.8, post-quantum safe)
// Re-exports winterfell types via xfg_stark::winterfell::*
use xfg_stark::burn_mint_verifier::XfgBurnMintVerifier;
use xfg_stark::burn_mint_air::BurnMintPublicInputs;
use xfg_stark::winterfell::{StarkProof as WinterfellStarkProof, Deserializable, SliceReader};
use xfg_stark::winterfell::math::fields::f64::BaseElement;

// ──────────────────────────────────────────────
// Constants (from fuego-suite CryptoNoteConfig.h)
// ──────────────────────────────────────────────

/// Minimum stake per 0xEF deposit: 800 XFG = 8,000,000,000 atomic units (7 decimal places)
pub const ELDERFIER_MIN_STAKE_PER_DEPOSIT: u64 = 8_000_000_000;

/// Number of deposits required for Elderfier registration
pub const ELDERFIER_REQUIRED_DEPOSITS: usize = 5;

/// Total minimum Elderfier stake: 4000 XFG = 40,000,000,000 atomic units
pub const ELDERFIER_TOTAL_MIN_STAKE: u64 = ELDERFIER_MIN_STAKE_PER_DEPOSIT * ELDERFIER_REQUIRED_DEPOSITS as u64;

/// Minimum HEAT native stake for L3 validator: 40 Billion HEAT
/// In fwei: 40,000,000,000 HEAT * 1,000,000,000 fwei/HEAT = 40_000_000_000_000_000_000 fwei
pub const HEAT_NATIVE_MIN_STAKE: u64 = 40_000_000_000; // In HEAT tokens (not fwei, for display)

/// Commitment type for Elderfier staking deposits on Fuego
pub const COMMITMENT_TYPE_ELDERFIER: u8 = 2;

// ──────────────────────────────────────────────
// Stake types and status
// ──────────────────────────────────────────────

/// How a validator qualified for L3
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StakeType {
    /// Path 1: Bridged Elderfier stake from Fuego (4000 XFG via STARK proof)
    Elderfier,
    /// Path 2: Native HEAT stake on L3 (40B HEAT)
    HeatNative,
    /// Both paths combined (highest weight)
    Both,
}

/// Slash/suspension status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SlashStatus {
    /// No slashing events
    Clean,
    /// Suspended due to L1 Elderfier slash (cascading)
    L1Suspended,
    /// Slashed on L3 (HEAT burned, flagged for Elder Council review)
    L3Slashed,
}

// ──────────────────────────────────────────────
// Validator types
// ──────────────────────────────────────────────

/// STARK proof of Elderfier stake on Fuego (Path 1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarkStakeProof {
    /// Fuego address that made the 0xEF deposits
    pub elderfier_address: String,
    /// Commitment hashes for the 5 deposits (0xEF type)
    pub deposit_commitments: Vec<String>,
    /// Serialized Winterfell STARK proof bytes
    pub stark_proof_data: Vec<u8>,
    /// Merkle proofs for each commitment (proving inclusion in CommitmentIndex)
    pub merkle_proofs: Vec<Vec<String>>,
    /// Total claimed stake in atomic units (should be >= 40,000,000,000)
    pub total_stake: u64,
    /// Timestamp of proof generation
    pub proof_timestamp: u64,
}

/// HEAT native stake proof (Path 2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeatStakeProof {
    /// L3 address holding the HEAT stake
    pub l3_address: String,
    /// Amount staked in HEAT tokens (should be >= 40B)
    pub heat_amount: u64,
    /// Staking transaction hash on L3
    pub stake_tx_hash: String,
}

/// Validator registration request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorRegistration {
    /// Elderfier STARK stake proof (Path 1, optional)
    pub elderfier_proof: Option<StarkStakeProof>,
    /// HEAT native stake proof (Path 2, optional)
    pub heat_proof: Option<HeatStakeProof>,
    /// Public key for L3 block signing (hex-encoded)
    pub signing_pubkey: String,
}

/// Information about a registered validator (Elderado)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorInfo {
    /// Unique validator ID (derived from signing public key)
    pub validator_id: String,
    /// Fuego Elderfier address (if Path 1)
    pub elderfier_address: Option<String>,
    /// HEAT native stake amount (if Path 2)
    pub heat_stake: u64,
    /// Public key for L3 block signing
    pub signing_pubkey: String,
    /// How this validator qualified
    pub stake_type: StakeType,
    /// Validator weight for block proposal selection (1 or 2)
    pub weight: u32,
    /// Verified XFG stake in atomic units (if Path 1)
    pub verified_stake_xfg: u64,
    /// Whether currently active for block production
    pub is_active: bool,
    /// Slash/suspension status
    pub slash_status: SlashStatus,
    /// When the validator was registered (unix timestamp)
    pub registered_at: u64,
    /// Number of blocks produced
    pub blocks_produced: u64,
    /// Last block produced height
    pub last_block_height: u64,
}

// ──────────────────────────────────────────────
// Validator Registry
// ──────────────────────────────────────────────

/// Registry of all known L3 validators (Elderados)
pub struct ValidatorRegistry {
    /// Registered validators keyed by validator_id
    validators: Arc<Mutex<HashMap<String, ValidatorInfo>>>,
    /// Fuego L1 client for proof verification (not settlement)
    fuego_client: Arc<FuegoL1Client>,
    /// Whether testnet mode (relaxed validation)
    testnet_mode: bool,
}

impl ValidatorRegistry {
    /// Create a new validator registry
    pub fn new(fuego_client: Arc<FuegoL1Client>, testnet_mode: bool) -> Self {
        Self {
            validators: Arc::new(Mutex::new(HashMap::new())),
            fuego_client,
            testnet_mode,
        }
    }

    /// Register a new validator (Elderado) with their stake proof(s)
    pub async fn register_validator(&self, registration: ValidatorRegistration) -> Result<ValidatorInfo> {
        // Must have at least one stake path
        if registration.elderfier_proof.is_none() && registration.heat_proof.is_none() {
            return Err(anyhow!("Registration requires at least one stake proof (Elderfier STARK or HEAT native)"));
        }

        let mut verified_xfg: u64 = 0;
        let mut verified_heat: u64 = 0;
        let mut elderfier_addr: Option<String> = None;

        // Verify Path 1: Elderfier bridge-stake
        if let Some(ref proof) = registration.elderfier_proof {
            info!("Verifying Elderfier stake proof for address: {}", proof.elderfier_address);
            verified_xfg = self.verify_elderfier_stake(proof).await?;
            elderfier_addr = Some(proof.elderfier_address.clone());
            info!("Elderfier stake verified: {} atomic units ({} XFG)",
                verified_xfg, verified_xfg as f64 / 10_000_000.0);
        }

        // Verify Path 2: HEAT native stake
        if let Some(ref proof) = registration.heat_proof {
            info!("Verifying HEAT native stake for address: {}", proof.l3_address);
            verified_heat = self.verify_heat_stake(proof).await?;
            info!("HEAT stake verified: {} HEAT", verified_heat);
        }

        // Determine stake type and weight
        let stake_type = match (registration.elderfier_proof.is_some(), registration.heat_proof.is_some()) {
            (true, true) => StakeType::Both,
            (true, false) => StakeType::Elderfier,
            (false, true) => StakeType::HeatNative,
            (false, false) => unreachable!(), // Already checked above
        };

        let weight = match stake_type {
            StakeType::Both => 2,
            StakeType::Elderfier | StakeType::HeatNative => 1,
        };

        let validator_id = self.derive_validator_id(&registration.signing_pubkey);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let validator_info = ValidatorInfo {
            validator_id: validator_id.clone(),
            elderfier_address: elderfier_addr,
            heat_stake: verified_heat,
            signing_pubkey: registration.signing_pubkey.clone(),
            stake_type: stake_type.clone(),
            weight,
            verified_stake_xfg: verified_xfg,
            is_active: true,
            slash_status: SlashStatus::Clean,
            registered_at: now,
            blocks_produced: 0,
            last_block_height: 0,
        };

        // Add to registry
        {
            let mut validators = self.validators.lock().unwrap();
            validators.insert(validator_id.clone(), validator_info.clone());
        }

        info!("Elderado validator registered: {} (type: {:?}, weight: {})",
            validator_id, stake_type, weight);

        Ok(validator_info)
    }

    // ──────────────────────────────────────────
    // Stake verification
    // ──────────────────────────────────────────

    /// Verify Elderfier stake via STARK proof + Fuego L1 cross-check
    async fn verify_elderfier_stake(&self, proof: &StarkStakeProof) -> Result<u64> {
        // Check minimum number of deposits
        if proof.deposit_commitments.len() < ELDERFIER_REQUIRED_DEPOSITS {
            return Err(anyhow!(
                "Insufficient deposits: {} (need {} for Elderfier)",
                proof.deposit_commitments.len(), ELDERFIER_REQUIRED_DEPOSITS
            ));
        }

        // Check minimum total stake
        if proof.total_stake < ELDERFIER_TOTAL_MIN_STAKE {
            return Err(anyhow!(
                "Insufficient stake: {} atomic units (need {} = 4000 XFG)",
                proof.total_stake, ELDERFIER_TOTAL_MIN_STAKE
            ));
        }

        // Check STARK proof data present
        if proof.stark_proof_data.is_empty() {
            return Err(anyhow!("STARK proof data is empty"));
        }

        // Check merkle proofs match deposits
        if proof.merkle_proofs.len() != proof.deposit_commitments.len() {
            return Err(anyhow!(
                "Merkle proof count ({}) doesn't match deposit count ({})",
                proof.merkle_proofs.len(), proof.deposit_commitments.len()
            ));
        }

        // Cross-check against Fuego L1
        if self.testnet_mode {
            self.verify_elderfier_testnet(proof).await
        } else {
            self.verify_elderfier_mainnet(proof).await
        }
    }

    /// Full Fuego L1 verification for Elderfier stake
    async fn verify_elderfier_mainnet(&self, proof: &StarkStakeProof) -> Result<u64> {
        let mut total_verified: u64 = 0;

        for (i, commitment_hash) in proof.deposit_commitments.iter().enumerate() {
            let commitment = self.fuego_client.get_commitment(commitment_hash).await?;

            if !commitment.found {
                return Err(anyhow!("Deposit commitment {} not found on Fuego L1", commitment_hash));
            }

            if commitment.commitment_type != COMMITMENT_TYPE_ELDERFIER {
                return Err(anyhow!(
                    "Commitment {} is type {} (expected {} for ELDERFIER_STAKING)",
                    commitment_hash, commitment.commitment_type, COMMITMENT_TYPE_ELDERFIER
                ));
            }

            if commitment.amount < ELDERFIER_MIN_STAKE_PER_DEPOSIT {
                return Err(anyhow!(
                    "Deposit {} has amount {} (minimum {} per deposit = 800 XFG)",
                    i, commitment.amount, ELDERFIER_MIN_STAKE_PER_DEPOSIT
                ));
            }

            total_verified += commitment.amount;
            debug!("Verified Elderfier deposit {}: {} atomic units", i, commitment.amount);
        }

        if total_verified < ELDERFIER_TOTAL_MIN_STAKE {
            return Err(anyhow!(
                "Total verified stake {} < minimum {} (4000 XFG)",
                total_verified, ELDERFIER_TOTAL_MIN_STAKE
            ));
        }

        // Verify the Winterfell STARK proof using xfg-stark verifier
        // STARKs are post-quantum safe (hash-based, no elliptic curves)
        if !proof.stark_proof_data.is_empty() {
            match self.verify_stark_proof(proof) {
                Ok(true) => {
                    info!("STARK proof verified successfully for Elderfier stake of {} atomic units", total_verified);
                }
                Ok(false) => {
                    return Err(anyhow!("STARK proof verification failed: proof does not validate"));
                }
                Err(e) => {
                    return Err(anyhow!("STARK proof verification error: {}", e));
                }
            }
        } else {
            return Err(anyhow!("Empty STARK proof data — cannot verify Elderfier stake"));
        }

        Ok(total_verified)
    }

    /// Verify a Winterfell STARK proof for Elderfier stake
    ///
    /// Deserializes the proof bytes and verifies using xfg-stark's XfgBurnMintVerifier.
    /// The STARK proof cryptographically binds the stake deposits to the Elderfier address
    /// without revealing private deposit details.
    fn verify_stark_proof(&self, proof: &StarkStakeProof) -> Result<bool> {
        // Deserialize Winterfell StarkProof from bytes
        let mut reader = SliceReader::new(&proof.stark_proof_data);
        let winterfell_proof = WinterfellStarkProof::read_from(&mut reader)
            .map_err(|e| anyhow!("Failed to deserialize STARK proof: {:?}", e))?;

        // Create verifier with 128-bit security parameter
        let verifier = XfgBurnMintVerifier::new(128);

        // Construct public inputs from the stake proof data.
        // For Elderfier staking: total_stake is both burn (from Fuego) and mint (L3 rights).
        let public_inputs = BurnMintPublicInputs {
            burn_amount: BaseElement::from(proof.total_stake as u32),
            mint_amount: BaseElement::from(proof.total_stake as u32),
            txn_hash: BaseElement::from(0u32),
            recipient_hash: BaseElement::from(0u32),
            state: BaseElement::from(3u32), // 3 = complete
            tx_prefix_hash_0: BaseElement::from(0u32),
            tx_prefix_hash_1: BaseElement::from(0u32),
            tx_prefix_hash_2: BaseElement::from(0u32),
            tx_prefix_hash_3: BaseElement::from(0u32),
            network_id: BaseElement::from(1u32),       // Fuego network
            target_chain_id: BaseElement::from(324u32), // C0DL3 chain
            commitment_version: BaseElement::from(1u32),
        };

        verifier.verify_with_public_inputs(&winterfell_proof, &public_inputs)
            .map_err(|e| anyhow!("STARK proof verification failed: {:?}", e))
    }

    /// Testnet Elderfier verification (fallback to claimed stake if L1 unavailable)
    async fn verify_elderfier_testnet(&self, proof: &StarkStakeProof) -> Result<u64> {
        if self.fuego_client.is_connected() {
            match self.verify_elderfier_mainnet(proof).await {
                Ok(stake) => return Ok(stake),
                Err(e) => warn!("Testnet: L1 verification failed ({}), using claimed stake", e),
            }
        } else {
            warn!("Testnet: Fuego L1 not connected, accepting claimed stake");
        }
        Ok(proof.total_stake)
    }

    /// Verify HEAT native stake on L3
    async fn verify_heat_stake(&self, proof: &HeatStakeProof) -> Result<u64> {
        if proof.heat_amount < HEAT_NATIVE_MIN_STAKE {
            return Err(anyhow!(
                "Insufficient HEAT stake: {} (minimum {} = 40B HEAT)",
                proof.heat_amount, HEAT_NATIVE_MIN_STAKE
            ));
        }

        // TODO: Verify the stake transaction exists in L3 state
        // For testnet, accept the claimed amount
        if self.testnet_mode {
            info!("Testnet: accepting claimed HEAT stake of {}", proof.heat_amount);
            return Ok(proof.heat_amount);
        }

        // Mainnet: verify against RollupState
        // This would check that the staking tx is confirmed and the funds are locked
        Ok(proof.heat_amount)
    }

    /// Derive a validator ID from the signing public key
    fn derive_validator_id(&self, signing_pubkey: &str) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(signing_pubkey.as_bytes());
        let result = hasher.finalize();
        format!("v-{}", hex::encode(&result[..8]))
    }

    // ──────────────────────────────────────────
    // Cascading slash
    // ──────────────────────────────────────────

    /// Process L1 Elderfier slash → cascade to L3 suspension
    pub fn process_l1_slash(&self, elderfier_address: &str) {
        let mut validators = self.validators.lock().unwrap();
        for validator in validators.values_mut() {
            if validator.elderfier_address.as_deref() == Some(elderfier_address) {
                if validator.slash_status == SlashStatus::Clean {
                    warn!("Cascading L1 slash: suspending L3 validator {} (Elderfier {} slashed on Fuego)",
                        validator.validator_id, elderfier_address);
                    validator.slash_status = SlashStatus::L1Suspended;
                    validator.is_active = false;
                }
            }
        }
    }

    /// Process L3 slash → burn HEAT + flag Elderfier
    pub fn process_l3_slash(&self, validator_id: &str) -> Option<String> {
        let mut validators = self.validators.lock().unwrap();
        if let Some(validator) = validators.get_mut(validator_id) {
            warn!("L3 slash: validator {} slashed, HEAT stake burned", validator_id);
            validator.slash_status = SlashStatus::L3Slashed;
            validator.is_active = false;
            validator.heat_stake = 0; // HEAT burned

            // Return Elderfier address for Elder Council flagging (soft signal)
            return validator.elderfier_address.clone();
        }
        None
    }

    // ──────────────────────────────────────────
    // Query methods
    // ──────────────────────────────────────────

    /// Get all active validators
    pub fn get_active_validators(&self) -> Vec<ValidatorInfo> {
        let validators = self.validators.lock().unwrap();
        validators.values()
            .filter(|v| v.is_active && v.slash_status == SlashStatus::Clean)
            .cloned()
            .collect()
    }

    /// Get all validators (any status)
    pub fn get_all_validators(&self) -> Vec<ValidatorInfo> {
        let validators = self.validators.lock().unwrap();
        validators.values().cloned().collect()
    }

    /// Get a specific validator by ID
    pub fn get_validator(&self, validator_id: &str) -> Option<ValidatorInfo> {
        let validators = self.validators.lock().unwrap();
        validators.get(validator_id).cloned()
    }

    /// Check if a validator ID is registered and active
    pub fn is_valid_block_proposer(&self, validator_id: &str) -> bool {
        let validators = self.validators.lock().unwrap();
        validators.get(validator_id)
            .map(|v| v.is_active && v.slash_status == SlashStatus::Clean)
            .unwrap_or(false)
    }

    /// Get the number of active validators
    pub fn active_validator_count(&self) -> usize {
        self.get_active_validators().len()
    }

    /// Select the next block proposer (weighted round-robin)
    /// Higher-weight validators get proportionally more proposals
    pub fn select_block_proposer(&self, block_height: u64) -> Option<ValidatorInfo> {
        let active = self.get_active_validators();
        if active.is_empty() {
            return None;
        }

        // Build weighted list: weight=2 validators appear twice
        let mut weighted: Vec<&ValidatorInfo> = Vec::new();
        for v in &active {
            for _ in 0..v.weight {
                weighted.push(v);
            }
        }

        if weighted.is_empty() {
            return None;
        }

        let index = (block_height as usize) % weighted.len();
        Some(weighted[index].clone())
    }

    /// Record a block produced by a validator
    pub fn record_block_produced(&self, validator_id: &str, block_height: u64) {
        let mut validators = self.validators.lock().unwrap();
        if let Some(validator) = validators.get_mut(validator_id) {
            validator.blocks_produced += 1;
            validator.last_block_height = block_height;
        }
    }

    /// Get validator registry status as JSON for RPC
    pub fn get_status_json(&self) -> serde_json::Value {
        let all = self.get_all_validators();
        let active = all.iter().filter(|v| v.is_active && v.slash_status == SlashStatus::Clean);
        let active_count = active.clone().count();
        let total_xfg_stake: u64 = active.clone().map(|v| v.verified_stake_xfg).sum();
        let total_heat_stake: u64 = active.clone().map(|v| v.heat_stake).sum();
        let total_weight: u32 = active.map(|v| v.weight).sum();

        serde_json::json!({
            "total_validators": all.len(),
            "active_validators": active_count,
            "total_xfg_stake_atomic": total_xfg_stake,
            "total_xfg_stake_xfg": total_xfg_stake as f64 / 10_000_000.0,
            "total_heat_stake": total_heat_stake,
            "total_weight": total_weight,
            "validators": all,
        })
    }
}
