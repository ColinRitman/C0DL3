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

/// Guest-side UserOperation for AA wallet verification.
///
/// Minimal representation for in-circuit verification of AA transfers.
/// The guest verifies: Schnorr auth, conservation, and knowledge proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestUserOperation {
    pub sender: String,
    pub nonce: u64,
    pub to: String,
    /// Sender's current (old) balance commitment.
    pub sender_old_commitment: [u8; 32],
    /// Sender's new balance commitment after transfer.
    pub sender_new_commitment: [u8; 32],
    /// Recipient's current (old) balance commitment.
    pub recipient_old_commitment: [u8; 32],
    /// Recipient's new balance commitment after transfer.
    pub recipient_new_commitment: [u8; 32],
    /// Pedersen commitment to the transfer amount.
    pub amount_commitment: [u8; 32],
    /// Knowledge proof for amount commitment.
    pub knowledge_proof: privacy::CommitmentKnowledgeProof,
    /// Schnorr signature R point (auth).
    pub auth_signature_r: [u8; 32],
    /// Schnorr signature s scalar (auth).
    pub auth_signature_s: [u8; 32],
    /// Sender's owner public key (Ristretto255).
    pub sender_pubkey: [u8; 32],
    /// Gas limit for this operation.
    pub gas_limit: u64,
    /// Gas price in fwei.
    pub gas_price: u64,
    /// Optional paymaster address.
    pub paymaster: Option<String>,
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
    /// AA UserOperations for this block.
    #[serde(default)]
    pub user_operations: Vec<GuestUserOperation>,
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

    // ═══ Phase 7: Verify AA UserOperations ═══
    //
    // Each UserOperation is verified inside the ZK circuit:
    //   1. Schnorr auth: sender proves wallet ownership
    //   2. Conservation: old_sender - new_sender == new_recipient - old_recipient
    //   3. Knowledge proof: sender knows (amount, blinding) for amount_commitment
    //
    // This makes AA verification trustless — the sequencer's validation is
    // "early rejection" only, the guest re-verifies everything.
    let mut aa_gas_used: u64 = 0;
    for (i, op) in input.user_operations.iter().enumerate() {
        // 1. Verify Schnorr auth signature
        let op_hash = compute_aa_op_hash(op);
        assert!(
            verify_schnorr_in_guest(
                &op.sender_pubkey,
                &op_hash,
                &op.auth_signature_r,
                &op.auth_signature_s,
            ),
            "AA op {}: invalid auth signature", i
        );

        // 2. Verify conservation: what sender lost = what recipient gained
        assert!(
            verify_aa_conservation(
                &op.sender_old_commitment,
                &op.sender_new_commitment,
                &op.recipient_old_commitment,
                &op.recipient_new_commitment,
            ),
            "AA op {}: conservation check failed", i
        );

        // 3. Verify knowledge proof for amount commitment
        assert!(
            privacy::verify_commitment_knowledge_proof(&op.knowledge_proof),
            "AA op {}: invalid knowledge proof", i
        );

        // 4. Verify knowledge proof matches amount commitment
        assert_eq!(
            op.knowledge_proof.commitment, op.amount_commitment,
            "AA op {}: knowledge proof commitment mismatch", i
        );

        // Accumulate gas for AA operations
        // base(21k) + conservation(4k) + auth(3k) = 28k minimum
        aa_gas_used += 28_000;
    }
    total_gas_used += aa_gas_used;

    // ═══ Phase 8: Commit public outputs ═══
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

// ── AA Verification Helpers ─────────────────────────────────────────────────
//
// These functions verify AA UserOperation fields inside the ZK circuit.
// They must match the host-side implementations in `src/aa/` exactly.

use curve25519_dalek_ng::{
    constants::RISTRETTO_BASEPOINT_POINT as G,
    ristretto::{CompressedRistretto, RistrettoPoint},
    scalar::Scalar,
    traits::Identity,
};

/// Compute the hash of a GuestUserOperation for signature verification.
///
/// H("C0DL3:userop:" || sender || nonce || to || amount_commitment || ...)
/// Must match host-side `compute_user_op_hash()` in `src/aa/validation.rs`.
fn compute_aa_op_hash(op: &GuestUserOperation) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(b"C0DL3:userop:");
    hasher.update(op.sender.as_bytes());
    hasher.update(op.nonce.to_le_bytes());
    hasher.update(op.to.as_bytes());
    hasher.update(&op.amount_commitment);
    hasher.update(&op.sender_new_commitment);
    hasher.update(&op.recipient_new_commitment);
    hasher.update(op.gas_limit.to_le_bytes());
    hasher.update(op.gas_price.to_le_bytes());
    if let Some(ref pm) = op.paymaster {
        hasher.update(pm.as_bytes());
    }
    hasher.finalize().to_vec()
}

/// Verify a Schnorr signature inside the guest.
///
/// Checks: s*G == R + e*pubkey
/// where e = H("C0DL3:schnorr:" || R || pubkey || message)
///
/// Must match host-side `schnorr_verify()` in `src/aa/schnorr.rs`.
fn verify_schnorr_in_guest(
    pubkey: &[u8; 32],
    message: &[u8],
    sig_r: &[u8; 32],
    sig_s: &[u8; 32],
) -> bool {
    let pk_point = match CompressedRistretto(*pubkey).decompress() {
        Some(p) => p,
        None => return false,
    };
    let r_point = match CompressedRistretto(*sig_r).decompress() {
        Some(p) => p,
        None => return false,
    };

    // Challenge: e = H("C0DL3:schnorr:" || R || pubkey || message)
    let mut hasher = Sha256::new();
    hasher.update(b"C0DL3:schnorr:");
    hasher.update(sig_r);
    hasher.update(pubkey);
    hasher.update(message);
    let hash: [u8; 32] = hasher.finalize().into();
    let e = Scalar::from_bytes_mod_order(hash);

    let s = Scalar::from_bytes_mod_order(*sig_s);

    // Verify: s*G == R + e*pubkey
    let lhs = s * G;
    let rhs = r_point + e * pk_point;

    lhs == rhs
}

/// Verify Pedersen commitment conservation for an AA transfer.
///
/// Checks: (old_sender - new_sender) == (new_recipient - old_recipient)
/// This means: what sender lost = what recipient gained.
///
/// Must match host-side `verify_conservation()` in `src/aa/validation.rs`.
fn verify_aa_conservation(
    old_sender: &[u8; 32],
    new_sender: &[u8; 32],
    old_recipient: &[u8; 32],
    new_recipient: &[u8; 32],
) -> bool {
    let os = match CompressedRistretto(*old_sender).decompress() {
        Some(p) => p,
        None => return false,
    };
    let ns = match CompressedRistretto(*new_sender).decompress() {
        Some(p) => p,
        None => return false,
    };
    let or = match CompressedRistretto(*old_recipient).decompress() {
        Some(p) => p,
        None => return false,
    };
    let nr = match CompressedRistretto(*new_recipient).decompress() {
        Some(p) => p,
        None => return false,
    };

    // sender_delta = old_sender - new_sender (what sender lost)
    // recipient_delta = new_recipient - old_recipient (what recipient gained)
    let sender_delta = os - ns;
    let recipient_delta = nr - or;

    // Conservation: sender_delta == recipient_delta
    let excess = sender_delta - recipient_delta;
    excess == RistrettoPoint::identity()
}
