// State witness + WitnessDatabase for revm + state root computation.
//
// The guest receives a pre-state witness (all accounts with plaintext balances,
// blinding factors, code, and storage). It:
//   1. Verifies the witness against prev_state_root
//   2. Feeds account data to revm via WitnessDatabase
//   3. After execution, computes new state root from updated accounts
//
// State root computation matches the host exactly:
//   SHA-256(for each account sorted by address string:
//     address.as_bytes() || balance_commitment(32) || nonce.to_le_bytes(8))
//
// Balance commitment is deterministic Pedersen:
//   r = SHA-256("C0DL3:balance:" || address.as_bytes() || nonce.to_le_bytes())
//   C = balance * G + r * H   (using bulletproofs::PedersenGens)

use bulletproofs::PedersenGens;
use curve25519_dalek_ng::scalar::Scalar;
use revm::{
    db::DatabaseRef,
    primitives::{AccountInfo, Address, Bytecode, B256, U256},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

// ── Account Witness ─────────────────────────────────────────────────────────

/// Pre-state account data fed to the guest as private input.
/// The prover constructs this from the node's `RollupState.accounts`.
///
/// PRIVACY: For accounts that don't participate in EVM transactions,
/// `precomputed_commitment` can be provided instead of the plaintext balance.
/// This prevents the prover from learning balances of shielded-only accounts.
/// The guest uses the precomputed commitment directly in state root computation
/// and verifies correctness by checking the final root matches prev_state_root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountWitness {
    /// Address as string (e.g. "0x1234...") — matches host HashMap key format.
    /// Used for state root computation (hashed as UTF-8 bytes, matching host).
    pub address: String,
    /// Plaintext balance in fwei. Private input — not revealed in proof output.
    /// For commitment-only witnesses, this is 0 (unused — commitment is authoritative).
    pub balance: u64,
    /// Account nonce.
    pub nonce: u64,
    /// Contract bytecode (empty for EOAs).
    pub code: Vec<u8>,
    /// Storage slots: (key_bytes_32, value_bytes_32).
    pub storage: Vec<([u8; 32], [u8; 32])>,
    /// Pre-computed balance commitment. When Some, the guest uses this directly
    /// instead of computing from plaintext balance. This hides the balance from
    /// the prover for accounts not involved in EVM execution.
    ///
    /// Security: The commitment is verified by the state root check — if the
    /// prover provides a wrong commitment, the state root won't match, and
    /// the SP1 proof fails.
    #[serde(default)]
    pub precomputed_commitment: Option<[u8; 32]>,
}

// ── Balance Commitment ──────────────────────────────────────────────────────
//
// Must match host's `compute_balance_commitment()` in `src/privacy/shielded_pool.rs` exactly.

/// Compute deterministic Pedersen commitment for an account balance.
///
/// C = balance * G + r * H
/// where r = SHA-256("C0DL3:balance:" || address.as_bytes() || nonce.to_le_bytes())
///
/// This is deterministic: given (address, balance, nonce), the commitment is fixed.
/// The "blinding" is derived from public data, so this is a binding commitment
/// (not hiding from the sequencer, but hiding from external observers who don't
/// know the balance).
pub fn compute_balance_commitment(address: &str, balance: u64, nonce: u64) -> [u8; 32] {
    // Derive deterministic blinding factor — must match host exactly
    let mut blinding_input = Vec::new();
    blinding_input.extend_from_slice(b"C0DL3:balance:");
    blinding_input.extend_from_slice(address.as_bytes());
    blinding_input.extend_from_slice(&nonce.to_le_bytes());
    let blinding_hash: [u8; 32] = Sha256::digest(&blinding_input).into();
    let r = Scalar::from_bytes_mod_order(blinding_hash);

    // Encode balance as scalar
    let mut amount_padded = [0u8; 32];
    amount_padded[..8].copy_from_slice(&balance.to_le_bytes());
    let v = Scalar::from_bytes_mod_order(amount_padded);

    // Commit: C = v*G + r*H using bulletproofs default generators
    let gens = PedersenGens::default();
    gens.commit(v, r).compress().to_bytes()
}

/// Compute state root from account witnesses — must match host's `compute_state_root()`.
///
/// SHA-256 of sorted (address.as_bytes() || balance_commitment(32) || nonce.to_le_bytes(8))
///
/// For accounts with a `precomputed_commitment`, uses it directly instead of
/// computing from plaintext balance. This lets the prover verify state roots
/// without knowing every account's plaintext balance.
pub fn compute_state_root(accounts: &[AccountWitness]) -> [u8; 32] {
    let mut hasher = Sha256::new();

    // Sort by address string (matching host's sort_by_key)
    let mut sorted: Vec<&AccountWitness> = accounts.iter().collect();
    sorted.sort_by_key(|a| a.address.clone());

    for account in &sorted {
        let commitment = match account.precomputed_commitment {
            Some(c) => c,
            None => compute_balance_commitment(
                &account.address,
                account.balance,
                account.nonce,
            ),
        };
        hasher.update(account.address.as_bytes());
        hasher.update(&commitment);
        hasher.update(account.nonce.to_le_bytes());
    }

    hasher.finalize().into()
}

/// Format state root as hex string (matching host's "0x..." format).
pub fn state_root_hex(root: &[u8; 32]) -> String {
    let mut s = String::with_capacity(66);
    s.push_str("0x");
    for byte in root {
        s.push_str(&format!("{:02x}", byte));
    }
    s
}

