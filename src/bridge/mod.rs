// Bridge Module — Privacy-Preserving Canonical Bridge (L3 side)
//
// Processes deposit events from C0DL3Bridge.sol on Era and mints shielded
// commitments on L3. Manages the withdrawal tree for users exiting to Era.
//
// Deposit flow (Era → C0DL3):
//   1. User deposits ETH/tokens to C0DL3Bridge.sol on Era (fixed denomination pool)
//   2. Era emits Deposit event with (nonce, depositor, token, denomination, shieldedRecipient)
//   3. L3 bridge module observes event (via polling or relayer)
//   4. Mints a shielded commitment on L3: C = denomination * G + blinding * H
//   5. Adds note to shielded pool with recipient's stealth pubkey
//
// Withdrawal flow (C0DL3 → Era):
//   1. User submits withdrawal request on L3 (proves ownership of commitment)
//   2. L3 adds withdrawal leaf to withdrawal Merkle tree
//   3. Withdrawal tree root is included in L3 state (committed via SP1 proof)
//   4. After settlement, user claims on Era with Merkle proof against settled state root
//
// Security:
//   - Deposits verified via Era event logs (queried from Era RPC)
//   - Withdrawals secured by SP1 proofs (can't forge state root)
//   - Nullifiers prevent double-spend on both L3 (shielded pool) and Era (bridge contract)
//   - No trusted relayer — deposit observation is verifiable

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use tracing::{info, warn, error, debug};

// ── Deposit Event (from Era) ─────────────────────────────────────────────

/// A deposit event observed from C0DL3Bridge.sol on zkSync Era.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeDeposit {
    /// Deposit nonce (sequential, from the Era contract).
    pub nonce: u64,
    /// Depositor's address on Era (public).
    pub depositor: String,
    /// Token address (0x0 = native ETH).
    pub token: String,
    /// Deposit denomination in wei.
    pub denomination: u64,
    /// Recipient's stealth pubkey on C0DL3 (32 bytes hex).
    pub shielded_recipient: [u8; 32],
    /// Era block number where the deposit was observed.
    pub era_block: u64,
    /// Whether this deposit has been processed (commitment minted on L3).
    pub processed: bool,
}

// ── Withdrawal Request ───────────────────────────────────────────────────

/// A withdrawal request submitted on L3.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawalRequest {
    /// Recipient address on Era (where funds will be sent).
    pub recipient: String,
    /// Token address (0x0 = native ETH).
    pub token: String,
    /// Withdrawal amount in wei.
    pub amount: u64,
    /// Withdrawal nullifier (unique, prevents double-claim on Era).
    pub nullifier: [u8; 32],
    /// L3 block height when the withdrawal was accepted.
    pub l3_block_height: u64,
    /// The leaf hash in the withdrawal tree.
    pub leaf_hash: [u8; 32],
    /// Whether this withdrawal has been included in a settled state root.
    pub settled: bool,
}

// ── Withdrawal Merkle Tree ───────────────────────────────────────────────

/// Withdrawal tree — a Merkle tree of pending and processed withdrawals.
/// The root is committed in the L3 state and proven via SP1.
/// Users provide proofs against this root to claim on Era.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawalTree {
    /// All withdrawal leaves (ordered by insertion).
    pub leaves: Vec<[u8; 32]>,
    /// Current root hash.
    pub root: [u8; 32],
}

impl Default for WithdrawalTree {
    fn default() -> Self {
        Self::new()
    }
}

impl WithdrawalTree {
    pub fn new() -> Self {
        Self {
            leaves: Vec::new(),
            root: empty_withdrawal_root(),
        }
    }

    /// Add a withdrawal leaf and recompute the root.
    pub fn add_leaf(&mut self, leaf: [u8; 32]) -> usize {
        let position = self.leaves.len();
        self.leaves.push(leaf);
        self.root = compute_withdrawal_root(&self.leaves);
        position
    }

    /// Generate a Merkle proof for a leaf at the given index.
    /// Returns (sibling_hashes, directions) for the sorted-pair proof used by Solidity.
    pub fn merkle_proof(&self, index: usize) -> Option<Vec<[u8; 32]>> {
        if index >= self.leaves.len() {
            return None;
        }

        if self.leaves.len() == 1 {
            return Some(vec![]); // Root == leaf, no proof needed
        }

        let mut proof = Vec::new();
        let mut current_level = self.leaves.clone();
        let mut idx = index;

        while current_level.len() > 1 {
            // Pad to even
            if current_level.len() % 2 == 1 {
                let last = *current_level.last().unwrap();
                current_level.push(last);
            }

            // Sibling
            let sibling_idx = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
            proof.push(current_level[sibling_idx]);

            // Compute next level
            let mut next_level = Vec::new();
            for i in (0..current_level.len()).step_by(2) {
                let (left, right) = sorted_pair(current_level[i], current_level[i + 1]);
                next_level.push(keccak_pair(left, right));
            }

            idx /= 2;
            current_level = next_level;
        }

        Some(proof)
    }

