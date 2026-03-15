// zkC0DL3 SP1 Guest Program — Block Execution Prover
//
// This program runs inside SP1's RISC-V zkVM. It proves:
//   1. Correct EVM execution (via revm) for all transactions in a block
//   2. Privacy validation (Pedersen commitment conservation, nullifier freshness)
//   3. State root transition (prev_state_root → new_state_root)
//
// Architecture (Option B — SP1 + revm Hybrid):
//   - revm executes EVM transactions (Solidity contracts, ERC-20 ops)
//   - Custom precompiles handle privacy ops (Ristretto255, Pedersen, Bulletproofs)
//   - Shielded pool state transitions are validated in-circuit
//   - SP1 proves the entire execution and outputs a SNARK
//
// Build:
//   cd program && cargo prove build
//
// The host (prover node) feeds block data via SP1Stdin. The guest reads it,
// re-executes all transactions, validates privacy invariants, and
// commits the BlockExecutionClaim as public output.

#![no_main]
sp1_zkvm::entrypoint!(main);

mod precompiles;
mod privacy;
mod state;

use privacy::ShieldedBlockData;
use revm::{
    db::CacheDB,
    primitives::{
        Address, ExecutionResult, SpecId, TransactTo, TxEnv, U256,
    },
    Evm,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use state::{AccountWitness, WitnessDatabase};

// ── Shared Types ────────────────────────────────────────────────────────────
//
// These must match the host-side types in `src/proving/mod.rs`.

/// Block execution claim — public outputs committed by this proof.
/// Both the prover (this guest) and verifier (node) agree on these values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockExecutionClaim {
    pub block_height: u64,
    pub prev_state_root: [u8; 32],
    pub new_state_root: [u8; 32],
    pub tx_merkle_root: [u8; 32],
    pub tx_count: u32,
    pub total_gas_used: u64,
    pub note_tree_root: [u8; 32],
    pub nullifier_count: u32,
}

impl BlockExecutionClaim {
    /// Encode as deterministic bytes for SP1 public values commitment.
    /// Layout: block_height(8) || prev_state_root(32) || new_state_root(32)
    ///         || tx_merkle_root(32) || tx_count(4) || total_gas_used(8)
    ///         || note_tree_root(32) || nullifier_count(4)
    /// Total: 152 bytes
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(152);
        buf.extend_from_slice(&self.block_height.to_le_bytes());
        buf.extend_from_slice(&self.prev_state_root);
        buf.extend_from_slice(&self.new_state_root);
        buf.extend_from_slice(&self.tx_merkle_root);
        buf.extend_from_slice(&self.tx_count.to_le_bytes());
        buf.extend_from_slice(&self.total_gas_used.to_le_bytes());
        buf.extend_from_slice(&self.note_tree_root);
        buf.extend_from_slice(&self.nullifier_count.to_le_bytes());
        buf
    }
}

/// Transaction for guest-side re-execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestTransaction {
    pub from: [u8; 20],
    /// None = contract creation, Some = call
    pub to: Option<[u8; 20]>,
    pub value: u64,
    pub gas_limit: u64,
    pub gas_price: u64,
    pub nonce: u64,
    pub data: Vec<u8>,
}

/// Block input data fed to the guest via SP1Stdin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestBlockInput {
    pub block_height: u64,
    pub prev_state_root: [u8; 32],
    pub transactions: Vec<GuestTransaction>,
    /// Pre-state account witnesses (private input — balances are plaintext here).
    pub accounts: Vec<AccountWitness>,
    /// Shielded pool note tree root from the previous block.
    pub prev_note_tree_root: [u8; 32],
    /// Shielded pool transitions for this block.
    pub shielded: ShieldedBlockData,
    /// Block gas limit.
    pub block_gas_limit: u64,
    /// Block timestamp (seconds since epoch).
    pub timestamp: u64,
}

// ── Main Entry Point ────────────────────────────────────────────────────────