// ── Witness Database for revm ───────────────────────────────────────────────
//
// Implements revm's `DatabaseRef` trait backed by the pre-state witness.
// No external RPC calls — everything comes from the witness data.
// This is a read-only database; revm's CacheDB wraps it to track mutations.

/// Database backed by the pre-state witness. Immutable after construction.
pub struct WitnessDatabase {
    /// revm Address → (AccountInfo, storage)
    accounts: HashMap<Address, (AccountInfo, HashMap<U256, U256>)>,
    /// revm Address → original String address (for post-execution state root)
    pub address_map: HashMap<Address, String>,
}

impl WitnessDatabase {
    /// Build from account witnesses.
    pub fn new(witnesses: &[AccountWitness]) -> Self {
        let mut accounts = HashMap::new();
        let mut address_map = HashMap::new();

        for w in witnesses {
            let addr = string_to_address(&w.address);

            let info = AccountInfo {
                balance: U256::from(w.balance),
                nonce: w.nonce,
                code_hash: if w.code.is_empty() {
                    // keccak256 of empty bytes — standard "no code" marker
                    B256::from(revm::primitives::KECCAK_EMPTY)
                } else {
                    revm::primitives::keccak256(&w.code)
                },
                code: if w.code.is_empty() {
                    None
                } else {
                    Some(Bytecode::new_raw(w.code.clone().into()))
                },
            };

            let mut storage = HashMap::new();
            for (key, val) in &w.storage {
                storage.insert(U256::from_be_bytes(*key), U256::from_be_bytes(*val));
            }

            address_map.insert(addr, w.address.clone());
            accounts.insert(addr, (info, storage));
        }

        Self {
            accounts,
            address_map,
        }
    }
}

/// Convert hex address string to revm Address.
fn string_to_address(s: &str) -> Address {
    let hex_str = s.trim_start_matches("0x");
    let mut bytes = [0u8; 20];
    // Manual hex decode (no hex crate dependency needed)
    let chars: Vec<u8> = hex_str.bytes().collect();
    let len = chars.len().min(40);
    for i in (0..len).step_by(2) {
        if i + 1 < len {
            bytes[i / 2] = hex_nibble(chars[i]) << 4 | hex_nibble(chars[i + 1]);
        }
    }
    Address::from(bytes)
}

fn hex_nibble(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => 0,
    }
}

/// Error type for witness database lookups.
#[derive(Debug, Clone)]
pub struct WitnessError(pub String);

impl core::fmt::Display for WitnessError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "WitnessError: {}", self.0)
    }
}

impl DatabaseRef for WitnessDatabase {
    type Error = WitnessError;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        Ok(self.accounts.get(&address).map(|(info, _)| info.clone()))
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        // Search all accounts for matching code hash
        for (info, _) in self.accounts.values() {
            if info.code_hash == code_hash {
                if let Some(ref code) = info.code {
                    return Ok(code.clone());
                }
            }
        }
        Ok(Bytecode::default())
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        Ok(self
            .accounts
            .get(&address)
            .and_then(|(_, storage)| storage.get(&index))
            .copied()
            .unwrap_or(U256::ZERO))
    }

    fn block_hash_ref(&self, _number: u64) -> Result<B256, Self::Error> {
        // Inside the guest, we don't have access to historical block hashes.
        // Return zero — contracts using BLOCKHASH will get 0x00...00.
        // This is acceptable for L3 where BLOCKHASH is rarely used.
        Ok(B256::ZERO)
    }
}

// ── Post-Execution State ────────────────────────────────────────────────────

/// Build post-execution AccountWitness list from CacheDB state.
///
/// After revm executes all transactions, the CacheDB contains the updated
/// account states. We convert these back to AccountWitness format for
/// state root computation.
pub fn build_post_state(
    cache_accounts: &HashMap<Address, revm::db::DbAccount>,
    address_map: &HashMap<Address, String>,
    original_witnesses: &[AccountWitness],
) -> Vec<AccountWitness> {
    // Start with originals (for accounts not touched by execution)
    let mut result: HashMap<String, AccountWitness> = original_witnesses
        .iter()
        .map(|w| (w.address.clone(), w.clone()))
        .collect();

    // Apply changes from CacheDB
    for (addr, db_account) in cache_accounts {
        let address_str = address_map
            .get(addr)
            .cloned()
            .unwrap_or_else(|| format!("0x{:x}", addr));

        // Convert U256 balance back to u64. If it overflows u64, cap at u64::MAX.
        let balance = if db_account.info.balance > U256::from(u64::MAX) {
            u64::MAX
        } else {
            db_account.info.balance.as_limbs()[0]
        };

        let code = db_account
            .info
            .code
            .as_ref()
            .map(|c| c.bytecode().to_vec())
            .unwrap_or_default();

        let storage: Vec<([u8; 32], [u8; 32])> = db_account
            .storage
            .iter()
            .map(|(k, v)| (k.to_be_bytes(), v.to_be_bytes()))
            .collect();

        result.insert(
            address_str.clone(),
            AccountWitness {
                address: address_str,
                balance,
                nonce: db_account.info.nonce,
                code,
                storage,
                // EVM-touched accounts: recompute commitment from plaintext
                // (the prover already knows these balances from EVM execution)
                precomputed_commitment: None,
            },
        );
    }

    result.into_values().collect()
}
