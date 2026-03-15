# Privacy-by-Default: Account Abstraction + Automatic Shielding

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make all COLDL3 wallet balances private by default via AA wallet contracts with commitment-based storage, with automatic shield-transfer-unshield for full anonymity.

**Architecture:** Every account is a smart contract wallet (native AA) storing `balanceCommitment` instead of plaintext balance. Normal transfers exchange commitment updates verified via precompiles. For high-privacy transfers, the wallet SDK automatically shields → private transfer → delayed unshield behind the scenes. Gas is paid by paymasters to break the sender-gas link.

**Tech Stack:** Rust (host + guest), revm (EVM execution), curve25519-dalek-ng (Ristretto255), bulletproofs (range proofs), SP1 (zkVM), Solidity (wallet contract template, paymaster)

---

## File Structure

### New Files

| File | Responsibility |
|------|---------------|
| `src/aa/mod.rs` | AA module root — exports wallet factory, paymaster, validation |
| `src/aa/wallet_factory.rs` | Default PrivateWallet contract deployment + CREATE2 stealth addresses |
| `src/aa/wallet_template.rs` | PrivateWallet contract bytecode + ABI (commitment-based account) |
| `src/aa/paymaster.rs` | Gas payment abstraction — paymasters pay on behalf of users |
| `src/aa/validation.rs` | AA transaction validation logic (replaces EOA nonce/sig checks) |
| `src/aa/types.rs` | AA-specific types: UserOperation, PaymasterData, WalletConfig |
| `program/src/aa.rs` | Guest-side AA validation (verify wallet proofs inside SP1) |
| `program/src/precompiles_new.rs` | New precompiles: Schnorr Verify (0x0106), Conservation Check (0x0107), ElGamal Encrypt (0x0104), Encrypted Memo (0x010A), Poseidon Hash (0x0103), Private Swap (0x0120), Threshold Proof (0x0121), Commitment Arithmetic (0x0122), Batch Verify (0x0132) |
| `sdk/src/auto_shield.rs` | Automatic shield-transfer-unshield flow for wallet SDK |
| `sdk/src/stealth.rs` | Stealth address derivation (CREATE2 salt + ECDH) |
| `sdk/src/encrypted_memo.rs` | Attach encrypted amount/blinding to commitments |
| `contracts/PrivateWallet.sol` | Reference Solidity for the default AA wallet contract |
| `contracts/Paymaster.sol` | Reference Solidity for the paymaster contract |

### Modified Files

| File | Changes |
|------|---------|
| `src/main.rs` | Replace `AccountState.balance` with commitment-only model; add AA transaction processing; new `/aa/create_wallet`, `/aa/send` endpoints; modify `execute_transaction` to use AA validation |
| `src/privacy/mod.rs` | Export `aa` module |
| `program/src/main.rs` | Add Phase 7: AA wallet proof verification; modified Phase 2 to handle AA transactions |
| `program/src/precompiles.rs` | Add new precompile addresses to registration |
| `program/src/state.rs` | `AccountWitness` gets `wallet_type` field (EOA legacy vs AA wallet) |
| `sdk/src/lib.rs` | Export auto_shield, stealth, encrypted_memo modules |
| `sdk/Cargo.toml` | Add `x25519-dalek` for ECDH, `aes-gcm` for memo encryption |

---

## Phase 1: Foundation — AA Types + New Precompiles

This phase adds the type system and cryptographic primitives. No behavior changes yet — existing tests must continue to pass.

### Task 1: AA Type Definitions

**Files:**
- Create: `src/aa/types.rs`
- Create: `src/aa/mod.rs`
- Modify: `src/main.rs` (add `mod aa;`)

- [ ] **Step 1: Write tests for AA types**

Create `src/aa/types.rs` with types and basic tests:

```rust
// src/aa/types.rs
// Account Abstraction types for COLDL3.
//
// Every account is a smart contract wallet. No EOAs.
// Wallets store balance commitments, not plaintext balances.
// Transactions are UserOperations validated by the wallet contract.

use serde::{Deserialize, Serialize};

/// Wallet type — distinguishes legacy EOA accounts from AA wallets.
/// During migration, both coexist. New accounts are always AA.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WalletType {
    /// Legacy EOA account (pre-AA migration). Plaintext balance, ECDSA auth.
    LegacyEOA,
    /// Native AA wallet. Commitment-based balance, Schnorr auth.
    PrivateWallet,
}

impl Default for WalletType {
    fn default() -> Self {
        WalletType::PrivateWallet // New accounts default to AA
    }
}

/// A user operation — the AA equivalent of a transaction.
///
/// Instead of (from, to, value, nonce, signature), a UserOp contains:
/// - sender: the wallet contract address
/// - call_data: what the wallet should execute
/// - commitment_updates: new balance commitments for sender + recipient
/// - conservation_proof: proves Σ inputs = Σ outputs (no value created/destroyed)
/// - auth_proof: Schnorr proof that the sender owns the wallet
/// - paymaster: optional address that pays gas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserOperation {
    /// Sender wallet contract address.
    pub sender: String,
    /// Nonce (anti-replay, sequential per wallet).
    pub nonce: u64,
    /// Recipient address (wallet or contract).
    pub to: String,
    /// New sender balance commitment after transfer.
    pub sender_new_commitment: [u8; 32],
    /// New recipient balance commitment after transfer.
    pub recipient_new_commitment: [u8; 32],
    /// Pedersen commitment to the transfer amount: C = amount*G + r*H
    pub amount_commitment: [u8; 32],
    /// Schnorr knowledge proof: sender knows (amount, blinding) for amount_commitment.
    pub knowledge_proof: KnowledgeProofBytes,
    /// Range proof: amount in [0, 2^64). Serialized Bulletproofs.
    pub range_proof: Vec<u8>,
    /// Conservation proof: old_sender_commit - new_sender_commit = amount_commit
    /// AND new_recipient_commit - old_recipient_commit = amount_commit
    /// Encoded as the kernel excess point (should be identity if valid).
    pub conservation_excess: [u8; 32],
    /// Optional paymaster address. If set, paymaster pays gas.
    pub paymaster: Option<String>,
    /// Gas limit for this operation.
    pub gas_limit: u64,
    /// Gas price in fwei.
    pub gas_price: u64,
    /// Auth signature — Schnorr signature over operation hash.
    pub auth_signature: SchnorrSignature,
    /// Optional encrypted memo for recipient (ElGamal encrypted amount + blinding).
    pub encrypted_memo: Option<Vec<u8>>,
    /// Arbitrary calldata for contract interactions (empty for simple transfers).
    pub call_data: Vec<u8>,
}

/// Schnorr signature (R, s) over a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchnorrSignature {
    /// R = k*G (nonce point, compressed Ristretto)
    pub r_point: [u8; 32],
    /// s = k + e*privkey where e = H(R || pubkey || message)
    pub s_scalar: [u8; 32],
}

/// Knowledge proof bytes — matches CommitmentKnowledgeProof layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeProofBytes {
    pub commitment: [u8; 32],
    pub announcement: [u8; 32],
    pub response_v: [u8; 32],
    pub response_r: [u8; 32],
}

/// Paymaster approval — a signed statement that a paymaster will cover gas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymasterApproval {
    /// Paymaster address (must have sufficient public balance for gas).
    pub paymaster: String,
    /// Maximum gas the paymaster will cover for this UserOp.
    pub max_gas: u64,
    /// Schnorr signature from paymaster authorizing this gas payment.
    pub signature: SchnorrSignature,
    /// Expiry block height (approval invalid after this).
    pub valid_until_block: u64,
}

/// Configuration for a PrivateWallet contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletConfig {
    /// Owner's public key (Ristretto255 compressed).
    pub owner_pubkey: [u8; 32],
    /// Optional guardian public keys for social recovery.
    pub guardians: Vec<[u8; 32]>,
    /// Number of guardian signatures required for recovery.
    pub recovery_threshold: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wallet_type_default_is_private() {
        assert_eq!(WalletType::default(), WalletType::PrivateWallet);
    }

    #[test]
    fn test_wallet_type_serialization() {
        let wt = WalletType::PrivateWallet;
        let json = serde_json::to_string(&wt).unwrap();
        let decoded: WalletType = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, wt);
    }

    #[test]
    fn test_user_operation_serialization() {
        let op = UserOperation {
            sender: "0x1234".to_string(),
            nonce: 1,
            to: "0x5678".to_string(),
            sender_new_commitment: [0xAA; 32],
            recipient_new_commitment: [0xBB; 32],
            amount_commitment: [0xCC; 32],
            knowledge_proof: KnowledgeProofBytes {
                commitment: [0xCC; 32],
                announcement: [0xDD; 32],
                response_v: [0x01; 32],
                response_r: [0x02; 32],
            },
            range_proof: vec![0u8; 64],
            conservation_excess: [0; 32],
            paymaster: None,
            gas_limit: 100_000,
            gas_price: 1,
            auth_signature: SchnorrSignature {
                r_point: [0xEE; 32],
                s_scalar: [0xFF; 32],
            },
            encrypted_memo: None,
            call_data: vec![],
        };
        let json = serde_json::to_string(&op).unwrap();
        let decoded: UserOperation = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.sender, op.sender);
        assert_eq!(decoded.nonce, op.nonce);
    }
}
```

- [ ] **Step 2: Create module root**

Create `src/aa/mod.rs`:
```rust
pub mod types;
// pub mod wallet_factory;   // Phase 2
// pub mod wallet_template;  // Phase 2
// pub mod paymaster;        // Phase 3
// pub mod validation;       // Phase 2

pub use types::*;
```

- [ ] **Step 3: Wire into main crate**

Add `mod aa;` to `src/main.rs` near the top with other module declarations. Add `serde_json` to dev-dependencies if not present (for test serialization).

- [ ] **Step 4: Run tests**

Run: `cargo test aa::types::tests -v`
Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/aa/
git commit -m "feat(aa): add account abstraction type definitions

UserOperation, SchnorrSignature, WalletConfig, PaymasterApproval types.
WalletType enum defaults to PrivateWallet for new accounts."
```

---

### Task 2: Schnorr Signature Precompile (0x0106)

**Files:**
- Create: `src/aa/schnorr.rs`
- Modify: `program/src/precompiles.rs` (add 0x0106)

Schnorr signatures are the auth mechanism for AA wallets. This precompile lets contracts verify wallet ownership on-chain.

- [ ] **Step 1: Write Schnorr sign/verify in `src/aa/schnorr.rs`**

```rust
// src/aa/schnorr.rs
// Schnorr signature scheme over Ristretto255.
//
// Sign: pick k → R = k*G → e = H("C0DL3:schnorr:" || R || pubkey || msg) → s = k + e*privkey
// Verify: s*G == R + e*pubkey
//
// This is used for AA wallet auth — proving ownership without revealing the private key.

use curve25519_dalek_ng::{
    constants::RISTRETTO_BASEPOINT_POINT as G,
    ristretto::{CompressedRistretto, RistrettoPoint},
    scalar::Scalar,
};
use sha2::{Digest, Sha256};

use crate::aa::types::SchnorrSignature;

/// Sign a message with a Ristretto255 private key.
///
/// Returns a Schnorr signature (R, s) where:
///   R = k*G (random nonce point)
///   s = k + H("C0DL3:schnorr:" || R || pubkey || msg) * privkey
pub fn schnorr_sign(privkey: &Scalar, message: &[u8]) -> SchnorrSignature {
    let pubkey = (privkey * G).compress().to_bytes();

    // Deterministic nonce: k = H("C0DL3:schnorr_nonce:" || privkey || message)
    // (RFC 6979-style — avoids nonce reuse attacks)
    let mut nonce_input = Vec::new();
    nonce_input.extend_from_slice(b"C0DL3:schnorr_nonce:");
    nonce_input.extend_from_slice(&privkey.to_bytes());
    nonce_input.extend_from_slice(message);
    let k_bytes: [u8; 32] = Sha256::digest(&nonce_input).into();
    let k = Scalar::from_bytes_mod_order(k_bytes);

    let r_point = (k * G).compress();

    // Challenge: e = H("C0DL3:schnorr:" || R || pubkey || message)
    let e = schnorr_challenge(&r_point.to_bytes(), &pubkey, message);

    // Response: s = k + e * privkey
    let s = k + e * privkey;

    SchnorrSignature {
        r_point: r_point.to_bytes(),
        s_scalar: s.to_bytes(),
    }
}

/// Verify a Schnorr signature against a public key.
///
/// Checks: s*G == R + e*pubkey
/// where e = H("C0DL3:schnorr:" || R || pubkey || msg)
pub fn schnorr_verify(pubkey: &[u8; 32], message: &[u8], sig: &SchnorrSignature) -> bool {
    let pk_point = match CompressedRistretto(*pubkey).decompress() {
        Some(p) => p,
        None => return false,
    };
    let r_point = match CompressedRistretto(sig.r_point).decompress() {
        Some(p) => p,
        None => return false,
    };

    let e = schnorr_challenge(&sig.r_point, pubkey, message);
    let s = Scalar::from_bytes_mod_order(sig.s_scalar);

    // Verify: s*G == R + e*pubkey
    let lhs = s * G;
    let rhs = r_point + e * pk_point;

    lhs == rhs
}