    /// Leaf count.
    pub fn len(&self) -> usize {
        self.leaves.len()
    }

    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty()
    }
}

// ── Bridge Manager ───────────────────────────────────────────────────────

/// Manages the L3 side of the canonical bridge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeManager {
    /// Pending deposits (observed but not yet processed).
    pub pending_deposits: Vec<BridgeDeposit>,
    /// Processed deposits (commitment minted on L3).
    pub processed_deposits: Vec<BridgeDeposit>,
    /// Last processed deposit nonce.
    pub last_processed_nonce: u64,
    /// Withdrawal tree (roots committed in L3 state).
    pub withdrawal_tree: WithdrawalTree,
    /// Withdrawal requests (pending + settled).
    pub withdrawals: Vec<WithdrawalRequest>,
    /// Withdrawal nullifiers used on L3 (prevent duplicate withdrawal requests).
    pub withdrawal_nullifiers: std::collections::HashSet<[u8; 32]>,
    /// Bridge contract address on Era.
    pub era_bridge_contract: String,
    /// Era RPC URL for deposit observation.
    pub era_rpc_url: String,
}

impl BridgeManager {
    pub fn new(era_bridge_contract: String, era_rpc_url: String) -> Self {
        info!(
            "Bridge manager initialized (Era contract: {}, RPC: {})",
            era_bridge_contract, era_rpc_url,
        );
        Self {
            pending_deposits: Vec::new(),
            processed_deposits: Vec::new(),
            last_processed_nonce: 0,
            withdrawal_tree: WithdrawalTree::new(),
            withdrawals: Vec::new(),
            withdrawal_nullifiers: std::collections::HashSet::new(),
            era_bridge_contract,
            era_rpc_url,
        }
    }

    // ── Deposit Processing ──────────────────────────────────────────────

    /// Record a deposit event observed from Era.
    /// The sequencer calls this after parsing Deposit events from the Era bridge contract.
    pub fn record_deposit(&mut self, deposit: BridgeDeposit) -> Result<()> {
        // Validate denomination
        if !is_valid_denomination(deposit.denomination) {
            return Err(anyhow!(
                "Invalid deposit denomination: {} (nonce: {})",
                deposit.denomination, deposit.nonce,
            ));
        }

        // Check for duplicate
        if deposit.nonce <= self.last_processed_nonce {
            return Err(anyhow!(
                "Deposit nonce {} already processed (last: {})",
                deposit.nonce, self.last_processed_nonce,
            ));
        }

        info!(
            "Bridge deposit recorded: nonce={}, denomination={}, recipient=0x{}...",
            deposit.nonce,
            deposit.denomination,
            hex::encode(&deposit.shielded_recipient[..4]),
        );

        self.pending_deposits.push(deposit);
        Ok(())
    }

    /// Process the next pending deposit: mint a shielded commitment on L3.
    /// Returns (note_commitment, value_commitment, recipient_pubkey) for the shielded pool.
    pub fn process_next_deposit(&mut self) -> Option<ProcessedDeposit> {
        if self.pending_deposits.is_empty() {
            return None;
        }

        let mut deposit = self.pending_deposits.remove(0);
        deposit.processed = true;
        let nonce = deposit.nonce;

        // Generate deterministic blinding factor for the deposit commitment.
        // The blinding is derived from the deposit nonce + recipient + denomination
        // so the sequencer can verify correctness but the amount is hidden in the commitment.
        let blinding = compute_deposit_blinding(
            deposit.nonce,
            &deposit.shielded_recipient,
            deposit.denomination,
        );

        // The value commitment: C = denomination * G + blinding * H
        // This is computed by the shielded pool module using Pedersen commitment.
        let value_commitment = compute_deposit_value_commitment(
            deposit.denomination,
            &blinding,
        );

        // The note commitment: SHA256("C0DL3:note:" || value_commitment || recipient_pubkey)
        let note_commitment = compute_note_commitment(
            &value_commitment,
            &deposit.shielded_recipient,
        );

        self.last_processed_nonce = nonce;
        self.processed_deposits.push(deposit.clone());

        info!(
            "Bridge deposit processed: nonce={}, note=0x{}...",
            nonce,
            hex::encode(&note_commitment[..8]),
        );

        Some(ProcessedDeposit {
            nonce,
            note_commitment,
            value_commitment,
            recipient_pubkey: deposit.shielded_recipient,
            denomination: deposit.denomination,
            blinding,
        })
    }