pub fn main() {
    // ═══ Phase 1: Read block input from prover host ═══
    let input: GuestBlockInput = sp1_zkvm::io::read();

    // ═══ Phase 2: Verify pre-state witness ═══
    //
    // The prover claims these accounts constitute the pre-state.
    // We verify by computing the state root from the witness and comparing
    // against the publicly known prev_state_root.
    let computed_prev_root = state::compute_state_root(&input.accounts);
    assert_eq!(
        computed_prev_root, input.prev_state_root,
        "pre-state witness does not match prev_state_root — prover submitted invalid witness"
    );

    // ═══ Phase 3: Execute EVM transactions via revm ═══
    //
    // Build revm with:
    //   - WitnessDatabase: serves account state from the verified pre-state
    //   - CacheDB: tracks state mutations during execution
    //   - Custom precompiles: Ristretto255, Pedersen, Bulletproofs, Shield, Unshield
    let witness_db = WitnessDatabase::new(&input.accounts);
    let address_map = witness_db.address_map.clone();
    let cache_db = CacheDB::new(witness_db);

    let mut evm = Evm::builder()
        .with_db(cache_db)
        .with_spec_id(SpecId::CANCUN)
        .modify_env(|env| {
            env.block.number = U256::from(input.block_height);
            env.block.gas_limit = U256::from(input.block_gas_limit);
            env.block.timestamp = U256::from(input.timestamp);
            env.block.basefee = U256::ZERO;
        })
        .append_handler_register(precompiles::register_cold_precompiles)
        .build();

    let mut total_gas_used: u64 = 0;

    for tx in &input.transactions {
        // Configure transaction environment
        *evm.tx_mut() = TxEnv {
            caller: Address::from(tx.from),
            transact_to: match tx.to {
                Some(addr) => TransactTo::Call(Address::from(addr)),
                None => TransactTo::Create,
            },
            value: U256::from(tx.value),
            gas_limit: tx.gas_limit,
            gas_price: U256::from(tx.gas_price),
            nonce: Some(tx.nonce),
            data: tx.data.clone().into(),
            ..Default::default()
        };

        // Execute transaction — all state changes applied to CacheDB
        let result = evm
            .transact_commit()
            .expect("EVM execution failed — this should not happen with valid block data");

        // Accumulate gas
        let gas = gas_used_from_result(&result);
        total_gas_used += gas;
    }

    // ═══ Phase 4: Compute new state root ═══
    //
    // Extract post-execution state from CacheDB and compute the new state root.
    // The state root is SHA-256 of sorted (address_string || pedersen_commitment || nonce).
    let (cache_db, _) = evm.into_db_and_env_with_handler_cfg();
    let new_accounts = state::build_post_state(
        &cache_db.accounts,
        &address_map,
        &input.accounts,
    );
    let new_state_root = state::compute_state_root(&new_accounts);

    // ═══ Phase 5: Compute transaction Merkle root ═══
    let tx_merkle_root = compute_tx_merkle_root(&input.transactions);
    let tx_count = input.transactions.len() as u32;

    // ═══ Phase 6: Validate shielded pool transitions ═══
    //
    // Check conservation, nullifier freshness, and compute updated note tree root.
    let (note_tree_root, nullifier_count) = if input.shielded.spend_proofs.is_empty()
        && input.shielded.new_notes.is_empty()
        && input.shielded.nullifiers.is_empty()
    {
        // No shielded activity — note tree unchanged
        (input.prev_note_tree_root, 0u32)
    } else {
        privacy::validate_shielded_block(&input.shielded, &input.prev_note_tree_root)
    };

    // ═══ Phase 7: Commit public outputs ═══
    //
    // The BlockExecutionClaim is the proof's public output. The host verifier
    // reconstructs the same claim from block data and checks it matches.
    let claim = BlockExecutionClaim {
        block_height: input.block_height,
        prev_state_root: input.prev_state_root,
        new_state_root,
        tx_merkle_root,
        tx_count,
        total_gas_used,
        note_tree_root,
        nullifier_count,
    };

    let encoded = claim.encode();
    assert_eq!(
        encoded.len(),
        152,
        "BlockExecutionClaim encoding must be exactly 152 bytes"
    );

    // Commit to SP1's public values — this is what the verifier checks against
    sp1_zkvm::io::commit_slice(&encoded);
}

// ── Helper Functions ────────────────────────────────────────────────────────

/// Extract gas used from an EVM execution result.
fn gas_used_from_result(result: &ExecutionResult) -> u64 {
    match result {
        ExecutionResult::Success { gas_used, .. } => *gas_used,
        ExecutionResult::Revert { gas_used, .. } => *gas_used,
        ExecutionResult::Halt { gas_used, .. } => *gas_used,
    }
}

/// Compute SHA-256 Merkle root over transaction hashes.
///
/// Must match the host-side transaction Merkle root computation.
fn compute_tx_merkle_root(txs: &[GuestTransaction]) -> [u8; 32] {
    if txs.is_empty() {
        let mut h = Sha256::new();
        h.update(b"empty_block");
        return h.finalize().into();
    }

    // Hash each tx to a leaf
    let leaves: Vec<[u8; 32]> = txs
        .iter()
        .map(|tx| {
            let mut h = Sha256::new();
            h.update(b"COLDL3:tx:v1");
            h.update(&tx.from);
            h.update(tx.to.as_ref().unwrap_or(&[0u8; 20]));
            h.update(&tx.value.to_le_bytes());
            h.update(&tx.nonce.to_le_bytes());
            h.update(&tx.data);
            h.finalize().into()
        })
        .collect();

    // Build Merkle tree
    let mut current = leaves;
    while current.len() > 1 {
        let mut next = Vec::new();
        let mut i = 0;
        while i < current.len() {
            let left = current[i];
            let right = if i + 1 < current.len() {
                current[i + 1]
            } else {
                current[i] // duplicate last if odd
            };
            let mut h = Sha256::new();
            h.update(left);
            h.update(right);
            next.push(h.finalize().into());
            i += 2;
        }
        current = next;
    }
    current[0]
}