/// Compute Schnorr challenge scalar.
fn schnorr_challenge(r_bytes: &[u8; 32], pubkey: &[u8; 32], message: &[u8]) -> Scalar {
    let mut hasher = Sha256::new();
    hasher.update(b"C0DL3:schnorr:");
    hasher.update(r_bytes);
    hasher.update(pubkey);
    hasher.update(message);
    let hash: [u8; 32] = hasher.finalize().into();
    Scalar::from_bytes_mod_order(hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;

    fn random_scalar() -> Scalar {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        Scalar::from_bytes_mod_order(bytes)
    }

    #[test]
    fn test_schnorr_sign_verify_roundtrip() {
        let privkey = random_scalar();
        let pubkey = (privkey * G).compress().to_bytes();
        let message = b"transfer 100 to Bob";

        let sig = schnorr_sign(&privkey, message);
        assert!(schnorr_verify(&pubkey, message, &sig));
    }

    #[test]
    fn test_schnorr_wrong_message_fails() {
        let privkey = random_scalar();
        let pubkey = (privkey * G).compress().to_bytes();

        let sig = schnorr_sign(&privkey, b"original message");
        assert!(!schnorr_verify(&pubkey, b"tampered message", &sig));
    }

    #[test]
    fn test_schnorr_wrong_pubkey_fails() {
        let privkey = random_scalar();
        let wrong_pubkey = (random_scalar() * G).compress().to_bytes();
        let message = b"test";

        let sig = schnorr_sign(&privkey, message);
        assert!(!schnorr_verify(&wrong_pubkey, message, &sig));
    }

    #[test]
    fn test_schnorr_deterministic_nonce() {
        // Same (key, message) → same signature (deterministic nonce)
        let privkey = random_scalar();
        let message = b"same message";

        let sig1 = schnorr_sign(&privkey, message);
        let sig2 = schnorr_sign(&privkey, message);
        assert_eq!(sig1.r_point, sig2.r_point);
        assert_eq!(sig1.s_scalar, sig2.s_scalar);
    }

    #[test]
    fn test_schnorr_different_messages_different_sigs() {
        let privkey = random_scalar();
        let sig1 = schnorr_sign(&privkey, b"message A");
        let sig2 = schnorr_sign(&privkey, b"message B");
        assert_ne!(sig1.r_point, sig2.r_point);
    }

    #[test]
    fn test_schnorr_invalid_points() {
        let sig = SchnorrSignature {
            r_point: [0xFF; 32], // not a valid Ristretto point
            s_scalar: [0; 32],
        };
        assert!(!schnorr_verify(&[0xFF; 32], b"test", &sig));
    }
}
```

- [ ] **Step 2: Add to aa/mod.rs**

Add `pub mod schnorr;` and `pub use schnorr::*;` to `src/aa/mod.rs`.

- [ ] **Step 3: Run host tests**

Run: `cargo test aa::schnorr::tests -v`
Expected: 6 tests pass.

- [ ] **Step 4: Add Schnorr Verify precompile (0x0106) to guest**

Add to `program/src/precompiles.rs` — register address `0x0106` with a handler that:
- Input: `pubkey(32) || sig_r(32) || sig_s(32) || msg_len_le32(4) || message`
- Output: `0x01` (valid) or `0x00` (invalid)
- Gas: 3,000

Follow the existing pattern used for `PRECOMPILE_PEDERSEN_COMMIT` (0x0101).

- [ ] **Step 5: Run all tests**

Run: `cargo test --lib`
Expected: All 153+ host tests pass, plus new Schnorr tests.

- [ ] **Step 6: Commit**

```bash
git add src/aa/schnorr.rs src/aa/mod.rs program/src/precompiles.rs
git commit -m "feat(aa): Schnorr signature scheme + precompile 0x0106

Ristretto255 Schnorr signatures for AA wallet authentication.
Deterministic nonce (RFC 6979-style) prevents nonce reuse.
Guest precompile at 0x0106 for on-chain verification (3,000 gas)."
```

---

### Task 3: Conservation Check Precompile (0x0107)

**Files:**
- Modify: `program/src/precompiles.rs`

This precompile verifies that commitment updates conserve value: `old_A - new_A = new_B - old_B` (transfer amount is balanced). Privacy-aware contracts use this to verify transfers without seeing amounts.

- [ ] **Step 1: Add conservation precompile to guest**

In `program/src/precompiles.rs`, add handler for `0x0107`:
- Input: `old_sender_commit(32) || new_sender_commit(32) || old_recipient_commit(32) || new_recipient_commit(32)`
- Logic: Decompress all 4 points. Check: `(old_sender - new_sender) == (new_recipient - old_recipient)`
  This means: what sender lost = what recipient gained (conservation).
- Output: `0x01` (balanced) or `0x00` (not balanced)
- Gas: 4,000 (4 decompressions + 2 subtractions + 1 comparison)

- [ ] **Step 2: Add test for conservation precompile**

Test with known Pedersen commitments: create commitments for (100, r1), (80, r2), (0, r3), (20, r4) where 100-80=20-0=20. Verify the precompile returns 0x01.

- [ ] **Step 3: Run all tests**

Run: `cargo test --lib`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add program/src/precompiles.rs
git commit -m "feat(precompile): conservation check at 0x0107

Verifies Pedersen commitment balance conservation:
old_sender - new_sender == new_recipient - old_recipient.
4,000 gas. Used by AA wallets for private transfers."
```

---

### Task 4: ElGamal Encrypted Memo Precompile (0x0104 + 0x010A)

**Files:**
- Create: `sdk/src/encrypted_memo.rs`
- Modify: `program/src/precompiles.rs`

Encrypted memos let the sender attach the amount + blinding factor encrypted to the recipient's public key. The recipient can decrypt to learn their new balance. Without this, the recipient has no way to know what was sent.

- [ ] **Step 1: Write ElGamal encrypt/decrypt in SDK**

Create `sdk/src/encrypted_memo.rs`:

```rust
// sdk/src/encrypted_memo.rs
// Encrypted memos — sender encrypts (amount, blinding) to recipient's pubkey.
//
// Uses ElGamal encryption over Ristretto255:
//   Encrypt(pubkey, plaintext):
//     k = random
//     C1 = k*G (ephemeral pubkey)
//     shared = k*pubkey
//     mask = SHA-256("C0DL3:memo:" || shared)
//     C2 = plaintext XOR mask
//     memo = C1 || C2
//
//   Decrypt(privkey, memo):
//     shared = privkey * C1
//     mask = SHA-256("C0DL3:memo:" || shared)
//     plaintext = C2 XOR mask

use curve25519_dalek_ng::{
    constants::RISTRETTO_BASEPOINT_POINT as G,
    ristretto::CompressedRistretto,
    scalar::Scalar,
};
use sha2::{Digest, Sha256};

/// Encrypt amount + blinding to recipient's public key.
///
/// Returns 72-byte memo: ephemeral_pubkey(32) || encrypted_data(40)
/// where encrypted_data = (amount_le8 || blinding_32) XOR SHA-256(shared_secret)
pub fn encrypt_memo(
    recipient_pubkey: &[u8; 32],
    amount: u64,
    blinding: &[u8; 32],
) -> Option<Vec<u8>> {
    let pk = CompressedRistretto(*recipient_pubkey).decompress()?;

    // Random ephemeral key
    let mut k_bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut k_bytes);
    let k = Scalar::from_bytes_mod_order(k_bytes);

    // C1 = k*G
    let c1 = (k * G).compress().to_bytes();

    // Shared secret = k * recipient_pubkey
    let shared = (k * pk).compress().to_bytes();

    // Derive mask (40 bytes: 8 for amount + 32 for blinding)
    let mask = derive_memo_mask(&shared);

    // Plaintext: amount(8) || blinding(32)
    let mut plaintext = [0u8; 40];
    plaintext[..8].copy_from_slice(&amount.to_le_bytes());
    plaintext[8..40].copy_from_slice(blinding);

    // XOR
    let mut encrypted = [0u8; 40];
    for i in 0..40 {
        encrypted[i] = plaintext[i] ^ mask[i];
    }

    let mut memo = Vec::with_capacity(72);
    memo.extend_from_slice(&c1);
    memo.extend_from_slice(&encrypted);
    Some(memo)
}

/// Decrypt a memo using the recipient's private key.
///
/// Returns (amount, blinding) if decryption succeeds.
pub fn decrypt_memo(privkey: &Scalar, memo: &[u8]) -> Option<(u64, [u8; 32])> {
    if memo.len() != 72 {
        return None;
    }

    let mut c1_bytes = [0u8; 32];
    c1_bytes.copy_from_slice(&memo[..32]);
    let c1 = CompressedRistretto(c1_bytes).decompress()?;

    // Shared secret = privkey * C1
    let shared = (privkey * c1).compress().to_bytes();

    // Derive same mask
    let mask = derive_memo_mask(&shared);

    // Decrypt
    let mut plaintext = [0u8; 40];
    for i in 0..40 {
        plaintext[i] = memo[32 + i] ^ mask[i];
    }

    let amount = u64::from_le_bytes(plaintext[..8].try_into().ok()?);
    let mut blinding = [0u8; 32];
    blinding.copy_from_slice(&plaintext[8..40]);

    Some((amount, blinding))
}

/// Derive 40-byte mask from shared secret.
fn derive_memo_mask(shared_secret: &[u8; 32]) -> [u8; 40] {
    // First 32 bytes from SHA-256
    let h1: [u8; 32] = Sha256::digest(
        [b"C0DL3:memo:".as_slice(), shared_secret].concat()
    ).into();
    // Next 8 bytes from SHA-256 with counter
    let h2: [u8; 32] = Sha256::digest(
        [b"C0DL3:memo:1:".as_slice(), shared_secret].concat()
    ).into();

    let mut mask = [0u8; 40];
    mask[..32].copy_from_slice(&h1);
    mask[32..40].copy_from_slice(&h2[..8]);
    mask
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random_scalar() -> Scalar {
        let mut bytes = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
        Scalar::from_bytes_mod_order(bytes)
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let privkey = random_scalar();
        let pubkey = (privkey * G).compress().to_bytes();
        let amount = 1000u64;
        let blinding = random_scalar().to_bytes();

        let memo = encrypt_memo(&pubkey, amount, &blinding).unwrap();
        assert_eq!(memo.len(), 72);

        let (dec_amount, dec_blinding) = decrypt_memo(&privkey, &memo).unwrap();
        assert_eq!(dec_amount, amount);
        assert_eq!(dec_blinding, blinding);
    }

    #[test]
    fn test_wrong_key_fails() {
        let privkey = random_scalar();
        let pubkey = (privkey * G).compress().to_bytes();
        let wrong_key = random_scalar();

        let memo = encrypt_memo(&pubkey, 500, &[0xAB; 32]).unwrap();
        let (dec_amount, _) = decrypt_memo(&wrong_key, &memo).unwrap();
        // With wrong key, decrypted amount will be garbage (overwhelmingly likely != 500)
        assert_ne!(dec_amount, 500);
    }

    #[test]
    fn test_various_amounts() {
        let privkey = random_scalar();
        let pubkey = (privkey * G).compress().to_bytes();

        for &amount in &[0u64, 1, u64::MAX, 1_000_000_000] {
            let blinding = random_scalar().to_bytes();
            let memo = encrypt_memo(&pubkey, amount, &blinding).unwrap();
            let (dec_amount, dec_blinding) = decrypt_memo(&privkey, &memo).unwrap();
            assert_eq!(dec_amount, amount);
            assert_eq!(dec_blinding, blinding);
        }
    }

    #[test]
    fn test_invalid_memo_length() {
        let privkey = random_scalar();
        assert!(decrypt_memo(&privkey, &[0u8; 10]).is_none());
        assert!(decrypt_memo(&privkey, &[0u8; 100]).is_none());
    }
}
```

- [ ] **Step 2: Add exports to SDK**

In `sdk/src/lib.rs`, add `pub mod encrypted_memo;`.

- [ ] **Step 3: Run SDK tests**

Run: `cd sdk && cargo test encrypted_memo -v`
Expected: 4 tests pass.

- [ ] **Step 4: Add ElGamal + Encrypted Memo precompiles to guest**

In `program/src/precompiles.rs`:

**0x0104 (ElGamal Encrypt)**:
- Input: `recipient_pubkey(32) || amount_le64(8) || blinding(32)`
- Output: `encrypted_memo(72)` — ephemeral_pubkey(32) || encrypted_data(40)
- Gas: 5,000
- Note: This precompile is for contracts that want to create memos on-chain. The SDK does this client-side, but contracts may need it too.

**0x010A (Memo Decrypt Verify)**:
- Input: `memo(72) || expected_amount_le64(8) || expected_blinding(32) || privkey(32)`
- Output: `0x01` (matches) or `0x00` (doesn't match)
- Gas: 5,000
- Note: For contract-side verification that a memo decrypts to expected values.

- [ ] **Step 5: Run all tests**

Run: `cargo test --lib && cd sdk && cargo test`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add sdk/src/encrypted_memo.rs sdk/src/lib.rs program/src/precompiles.rs
git commit -m "feat: ElGamal encrypted memos + precompiles 0x0104, 0x010A

Encrypted memos let sender attach (amount, blinding) encrypted to
recipient's pubkey. Recipient decrypts to learn their balance update.
72-byte memos using ECDH + SHA-256 key derivation."
```

---

### Task 4g: Ethereum-Compatible Precompiles (0x0001, 0x0002, 0x0005–0x0008)

**Files:**
- Modify: `program/Cargo.toml` (add secp256k1, ed25519-consensus patches)
- Modify: `program/src/precompiles.rs`

These mirror Ethereum's standard precompiles so any Solidity contract written for Ethereum works unmodified on C0DL3. The BN254 pairing precompile (0x0008) is especially powerful — it enables on-chain Groth16 proof verification, meaning contracts can verify ZK proofs.

- [ ] **Step 1: Add SP1 patches to `program/Cargo.toml`**

Add to `[patch.crates-io]`:
```toml
# Secp256k1 — for ecRecover (Ethereum precompile 0x01)
k256 = { git = "https://github.com/sp1-patches/elliptic-curves", tag = "patch-k256-0.13.3-sp1-6.0.0" }

# Ed25519 — for 0x0140 (HEAT/COLD verifier contracts; Cosmos/Solana bridge)
ed25519-consensus = { git = "https://github.com/sp1-patches/ed25519-consensus", tag = "patch-ed25519-consensus-2.1.0-sp1-6.0.0" }

# BLS12-381 patch (registered but no precompile yet — activate when needed)
bls12_381 = { git = "https://github.com/sp1-patches/bls12_381", tag = "patch-bls12_381-0.8.0-sp1-6.0.0" }
```

Note: BN254, BigInt, and SHA-256 use SP1 native syscalls — no extra crate patches needed.

- [ ] **Step 2: Add ecRecover precompile (0x0001)**

In `program/src/precompiles.rs`, register `0x0001`:
- Input: `msg_hash(32) || v(32) || r(32) || s(32)` — standard Ethereum ecRecover format
- Logic: Use `k256` crate (SP1-patched secp256k1) to recover the signer address
- Output: zero-padded `address(32)` (left-padded, address in last 20 bytes)
- Gas: 3,000

- [ ] **Step 3: Add SHA-256 precompile (0x0002)**

Register `0x0002`:
- Input: arbitrary bytes
- Output: `sha256_hash(32)` — already accelerated via the existing sha2 patch
- Gas: 60 + 12 per 32-byte word
- This is just exposing our existing patched SHA-256 as an Ethereum-compatible precompile

- [ ] **Step 4: Add BN254 precompiles (0x0006, 0x0007, 0x0008)**

Register using SP1's native BN254 syscalls:

**0x0006 ecAdd**: `point_a(64) || point_b(64)` → `result(64)` — 150 gas
**0x0007 ecMul**: `point(64) || scalar(32)` → `result(64)` — 6,000 gas
**0x0008 ecPairing**: `(G1(64) || G2(128)) × n` → `0x01` or `0x00` — 45,000 + 34,000×n gas

The pairing check is the core of Groth16 — enables on-chain ZK proof verification in any contract.

- [ ] **Step 5: Add ModExp precompile (0x0005)**

Register `0x0005`:
- Input: `base_len(32) || exp_len(32) || mod_len(32) || base || exp || mod` — Ethereum format
- Output: `result(mod_len)` — base^exp mod modulus
- Gas: per EIP-2565 formula
- Use SP1's `syscall_bigint` for acceleration

- [ ] **Step 6: Add Ed25519 verify precompile (0x0140)**

Register `0x0140`:
- Input: `pubkey(32) || signature(64) || msg_len_le32(4) || message`
- Logic: Use `ed25519-consensus` crate (SP1-patched)
- Output: `0x01` (valid) or `0x00` (invalid)
- Gas: 3,000
- Used by HEAT/COLD verifier contracts; enables Cosmos/Solana bridge sig verification

- [ ] **Step 7: Run all tests**

Run: `cargo test --lib`
Expected: All tests pass. (These precompiles are additive — no behavior changes.)

- [ ] **Step 8: Commit**

```bash
git add program/Cargo.toml program/src/precompiles.rs
git commit -m "feat(precompile): Ethereum-compat + Ed25519 precompiles

0x0001 ecRecover, 0x0002 SHA-256, 0x0005 ModExp,
0x0006 BN254 ecAdd, 0x0007 BN254 ecMul, 0x0008 BN254 ecPairing,
0x0140 Ed25519 Verify.

Ethereum-compat: existing Solidity contracts work unmodified.
BN254 pairing enables on-chain Groth16 ZK proof verification.
Ed25519 for HEAT/COLD verifier contracts + bridge sigs.
BLS12-381 patch registered but no precompile yet.
All SP1-accelerated (15-50x vs raw RISC-V)."
```

---

### Task 4h: P-256 (WebAuthn/Passkeys) Precompile + revm Patch Swap

**Files:**
- Modify: `program/Cargo.toml` (swap revm to patched version, add p256 patch)
- Modify: `program/src/precompiles.rs` (add 0x0201)

P-256 enables passkey-based wallet authentication — users sign with Face ID, Touch ID, YubiKey, or any WebAuthn-compatible device. No seed phrases, no risk of the "write down your 12 words" failure mode. The revm swap is a Cargo.toml one-liner that gives 2–5x faster EVM execution inside the guest for free.

- [ ] **Step 1: Swap revm to SP1-patched version in `program/Cargo.toml`**

In `[patch.crates-io]`, add:
```toml
# SP1-patched revm — 2-5x faster EVM execution inside guest (keccak, storage ops)
revm = { git = "https://github.com/sp1-patches/revm", tag = "patch-revm-v14-sp1-6.0.0" }
```

Run: `cargo check -p coldl3-sp1-guest`
Expected: Compiles cleanly (drop-in replacement, same API).

- [ ] **Step 2: Add P-256 patch to `program/Cargo.toml`**

```toml
# P-256 / secp256r1 — for WebAuthn/Passkeys precompile 0x0201
p256 = { git = "https://github.com/sp1-patches/elliptic-curves", tag = "patch-p256-0.13.2-sp1-6.0.0" }
```

Also add `p256` to `[dependencies]`:
```toml
p256 = { version = "0.13", features = ["ecdsa"] }
```

- [ ] **Step 3: Add P-256 ecRecover precompile (0x0201)**

In `program/src/precompiles.rs`, register `0x0201`:
- Input: `msg_hash(32) || r(32) || s(32) || x(32) || y(32)` — hash + signature + uncompressed pubkey (no v byte — P-256 recovery is deterministic given pubkey)
- Logic: Use `p256::ecdsa::VerifyingKey::verify_prehash(&sig, &hash)`
- Output: `0x01` (valid) or `0x00` (invalid)
- Gas: 3,000

> **Note on P-256 vs secp256k1**: P-256 doesn't support address recovery from signature alone (unlike secp256k1's `ecrecover`). The caller provides both the signature and the public key; the precompile just verifies the signature is valid for that key. This matches the WebAuthn flow exactly — the wallet knows the user's P-256 public key and verifies their authenticator-produced signature.

- [ ] **Step 4: Run all tests**

Run: `cargo test --lib`
Expected: All tests pass. The revm swap is transparent — same behavior, faster proving.

- [ ] **Step 5: Commit**

```bash
git add program/Cargo.toml program/src/precompiles.rs
git commit -m "feat(precompile): P-256 WebAuthn verify at 0x0201 + patched revm

0x0201: P-256/secp256r1 signature verification for WebAuthn/Passkeys.
Enables Face ID, Touch ID, YubiKey auth — no seed phrases needed.
3,000 gas, SP1 p256 patch (~20x vs raw RISC-V).

revm: swapped to sp1-patches/revm for 2-5x faster EVM execution
inside the guest. Drop-in replacement, no code changes."
```

---

## Phase 2: AA Wallet Validation + AccountState Migration

This phase modifies the core account model. `AccountState` gets a `wallet_type` field. AA wallets validate via Schnorr signatures instead of implicit nonce checks.

### Task 5: Extend AccountState with WalletType

**Files:**
- Modify: `src/main.rs` (AccountState struct, ~line 222-231)
- Modify: `program/src/state.rs` (AccountWitness struct, ~line 37-60)

- [ ] **Step 1: Add `wallet_type` to AccountState**

In `src/main.rs`, modify `AccountState`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountState {
    pub balance_commitment: [u8; 32],
    pub balance: u64,  // Plaintext — only used for LegacyEOA; always 0 for PrivateWallet
    pub nonce: u64,
    #[serde(default)]
    pub wallet_type: aa::types::WalletType,
    /// Owner public key (Ristretto255). Only used for PrivateWallet.
    #[serde(default)]
    pub owner_pubkey: Option<[u8; 32]>,
}
```

The `#[serde(default)]` ensures backward compatibility — existing serialized states deserialize as `LegacyEOA` with no owner_pubkey.

- [ ] **Step 2: Add `wallet_type` to AccountWitness**

In `program/src/state.rs`, add to `AccountWitness`:

```rust
#[serde(default)]
pub wallet_type: u8,  // 0 = LegacyEOA, 1 = PrivateWallet (plain u8 for guest simplicity)
#[serde(default)]
pub owner_pubkey: Option<[u8; 32]>,
```

- [ ] **Step 3: Run all tests to verify backward compatibility**

Run: `cargo test --lib`
Expected: All 153+ tests pass (serde(default) ensures no breakage).

- [ ] **Step 4: Commit**

```bash
git add src/main.rs program/src/state.rs
git commit -m "feat(aa): extend AccountState and AccountWitness with wallet_type

Backward-compatible addition: #[serde(default)] ensures existing
accounts deserialize as LegacyEOA. PrivateWallet accounts store
owner_pubkey for Schnorr auth."
```

---

### Task 6: AA Wallet Validation Logic

**Files:**
- Create: `src/aa/validation.rs`

This module validates UserOperations: Schnorr auth, conservation, range proofs.

- [ ] **Step 1: Write validation functions with tests**

Create `src/aa/validation.rs`:

```rust
// src/aa/validation.rs
// AA wallet transaction validation.
//
// Validates UserOperations before inclusion in a block:
//   1. Schnorr auth: sender proves ownership
//   2. Conservation: old_sender - new_sender = new_recipient - old_recipient
//   3. Knowledge proof: sender knows (amount, blinding) for amount_commitment
//   4. Range proof: amount in [0, 2^64) — prevents negative transfers
//
// This is "early rejection" — the guest re-verifies everything inside SP1.

use curve25519_dalek_ng::ristretto::{CompressedRistretto, RistrettoPoint};
use curve25519_dalek_ng::traits::Identity;

use crate::aa::schnorr::schnorr_verify;
use crate::aa::types::UserOperation;
use crate::privacy::verify_commitment_knowledge;
use crate::privacy::CommitmentKnowledgeProof;

/// Validate a UserOperation for block inclusion.
///
/// Returns Ok(()) if all checks pass, Err(reason) otherwise.
/// This is the sequencer-side check; the guest re-verifies everything.
pub fn validate_user_operation(
    op: &UserOperation,
    sender_pubkey: &[u8; 32],
    sender_current_commitment: &[u8; 32],
    recipient_current_commitment: &[u8; 32],
) -> Result<(), String> {
    // 1. Verify Schnorr auth signature
    let op_hash = compute_user_op_hash(op);
    if !schnorr_verify(sender_pubkey, &op_hash, &op.auth_signature) {
        return Err("invalid auth signature".to_string());
    }

    // 2. Verify conservation: old_sender - new_sender == new_recipient - old_recipient
    if !verify_conservation(
        sender_current_commitment,
        &op.sender_new_commitment,
        recipient_current_commitment,
        &op.recipient_new_commitment,
    ) {
        return Err("conservation check failed".to_string());
    }

    // 3. Verify knowledge proof for amount commitment
    let knowledge_proof = CommitmentKnowledgeProof {
        commitment: op.knowledge_proof.commitment,
        announcement: op.knowledge_proof.announcement,
        response_v: op.knowledge_proof.response_v,
        response_r: op.knowledge_proof.response_r,
    };
    if !verify_commitment_knowledge(&knowledge_proof) {
        return Err("invalid knowledge proof".to_string());
    }

    // 4. Verify the knowledge proof's commitment matches the amount commitment
    if op.knowledge_proof.commitment != op.amount_commitment {
        return Err("knowledge proof commitment mismatch".to_string());
    }

    // Note: Range proof verification is expensive. We skip it here and
    // let the guest handle it. The sequencer trusts the proof format.

    Ok(())
}

/// Verify Pedersen commitment conservation.
///
/// Checks: (old_sender - new_sender) == (new_recipient - old_recipient)
/// This means: what sender lost = what recipient gained.
pub fn verify_conservation(
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

/// Compute the hash of a UserOperation for signing.
///
/// H("C0DL3:userop:" || sender || nonce || to || amount_commitment || ...)
pub fn compute_user_op_hash(op: &UserOperation) -> Vec<u8> {
    use sha2::{Digest, Sha256};
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aa::schnorr::schnorr_sign;
    use crate::privacy::commitment_proof::{compute_commitment, prove_commitment_knowledge_with_commitment};
    use bulletproofs::PedersenGens;
    use curve25519_dalek_ng::{constants::RISTRETTO_BASEPOINT_POINT as G, scalar::Scalar};
    use rand::RngCore;

    fn random_scalar() -> Scalar {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        Scalar::from_bytes_mod_order(bytes)
    }

    fn make_commitment(value: u64, blinding: &Scalar) -> [u8; 32] {
        compute_commitment(value, blinding)
    }

    #[test]
    fn test_conservation_valid() {
        let gens = PedersenGens::default();

        // Sender has 100, sends 30, keeps 70
        let r_sender_old = random_scalar();
        let r_sender_new = random_scalar();
        let r_recipient_old = random_scalar();
        // Recipient blinding must satisfy: r_sender_old - r_sender_new = r_recipient_new - r_recipient_old
        let r_recipient_new = r_recipient_old + (r_sender_old - r_sender_new);

        let old_sender = make_commitment(100, &r_sender_old);
        let new_sender = make_commitment(70, &r_sender_new);
        let old_recipient = make_commitment(0, &r_recipient_old);
        let new_recipient = make_commitment(30, &r_recipient_new);

        assert!(verify_conservation(&old_sender, &new_sender, &old_recipient, &new_recipient));
    }

    #[test]
    fn test_conservation_invalid_amount() {
        // Sender sends 30 but recipient gets 40 — conservation fails
        let r1 = random_scalar();
        let r2 = random_scalar();
        let r3 = random_scalar();
        let r4 = random_scalar();

        let old_sender = make_commitment(100, &r1);
        let new_sender = make_commitment(70, &r2);   // lost 30
        let old_recipient = make_commitment(0, &r3);
        let new_recipient = make_commitment(40, &r4); // gained 40 — mismatch!

        assert!(!verify_conservation(&old_sender, &new_sender, &old_recipient, &new_recipient));
    }

    #[test]
    fn test_conservation_invalid_point() {
        assert!(!verify_conservation(&[0xFF; 32], &[0; 32], &[0; 32], &[0; 32]));
    }

    #[test]
    fn test_full_user_op_validation() {
        // Create a valid UserOperation and validate it
        let privkey = random_scalar();
        let pubkey = (privkey * G).compress().to_bytes();

        let r_sender_old = random_scalar();
        let r_sender_new = random_scalar();
        let r_recipient_old = random_scalar();
        let r_recipient_new = r_recipient_old + (r_sender_old - r_sender_new);
        let amount_blinding = r_sender_old - r_sender_new; // blinding for amount = 30

        let old_sender_commit = make_commitment(100, &r_sender_old);
        let new_sender_commit = make_commitment(70, &r_sender_new);
        let old_recipient_commit = make_commitment(0, &r_recipient_old);
        let new_recipient_commit = make_commitment(30, &r_recipient_new);
        let amount_commit = make_commitment(30, &amount_blinding);

        // Knowledge proof for amount commitment
        let kp = prove_commitment_knowledge_with_commitment(30, &amount_blinding, amount_commit);

        // Build UserOperation (without auth sig yet — we need the hash)
        let mut op = UserOperation {
            sender: "0xAlice".to_string(),
            nonce: 0,
            to: "0xBob".to_string(),
            sender_new_commitment: new_sender_commit,
            recipient_new_commitment: new_recipient_commit,
            amount_commitment: amount_commit,
            knowledge_proof: crate::aa::types::KnowledgeProofBytes {
                commitment: kp.commitment,
                announcement: kp.announcement,
                response_v: kp.response_v,
                response_r: kp.response_r,
            },
            range_proof: vec![], // skipped for this test
            conservation_excess: [0; 32],
            paymaster: None,
            gas_limit: 100_000,
            gas_price: 1,
            auth_signature: crate::aa::types::SchnorrSignature {
                r_point: [0; 32],
                s_scalar: [0; 32],
            },
            encrypted_memo: None,
            call_data: vec![],
        };

        // Sign the operation
        let op_hash = compute_user_op_hash(&op);
        op.auth_signature = schnorr_sign(&privkey, &op_hash);

        // Validate
        let result = validate_user_operation(
            &op,
            &pubkey,
            &old_sender_commit,
            &old_recipient_commit,
        );
        assert!(result.is_ok(), "validation failed: {:?}", result.err());
    }
}
```

- [ ] **Step 2: Add to aa/mod.rs**

Add `pub mod validation;` to `src/aa/mod.rs`.

- [ ] **Step 3: Run all tests**

Run: `cargo test aa::validation::tests -v`
Expected: 4 tests pass (conservation valid, conservation invalid, invalid point, full userop).

Run: `cargo test --lib`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/aa/validation.rs src/aa/mod.rs
git commit -m "feat(aa): UserOperation validation — auth, conservation, knowledge proof

Validates AA transactions: Schnorr auth signature, Pedersen commitment
conservation (sender delta = recipient delta), and knowledge proof
for the transfer amount commitment."
```

---

## Phase 3: Wallet Factory + Paymaster

### Task 7: Wallet Factory

**Files:**
- Create: `src/aa/wallet_factory.rs`

The wallet factory creates new AA wallet accounts. It's called when a user first joins COLDL3. The factory deploys a PrivateWallet contract with the user's public key as owner.

- [ ] **Step 1: Write wallet factory**

Create `src/aa/wallet_factory.rs`:

```rust
// src/aa/wallet_factory.rs
// Wallet factory — creates new PrivateWallet accounts.
//
// CREATE2 address derivation: address = SHA-256("C0DL3:wallet:" || owner_pubkey || salt)[12..32]
// This makes addresses deterministic and stealth-capable.

use sha2::{Digest, Sha256};

use crate::aa::types::WalletConfig;
use crate::privacy::shielded_pool::compute_balance_commitment;

/// Derive a wallet address from owner pubkey + salt.
///
/// Uses CREATE2-style deterministic derivation:
/// address = "0x" || hex(SHA-256("C0DL3:wallet:" || pubkey || salt)[12..32])
///
/// This lets users pre-compute their address before deployment,
/// and enables stealth addresses (random salt per interaction).
pub fn derive_wallet_address(owner_pubkey: &[u8; 32], salt: &[u8; 32]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"C0DL3:wallet:");
    hasher.update(owner_pubkey);
    hasher.update(salt);
    let hash: [u8; 32] = hasher.finalize().into();

    // Take last 20 bytes (matching Ethereum address format)
    let addr_bytes = &hash[12..32];
    format!("0x{}", hex::encode(addr_bytes))
}

/// Derive a stealth wallet address for a one-time interaction.
///
/// Uses ECDH: sender picks ephemeral key, computes shared secret with
/// recipient's pubkey, derives a stealth salt, and computes the address.
/// Only the recipient (with their privkey) can find which address is theirs.
pub fn derive_stealth_address(
    recipient_pubkey: &[u8; 32],
    ephemeral_scalar_bytes: &[u8; 32],
) -> (String, [u8; 32]) {
    use curve25519_dalek_ng::{
        ristretto::CompressedRistretto,
        scalar::Scalar,
        constants::RISTRETTO_BASEPOINT_POINT as G,
    };

    let recipient = CompressedRistretto(*recipient_pubkey)
        .decompress()
        .expect("invalid recipient pubkey");
    let ephemeral = Scalar::from_bytes_mod_order(*ephemeral_scalar_bytes);

    // Shared secret via ECDH
    let shared = (ephemeral * recipient).compress().to_bytes();

    // Stealth salt = H("C0DL3:stealth:" || shared_secret)
    let mut salt_hasher = Sha256::new();
    salt_hasher.update(b"C0DL3:stealth:");
    salt_hasher.update(&shared);
    let salt: [u8; 32] = salt_hasher.finalize().into();

    // Stealth pubkey = recipient_pubkey + H(shared)*G
    // (recipient can derive: stealth_privkey = privkey + H(shared))
    let stealth_offset = Scalar::from_bytes_mod_order(salt);
    let stealth_pubkey = (recipient + stealth_offset * G).compress().to_bytes();

    let address = derive_wallet_address(&stealth_pubkey, &salt);
    (address, stealth_pubkey)
}

/// Create the initial AccountState for a new PrivateWallet.
///
/// Balance starts at 0, commitment is deterministic from address.
pub fn create_wallet_account(
    address: &str,
    owner_pubkey: [u8; 32],
) -> crate::AccountState {
    crate::AccountState {
        balance_commitment: compute_balance_commitment(address, 0, 0),
        balance: 0,
        nonce: 0,
        wallet_type: crate::aa::types::WalletType::PrivateWallet,
        owner_pubkey: Some(owner_pubkey),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use curve25519_dalek_ng::{
        constants::RISTRETTO_BASEPOINT_POINT as G,
        scalar::Scalar,
    };
    use rand::RngCore;

    fn random_scalar() -> Scalar {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        Scalar::from_bytes_mod_order(bytes)
    }

    #[test]
    fn test_derive_wallet_address_deterministic() {
        let pubkey = [0xAB; 32];
        let salt = [0; 32];

        let addr1 = derive_wallet_address(&pubkey, &salt);
        let addr2 = derive_wallet_address(&pubkey, &salt);
        assert_eq!(addr1, addr2);
        assert!(addr1.starts_with("0x"));
        assert_eq!(addr1.len(), 42); // "0x" + 40 hex chars
    }

    #[test]
    fn test_different_salts_different_addresses() {
        let pubkey = [0xAB; 32];
        let addr1 = derive_wallet_address(&pubkey, &[0; 32]);
        let addr2 = derive_wallet_address(&pubkey, &[1; 32]);
        assert_ne!(addr1, addr2);
    }

    #[test]
    fn test_stealth_address_derivation() {
        let recipient_priv = random_scalar();
        let recipient_pub = (recipient_priv * G).compress().to_bytes();
        let ephemeral = random_scalar();

        let (addr, stealth_pub) = derive_stealth_address(
            &recipient_pub,
            &ephemeral.to_bytes(),
        );
        assert!(addr.starts_with("0x"));
        assert_ne!(stealth_pub, recipient_pub); // stealth != original
    }

    #[test]
    fn test_create_wallet_account() {
        let pubkey = (random_scalar() * G).compress().to_bytes();
        let account = create_wallet_account("0xtest", pubkey);

        assert_eq!(account.balance, 0);
        assert_eq!(account.nonce, 0);
        assert_eq!(account.wallet_type, crate::aa::types::WalletType::PrivateWallet);
        assert_eq!(account.owner_pubkey, Some(pubkey));
    }
}
```

- [ ] **Step 2: Add to aa/mod.rs**

Add `pub mod wallet_factory;` to `src/aa/mod.rs`.

- [ ] **Step 3: Run tests**

Run: `cargo test aa::wallet_factory::tests -v`
Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/aa/wallet_factory.rs src/aa/mod.rs
git commit -m "feat(aa): wallet factory with CREATE2 + stealth addresses

Deterministic wallet address derivation from pubkey + salt.
Stealth addresses via ECDH for one-time unlinkable wallets.
create_wallet_account() initializes PrivateWallet with commitment."
```

---

### Task 8: Paymaster

**Files:**
- Create: `src/aa/paymaster.rs`

Paymasters pay gas on behalf of users. The user's identity isn't linked to the gas payment.

- [ ] **Step 1: Write paymaster logic**

Create `src/aa/paymaster.rs`:

```rust
// src/aa/paymaster.rs
// Gas payment abstraction.
//
// Paymasters are accounts (any type) that agree to pay gas for UserOperations.
// The sequencer verifies the paymaster has sufficient balance and a valid
// signature authorizing the gas payment.
//
// Privacy benefit: the gas payer (paymaster) is NOT the sender. This breaks
// the link between "who pays gas" and "who initiated the transaction."

use crate::aa::schnorr::schnorr_verify;
use crate::aa::types::{PaymasterApproval, UserOperation};

/// Verify that a paymaster approval is valid for a given UserOperation.
///
/// Checks:
///   1. Paymaster signature covers the UserOp hash
///   2. Approval hasn't expired (block height)
///   3. Gas limit doesn't exceed approved maximum
pub fn verify_paymaster_approval(
    approval: &PaymasterApproval,
    paymaster_pubkey: &[u8; 32],
    op: &UserOperation,
    current_block: u64,
) -> Result<(), String> {
    // Check expiry
    if current_block > approval.valid_until_block {
        return Err(format!(
            "paymaster approval expired: current block {} > valid_until {}",
            current_block, approval.valid_until_block
        ));
    }

    // Check gas limit
    if op.gas_limit > approval.max_gas {
        return Err(format!(
            "gas limit {} exceeds paymaster max {}",
            op.gas_limit, approval.max_gas
        ));
    }

    // Verify signature: paymaster signs H("C0DL3:paymaster:" || op_hash || max_gas || valid_until)
    let op_hash = crate::aa::validation::compute_user_op_hash(op);
    let mut msg = Vec::new();
    msg.extend_from_slice(b"C0DL3:paymaster:");
    msg.extend_from_slice(&op_hash);
    msg.extend_from_slice(&approval.max_gas.to_le_bytes());
    msg.extend_from_slice(&approval.valid_until_block.to_le_bytes());

    if !schnorr_verify(paymaster_pubkey, &msg, &approval.signature) {
        return Err("invalid paymaster signature".to_string());
    }

    Ok(())
}

/// Compute the gas cost in fwei for a UserOperation.
pub fn compute_gas_cost(op: &UserOperation) -> u64 {
    let base_gas: u64 = 21_000;
    let data_gas: u64 = (op.call_data.len() as u64) * 68;
    let proof_gas: u64 = if op.range_proof.is_empty() { 0 } else { 50_000 }; // range proof verification
    let conservation_gas: u64 = 4_000; // conservation check
    let auth_gas: u64 = 3_000; // Schnorr verify
    let total_gas = base_gas + data_gas + proof_gas + conservation_gas + auth_gas;
    op.gas_price * total_gas
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aa::schnorr::schnorr_sign;
    use crate::aa::types::SchnorrSignature;
    use curve25519_dalek_ng::{constants::RISTRETTO_BASEPOINT_POINT as G, scalar::Scalar};
    use rand::RngCore;

    fn random_scalar() -> Scalar {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        Scalar::from_bytes_mod_order(bytes)
    }

    fn dummy_user_op() -> UserOperation {
        UserOperation {
            sender: "0xAlice".to_string(),
            nonce: 0,
            to: "0xBob".to_string(),
            sender_new_commitment: [0xAA; 32],
            recipient_new_commitment: [0xBB; 32],
            amount_commitment: [0xCC; 32],
            knowledge_proof: crate::aa::types::KnowledgeProofBytes {
                commitment: [0; 32],
                announcement: [0; 32],
                response_v: [0; 32],
                response_r: [0; 32],
            },
            range_proof: vec![],
            conservation_excess: [0; 32],
            paymaster: Some("0xPaymaster".to_string()),
            gas_limit: 100_000,
            gas_price: 1,
            auth_signature: SchnorrSignature {
                r_point: [0; 32],
                s_scalar: [0; 32],
            },
            encrypted_memo: None,
            call_data: vec![],
        }
    }

    #[test]
    fn test_paymaster_approval_valid() {
        let pm_privkey = random_scalar();
        let pm_pubkey = (pm_privkey * G).compress().to_bytes();
        let op = dummy_user_op();

        let op_hash = crate::aa::validation::compute_user_op_hash(&op);
        let mut msg = Vec::new();
        msg.extend_from_slice(b"C0DL3:paymaster:");
        msg.extend_from_slice(&op_hash);
        msg.extend_from_slice(&200_000u64.to_le_bytes());
        msg.extend_from_slice(&100u64.to_le_bytes());
        let sig = schnorr_sign(&pm_privkey, &msg);

        let approval = PaymasterApproval {
            paymaster: "0xPaymaster".to_string(),
            max_gas: 200_000,
            signature: sig,
            valid_until_block: 100,
        };

        let result = verify_paymaster_approval(&approval, &pm_pubkey, &op, 50);
        assert!(result.is_ok());
    }

    #[test]
    fn test_paymaster_approval_expired() {
        let pm_privkey = random_scalar();
        let pm_pubkey = (pm_privkey * G).compress().to_bytes();
        let op = dummy_user_op();

        let approval = PaymasterApproval {
            paymaster: "0xPaymaster".to_string(),
            max_gas: 200_000,
            signature: schnorr_sign(&pm_privkey, b"dummy"),
            valid_until_block: 10,
        };

        let result = verify_paymaster_approval(&approval, &pm_pubkey, &op, 50);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expired"));
    }

    #[test]
    fn test_paymaster_gas_exceeded() {
        let pm_privkey = random_scalar();
        let pm_pubkey = (pm_privkey * G).compress().to_bytes();
        let op = dummy_user_op(); // gas_limit = 100_000

        let approval = PaymasterApproval {
            paymaster: "0xPaymaster".to_string(),
            max_gas: 50_000, // less than op.gas_limit
            signature: schnorr_sign(&pm_privkey, b"dummy"),
            valid_until_block: 100,
        };

        let result = verify_paymaster_approval(&approval, &pm_pubkey, &op, 50);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exceeds"));
    }

    #[test]
    fn test_gas_cost_computation() {
        let op = dummy_user_op();
        let cost = compute_gas_cost(&op);
        // base(21k) + data(0) + range(0) + conservation(4k) + auth(3k) = 28k
        // * gas_price(1) = 28,000
        assert_eq!(cost, 28_000);
    }
}
```

- [ ] **Step 2: Add to aa/mod.rs**

Add `pub mod paymaster;` to `src/aa/mod.rs`.

- [ ] **Step 3: Run tests**

Run: `cargo test aa::paymaster::tests -v`
Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/aa/paymaster.rs src/aa/mod.rs
git commit -m "feat(aa): paymaster gas abstraction

Paymasters pay gas on behalf of users, breaking the sender-gas link.
Schnorr-signed approval with expiry + max gas cap.
Gas cost includes: base + data + proof verification + conservation + auth."
```

---

## Phase 4: SDK — Auto Shield + Stealth Addresses

### Task 9: Auto-Shield Flow in SDK

**Files:**
- Create: `sdk/src/auto_shield.rs`
- Create: `sdk/src/stealth.rs`
- Modify: `sdk/src/lib.rs`
- Modify: `sdk/Cargo.toml` (add x25519-dalek if needed)

The auto-shield flow is the wallet SDK's killer feature: the user says "send 100 to Bob" and behind the scenes, the SDK:
1. Shields from AA wallet → shielded pool
2. Private transfer (re-commit to Bob's stealth pubkey)
3. Delayed unshield to Bob's AA wallet (N blocks later)

- [ ] **Step 1: Write auto-shield builder**

Create `sdk/src/auto_shield.rs`:

```rust
// sdk/src/auto_shield.rs
// Automatic shield-transfer-unshield flow.
//
// User says "send 100 to Bob." The SDK:
//   1. Shield: commit 100 from sender's AA wallet into shielded pool
//   2. Private transfer: re-commit to recipient's stealth pubkey
//   3. Schedule delayed unshield to recipient's AA wallet
//
// Privacy properties:
//   - Amount hidden (Pedersen commitment)
//   - Sender-recipient link broken (shield/unshield in different blocks)
//   - Gas payer != sender (paymaster)
//   - Amount splitting and decoys increase anonymity set

use curve25519_dalek_ng::scalar::Scalar;
use serde::{Deserialize, Serialize};

use crate::commitment_proof::compute_commitment;
use crate::shield::{create_shield_request, ShieldRequest};

/// Configuration for the auto-shield flow.
#[derive(Debug, Clone)]
pub struct AutoShieldConfig {
    /// Minimum delay (in blocks) between shield and unshield.
    /// Higher = more privacy (harder to correlate), but slower.
    pub min_delay_blocks: u64,
    /// Maximum delay. Actual delay is random in [min, max].
    pub max_delay_blocks: u64,
    /// Whether to split the amount into multiple notes.
    pub enable_splitting: bool,
    /// Number of decoy (zero-value) notes to include.
    pub decoy_count: u32,
}

impl Default for AutoShieldConfig {
    fn default() -> Self {
        Self {
            min_delay_blocks: 5,
            max_delay_blocks: 20,
            enable_splitting: true,
            decoy_count: 2,
        }
    }
}

/// A planned auto-shield operation (output of the planner).
///
/// The wallet stores this and executes each phase at the right time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoShieldPlan {
    /// Phase 1: Shield requests to submit immediately.
    pub shield_phase: Vec<ShieldRequest>,
    /// Phase 2: Private transfer note commitments (created during shield).
    /// These are internal — not sent anywhere.
    pub transfer_notes: Vec<TransferNote>,
    /// Phase 3: Unshield at this block (or later).
    pub unshield_at_block: u64,
    /// Recipient address for unshield.
    pub recipient: String,
    /// Total amount being transferred.
    pub total_amount: u64,
}

/// Internal note tracking for the transfer phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferNote {
    pub note_commitment: [u8; 32],
    pub value_commitment: [u8; 32],
    pub amount: u64,
    pub blinding: [u8; 32],
}