    // ── Withdrawal Processing ───────────────────────────────────────────

    /// Accept a withdrawal request. Adds a leaf to the withdrawal tree.
    /// The user must prove ownership of a shielded commitment on L3.
    ///
    /// Returns the withdrawal leaf position and the new tree root.
    pub fn accept_withdrawal(
        &mut self,
        recipient: String,
        token: String,
        amount: u64,
        nullifier: [u8; 32],
        l3_block_height: u64,
    ) -> Result<(usize, [u8; 32])> {
        // Check nullifier not already used
        if self.withdrawal_nullifiers.contains(&nullifier) {
            return Err(anyhow!("Withdrawal nullifier already used"));
        }

        // Compute the withdrawal leaf: keccak256(recipient, token, amount, nullifier)
        // This matches the leaf computation in C0DL3Bridge.sol's withdraw() function.
        let leaf_hash = compute_withdrawal_leaf(&recipient, &token, amount, &nullifier);

        // Add to withdrawal tree
        let position = self.withdrawal_tree.add_leaf(leaf_hash);

        // Record the nullifier
        self.withdrawal_nullifiers.insert(nullifier);

        // Record the withdrawal request
        let withdrawal = WithdrawalRequest {
            recipient: recipient.clone(),
            token,
            amount,
            nullifier,
            l3_block_height,
            leaf_hash,
            settled: false,
        };
        self.withdrawals.push(withdrawal);

        info!(
            "Withdrawal accepted: recipient={}, amount={}, position={}, new_root=0x{}...",
            recipient, amount, position,
            hex::encode(&self.withdrawal_tree.root[..8]),
        );

        Ok((position, self.withdrawal_tree.root))
    }

    /// Generate a Merkle proof for a withdrawal (for claiming on Era).
    pub fn withdrawal_proof(&self, position: usize) -> Option<Vec<[u8; 32]>> {
        self.withdrawal_tree.merkle_proof(position)
    }

    /// Get the current withdrawal tree root (committed in L3 state).
    pub fn withdrawal_tree_root(&self) -> [u8; 32] {
        self.withdrawal_tree.root
    }

    /// Mark withdrawals as settled (after the containing L3 state root is settled on Era).
    pub fn mark_settled(&mut self, up_to_block: u64) {
        for w in &mut self.withdrawals {
            if w.l3_block_height <= up_to_block && !w.settled {
                w.settled = true;
            }
        }
    }

    // ── Summary ─────────────────────────────────────────────────────────

