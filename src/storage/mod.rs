// Storage Module — Persistent State for C0DL3 Node
//
// Uses sled (pure Rust embedded KV store) for crash-resilient state.
// The node keeps hot state in memory (RollupState) and writes through
// to sled on every mutation. On restart, state is loaded from disk.
//
// Tree layout:
//   meta            — scalar values: state_root, block_height, note_tree_root
//   accounts        — address (UTF-8) → AccountState (JSON)
//   blocks          — height (8-byte BE) → Block (JSON)
//   nullifiers      — [u8; 32] → [] (shielded pool nullifier set)
//   acct_nullifiers — [u8; 32] → [] (account-level nullifier set)
//   shielded_notes  — position (8-byte BE) → ShieldedNote (JSON)
//   proven_blocks   — height (8-byte BE) → ProvenBlock (JSON)
//   block_economics — height (8-byte BE) → BlockEconomics (JSON)
//   prover_registry — address (UTF-8) → ProverStats (JSON)
//   settlement      — batch_id (8-byte BE) → SettlementBatch (JSON)
//   coldao          — "manager" → ColdaoManager (JSON)
//   prover_config   — "config" → ProverConfig (JSON)

use anyhow::{Result, anyhow, Context};
use serde::{Serialize, de::DeserializeOwned};
use tracing::info;

use crate::{
    Block, ProvenBlock,
    AccountState, RollupState,
    economics::{BlockEconomics, ProverRegistry, ProverStats},
    privacy::shielded_pool::{ShieldedPool, ShieldedNote},
    proving::ProverConfig,
    settlement::SettlementBatch,
    tokens::coldao::ColdaoManager,
};

// ── Key encoding helpers ─────────────────────────────────────────────────────

fn height_key(height: u64) -> [u8; 8] {
    height.to_be_bytes()
}

fn json_encode<T: Serialize>(val: &T) -> Result<Vec<u8>> {
    serde_json::to_vec(val).map_err(|e| anyhow!("serialize: {}", e))
}

fn json_decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    serde_json::from_slice(bytes).map_err(|e| anyhow!("deserialize: {}", e))
}

// ── StateDb ──────────────────────────────────────────────────────────────────

/// Persistent state database backed by sled.
pub struct StateDb {
    db: sled::Db,
    // Named trees (sled column families)
    meta: sled::Tree,
    accounts: sled::Tree,
    blocks: sled::Tree,
    nullifiers: sled::Tree,
    acct_nullifiers: sled::Tree,
    shielded_notes: sled::Tree,
    proven_blocks: sled::Tree,
    block_economics: sled::Tree,
    prover_registry: sled::Tree,
    settlement: sled::Tree,
    coldao: sled::Tree,
    prover_config_tree: sled::Tree,
}

impl StateDb {
    /// Open (or create) the state database at the given path.
    pub fn open(path: &str) -> Result<Self> {
        let db = sled::open(path)
            .with_context(|| format!("Failed to open state db at {}", path))?;

        let meta = db.open_tree("meta")?;
        let accounts = db.open_tree("accounts")?;
        let blocks = db.open_tree("blocks")?;
        let nullifiers = db.open_tree("nullifiers")?;
        let acct_nullifiers = db.open_tree("acct_nullifiers")?;
        let shielded_notes = db.open_tree("shielded_notes")?;
        let proven_blocks = db.open_tree("proven_blocks")?;
        let block_economics = db.open_tree("block_economics")?;
        let prover_registry = db.open_tree("prover_registry")?;
        let settlement = db.open_tree("settlement")?;
        let coldao = db.open_tree("coldao")?;
        let prover_config_tree = db.open_tree("prover_config")?;

        let size = db.size_on_disk()
            .unwrap_or(0) / 1024;

        info!("State database opened at {} ({} KB on disk)", path, size);

        Ok(Self {
            db,
            meta,
            accounts,
            blocks,
            nullifiers,
            acct_nullifiers,
            shielded_notes,
            proven_blocks,
            block_economics,
            prover_registry,
            settlement,
            coldao,
            prover_config_tree,
        })
    }

    // ── Load full state from disk ────────────────────────────────────────────