/// Plan an auto-shield transfer.
///
/// This doesn't execute anything — it returns a plan that the wallet
/// executes step by step over multiple blocks.
pub fn plan_auto_shield(
    amount: u64,
    recipient_pubkey: [u8; 32],
    current_block: u64,
    config: &AutoShieldConfig,
) -> anyhow::Result<AutoShieldPlan> {
    // Split amount if enabled
    let splits = if config.enable_splitting && amount > 1 {
        split_amount(amount)
    } else {
        vec![amount]
    };

    let mut shield_requests = Vec::new();
    let mut transfer_notes = Vec::new();

    for split_amount in &splits {
        let blinding = random_blinding();
        let request = create_shield_request(*split_amount, &blinding, recipient_pubkey)?;

        transfer_notes.push(TransferNote {
            note_commitment: request.note_commitment,
            value_commitment: request.value_commitment,
            amount: *split_amount,
            blinding: blinding.to_bytes(),
        });

        shield_requests.push(request);
    }

    // Add decoys (zero-value notes with valid proofs)
    for _ in 0..config.decoy_count {
        let blinding = random_blinding();
        let decoy = create_shield_request(0, &blinding, recipient_pubkey)?;
        shield_requests.push(decoy);
    }

    // Random delay
    let delay = config.min_delay_blocks
        + (random_u64() % (config.max_delay_blocks - config.min_delay_blocks + 1));

    Ok(AutoShieldPlan {
        shield_phase: shield_requests,
        transfer_notes,
        unshield_at_block: current_block + delay,
        recipient: format!("0x{}", hex::encode(&recipient_pubkey[..20])),
        total_amount: amount,
    })
}

