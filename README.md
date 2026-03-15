# C0DL3

[![Rust](https://img.shields.io/badge/rust-1.75+-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Branch](https://img.shields.io/badge/branch-zkc0dl3-blueviolet.svg)](https://github.com/ColinRitman/C0DL3/tree/zkc0dl3)

**C0DL3** is a privacy-first L3 rollup on zkSync Era. All balances are private by default — every account is an AA smart contract wallet storing Pedersen commitments instead of plaintext values. A shielded pool provides full sender/recipient anonymity on demand. ZK validity proofs are generated via SP1 (RISC-V zkVM).

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
│   └── privacy/      Shielded pool, commitment proofs, stealth addresses
├── program/          SP1 guest — zkVM block proof circuit
│   ├── src/main.rs   Block verification phases (state, EVM, privacy, AA)
│   ├── precompiles.rs All precompiles
│   └── privacy.rs    ZK-side privacy verification
├── sdk/              Wallet SDK — proof builders, auto-shield, stealth, memos
└── prover/           Standalone SP1 prover node
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
```

**Dependencies:** Rust 1.75+

---

## Fuego Bridge

C0DL3 maintains a bidirectional bridge to Fuego L1 for asset transfers. Bridge operations are privacy-preserving — amounts are shielded during cross-chain transit.

---

## Roadmap

- [x] Confidential transactions (Bulletproofs)
- [x] Shielded pool (nullifiers + Merkle proofs)
- [x] Client-side partial proving (commitment knowledge proofs)
- [x] Privacy precompile suite (Schnorr, conservation, ElGamal, Ed25519)
- [x] Ethereum-compatible precompiles (ecRecover, SHA-256, BN254, ModExp)
- [ ] Native AA wallets (privacy-by-default balances) — **in progress**
- [ ] Paymaster gas abstraction
- [ ] SDK auto-shield flow
- [ ] SP1 proving pipeline
- [ ] Settlement contracts on zkSync Era
- [ ] EIP-4844 DA (blob submission)
- [ ] WebAuthn wallet auth (P-256 passkeys)

---

## License

MIT — see [LICENSE](LICENSE)