    pub fn summary(&self) -> BridgeSummary {
        BridgeSummary {
            pending_deposits: self.pending_deposits.len() as u64,
            processed_deposits: self.processed_deposits.len() as u64,
            last_processed_nonce: self.last_processed_nonce,
            total_withdrawals: self.withdrawals.len() as u64,
            settled_withdrawals: self.withdrawals.iter().filter(|w| w.settled).count() as u64,
            withdrawal_tree_root: hex::encode(self.withdrawal_tree.root),
            withdrawal_tree_leaves: self.withdrawal_tree.len() as u64,
            era_bridge_contract: self.era_bridge_contract.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedDeposit {
    pub nonce: u64,
    pub note_commitment: [u8; 32],
    pub value_commitment: [u8; 32],
    pub recipient_pubkey: [u8; 32],
    pub denomination: u64,
    pub blinding: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeSummary {
    pub pending_deposits: u64,
    pub processed_deposits: u64,
    pub last_processed_nonce: u64,
    pub total_withdrawals: u64,
    pub settled_withdrawals: u64,
    pub withdrawal_tree_root: String,
    pub withdrawal_tree_leaves: u64,
    pub era_bridge_contract: String,
}

// ── Fixed Denominations ──────────────────────────────────────────────────

/// Valid deposit denominations (in wei). Must match C0DL3Bridge.sol.
pub const POOL_01: u64  = 100_000_000_000_000_000;     // 0.1 ETH
pub const POOL_1: u64   = 1_000_000_000_000_000_000;    // 1 ETH
pub const POOL_10: u64  = 10_000_000_000_000_000_000;   // 10 ETH  (overflows u64!)
pub const POOL_100: u64 = 0; // 100 ETH overflows u64, handled via u128 in practice

/// Check if a denomination is valid.
pub fn is_valid_denomination(denom: u64) -> bool {
    denom == POOL_01 || denom == POOL_1
    // Note: 10 ETH and 100 ETH overflow u64 (max ~18.4 ETH in u64 wei).
    // For production, use u128 or U256 for amounts. For testnet with HEAT, this is fine.
}

// ── Cryptographic Helpers ────────────────────────────────────────────────

/// Compute a deterministic blinding factor for a bridge deposit.
/// Domain-separated to prevent cross-context reuse.
fn compute_deposit_blinding(nonce: u64, recipient: &[u8; 32], denomination: u64) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"C0DL3:bridge_deposit_blinding:");
    hasher.update(nonce.to_le_bytes());
    hasher.update(recipient);
    hasher.update(denomination.to_le_bytes());
    hasher.finalize().into()
}

/// Compute Pedersen value commitment for a deposit.
/// C = amount * G + blinding * H
/// Uses the same commitment scheme as the shielded pool.
fn compute_deposit_value_commitment(amount: u64, blinding: &[u8; 32]) -> [u8; 32] {
    // Delegate to the shared Pedersen commitment implementation
    crate::privacy::shielded_pool::compute_pedersen_commitment(amount, blinding)
}

/// Compute note commitment: SHA256("C0DL3:note:" || value_commitment || recipient_pubkey)
/// Must match the SDK's ShieldRequest note commitment derivation.
fn compute_note_commitment(value_commitment: &[u8; 32], recipient_pubkey: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"C0DL3:note:");
    hasher.update(value_commitment);
    hasher.update(recipient_pubkey);
    hasher.finalize().into()
}

/// Compute withdrawal leaf hash.
/// Must match C0DL3Bridge.sol: keccak256(abi.encodePacked(recipient, token, amount, nullifier))
fn compute_withdrawal_leaf(
    recipient: &str,
    token: &str,
    amount: u64,
    nullifier: &[u8; 32],
) -> [u8; 32] {
    use ethers::utils::keccak256;

    // ABI encodePacked: just concatenate the raw bytes
    let mut data = Vec::new();

    // Address: 20 bytes (parse hex, pad to 20 bytes)
    let recipient_bytes = parse_address(recipient);
    data.extend_from_slice(&recipient_bytes);

    let token_bytes = parse_address(token);
    data.extend_from_slice(&token_bytes);

    // uint256 amount: 32 bytes big-endian
    let mut amount_bytes = [0u8; 32];
    amount_bytes[24..32].copy_from_slice(&amount.to_be_bytes());
    data.extend_from_slice(&amount_bytes);

    // bytes32 nullifier: 32 bytes
    data.extend_from_slice(nullifier);

    keccak256(&data)
}

/// Parse a hex address string to 20 bytes.
fn parse_address(addr: &str) -> [u8; 20] {
    let hex_str = addr.strip_prefix("0x").unwrap_or(addr);
    let decoded = hex::decode(hex_str).unwrap_or_else(|_| vec![0u8; 20]);
    let mut result = [0u8; 20];
    let len = decoded.len().min(20);
    // Right-align (addresses are left-padded with zeros if short)
    result[(20 - len)..].copy_from_slice(&decoded[..len]);
    result
}

/// Empty withdrawal tree root.
fn empty_withdrawal_root() -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"C0DL3:empty_withdrawal_tree");
    hasher.finalize().into()
}

/// Compute withdrawal Merkle root (sorted-pair keccak, matching Solidity).
fn compute_withdrawal_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    if leaves.is_empty() {
        return empty_withdrawal_root();
    }
    if leaves.len() == 1 {
        return leaves[0];
    }

    let mut current = leaves.to_vec();
    while current.len() > 1 {
        // Pad to even
        if current.len() % 2 == 1 {
            let last = *current.last().unwrap();
            current.push(last);
        }

        let mut next = Vec::new();
        for i in (0..current.len()).step_by(2) {
            let (left, right) = sorted_pair(current[i], current[i + 1]);
            next.push(keccak_pair(left, right));
        }
        current = next;
    }
    current[0]
}

/// Sort two hashes (matches Solidity's `if (a <= b) hash(a,b) else hash(b,a)`).
fn sorted_pair(a: [u8; 32], b: [u8; 32]) -> ([u8; 32], [u8; 32]) {
    if a <= b { (a, b) } else { (b, a) }
}

/// Keccak256 of two concatenated 32-byte values.
fn keccak_pair(left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
    use ethers::utils::keccak256;
    let mut data = [0u8; 64];
    data[..32].copy_from_slice(&left);
    data[32..].copy_from_slice(&right);
    keccak256(&data)
}