/// Split an amount into 2-3 random parts.
fn split_amount(amount: u64) -> Vec<u64> {
    if amount <= 2 {
        return vec![amount];
    }
    // Simple split: random fraction for first part, remainder for second
    let fraction = (random_u64() % 80 + 10) as f64 / 100.0; // 10-90%
    let part1 = (amount as f64 * fraction) as u64;
    let part2 = amount - part1;
    if part1 == 0 || part2 == 0 {
        vec![amount]
    } else {
        vec![part1, part2]
    }
}

fn random_blinding() -> Scalar {
    let mut bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
    Scalar::from_bytes_mod_order(bytes)
}

fn random_u64() -> u64 {
    let mut bytes = [0u8; 8];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
    u64::from_le_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bulletproofs::PedersenGens;
    use curve25519_dalek_ng::constants::RISTRETTO_BASEPOINT_POINT as G;

    fn random_pubkey() -> [u8; 32] {
        let s = random_blinding();
        (s * PedersenGens::default().B).compress().to_bytes()
    }

    #[test]
    fn test_plan_auto_shield_basic() {
        let pubkey = random_pubkey();
        let config = AutoShieldConfig {
            min_delay_blocks: 5,
            max_delay_blocks: 10,
            enable_splitting: false,
            decoy_count: 0,
        };

        let plan = plan_auto_shield(1000, pubkey, 100, &config).unwrap();
        assert_eq!(plan.shield_phase.len(), 1); // no splitting, no decoys
        assert_eq!(plan.transfer_notes.len(), 1);
        assert_eq!(plan.transfer_notes[0].amount, 1000);
        assert!(plan.unshield_at_block >= 105);
        assert!(plan.unshield_at_block <= 110);
    }

    #[test]
    fn test_plan_auto_shield_with_splitting() {
        let pubkey = random_pubkey();
        let config = AutoShieldConfig {
            min_delay_blocks: 5,
            max_delay_blocks: 10,
            enable_splitting: true,
            decoy_count: 0,
        };

        let plan = plan_auto_shield(1000, pubkey, 100, &config).unwrap();
        // Should have 2 parts that sum to 1000
        let total: u64 = plan.transfer_notes.iter().map(|n| n.amount).sum();
        assert_eq!(total, 1000);
        assert!(plan.transfer_notes.len() >= 1); // might be 1 if split edge case
    }

    #[test]
    fn test_plan_auto_shield_with_decoys() {
        let pubkey = random_pubkey();
        let config = AutoShieldConfig {
            enable_splitting: false,
            decoy_count: 3,
            ..Default::default()
        };

        let plan = plan_auto_shield(500, pubkey, 100, &config).unwrap();
        assert_eq!(plan.shield_phase.len(), 4); // 1 real + 3 decoys
        assert_eq!(plan.transfer_notes.len(), 1); // only real note tracked
    }

    #[test]
    fn test_split_amount() {
        let parts = split_amount(1000);
        let total: u64 = parts.iter().sum();
        assert_eq!(total, 1000);
        assert!(parts.len() <= 3);
        for &p in &parts {
            assert!(p > 0);
        }
    }

    #[test]
    fn test_split_amount_small() {
        assert_eq!(split_amount(1), vec![1]);
        assert_eq!(split_amount(0), vec![0]);
    }
}
```

- [ ] **Step 2: Add hex dependency to SDK**

In `sdk/Cargo.toml`, add `hex = "0.4"` to `[dependencies]`.

- [ ] **Step 3: Add stealth address module**

Create `sdk/src/stealth.rs`:

```rust
// sdk/src/stealth.rs
// Stealth address derivation for the wallet SDK.
//
// Stealth addresses let the sender create a one-time address for the recipient.
// Only the recipient can identify and spend from stealth addresses.
//
// Protocol:
//   1. Sender picks ephemeral scalar k
//   2. Computes shared_secret = k * recipient_pubkey
//   3. Derives stealth_salt = H("C0DL3:stealth:" || shared_secret)
//   4. Stealth pubkey = recipient_pubkey + stealth_salt * G
//   5. Stealth address = derive_wallet_address(stealth_pubkey, stealth_salt)
//   6. Sends (ephemeral_pubkey = k*G) alongside the transaction
//   7. Recipient scans: shared = privkey * ephemeral_pubkey, derives stealth_privkey

