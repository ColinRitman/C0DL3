# <sup><sub>ZK</sub></sup>`C0DL3`

[![Rust](https://img.shields.io/badge/rust-1.75+-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Branch](https://img.shields.io/badge/branch-zkc0dl3-blueviolet.svg)](https://github.com/ColinRitman/C0DL3/tree/zkc0dl3)

**C0DL3** is a sovereign ZK privacy darkpool Layer-3 for the public zkSync ecosystem. Every account is an AA smart contract wallet allowing all balances to be private by default-- storing Pedersen commitments instead of plaintext values. A shielded pool provides full sender/recipient anonymity on demand. Fixed-denomination bridge pools (HEAT, ZK, COLD, ETH) create anonymity sets at the privacy boundary. ZK validity proofs are generated via SP1 (RISC-V zkVM).

---

## Architecture

```
User Wallet (SDK)
    │ UserOperation (commitments + proofs, no plaintext amounts)
    ▼
Sequencer (C0DL3 node)
    │ Blind processing — sees only commitments
    ▼
SP1 Prover
    │ Groth16 proof of block validity
    ▼
zkSync Era (settlement) → Ethereum (L1)
```

**Privacy layers:**

| Layer | What's hidden | Mechanism |
|-------|--------------|-----------|
| AA wallet balances | Amount | Pedersen commitments |
| Transfers | Amount | Commitment arithmetic + conservation proof |
| Shielded pool transfers | Amount + sender + recipient | Nullifiers + Merkle proofs |
| Gas payment | Sender identity | Paymasters |
| Wallet creation | Recipient address | Stealth addresses (ECDH) |
| Balance discovery | Amount + blinding | ElGamal encrypted memos |

---

## Key Features

### Privacy-by-Default Account Abstraction
Every account is a `PrivateWallet` contract storing `balanceCommitment` — a Pedersen commitment `C = amount*G + r*H`. The sequencer never sees plaintext balances. Wallet auth uses Schnorr signatures (Ristretto255). Gas is paid by paymasters to break the sender-gas link.

### Shielded Pool
For full anonymity (hidden sender + recipient), the SDK auto-routes transfers through the shielded pool: shield → private transfer → delayed unshield. Commitments, nullifiers, Merkle membership proofs — nothing is revealed to the sequencer.

### Confidential Transactions
Transfers using Pedersen commitments are verified by the conservation check precompile: `old_sender_commit - new_sender_commit == new_recipient_commit - old_recipient_commit`. Bulletproofs ensure amounts stay in `[0, 2^64)`.

### Precompile Suite
C0DL3 ships privacy-native and Ethereum-compatible precompiles, all SP1-accelerated:

**Privacy primitives:**

| Address | Precompile | Gas |
|---------|-----------|-----|
| 0x0100 | Ristretto255 scalar mul | 2,000 |
| 0x0101 | Pedersen Commit | 2,500 |
| 0x0102 | Bulletproofs Range Verify | 10,000 |
| 0x0103 | Poseidon Hash | 1,500 |
| 0x0104 | ElGamal Encrypt | 5,000 |
| 0x0106 | Schnorr Verify | 3,000 |
| 0x0107 | Conservation Check | 4,000 |
| 0x010A | Encrypted Memo Verify | 5,000 |
| 0x0110 | Shield | 15,000 |
| 0x0111 | Unshield | 15,000 |
| 0x0120 | Private Swap | 12,000 |
| 0x0121 | Threshold Proof | 8,000 |
| 0x0122 | Commitment Arithmetic | 3,000 |
| 0x0132 | Batch Schnorr Verify | 2,000/sig |
| 0x0140 | Ed25519 Verify | 3,000 |

**Ethereum-compatible:**

| Address | Precompile | Gas |
|---------|-----------|-----|
| 0x0001 | ecRecover (secp256k1) | 3,000 |
| 0x0002 | SHA-256 | 60 + 12/word |
| 0x0005 | ModExp | EIP-2565 |
| 0x0006 | BN254 ecAdd | 150 |
| 0x0007 | BN254 ecMul | 6,000 |
| 0x0008 | BN254 ecPairing | 45,000 + 34,000/pair |
| 0x0201 | P-256 Verify (WebAuthn) | 3,000 |

BN254 ecPairing (0x0008) enables on-chain Groth16 ZK proof verification in any Solidity contract. P-256 (0x0201) enables passkey-based wallet auth — Face ID, Touch ID, YubiKey, no seed phrases.

### Sovereign Prover

C0DL3 uses a **sovereign prover model** — anyone with a GPU can prove blocks and earn HEAT rewards. No centralized prover network required.

**Proof flow:**

```
1. Block proposed (2s cadence) ──────────── soft-confirmed
2. Prover polls /proof/pending, downloads block data + pre-state
3. SP1 guest executes block (revm + privacy validation) inside zkVM
4. Prover submits proof via /proof/submit
5. Sequencer verifies proof (SP1 SDK / Groth16 verifier)
6. Block hard-confirmed ─── proof batched for L2 settlement
```

**Settlement path:** `L3 batch → SP1 Groth16 proof → COLDL3Settlement.sol on zkSync Era → Ethereum L1`

**Proof modes:**

| Mode | Speed | On-chain | Use case |
|------|-------|----------|----------|
| `execute` | ~1s | No | Test correctness |
| `compressed` | 30-60s | No | Internal verification |
| `groth16` | 60-120s | Yes | Production settlement |
| `plonk` | 60-120s | Yes | Alternative on-chain |

**Two-mode verification:** `real-proofs` feature off = mock/testnet (fast iteration), on = cryptographic SP1 Groth16 verification (mainnet).

**`BlockExecutionClaim` public inputs** — what the proof commits to:

| Field | Size | Description |
|-------|------|-------------|
| `prev_state_root` | 32B | State root before block |
| `new_state_root` | 32B | State root after block |
| `tx_merkle_root` | 32B | Merkle root of transaction hashes |
| `note_tree_root` | 32B | Shielded pool note tree root |
| `nullifier_count` | 4B | Nullifiers consumed |
| `total_gas_used` | 8B | Gas consumed |

See [`prover/README.md`](prover/README.md) for operator setup — how to run a prover node, export verification keys, and earn HEAT rewards.

### SP1 zkVM
Block proofs run on [SP1](https://github.com/succinctlabs/sp1) (RISC-V zkVM). The guest verifies all client-supplied proofs (Schnorr, Bulletproofs, Merkle membership) inside the ZK circuit — the sequencer's processing is early rejection only, trustless by design.

SP1 patches active: `sha2`, `curve25519-dalek-ng` (Ristretto255), `k256` (secp256k1), `p256` (P-256/WebAuthn), `ed25519-consensus`, BN254 native syscalls, `revm` (2–5x EVM speedup).

---

## Crate Structure

```
/
├── src/              Host — sequencer node, RPC, block building
│   ├── main.rs       HTTP server, block loop, EVM execution
│   ├── aa/           Account Abstraction — types, Schnorr, validation, factory, paymaster
│   ├── privacy/      Shielded pool, commitment proofs, stealth addresses
│   ├── genesis/      Chain identity, testnet bootstrap, initial allocations
│   ├── proving/      SP1 proof verification (real-proofs feature gate)
│   ├── bridge/       Canonical bridge (Era ↔ C0DL3, withdrawal tree)
│   ├── settlement/   L3→L2 batch settlement pipeline
│   └── storage/      Persistent state (sled KV store)
├── program/          SP1 guest — zkVM block proof circuit
│   ├── src/main.rs   Block verification phases (state, EVM, privacy, AA)
│   ├── precompiles.rs All precompiles
│   └── privacy.rs    ZK-side privacy verification
├── contracts/        Reference Solidity contracts
│   ├── PrivateWallet.sol   AA wallet (Pedersen commitments)
│   ├── Paymaster.sol       Gas payment abstraction
│   ├── COLDL3Settlement.sol  L2 settlement (SP1 proof verification)
│   └── C0DL3Bridge.sol       Canonical bridge (deposit pools, withdrawal proofs)
├── sdk/              Wallet SDK — proof builders, auto-shield, stealth, memos
├── prover/           Standalone SP1 prover node
├── explorer/         Minimal block explorer UI (served at /explorer)
└── docker/           Dockerfile + docker-compose
```

---

## Quick Start

```bash
git clone https://github.com/ColinRitman/C0DL3.git
cd C0DL3
cargo build --release

# Run sequencer node
cargo run --release

# Run tests
cargo test --lib

# Open block explorer
open http://localhost:9944/explorer
```

**Dependencies:** Rust 1.75+

**Docker:**

```bash
# Build and run
docker build -t c0dl3-node -f docker/Dockerfile .
docker run -p 9944:9944 -p 30333:30333 -v c0dl3-data:/app/data c0dl3-node

# Or with docker-compose
cd docker && docker compose up
```

---

## Settlement

C0DL3 settles to Ethereum via zkSync Era (L2). The settlement pipeline:

```
1. Prover generates SP1 Groth16 proof for L3 block
2. Sequencer verifies proof and records ProvenBlock
3. Settlement manager batches proven blocks (configurable batch_size)
4. Submits (publicValues, proofBytes) to COLDL3Settlement.sol on Era
5. Contract verifies proof via SP1VerifierGateway
6. State root committed on L2 → inherits Ethereum L1 finality
```

**Contracts:**

| Contract | Chain | Purpose |
|----------|-------|---------|
| `COLDL3Settlement.sol` | zkSync Era (L2) | Verifies SP1 proofs, commits L3 state roots |
| `SP1VerifierGateway` | zkSync Era (L2) | Succinct's on-chain Groth16/PLONK verifier |

**Modes:**

| Mode | Trigger | Behavior |
|------|---------|----------|
| Mock | No `--sequencer-key` or `--settlement-contract` | Synthetic tx hashes, auto-confirms (testnet dev) |
| Live | Both flags set | Signs real txs via ethers, polls Era for receipts |

```bash
# Mock mode (default — no Era deployment needed)
cargo run --release

# Live mode (requires deployed COLDL3Settlement.sol + funded sequencer)
cargo run --release -- \
  --era-rpc-url https://sepolia.era.zksync.dev \
  --settlement-contract 0xYOUR_CONTRACT \
  --sequencer-key YOUR_PRIVATE_KEY_HEX
```

**RPC endpoints:**

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/settlement` | GET | Settlement pipeline status (mode, batch counts) |
| `/settlement/batches` | GET | List all settlement batches |

---

## Testnet

**Chain ID:** `0xC0D13` (789779)

**Genesis accounts:**

| Address | Balance | Role |
|---------|---------|------|
| `0xC0DL3_FAUCET_...01` | 1B HEAT | Testnet faucet |
| `0xC0DL3_SEQUENCER_...01` | 100M HEAT | Sequencer operator |
| `0xC0DL3_PAYMASTER_...01` | 50M HEAT | Default paymaster |

**Testnet endpoints:**

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/rpc` | POST | Ethereum JSON-RPC (`eth_chainId`, `eth_blockNumber`, `eth_getBalance`, etc.) |
| `/faucet` | POST | Drip testnet HEAT (`{"address": "0x...", "amount": 1000000000000}`) |
| `/storage/status` | GET | Persistent state database info |
| `/aa/create_wallet` | POST | Create AA PrivateWallet |
| `/aa/send` | POST | Submit AA UserOperation |
| `/bridge/status` | GET | Bridge pipeline status (deposits, withdrawals, tree root) |
| `/bridge/pools` | GET | Darkpool anonymity set health per token/tier |
| `/bridge/withdraw` | POST | Request withdrawal from C0DL3 to Era |
| `/bridge/withdrawal_proof/{pos}` | GET | Merkle proof for claiming on Era |
| `/explorer` | GET | Block explorer UI |

**Persistent state:** The node stores all state to disk (sled). Survives restarts — resumes from last block height.

---

## Darkpool Bridge (Era ↔ C0DL3)

Privacy-preserving canon bridge — the privacy boundary between public DeFi and the C0DL3 darkpool. Fixed-denomination pools prevent amount-based correlation. Inside C0DL3, amounts are arbitrary (Pedersen commitments). No multisig, no oracle — security = SP1 proofs.

**Deposit (Era → C0DL3):**
```
1. User deposits token into a fixed-denomination pool on C0DL3Bridge.sol (Era)
2. Sequencer observes Deposit event on Era
3. L3 mints shielded Pedersen commitment to user's stealth address
4. Privacy begins — all activity inside C0DL3 is hidden
```

**Withdrawal (C0DL3 → Era):**
```
1. User proves ownership of commitment on L3 (spends nullifier)
2. L3 adds withdrawal leaf to withdrawal Merkle tree
3. Withdrawal tree root committed in SP1 proof (BlockExecutionClaim)
4. After settlement + delay (1-4h), user claims on Era with Merkle proof
```

**Launch tokens (4 tokens × 3 denominations = 12 pools):**

| Token | Small | Medium | Large | Role |
|-------|-------|--------|-------|------|
| HEAT | 100,000 | 10 Million | 1 Billion | Native gas token |
| ZK | 1000 | 10,000 | 100,000 | Era native — privacy for ZK holders |
| C0LD | 0.0001 | 0.001 | 0.1 | COLDAO governance |
| ETH | 0.1 | 1 | 10 | Based asset |

**Why 3 denominations per token:**
- Expected anonymity = N/K (N=total deposits, K=tiers) — 3 tiers leak only 1.58 bits about amount
- Exponential spacing covers 3 orders of magnitude (retail through whale)
- Each additional tier dilutes anonymity by 1/(K+1) — 4th tier costs 25% for only marginal efficiency gain
- Minimum anonymity set threshold: 50 deposits per pool before meaningful privacy begins

**Per-token denomination registry:**
- Configurable via governance (sequencer can `addToken()` / `addDenomination()`)
- Maximum 3 tiers per token (prevents over-fragmentation)
- On-chain anonymity set counters via `getPoolHealth()` lets users verify pool safety before depositing
- New tokens added by C0LDAO approval, only when existing pools have healthy anonymity sets

**Privacy features:**
- Fixed denomination pools prevent amount-based deposit/withdrawal correlation
- Withdrawal amounts need not match deposit amounts
- No on-chain link between deposit address and withdrawal address
- Time-delayed withdrawals (1-4 hour window for timing decorrelation)
- State root history — supports withdrawal proofs against historical settled roots
- SDK auto-splits deposits (e.g., 7 ETH → 7×1 ETH)

**Bridge endpoints:**

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/bridge/status` | GET | Bridge pipeline status (deposits, withdrawals, tree root) |
| `/bridge/pools` | GET | Darkpool anonymity sets for all tokens and tiers |
| `/bridge/withdraw` | POST | Request withdrawal from C0DL3 to Era |
| `/bridge/withdrawal_proof/{pos}` | GET | Merkle proof for claiming on Era |

| Contract | Chain | Purpose |
|----------|-------|---------|
| `C0DL3Bridge.sol` | zkSync Era | Per-token denomination pools, withdrawal Merkle verification |
| `COLDL3Settlement.sol` | zkSync Era | Commits L3 state roots (bridge verifies against these) |

## Fuego Bridge

C0DL3 maintains a bidirectional bridge to Fuego L1 for banking commitments. Bridge operations are privacy-preserving — amounts are shielded during cross-chain transit.

---

## Roadmap

- [x] Confidential transactions (Bulletproofs)
- [x] Shielded pool (nullifiers + Merkle proofs)
- [x] Client-side partial proving (commitment knowledge proofs)
- [x] Privacy precompile suite (Schnorr, conservation, ElGamal, Ed25519)
- [x] Ethereum-compatible precompiles (ecRecover, SHA-256, BN254, ModExp)
- [x] Native AA wallets (private-by-default balances)
- [x] Paymaster gas abstraction
- [x] SDK auto-shield flow
- [x] Guest-side AA verification (SP1 Phase 7)
- [x] Reference Solidity contracts (PrivateWallet.sol, Paymaster.sol)
- [x] WebAuthn wallet auth (P-256 passkeys precompile)
- [x] SP1 proving pipeline
- [x] Settlement contracts on zkSync Era
- [x] Data availability (via zkSync Era L2 → Ethereum L1)
- [x] Persistent state storage (sled)
- [x] Genesis config + chain ID (0xC0D13)
- [x] Testnet faucet
- [x] Ethereum JSON-RPC compatibility
- [x] Block explorer UI
- [x] Docker image + docker-compose
- [x] Live Era Sepolia settlement (ethers, dual-mode)
- [x] Darkpool bridge — per-token denominations, anonymity set tracking (HEAT, ZK, C0LD, ETH)
- [x] Withdrawal tree root committed in SP1 proofs (BlockExecutionClaim)
- [x] Bridge deposit → shielded pool (auto-mint on block production)

---

## License

MIT — see [LICENSE](LICENSE)