    /// Load the entire RollupState from persistent storage.
    /// Returns None if no state has been persisted yet (fresh node).
    pub fn load_rollup_state(&self) -> Result<Option<RollupState>> {
        // Check if we have any persisted state
        let block_height = match self.meta.get("block_height")? {
            Some(bytes) => {
                let arr: [u8; 8] = bytes.as_ref().try_into()
                    .map_err(|_| anyhow!("corrupt block_height"))?;
                u64::from_be_bytes(arr)
            }
            None => return Ok(None), // Fresh node — no persisted state
        };

        info!("Loading persisted state (block height: {})", block_height);

        // Load state root
        let state_root = self.meta.get("state_root")?
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .unwrap_or_else(|| "0x0000000000000000000000000000000000000000000000000000000000000000".to_string());

        // Load accounts
        let mut accounts = std::collections::HashMap::new();
        for entry in self.accounts.iter() {
            let (key, val) = entry?;
            let addr = String::from_utf8_lossy(&key).to_string();
            let acct: AccountState = json_decode(&val)?;
            accounts.insert(addr, acct);
        }
        info!("  Loaded {} accounts", accounts.len());

        // Load blocks
        let mut blocks = std::collections::HashMap::new();
        for entry in self.blocks.iter() {
            let (key, val) = entry?;
            let h = u64::from_be_bytes(
                key.as_ref().try_into().map_err(|_| anyhow!("corrupt block key"))?
            );
            let block: Block = json_decode(&val)?;
            blocks.insert(h, block);
        }
        info!("  Loaded {} blocks", blocks.len());

        // Load shielded pool
        let mut notes = Vec::new();
        for entry in self.shielded_notes.iter() {
            let (_key, val) = entry?;
            let note: ShieldedNote = json_decode(&val)?;
            notes.push(note);
        }
        // Sort by position to maintain tree order
        notes.sort_by_key(|n| n.position);

        let mut nullifier_set = std::collections::HashSet::new();
        for entry in self.nullifiers.iter() {
            let (key, _val) = entry?;
            let arr: [u8; 32] = key.as_ref().try_into()
                .map_err(|_| anyhow!("corrupt nullifier key"))?;
            nullifier_set.insert(arr);
        }

        let note_tree_root = self.meta.get("note_tree_root")?
            .map(|b| {
                let arr: [u8; 32] = b.as_ref().try_into().unwrap_or([0u8; 32]);
                arr
            })
            .unwrap_or([0u8; 32]);

        let shielded_pool = ShieldedPool {
            notes,
            nullifier_set,
            note_tree_root,
        };
        info!("  Loaded shielded pool ({} notes, {} nullifiers)",
            shielded_pool.notes.len(), shielded_pool.nullifier_set.len());

        // Load account nullifiers
        let mut account_nullifiers = std::collections::HashSet::new();
        for entry in self.acct_nullifiers.iter() {
            let (key, _val) = entry?;
            let arr: [u8; 32] = key.as_ref().try_into()
                .map_err(|_| anyhow!("corrupt acct nullifier"))?;
            account_nullifiers.insert(arr);
        }

        // Load proven blocks
        let mut proven_blocks_map = std::collections::HashMap::new();
        for entry in self.proven_blocks.iter() {
            let (key, val) = entry?;
            let h = u64::from_be_bytes(
                key.as_ref().try_into().map_err(|_| anyhow!("corrupt proven_block key"))?
            );
            let pb: ProvenBlock = json_decode(&val)?;
            proven_blocks_map.insert(h, pb);
        }
        info!("  Loaded {} proven blocks", proven_blocks_map.len());

        // Load block economics
        let mut block_economics_map = std::collections::HashMap::new();
        for entry in self.block_economics.iter() {
            let (key, val) = entry?;
            let h = u64::from_be_bytes(
                key.as_ref().try_into().map_err(|_| anyhow!("corrupt block_economics key"))?
            );
            let econ: BlockEconomics = json_decode(&val)?;
            block_economics_map.insert(h, econ);
        }

        // Load prover registry
        let mut prover_registry = ProverRegistry::new();
        for entry in self.prover_registry.iter() {
            let (key, val) = entry?;
            let addr = String::from_utf8_lossy(&key).to_string();
            let stats: ProverStats = json_decode(&val)?;
            prover_registry.provers.insert(addr, stats);
        }

        // Load COLDAO manager
        let coldao_manager = self.coldao.get("manager")?
            .map(|b| json_decode::<ColdaoManager>(&b))
            .transpose()?
            .unwrap_or_default();

        // Load prover config
        let prover_config = self.prover_config_tree.get("config")?
            .map(|b| json_decode::<ProverConfig>(&b))
            .transpose()?
            .unwrap_or_else(|| ProverConfig::new(vec![]));

        let state = RollupState {
            accounts,
            state_root,
            block_height,
            blocks,
            shielded_pool,
            account_nullifiers,
            pending_proofs: std::collections::HashMap::new(), // Transient — not persisted
            proven_blocks: proven_blocks_map,
            block_economics_map,
            prover_registry,
            coldao_manager,
            prover_config,
        };

        info!("State loaded successfully (height: {}, accounts: {}, blocks: {})",
            state.block_height, state.accounts.len(), state.blocks.len());

        Ok(Some(state))
    }