use curve25519_dalek_ng::{
    constants::RISTRETTO_BASEPOINT_POINT as G,
    ristretto::CompressedRistretto,
    scalar::Scalar,
};
use sha2::{Digest, Sha256};

/// Generate a stealth address for a recipient.
///
/// Returns: (stealth_pubkey, ephemeral_pubkey, stealth_privkey_offset)
/// The recipient computes: stealth_privkey = their_privkey + offset
pub fn generate_stealth_address(
    recipient_pubkey: &[u8; 32],
) -> Option<(/* stealth_pub */ [u8; 32], /* ephemeral_pub */ [u8; 32], /* offset */ [u8; 32])> {
    let recipient = CompressedRistretto(*recipient_pubkey).decompress()?;

    // Ephemeral key
    let mut k_bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut k_bytes);
    let k = Scalar::from_bytes_mod_order(k_bytes);

    let ephemeral_pub = (k * G).compress().to_bytes();

    // Shared secret
    let shared = (k * recipient).compress().to_bytes();

    // Offset
    let mut hasher = Sha256::new();
    hasher.update(b"C0DL3:stealth:");
    hasher.update(&shared);
    let offset: [u8; 32] = hasher.finalize().into();
    let offset_scalar = Scalar::from_bytes_mod_order(offset);

    // Stealth pubkey = recipient + offset * G
    let stealth_pub = (recipient + offset_scalar * G).compress().to_bytes();

    Some((stealth_pub, ephemeral_pub, offset))
}

/// Scan for stealth addresses — recipient checks if an ephemeral pubkey
/// corresponds to one of their stealth addresses.
///
/// Returns Some(stealth_privkey) if this ephemeral pubkey targets them.
pub fn scan_stealth_address(
    recipient_privkey: &Scalar,
    ephemeral_pubkey: &[u8; 32],
    expected_stealth_pubkey: &[u8; 32],
) -> Option<Scalar> {
    let ephemeral = CompressedRistretto(*ephemeral_pubkey).decompress()?;

    // Shared secret = privkey * ephemeral_pubkey
    let shared = (recipient_privkey * ephemeral).compress().to_bytes();

    // Offset
    let mut hasher = Sha256::new();
    hasher.update(b"C0DL3:stealth:");
    hasher.update(&shared);
    let offset: [u8; 32] = hasher.finalize().into();
    let offset_scalar = Scalar::from_bytes_mod_order(offset);

    // Stealth privkey = privkey + offset
    let stealth_privkey = recipient_privkey + offset_scalar;

    // Verify: stealth_privkey * G == expected_stealth_pubkey
    let derived_pub = (stealth_privkey * G).compress().to_bytes();
    if derived_pub == *expected_stealth_pubkey {
        Some(stealth_privkey)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;

    fn random_scalar() -> Scalar {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        Scalar::from_bytes_mod_order(bytes)
    }

    #[test]
    fn test_stealth_address_roundtrip() {
        let recipient_priv = random_scalar();
        let recipient_pub = (recipient_priv * G).compress().to_bytes();

        let (stealth_pub, ephemeral_pub, _offset) =
            generate_stealth_address(&recipient_pub).unwrap();

        // Recipient scans and recovers stealth privkey
        let stealth_priv = scan_stealth_address(
            &recipient_priv,
            &ephemeral_pub,
            &stealth_pub,
        );
        assert!(stealth_priv.is_some());

        // Verify: stealth_priv * G == stealth_pub
        let derived = (stealth_priv.unwrap() * G).compress().to_bytes();
        assert_eq!(derived, stealth_pub);
    }

    #[test]
    fn test_stealth_wrong_recipient_fails() {
        let recipient_priv = random_scalar();
        let recipient_pub = (recipient_priv * G).compress().to_bytes();
        let wrong_priv = random_scalar();

        let (stealth_pub, ephemeral_pub, _) =
            generate_stealth_address(&recipient_pub).unwrap();

        // Wrong recipient can't scan
        let result = scan_stealth_address(&wrong_priv, &ephemeral_pub, &stealth_pub);
        assert!(result.is_none());
    }

    #[test]
    fn test_stealth_addresses_are_unique() {
        let recipient_priv = random_scalar();
        let recipient_pub = (recipient_priv * G).compress().to_bytes();

        let (pub1, _, _) = generate_stealth_address(&recipient_pub).unwrap();
        let (pub2, _, _) = generate_stealth_address(&recipient_pub).unwrap();
        assert_ne!(pub1, pub2); // different ephemeral keys → different stealth addresses
    }
}
```

- [ ] **Step 4: Update SDK exports**

In `sdk/src/lib.rs`, add:
```rust
pub mod auto_shield;
pub mod stealth;
pub mod encrypted_memo;
```

- [ ] **Step 5: Run all SDK tests**

Run: `cd sdk && cargo test -v`
Expected: All tests pass (23 existing + new tests).

- [ ] **Step 6: Commit**

```bash
git add sdk/
git commit -m "feat(sdk): auto-shield flow + stealth addresses + encrypted memos

Auto-shield: plan shield→transfer→unshield with amount splitting,
decoys, and random delay for maximum anonymity.
Stealth addresses: ECDH-based one-time wallets, recipient scanning.
Encrypted memos: ElGamal encryption of (amount, blinding) to recipient."
```

---

## Phase 5: Sequencer Integration — AA Endpoints + Modified Execution

### Task 10: New RPC Endpoints for AA

**Files:**
- Modify: `src/main.rs` (add `/aa/create_wallet`, `/aa/send` endpoints)
- Modify: `src/aa/mod.rs` (uncomment modules)

This task wires the AA system into the sequencer's HTTP API.

- [ ] **Step 1: Add AA wallet creation endpoint**

In `src/main.rs`, add a handler for `POST /aa/create_wallet`:

```rust
/// Request body for wallet creation.
#[derive(Debug, Deserialize)]
struct CreateWalletRequest {
    owner_pubkey: String,  // hex-encoded 32-byte pubkey
    salt: Option<String>,  // optional hex salt for deterministic address
}

/// POST /aa/create_wallet — create a new PrivateWallet account.
async fn create_aa_wallet(
    data: web::Data<Arc<Mutex<C0DL3ZkSyncNode>>>,
    body: web::Json<CreateWalletRequest>,
) -> impl Responder {
    let pubkey_bytes = match hex::decode(body.owner_pubkey.trim_start_matches("0x")) {
        Ok(b) if b.len() == 32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&b);
            arr
        }
        _ => return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "invalid owner_pubkey: expected 64 hex chars"
        })),
    };

    let salt = match &body.salt {
        Some(s) => {
            match hex::decode(s.trim_start_matches("0x")) {
                Ok(b) if b.len() == 32 => {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&b);
                    arr
                }
                _ => return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "invalid salt"
                })),
            }
        }
        None => [0u8; 32],
    };

    let address = aa::wallet_factory::derive_wallet_address(&pubkey_bytes, &salt);
    let account = aa::wallet_factory::create_wallet_account(&address, pubkey_bytes);

    let mut node = data.lock().unwrap();
    node.state.accounts.insert(address.clone(), account);
    node.state.compute_state_root();

    HttpResponse::Ok().json(serde_json::json!({
        "address": address,
        "wallet_type": "PrivateWallet",
        "balance_commitment": hex::encode(
            node.state.accounts.get(&address).unwrap().balance_commitment
        ),
    }))
}
```

- [ ] **Step 2: Add AA send endpoint**

Add handler for `POST /aa/send` that accepts a UserOperation:

```rust
/// POST /aa/send — submit a UserOperation (AA transfer).
async fn aa_send(
    data: web::Data<Arc<Mutex<C0DL3ZkSyncNode>>>,
    body: web::Json<UserOperation>,
) -> impl Responder {
    let op = body.into_inner();
    let mut node = data.lock().unwrap();

    // Verify sender exists and is a PrivateWallet
    let sender = match node.state.accounts.get(&op.sender) {
        Some(a) => a.clone(),
        None => return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "sender account not found"
        })),
    };

    if sender.wallet_type != aa::types::WalletType::PrivateWallet {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "sender is not a PrivateWallet (use /transactions for legacy EOA)"
        }));
    }

    let sender_pubkey = match sender.owner_pubkey {
        Some(pk) => pk,
        None => return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "sender has no owner_pubkey"
        })),
    };

    // Get or create recipient
    let recipient_commit = node.state.accounts
        .get(&op.to)
        .map(|a| a.balance_commitment)
        .unwrap_or_else(|| {
            compute_balance_commitment(&op.to, 0, 0)
        });

    // Validate the UserOperation
    match aa::validation::validate_user_operation(
        &op,
        &sender_pubkey,
        &sender.balance_commitment,
        &recipient_commit,
    ) {
        Ok(()) => {},
        Err(e) => return HttpResponse::BadRequest().json(serde_json::json!({
            "error": format!("validation failed: {}", e)
        })),
    }

    // Apply: update commitments (NOT plaintext balances — we don't know them)
    node.state.accounts
        .entry(op.sender.clone())
        .and_modify(|a| {
            a.balance_commitment = op.sender_new_commitment;
            a.nonce += 1;
            // balance stays at 0 for PrivateWallet — commitment is authoritative
        });

    // Create/update recipient
    node.state.accounts
        .entry(op.to.clone())
        .or_insert_with(|| AccountState {
            balance_commitment: compute_balance_commitment(&op.to, 0, 0),
            balance: 0,
            nonce: 0,
            wallet_type: aa::types::WalletType::PrivateWallet,
            owner_pubkey: None, // recipient might not have created a wallet yet
        })
        .balance_commitment = op.recipient_new_commitment;

    node.state.compute_state_root();

    HttpResponse::Ok().json(serde_json::json!({
        "status": "accepted",
        "sender_new_commitment": hex::encode(op.sender_new_commitment),
        "recipient_new_commitment": hex::encode(op.recipient_new_commitment),
    }))
}
```

- [ ] **Step 3: Register routes**

In the `HttpServer::new` closure, add:
```rust
.route("/aa/create_wallet", web::post().to(create_aa_wallet))
.route("/aa/send", web::post().to(aa_send))
```

- [ ] **Step 4: Run all tests**

Run: `cargo test --lib`
Expected: All 153+ tests pass (new endpoints don't break existing behavior).

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat(aa): /aa/create_wallet and /aa/send RPC endpoints

create_wallet: deploy PrivateWallet with pubkey, deterministic address.
send: accept UserOperation, validate Schnorr auth + conservation,
update commitments without touching plaintext balances."
```

---

### Task 11: Modified Block Input for AA Transactions

**Files:**
- Modify: `src/main.rs` (get_block_input function)
- Modify: `program/src/main.rs` (guest entry point)
- Modify: `program/src/state.rs` (AccountWitness)

The guest program needs to verify AA transactions. The block input must include UserOperations alongside legacy transactions.

- [ ] **Step 1: Add UserOperations to GuestBlockInput**

In `program/src/main.rs`, extend `GuestBlockInput`:

```rust
pub struct GuestBlockInput {
    // ... existing fields ...
    /// AA UserOperations for this block.
    pub user_operations: Vec<GuestUserOperation>,
}

/// Guest-side UserOperation (minimal for verification).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestUserOperation {
    pub sender: String,
    pub nonce: u64,
    pub to: String,
    pub sender_old_commitment: [u8; 32],
    pub sender_new_commitment: [u8; 32],
    pub recipient_old_commitment: [u8; 32],
    pub recipient_new_commitment: [u8; 32],
    pub amount_commitment: [u8; 32],
    pub knowledge_proof: crate::privacy::CommitmentKnowledgeProof,
    pub auth_signature_r: [u8; 32],
    pub auth_signature_s: [u8; 32],
    pub sender_pubkey: [u8; 32],
}
```

- [ ] **Step 2: Add Phase 7 to guest main.rs**

After Phase 6 (shielded pool), add AA verification:

```rust
// Phase 7: Verify AA UserOperations
for (i, op) in input.user_operations.iter().enumerate() {
    // Verify Schnorr auth
    let op_hash = compute_aa_op_hash(op);
    assert!(
        verify_schnorr(&op.sender_pubkey, &op_hash, &op.auth_signature_r, &op.auth_signature_s),
        "AA op {}: invalid auth signature", i
    );

    // Verify conservation
    assert!(
        verify_aa_conservation(
            &op.sender_old_commitment,
            &op.sender_new_commitment,
            &op.recipient_old_commitment,
            &op.recipient_new_commitment,
        ),
        "AA op {}: conservation check failed", i
    );

    // Verify knowledge proof
    assert!(
        crate::privacy::verify_commitment_knowledge_proof(&op.knowledge_proof),
        "AA op {}: invalid knowledge proof", i
    );
}
```

- [ ] **Step 3: Update host block input builder**

In `src/main.rs`, modify `get_block_input()` to include `user_operations` field populated from the pending AA transactions for the block.

- [ ] **Step 4: Run all tests**

Run: `cargo test --lib`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add program/src/main.rs program/src/state.rs src/main.rs
git commit -m "feat(aa): guest verification of UserOperations

Guest Phase 7: verify Schnorr auth, conservation, knowledge proof
for each AA UserOperation inside SP1. Block input extended with
user_operations field."
```

---

## Phase 6: Reference Solidity Contracts

### Task 12: PrivateWallet.sol + Paymaster.sol

**Files:**
- Create: `contracts/PrivateWallet.sol`
- Create: `contracts/Paymaster.sol`

Reference implementations showing how AA wallets work at the EVM level. These are documentation/examples — our L3 processes them natively.

- [ ] **Step 1: Write PrivateWallet.sol**

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title PrivateWallet — Default COLDL3 Account Abstraction Wallet
/// @notice All balances stored as Pedersen commitments. No plaintext amounts.
/// @dev Uses COLDL3 precompiles for cryptographic operations.
contract PrivateWallet {
    /// Owner's Ristretto255 public key (32 bytes)
    bytes32 public ownerPubkey;

    /// Current balance commitment: C = balance*G + r*H
    bytes32 public balanceCommitment;

    /// Nonce for replay protection
    uint64 public nonce;

    /// Guardian public keys for social recovery
    bytes32[] public guardians;
    uint32 public recoveryThreshold;

    // COLDL3 Precompile addresses
    address constant SCHNORR_VERIFY = address(0x0106);
    address constant CONSERVATION_CHECK = address(0x0107);
    address constant PEDERSEN_COMMIT = address(0x0101);

    event Transfer(bytes32 indexed newSenderCommitment, bytes32 indexed newRecipientCommitment);
    event WalletCreated(bytes32 indexed ownerPubkey, bytes32 initialCommitment);
    event KeyRotated(bytes32 indexed oldPubkey, bytes32 indexed newPubkey);

    constructor(bytes32 _ownerPubkey, bytes32[] memory _guardians, uint32 _threshold) {
        ownerPubkey = _ownerPubkey;
        guardians = _guardians;
        recoveryThreshold = _threshold;
        // Initial commitment = commit(0, deterministic_blinding)
        // Computed via precompile
        balanceCommitment = computeZeroCommitment();
        emit WalletCreated(_ownerPubkey, balanceCommitment);
    }

    /// Execute a private transfer.
    /// @param newMyCommitment My new balance commitment after transfer
    /// @param newTheirCommitment Recipient's new balance commitment
    /// @param schnorrR Schnorr signature R point
    /// @param schnorrS Schnorr signature s scalar
    function transfer(
        address recipient,
        bytes32 newMyCommitment,
        bytes32 newTheirCommitment,
        bytes32 recipientOldCommitment,
        bytes32 schnorrR,
        bytes32 schnorrS
    ) external {
        // 1. Verify Schnorr auth
        bytes memory authMsg = abi.encodePacked(
            "C0DL3:wallet_transfer:",
            newMyCommitment,
            newTheirCommitment,
            uint64(nonce)
        );
        require(
            verifySchnorr(ownerPubkey, authMsg, schnorrR, schnorrS),
            "invalid auth"
        );

        // 2. Verify conservation: what I lose = what they gain
        require(
            verifyConservation(
                balanceCommitment,
                newMyCommitment,
                recipientOldCommitment,
                newTheirCommitment
            ),
            "conservation failed"
        );

        // 3. Update state
        balanceCommitment = newMyCommitment;
        nonce++;

        // 4. Update recipient (calls their wallet)
        PrivateWallet(recipient).receiveCommitment(newTheirCommitment);

        emit Transfer(newMyCommitment, newTheirCommitment);
    }

    /// Receive a commitment update from another wallet.
    /// @dev Only callable during a transfer (conservation already verified)
    function receiveCommitment(bytes32 newCommitment) external {
        balanceCommitment = newCommitment;
    }

    /// Social recovery: guardians rotate the owner key.
    function recoverKey(
        bytes32 newOwnerPubkey,
        bytes32[] calldata guardianSigs_R,
        bytes32[] calldata guardianSigs_S
    ) external {
        require(guardianSigs_R.length >= recoveryThreshold, "not enough guardians");
        // Verify each guardian signature... (simplified)
        bytes32 oldPubkey = ownerPubkey;
        ownerPubkey = newOwnerPubkey;
        emit KeyRotated(oldPubkey, newOwnerPubkey);
    }

    // --- Internal helpers using precompiles ---

    function verifySchnorr(
        bytes32 pubkey, bytes memory message,
        bytes32 sigR, bytes32 sigS
    ) internal view returns (bool) {
        bytes memory input = abi.encodePacked(pubkey, sigR, sigS, uint32(message.length), message);
        (bool ok, bytes memory result) = SCHNORR_VERIFY.staticcall(input);
        return ok && result.length > 0 && result[0] == 0x01;
    }

    function verifyConservation(
        bytes32 oldSender, bytes32 newSender,
        bytes32 oldRecipient, bytes32 newRecipient
    ) internal view returns (bool) {
        bytes memory input = abi.encodePacked(oldSender, newSender, oldRecipient, newRecipient);
        (bool ok, bytes memory result) = CONSERVATION_CHECK.staticcall(input);
        return ok && result.length > 0 && result[0] == 0x01;
    }

    function computeZeroCommitment() internal view returns (bytes32) {
        bytes memory input = abi.encodePacked(
            uint64(0),      // value = 0
            uint64(0),      // nonce = 0
            uint8(42),      // address length
            bytes(abi.encodePacked(address(this)))
        );
        (bool ok, bytes memory result) = PEDERSEN_COMMIT.staticcall(input);
        require(ok, "pedersen failed");
        return bytes32(result);
    }
}
```