    // ── Write-through methods (called after each state mutation) ─────────────

    /// Persist a new block and updated state root + block height.
    pub fn put_block(&self, height: u64, block: &Block, state_root: &str) -> Result<()> {
        let key = height_key(height);

        // Write block
        self.blocks.insert(key, json_encode(block)?)?;

        // Update meta
        self.meta.insert("block_height", &height.to_be_bytes())?;
        self.meta.insert("state_root", state_root.as_bytes())?;

        Ok(())
    }

    /// Persist an account state update.
    pub fn put_account(&self, address: &str, state: &AccountState) -> Result<()> {
        self.accounts.insert(address.as_bytes(), json_encode(state)?)?;
        Ok(())
    }

    /// Persist multiple account updates in a batch (used after block execution).
    pub fn put_accounts(&self, updates: &[(String, AccountState)]) -> Result<()> {
        let mut batch = sled::Batch::default();
        for (addr, state) in updates {
            batch.insert(addr.as_bytes(), json_encode(state)?);
        }
        self.accounts.apply_batch(batch)?;
        Ok(())
    }

    /// Persist a new shielded note.
    pub fn put_shielded_note(&self, note: &ShieldedNote) -> Result<()> {
        let key = height_key(note.position);
        self.shielded_notes.insert(key, json_encode(note)?)?;
        Ok(())
    }

    /// Persist a spent nullifier (shielded pool).
    pub fn put_nullifier(&self, nullifier: &[u8; 32]) -> Result<()> {
        self.nullifiers.insert(nullifier.as_ref(), &[])?;
        Ok(())
    }

    /// Persist an account-level execution nullifier.
    pub fn put_account_nullifier(&self, nullifier: &[u8; 32]) -> Result<()> {
        self.acct_nullifiers.insert(nullifier.as_ref(), &[])?;
        Ok(())
    }

    /// Update shielded pool note tree root.
    pub fn put_note_tree_root(&self, root: &[u8; 32]) -> Result<()> {
        self.meta.insert("note_tree_root", root.as_ref())?;
        Ok(())
    }

    /// Persist a proven block record.
    pub fn put_proven_block(&self, height: u64, proven: &ProvenBlock) -> Result<()> {
        self.proven_blocks.insert(height_key(height), json_encode(proven)?)?;
        Ok(())
    }

    /// Persist block economics for a height.
    pub fn put_block_economics(&self, height: u64, econ: &BlockEconomics) -> Result<()> {
        self.block_economics.insert(height_key(height), json_encode(econ)?)?;
        Ok(())
    }

    /// Persist prover stats update.
    pub fn put_prover_stats(&self, address: &str, stats: &ProverStats) -> Result<()> {
        self.prover_registry.insert(address.as_bytes(), json_encode(stats)?)?;
        Ok(())
    }

    /// Persist COLDAO manager state.
    pub fn put_coldao(&self, manager: &ColdaoManager) -> Result<()> {
        self.coldao.insert("manager", json_encode(manager)?)?;
        Ok(())
    }

    /// Persist prover config (vkey, elf hash).
    pub fn put_prover_config(&self, config: &ProverConfig) -> Result<()> {
        self.prover_config_tree.insert("config", json_encode(config)?)?;
        Ok(())
    }

    /// Persist a settlement batch.
    pub fn put_settlement_batch(&self, batch: &SettlementBatch) -> Result<()> {
        self.settlement.insert(
            height_key(batch.batch_id),
            json_encode(batch)?,
        )?;
        Ok(())
    }

    /// Load all settlement batches (for SettlementManager reconstruction).
    pub fn load_settlement_batches(&self) -> Result<Vec<SettlementBatch>> {
        let mut batches = Vec::new();
        for entry in self.settlement.iter() {
            let (_key, val) = entry?;
            batches.push(json_decode(&val)?);
        }
        Ok(batches)
    }

    // ── Flush ────────────────────────────────────────────────────────────────

    /// Force flush all pending writes to disk.
    /// sled is crash-safe by default, but this ensures immediate durability.
    pub fn flush(&self) -> Result<()> {
        self.db.flush().map_err(|e| anyhow!("flush: {}", e))?;
        Ok(())
    }

    /// Get approximate database size on disk (bytes).
    pub fn size_on_disk(&self) -> u64 {
        self.db.size_on_disk().unwrap_or(0)
    }
}