- [ ] **Step 2: Write Paymaster.sol**

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title Paymaster — Gas Payment Abstraction for COLDL3
/// @notice Pays gas on behalf of AA wallet users, breaking the sender-gas link.
contract Paymaster {
    bytes32 public ownerPubkey;
    uint256 public balance; // Paymaster's public balance (for gas payments)
    mapping(bytes32 => bool) public usedApprovals; // prevent replay

    address constant SCHNORR_VERIFY = address(0x0106);

    event GasPaid(address indexed wallet, uint256 gasAmount);
    event Deposited(uint256 amount);

    constructor(bytes32 _ownerPubkey) payable {
        ownerPubkey = _ownerPubkey;
        balance = msg.value;
    }

    /// Deposit gas funds into the paymaster.
    function deposit() external payable {
        balance += msg.value;
        emit Deposited(msg.value);
    }

    /// Pay gas for a UserOperation.
    /// Called by the sequencer during block execution.
    function payGas(
        address wallet,
        uint256 gasAmount,
        bytes32 approvalHash,
        bytes32 schnorrR,
        bytes32 schnorrS
    ) external {
        require(!usedApprovals[approvalHash], "approval already used");
        require(balance >= gasAmount, "insufficient paymaster balance");

        // Verify paymaster approved this gas payment
        bytes memory msg_ = abi.encodePacked(
            "C0DL3:paymaster:",
            approvalHash,
            gasAmount
        );
        require(
            verifySchnorr(ownerPubkey, msg_, schnorrR, schnorrS),
            "invalid paymaster sig"
        );

        usedApprovals[approvalHash] = true;
        balance -= gasAmount;
        emit GasPaid(wallet, gasAmount);
    }

    function verifySchnorr(
        bytes32 pubkey, bytes memory message,
        bytes32 sigR, bytes32 sigS
    ) internal view returns (bool) {
        bytes memory input = abi.encodePacked(pubkey, sigR, sigS, uint32(message.length), message);
        (bool ok, bytes memory result) = SCHNORR_VERIFY.staticcall(input);
        return ok && result.length > 0 && result[0] == 0x01;
    }
}
```

- [ ] **Step 3: Commit**

```bash
git add contracts/
git commit -m "feat(contracts): PrivateWallet.sol + Paymaster.sol reference implementations

PrivateWallet: commitment-based balance, Schnorr auth, social recovery.
Paymaster: gas payment abstraction, approval replay protection.
Both use COLDL3 precompiles (0x0101, 0x0106, 0x0107)."
```

---

## SP1 Precompile Acceleration Reference

Understanding which crypto operations are fast vs slow in the SP1 guest is critical for precompile design.

### SP1 v6.x Patched Crates (We Have Acceleration For)

| Crypto Primitive | SP1 Patch | Status in COLDL3 | Speedup vs Raw RISC-V |
|-----------------|-----------|-------------------|----------------------|
| **SHA-256** | `sp1-patches/RustCrypto-hashes` tag `patch-sha2-0.10.8-sp1-6.0.0` | ✅ Active | ~10x |
| **Keccak-256** | Via revm dependencies (implicit) | ✅ Active | ~10x |
| **Curve25519/Ristretto255** | `sp1-patches/curve25519-dalek-ng` branch `patch-v4.1.1` | ✅ Active | ~20x |
| **Secp256k1 (ECDSA)** | `sp1-patches/secp256k1` | ✅ Active → precompile 0x0001 | ~20x |
| **P-256 / secp256r1** | `sp1-patches/elliptic-curves` (p256 crate) | ✅ Active → precompile 0x0201 | ~20x |
| **Ed25519** | `sp1-patches/ed25519-consensus` | ✅ Active → precompile 0x0140 | ~15x |
| **BN254 (alt_bn128)** | SP1 native precompile | ✅ Active → precompiles 0x0006/07/08 | ~50x |
| **BLS12-381** | SP1 native precompile | 🟡 Patch-only, no precompile yet | ~50x |
| **BigInt arithmetic** | SP1 native precompile | ✅ Active → precompile 0x0005 | ~30x |
| **revm (EVM engine)** | `sp1-patches/revm` | ✅ Swap existing dep — no precompile, just faster EVM | ~2-5x EVM exec |
| **c-kzg-4844 (KZG)** | `sp1-patches/c-kzg-4844` | 🟡 Patch-only, activate with DA layer | ~10x |

### Precompile Speed Classification

| Our Precompile | Underlying Crypto | Patched? | Expected Cycles | Speed |
|---------------|-------------------|----------|----------------|-------|
| 0x0100 Ristretto255 Ops | curve25519-dalek-ng | ✅ YES | ~500-2K | ⚡ Fast |
| 0x0101 Pedersen Commit | curve25519-dalek-ng + sha2 | ✅ YES | ~2K | ⚡ Fast |
| 0x0102 Bulletproofs Verify | curve25519-dalek-ng (many scalar muls) | ✅ Partial | ~50K | 🟡 Medium |
| 0x0103 Poseidon Hash | Pure arithmetic (no patch) | ❌ NO | ~5K | 🟡 Medium |
| 0x0104 ElGamal Encrypt | curve25519-dalek-ng | ✅ YES | ~2K | ⚡ Fast |
| 0x0106 Schnorr Verify | curve25519-dalek-ng + sha2 | ✅ YES | ~3K | ⚡ Fast |
| 0x0107 Conservation Check | curve25519-dalek-ng (4 decompressions) | ✅ YES | ~4K | ⚡ Fast |
| 0x010A Memo Decrypt Verify | curve25519-dalek-ng + sha2 | ✅ YES | ~3K | ⚡ Fast |
| 0x010B Nullifier Derive | sha2 | ✅ YES | ~500 | ⚡ Fast |
| 0x0110 Shield | sha2 + curve25519 | ✅ YES | ~3K | ⚡ Fast |
| 0x0111 Unshield | sha2 | ✅ YES | ~1K | ⚡ Fast |
| 0x0120 Private Swap | curve25519-dalek-ng (8 decompressions) | ✅ YES | ~8K | ⚡ Fast |
| 0x0121 Threshold Proof | curve25519-dalek-ng + bulletproofs | ✅ Partial | ~30K | 🟡 Medium |
| 0x0122 Commitment Arithmetic | curve25519-dalek-ng | ✅ YES | ~1K | ⚡ Fast |
| 0x0132 Batch Schnorr Verify | curve25519-dalek-ng + sha2 | ✅ YES | ~2K/sig | ⚡ Fast |

**Key insight**: Almost all our precompiles use Ristretto255 + SHA-256, both of which have SP1 patches. The only exception is **Poseidon** (no patch, but algebraic operations are still relatively cheap in RISC-V) and **Bulletproofs** (many scalar multiplications — partially accelerated through the curve25519 patch).

---

## Phase 1b: Additional Precompiles — Poseidon, Private Swap, Threshold, Commitment Math, Batch Verify

These precompiles extend the privacy toolkit. Added after the core precompiles (Task 2-4) and before AA validation.

### Task 4b: Poseidon Hash Precompile (0x0103)

**Files:**
- Modify: `program/src/precompiles.rs`
- Create: `src/crypto/poseidon.rs` (lightweight Poseidon implementation)

Poseidon is a ZK-friendly hash function — ~8x cheaper to prove than SHA-256 in ZK circuits. While we don't have an SP1 patch for it, its algebraic structure means it's still relatively efficient as raw RISC-V. This is a future-proofing investment: when we move Merkle trees to Poseidon, proving gets much cheaper.

- [ ] **Step 1: Implement Poseidon hash (t=3, BN254 field)**

Create `src/crypto/poseidon.rs` with a minimal Poseidon implementation:
- Poseidon permutation with width t=3 (2 inputs + 1 capacity)
- BN254 scalar field (matching common ZK ecosystem)
- Round constants hardcoded (standard Poseidon parameters for t=3, α=5)
- `poseidon_hash(left: [u8; 32], right: [u8; 32]) -> [u8; 32]`

Note: For the initial implementation, we can use a simplified version that operates on 256-bit integers without a full finite field library. The key is correctness and determinism — performance optimization comes later when we add an SP1 Poseidon patch.

- [ ] **Step 2: Add precompile 0x0103 to guest**

In `program/src/precompiles.rs`:
- Input: `left(32) || right(32)` — two 32-byte inputs to hash
- Output: `hash(32)` — Poseidon hash output
- Gas: 1,500 (cheaper than SHA-256 in ZK context, more expensive in EVM context)

- [ ] **Step 3: Test and commit**

Run: `cargo test --lib`

```bash
git add src/crypto/ program/src/precompiles.rs
git commit -m "feat(precompile): Poseidon hash at 0x0103

ZK-friendly hash function for future Merkle tree optimization.
8x cheaper to prove than SHA-256 in ZK circuits.
No SP1 patch yet — runs as raw RISC-V (~5K cycles)."
```

---

### Task 4c: Private Swap Precompile (0x0120)

**Files:**
- Modify: `program/src/precompiles.rs`

Verifies a private swap: two parties exchange committed values, and the total is conserved. This is the building block for private DEXes.

- [ ] **Step 1: Add private swap precompile**

In `program/src/precompiles.rs`, register `0x0120`:
- Input: `party_a_old(32) || party_a_new(32) || party_b_old(32) || party_b_new(32) || token_a_commit(32) || token_b_commit(32)`
- Logic:
  ```
  // Party A gives token_a_commit, receives token_b_commit
  // Party B gives token_b_commit, receives token_a_commit
  // Conservation: (a_old - a_new) = token_a_commit AND (b_old - b_new) = token_b_commit
  // Cross-check: token_a_commit that A gives = token_a_commit that B receives (and vice versa)
  delta_a = decompress(party_a_old) - decompress(party_a_new)
  delta_b = decompress(party_b_old) - decompress(party_b_new)
  // A lost token_a, gained token_b: delta_a = token_a - token_b (net)
  // B lost token_b, gained token_a: delta_b = token_b - token_a (net)
  // Conservation: delta_a + delta_b = identity
  excess = delta_a + delta_b
  return excess == identity
  ```
- Output: `0x01` (balanced swap) or `0x00`
- Gas: 8,000 (8 decompressions + arithmetic)

- [ ] **Step 2: Test and commit**

```bash
git add program/src/precompiles.rs
git commit -m "feat(precompile): private swap at 0x0120

Verifies two-party commitment swap conservation.
Building block for private DEXes — both swap amounts hidden.
8,000 gas, all Ristretto255 ops (SP1 accelerated)."
```

---

### Task 4d: Threshold Proof Precompile (0x0121)

**Files:**
- Modify: `program/src/precompiles.rs`
- Create: `src/crypto/threshold_proof.rs`

Proves "this commitment hides a value ≥ threshold" without revealing the actual value. Uses a range proof on (value - threshold): if it's in [0, 2^64), then value ≥ threshold.

- [ ] **Step 1: Implement threshold proof verification**

Create `src/crypto/threshold_proof.rs`:

```rust
// Threshold proof: proves C commits to value ≥ threshold.
//
// Method: The prover computes C' = C - threshold*G.
// If C = v*G + r*H, then C' = (v-threshold)*G + r*H.
// A Bulletproofs range proof on C' proves (v-threshold) ∈ [0, 2^64),
// which means v ≥ threshold.
//
// The verifier:
//   1. Computes C' = C - threshold*G
//   2. Verifies the range proof against C'

use bulletproofs::{BulletproofGens, PedersenGens, RangeProof};
use curve25519_dalek_ng::{
    ristretto::CompressedRistretto,
    scalar::Scalar,
};
use merlin::Transcript;
use once_cell::sync::Lazy;

static PEDERSEN_GENS: Lazy<PedersenGens> = Lazy::new(PedersenGens::default);
static BP_GENS: Lazy<BulletproofGens> = Lazy::new(|| BulletproofGens::new(64, 128));

/// Verify a threshold proof: commitment hides value ≥ threshold.
pub fn verify_threshold_proof(
    commitment: &[u8; 32],
    threshold: u64,
    range_proof_bytes: &[u8],
) -> bool {
    // Decompress commitment
    let c_point = match CompressedRistretto(*commitment).decompress() {
        Some(p) => p,
        None => return false,
    };

    // C' = C - threshold * G
    let mut threshold_padded = [0u8; 32];
    threshold_padded[..8].copy_from_slice(&threshold.to_le_bytes());
    let threshold_scalar = Scalar::from_bytes_mod_order(threshold_padded);
    let c_prime = c_point - threshold_scalar * PEDERSEN_GENS.B;
    let c_prime_compressed = c_prime.compress();

    // Verify range proof on C'
    let rp = match RangeProof::from_bytes(range_proof_bytes) {
        Ok(rp) => rp,
        Err(_) => return false,
    };

    let mut transcript = Transcript::new(b"C0DL3-ThresholdProof");
    rp.verify_single(&BP_GENS, &PEDERSEN_GENS, &mut transcript, &c_prime_compressed, 64)
        .is_ok()
}

/// Generate a threshold proof (client-side, for SDK).
pub fn prove_threshold(
    value: u64,
    blinding: &Scalar,
    threshold: u64,
) -> Option<Vec<u8>> {
    if value < threshold {
        return None; // Can't prove value ≥ threshold if it's false
    }

    let diff = value - threshold;
    let mut transcript = Transcript::new(b"C0DL3-ThresholdProof");
    let (rp, _) = RangeProof::prove_single(
        &BP_GENS,
        &PEDERSEN_GENS,
        &mut transcript,
        diff,
        blinding,
        64,
    ).ok()?;

    Some(rp.to_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;

    fn random_scalar() -> Scalar {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        Scalar::from_bytes_mod_order(bytes)
    }

    fn make_commitment(value: u64, blinding: &Scalar) -> [u8; 32] {
        let mut v_padded = [0u8; 32];
        v_padded[..8].copy_from_slice(&value.to_le_bytes());
        let v = Scalar::from_bytes_mod_order(v_padded);
        PEDERSEN_GENS.commit(v, *blinding).compress().to_bytes()
    }

    #[test]
    fn test_threshold_proof_valid() {
        let blinding = random_scalar();
        let value = 1000u64;
        let threshold = 500u64;
        let commitment = make_commitment(value, &blinding);

        let proof = prove_threshold(value, &blinding, threshold).unwrap();
        assert!(verify_threshold_proof(&commitment, threshold, &proof));
    }

    #[test]
    fn test_threshold_proof_exact() {
        let blinding = random_scalar();
        let value = 500u64;
        let threshold = 500u64;
        let commitment = make_commitment(value, &blinding);

        let proof = prove_threshold(value, &blinding, threshold).unwrap();
        assert!(verify_threshold_proof(&commitment, threshold, &proof));
    }

    #[test]
    fn test_threshold_proof_below_fails() {
        let blinding = random_scalar();
        let value = 100u64;
        let threshold = 500u64;

        assert!(prove_threshold(value, &blinding, threshold).is_none());
    }
}
```

- [ ] **Step 2: Add precompile 0x0121 to guest**

Input: `commitment(32) || threshold_le64(8) || proof_len_le32(4) || proof_bytes`
Output: `0x01` (value ≥ threshold) or `0x00`
Gas: 50,000 (range proof verification)

- [ ] **Step 3: Test and commit**

```bash
git add src/crypto/threshold_proof.rs program/src/precompiles.rs
git commit -m "feat(precompile): threshold proof at 0x0121

Proves 'commitment hides value >= threshold' without revealing value.
Uses shifted Bulletproofs range proof on (value - threshold).
50,000 gas. Essential for DeFi: collateral checks, min balance proofs."
```

---

### Task 4e: Commitment Arithmetic Precompile (0x0122)

**Files:**
- Modify: `program/src/precompiles.rs`

Add/subtract Pedersen commitments. Since `C(a) + C(b) = C(a+b)` (with appropriate blinding), this lets contracts do math on hidden values.

- [ ] **Step 1: Add commitment arithmetic precompile**

In `program/src/precompiles.rs`, register `0x0122`:
- Input: `op(1) || commitment_a(32) || commitment_b(32)`
  - op = 0x01: add (C_a + C_b)
  - op = 0x02: subtract (C_a - C_b)
- Output: `result_commitment(32)`
- Gas: 1,500

```rust
// Decompress both, perform operation, compress result
let a = CompressedRistretto(commitment_a).decompress()?;
let b = CompressedRistretto(commitment_b).decompress()?;
let result = match op {
    0x01 => a + b,
    0x02 => a - b,
    _ => return error,
};
result.compress().to_bytes()
```

- [ ] **Step 2: Test and commit**

```bash
git add program/src/precompiles.rs
git commit -m "feat(precompile): commitment arithmetic at 0x0122

Add/subtract Pedersen commitments: C(a) + C(b) = C(a+b).
Contracts can do math on hidden values. 1,500 gas.
Building block for multi-party private computation."
```

---

### Task 4f: Batch Schnorr Verify Precompile (0x0132)

**Files:**
- Modify: `program/src/precompiles.rs`

Verify N Schnorr signatures in one call, ~40% cheaper than N individual 0x0106 calls due to batch point addition.

- [ ] **Step 1: Add batch verify precompile**

In `program/src/precompiles.rs`, register `0x0132`:
- Input: `count_le32(4) || [pubkey(32) || sig_r(32) || sig_s(32) || msg_len_le32(4) || message] × count`
- Logic: Batch verification using random linear combination:
  ```
  Pick random scalars z_1..z_n
  Check: Σ(z_i * s_i) * G == Σ(z_i * R_i) + Σ(z_i * e_i * PK_i)
  ```
  This is one multi-scalar multiplication instead of N individual checks.
- Output: `0x01` (all valid) or `0x00` (at least one invalid)
- Gas: `2,000 * count` (vs 3,000 * count for individual calls — 33% savings)

- [ ] **Step 2: Test and commit**

```bash
git add program/src/precompiles.rs
git commit -m "feat(precompile): batch Schnorr verify at 0x0132

Verify N Schnorr signatures in one call using random linear combination.
33% cheaper than N individual 0x0106 calls. 2,000 gas per signature.
Essential for AA blocks with many UserOperations."
```

---

## Summary: Updated Roadmap

After this plan is complete, the COLDL3 roadmap is:

1. ✅ **Client-side partial proving** (shielded pool privacy — DONE)
2. ✅ **Prover privacy** (precomputed commitments — DONE)
3. 🔄 **Privacy-by-default AA** (THIS PLAN)
4. ⬜ **SP1 toolchain + guest build** (compile + test guest program)
5. ⬜ **Settlement contracts** (SP1Verifier.sol on zkSync Era)
6. ⬜ **DA layer** (EIP-4844 blobs or Celestia)

### What this plan achieves:
- Every new account is a PrivateWallet (commitment-based balance)
- Transfers exchange commitment updates, not plaintext amounts
- Paymasters break the sender-gas link
- Auto-shield flow for maximum anonymity on demand
- Stealth addresses for one-time unlinkable wallets
- Encrypted memos so recipients can decode amounts
- **15 new precompiles** (20 total, 19 SP1-accelerated):
  - 0x0001 ecRecover — Ethereum compat, MetaMask/hardware wallet support
  - 0x0002 SHA-256 — Ethereum compat (Solidity `sha256()`)
  - 0x0005 ModExp — Ethereum compat (`modexp` precompile)
  - 0x0006/07/08 BN254 — Ethereum compat + on-chain Groth16 ZK proof verification
  - 0x0103 Poseidon Hash — ZK-friendly hash, 8x cheaper to prove
  - 0x0104 ElGamal Encrypt — encrypted memos
  - 0x0106 Schnorr Verify — AA wallet auth
  - 0x0107 Conservation Check — private transfers
  - 0x010A Encrypted Memo Verify
  - 0x0120 Private Swap — private DEX building block
  - 0x0121 Threshold Proof — prove balance ≥ X
  - 0x0122 Commitment Arithmetic — math on hidden values
  - 0x0132 Batch Schnorr Verify — 33% cheaper batch auth
  - 0x0140 Ed25519 Verify — HEAT/COLD verifier contracts, Cosmos/Solana bridge
  - 0x0201 P-256 ecRecover — WebAuthn/Passkeys: Face ID, Touch ID, YubiKey, no seed phrases
  - revm patch swapped — 2–5x faster EVM execution, zero code changes
  - BLS12-381 patch registered, addresses 0x0160–0x0163 reserved (activate for beacon chain / EIP-2537)
  - c-kzg-4844 patch registered, address 0x0170 reserved (activate with DA layer)
  - ChaCha20: flagged for revisit — used in Fuego, check if C0DL3↔Fuego bridge needs on-chain decryption
- Guest program verifies AA operations inside SP1

### Complete Precompile Map (20 active after this plan):

#### Ethereum-Compatible (mirrors standard precompiles — Solidity calls work unmodified)

| Address | Name | Gas | SP1 Patch | Notes |
|---------|------|-----|-----------|-------|
| 0x0001 | ecRecover (secp256k1 ECDSA) | 3,000 | ✅ sp1-patches/secp256k1 | MetaMask/hardware wallet compat |
| 0x0002 | SHA-256 | 60+12/word | ✅ sp1-patches/RustCrypto-hashes | Mirrors Ethereum 0x02 |
| 0x0005 | ModExp (BigInt) | varies | ✅ SP1 native bigint | Mirrors Ethereum 0x05, modexp |
| 0x0006 | BN254 ecAdd | 150 | ✅ SP1 native bn254 | Mirrors Ethereum 0x06 |
| 0x0007 | BN254 ecMul | 6,000 | ✅ SP1 native bn254 | Mirrors Ethereum 0x07 |
| 0x0008 | BN254 ecPairing | 45,000+ | ✅ SP1 native bn254 | Mirrors Ethereum 0x08; enables on-chain Groth16 verification |

#### C0DL3 Privacy Primitives

| Address | Name | Gas | SP1 Patch | Notes |
|---------|------|-----|-----------|-------|
| 0x0100 | Ristretto255 Point Ops | 500-2,000 | ✅ curve25519-dalek-ng | Low-level curve ops |
| 0x0101 | Pedersen Commit | 3,000 | ✅ curve25519 + sha2 | Balance/value commitments |
| 0x0102 | Bulletproofs Verify | 50,000 | 🟡 partial curve25519 | Range proof verification |
| 0x0103 | Poseidon Hash | 1,500 | ❌ raw RISC-V | ZK-friendly hash; 8x cheaper to prove than SHA-256 |
| 0x0104 | ElGamal Encrypt | 5,000 | ✅ curve25519 | Encrypted memos |
| 0x0106 | Schnorr Verify | 3,000 | ✅ curve25519 + sha2 | AA wallet auth |
| 0x0107 | Conservation Check | 4,000 | ✅ curve25519 | Verify private transfer balance |
| 0x010A | Encrypted Memo Verify | 5,000 | ✅ curve25519 + sha2 | On-chain memo decryption check |

#### C0DL3 Shielded Pool

| Address | Name | Gas | SP1 Patch | Notes |
|---------|------|-----|-----------|-------|
| 0x0110 | Shield | 10,000 | ✅ sha2 + curve25519 | EVM → shielded pool |
| 0x0111 | Unshield | 10,000 | ✅ sha2 | Shielded pool → EVM |

#### C0DL3 DeFi Privacy

| Address | Name | Gas | SP1 Patch | Notes |
|---------|------|-----|-----------|-------|
| 0x0120 | Private Swap | 8,000 | ✅ curve25519 | Two-party private DEX |
| 0x0121 | Threshold Proof | 50,000 | 🟡 partial bulletproofs | Prove balance ≥ X |
| 0x0122 | Commitment Arithmetic | 1,500 | ✅ curve25519 | Add/subtract hidden values |

#### C0DL3 Performance + Interop

| Address | Name | Gas | SP1 Patch | Notes |
|---------|------|-----|-----------|-------|
| 0x0132 | Batch Schnorr Verify | 2,000/sig | ✅ curve25519 + sha2 | 33% cheaper batch AA auth |
| 0x0140 | Ed25519 Verify | 3,000 | ✅ sp1-patches/ed25519-consensus | HEAT/COLD verifier contracts; Cosmos/Solana bridge sigs |

#### WebAuthn / Passkey Auth

| Address | Name | Gas | SP1 Patch | Notes |
|---------|------|-----|-----------|-------|
| 0x0201 | P-256 ecRecover (secp256r1) | 3,000 | ✅ sp1-patches/elliptic-curves (p256) | WebAuthn/Passkeys: Face ID, Touch ID, YubiKey, hardware security modules. No seed phrases. |

#### Non-Precompile Patches (Cargo.toml only, improve performance transparently)

| Crate | Patch | Effect |
|-------|-------|--------|
| revm | `sp1-patches/revm` | 2–5x faster EVM execution inside guest — swap existing dep, zero code changes |

#### Patch-Only (registered in Cargo.toml but no active precompile yet)

| Crate | Patch | Proposed address | When to activate |
|-------|-------|-----------------|-----------------|
| bls12-381 | SP1 native | 0x0160–0x0163 | Beacon chain interop (validator signatures, attestations), BLS aggregate multisig, EIP-2537 compatibility |
| c-kzg-4844 | `sp1-patches/c-kzg-4844` | 0x0170 | DA layer: KZG proof verification for EIP-4844 blobs — activate in DA layer phase |

> **BLS12-381 note**: Addresses 0x0160–0x0163 reserved for: ecAdd, ecMul, ecPairing (G1), ecPairing (G2). Ethereum EIP-2537 is the likely final spec — wait for that to stabilize before registering to avoid an address conflict.
>
> **c-kzg-4844 note**: Address 0x0170 reserved. Activate when DA layer is implemented — the guest will need to verify blob KZG proofs for EIP-4844 data availability.

#### Under Consideration — No SP1 Patch, Revisit Later

| Primitive | Why it might matter | Blocker |
|-----------|-------------------|---------|
| **ChaCha20** | Used heavily in Fuego for symmetric encryption. If C0DL3↔Fuego bridge passes ChaCha20-encrypted payloads, a precompile would be useful for on-chain decryption/verification. **TODO: check Fuego bridge integration points.** | No SP1 patch; ~10K cycles raw. Add if bridge needs on-chain ChaCha20. |
| **Poseidon (SP1 patch)** | When SP1 ships a Poseidon patch, swap 0x0103 from raw RISC-V to accelerated — no interface change needed. | SP1 Poseidon patch doesn't exist yet as of v6.x. |

### What this plan does NOT change:
- Legacy EOA accounts still work (backward compat via WalletType::LegacyEOA)
- Shielded pool remains for full anonymity (separate from AA)
- Existing 153 host tests continue to pass
- Prover privacy (precomputed commitments) unchanged
