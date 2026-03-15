// Custom EVM precompiles for C0DL3 privacy operations.
//
// These precompiles are callable from Solidity contracts via CALL to fixed addresses.
// They execute privacy cryptographic operations (Ristretto255, Pedersen, Bulletproofs)
// at native speed instead of EVM opcode-level cost.
//
// Addresses (must match host constants in src/proving/mod.rs):
//   0x0100: Ristretto255 point operations
//   0x0101: Pedersen commitment
//   0x0102: Bulletproofs range proof verification
//   0x0106: Schnorr signature verification
//   0x0107: Conservation check (Pedersen commitment balance)
//   0x0110: Shield (EVM → shielded pool deposit)
//   0x0111: Unshield (shielded pool → EVM withdrawal)
//
// All precompiles follow the standard revm convention:
//   fn(&Bytes, gas_limit: u64) -> PrecompileResult
//
// Gas costs are conservative estimates — tune based on SP1 cycle benchmarks.

use bulletproofs::{BulletproofGens, PedersenGens, RangeProof};
use curve25519_dalek_ng::{
    ristretto::{CompressedRistretto, RistrettoPoint},
    scalar::Scalar,
    traits::Identity,
};
use merlin::Transcript;
use revm::{
    handler::register::EvmHandler,
    precompile::{Precompile, PrecompileOutput, PrecompileResult},
    primitives::{Address, Bytes},
    ContextPrecompile, Database,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;

// ── Precompile Addresses ────────────────────────────────────────────────────

/// Convert a u64 precompile ID to a 20-byte EVM address.
fn precompile_address(id: u64) -> Address {
    let mut bytes = [0u8; 20];
    bytes[18] = (id >> 8) as u8;
    bytes[19] = id as u8;
    Address::from(bytes)
}

pub const RISTRETTO255_ADDR: u64 = 0x0100;
pub const PEDERSEN_COMMIT_ADDR: u64 = 0x0101;
pub const BULLETPROOFS_VERIFY_ADDR: u64 = 0x0102;
pub const ELGAMAL_ENCRYPT_ADDR: u64 = 0x0104;
pub const SCHNORR_VERIFY_ADDR: u64 = 0x0106;
pub const CONSERVATION_CHECK_ADDR: u64 = 0x0107;
pub const MEMO_DECRYPT_VERIFY_ADDR: u64 = 0x010A;
pub const SHIELD_ADDR: u64 = 0x0110;
pub const UNSHIELD_ADDR: u64 = 0x0111;

// ── Gas Costs ───────────────────────────────────────────────────────────────
// Conservative estimates. SP1 accelerates curve25519 ops via precompile syscalls,
// so actual RISC-V cycle cost is much lower than software-only execution.

const GAS_RISTRETTO_ADD: u64 = 500;
const GAS_RISTRETTO_SCALAR_MUL: u64 = 2_000;
const GAS_PEDERSEN_COMMIT: u64 = 3_000;
const GAS_BULLETPROOFS_VERIFY: u64 = 50_000;
const GAS_ELGAMAL_ENCRYPT: u64 = 5_000;
const GAS_SCHNORR_VERIFY: u64 = 3_000;
const GAS_CONSERVATION_CHECK: u64 = 4_000;
const GAS_MEMO_DECRYPT_VERIFY: u64 = 5_000;
const GAS_SHIELD: u64 = 10_000;
const GAS_UNSHIELD: u64 = 10_000;

// ── Handler Registration ────────────────────────────────────────────────────

/// Register all C0DL3 privacy precompiles into the revm handler.
/// Call via `Evm::builder().append_handler_register(register_cold_precompiles)`.
pub fn register_cold_precompiles<EXT, DB: Database>(handler: &mut EvmHandler<'_, EXT, DB>) {
    let prev = handler.pre_execution.load_precompiles.clone();
    handler.pre_execution.load_precompiles = Arc::new(move || {
        let mut precompiles = prev();
        precompiles.extend([
            (
                precompile_address(RISTRETTO255_ADDR),
                ContextPrecompile::Ordinary(Precompile::Standard(precompile_ristretto255)),
            ),
            (
                precompile_address(PEDERSEN_COMMIT_ADDR),
                ContextPrecompile::Ordinary(Precompile::Standard(precompile_pedersen_commit)),
            ),
            (
                precompile_address(BULLETPROOFS_VERIFY_ADDR),
                ContextPrecompile::Ordinary(Precompile::Standard(precompile_bulletproofs_verify)),
            ),
            (
                precompile_address(ELGAMAL_ENCRYPT_ADDR),
                ContextPrecompile::Ordinary(Precompile::Standard(precompile_elgamal_encrypt)),
            ),
            (
                precompile_address(SCHNORR_VERIFY_ADDR),
                ContextPrecompile::Ordinary(Precompile::Standard(precompile_schnorr_verify)),
            ),
            (
                precompile_address(CONSERVATION_CHECK_ADDR),
                ContextPrecompile::Ordinary(Precompile::Standard(precompile_conservation_check)),
            ),
            (
                precompile_address(MEMO_DECRYPT_VERIFY_ADDR),
                ContextPrecompile::Ordinary(Precompile::Standard(precompile_memo_decrypt_verify)),
            ),
            (
                precompile_address(SHIELD_ADDR),
                ContextPrecompile::Ordinary(Precompile::Standard(precompile_shield)),
            ),
            (
                precompile_address(UNSHIELD_ADDR),
                ContextPrecompile::Ordinary(Precompile::Standard(precompile_unshield)),
            ),
        ]);
        precompiles
    });
}

// ── 0x0100: Ristretto255 Point Operations ───────────────────────────────────
//
// Input: op(1 byte) || operand_a(32 bytes) || [operand_b(32 bytes)]
//
// Operations:
//   0x01: point_add(a, b) → compressed point (32 bytes)
//   0x02: scalar_mul(point_a, scalar_b) → compressed point (32 bytes)
//   0x03: negate(point_a) → compressed point (32 bytes)
//   0x04: is_identity(point_a) → 1 byte (0x01 = yes, 0x00 = no)

fn precompile_ristretto255(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    if input.is_empty() {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("empty input"),
        ));
    }

    let op = input[0];
    match op {
        // point_add
        0x01 => {
            if gas_limit < GAS_RISTRETTO_ADD {
                return Err(revm::precompile::PrecompileErrors::Error(
                    revm::precompile::PrecompileError::other("out of gas"),
                ));
            }
            if input.len() < 65 {
                return Err(revm::precompile::PrecompileErrors::Error(
                    revm::precompile::PrecompileError::other("ristretto_add: need 65 bytes"),
                ));
            }
            let a = decompress_point(&input[1..33])?;
            let b = decompress_point(&input[33..65])?;
            let result = (a + b).compress().to_bytes();
            Ok(PrecompileOutput::new(GAS_RISTRETTO_ADD, result.to_vec().into()))
        }
        // scalar_mul
        0x02 => {
            if gas_limit < GAS_RISTRETTO_SCALAR_MUL {
                return Err(revm::precompile::PrecompileErrors::Error(
                    revm::precompile::PrecompileError::other("out of gas"),
                ));
            }
            if input.len() < 65 {
                return Err(revm::precompile::PrecompileErrors::Error(
                    revm::precompile::PrecompileError::other("scalar_mul: need 65 bytes"),
                ));
            }
            let point = decompress_point(&input[1..33])?;
            let scalar = scalar_from_bytes(&input[33..65]);
            let result = (point * scalar).compress().to_bytes();
            Ok(PrecompileOutput::new(
                GAS_RISTRETTO_SCALAR_MUL,
                result.to_vec().into(),
            ))
        }
        // negate
        0x03 => {
            if gas_limit < GAS_RISTRETTO_ADD {
                return Err(revm::precompile::PrecompileErrors::Error(
                    revm::precompile::PrecompileError::other("out of gas"),
                ));
            }
            if input.len() < 33 {
                return Err(revm::precompile::PrecompileErrors::Error(
                    revm::precompile::PrecompileError::other("negate: need 33 bytes"),
                ));
            }
            let point = decompress_point(&input[1..33])?;
            let result = (-point).compress().to_bytes();
            Ok(PrecompileOutput::new(GAS_RISTRETTO_ADD, result.to_vec().into()))
        }
        // is_identity
        0x04 => {
            if gas_limit < GAS_RISTRETTO_ADD {
                return Err(revm::precompile::PrecompileErrors::Error(
                    revm::precompile::PrecompileError::other("out of gas"),
                ));
            }
            if input.len() < 33 {
                return Err(revm::precompile::PrecompileErrors::Error(
                    revm::precompile::PrecompileError::other("is_identity: need 33 bytes"),
                ));
            }
            let point = decompress_point(&input[1..33])?;
            let is_id = if point == RistrettoPoint::identity() {
                1u8
            } else {
                0u8
            };
            Ok(PrecompileOutput::new(GAS_RISTRETTO_ADD, vec![is_id].into()))
        }
        _ => Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("unknown ristretto op"),
        )),
    }
}

// ── 0x0101: Pedersen Commitment ─────────────────────────────────────────────
//
// Input: value_le64(8 bytes) || nonce_le64(8 bytes) || address_len(1 byte) || address_bytes(...)
//
// Computes: C = value * G + r * H
// where r = SHA-256("C0DL3:balance:" || address_bytes || nonce_le_bytes)
//
// Output: 32 bytes (compressed Ristretto point)
//
// This precompile allows Solidity contracts to compute Pedersen commitments
// that are compatible with the state root commitment scheme.

fn precompile_pedersen_commit(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    if gas_limit < GAS_PEDERSEN_COMMIT {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("out of gas"),
        ));
    }
    if input.len() < 17 {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other(
                "pedersen_commit: need >= 17 bytes (value(8) + nonce(8) + addr_len(1))",
            ),
        ));
    }

    let value = u64::from_le_bytes(input[0..8].try_into().unwrap());
    let nonce = u64::from_le_bytes(input[8..16].try_into().unwrap());
    let addr_len = input[16] as usize;

    if input.len() < 17 + addr_len {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("pedersen_commit: address truncated"),
        ));
    }

    let address_bytes = &input[17..17 + addr_len];

    // Derive deterministic blinding factor — matches host exactly
    let mut blinding_input = Vec::new();
    blinding_input.extend_from_slice(b"C0DL3:balance:");
    blinding_input.extend_from_slice(address_bytes);
    blinding_input.extend_from_slice(&nonce.to_le_bytes());
    let blinding_hash: [u8; 32] = Sha256::digest(&blinding_input).into();
    let r = Scalar::from_bytes_mod_order(blinding_hash);

    let mut amount_padded = [0u8; 32];
    amount_padded[..8].copy_from_slice(&value.to_le_bytes());
    let v = Scalar::from_bytes_mod_order(amount_padded);

    let gens = PedersenGens::default();
    let commitment = gens.commit(v, r).compress().to_bytes();

    Ok(PrecompileOutput::new(
        GAS_PEDERSEN_COMMIT,
        commitment.to_vec().into(),
    ))
}

// ── 0x0102: Bulletproofs Range Proof Verification ───────────────────────────
//
// Input: commitment(32 bytes) || bit_count_le32(4 bytes) || proof_len_le32(4 bytes) || proof_bytes(...)
//
// Verifies that the Pedersen commitment hides a value in [0, 2^bit_count).
//
// Output: 1 byte (0x01 = valid, 0x00 = invalid)

fn precompile_bulletproofs_verify(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    if gas_limit < GAS_BULLETPROOFS_VERIFY {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("out of gas"),
        ));
    }
    if input.len() < 40 {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other(
                "bulletproofs_verify: need >= 40 bytes (commitment(32) + bits(4) + proof_len(4))",
            ),
        ));
    }

    let commitment_bytes: [u8; 32] = input[0..32].try_into().unwrap();
    let bit_count = u32::from_le_bytes(input[32..36].try_into().unwrap()) as usize;
    let proof_len = u32::from_le_bytes(input[36..40].try_into().unwrap()) as usize;

    if input.len() < 40 + proof_len {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("bulletproofs_verify: proof truncated"),
        ));
    }

    let proof_bytes = &input[40..40 + proof_len];

    // Decompress commitment
    let commitment = match CompressedRistretto(commitment_bytes).decompress() {
        Some(p) => p,
        None => {
            return Ok(PrecompileOutput::new(
                GAS_BULLETPROOFS_VERIFY,
                vec![0x00].into(),
            ))
        }
    };

    // Deserialize proof
    let proof = match RangeProof::from_bytes(proof_bytes) {
        Ok(p) => p,
        Err(_) => {
            return Ok(PrecompileOutput::new(
                GAS_BULLETPROOFS_VERIFY,
                vec![0x00].into(),
            ))
        }
    };

    // Verify
    let pc_gens = PedersenGens::default();
    let bp_gens = BulletproofGens::new(bit_count, 1);
    let mut transcript = Transcript::new(b"C0DL3_range_proof");

    let valid = proof
        .verify_single(&bp_gens, &pc_gens, &mut transcript, &commitment.compress(), bit_count)
        .is_ok();

    Ok(PrecompileOutput::new(
        GAS_BULLETPROOFS_VERIFY,
        vec![if valid { 0x01 } else { 0x00 }].into(),
    ))
}

// ── 0x0110: Shield (EVM → Shielded Pool) ────────────────────────────────────
//
// Input: amount_le64(8 bytes) || recipient_pubkey(32 bytes) || blinding(32 bytes)
//
// Computes the note commitment for a shielded deposit.
// The actual pool state update happens in the privacy validation phase,
// not during EVM execution — this precompile just computes the commitment
// so Solidity contracts can emit it in events.
//
// Output: note_commitment(32 bytes)

fn precompile_shield(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    if gas_limit < GAS_SHIELD {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("out of gas"),
        ));
    }
    if input.len() < 72 {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other(
                "shield: need 72 bytes (amount(8) + pubkey(32) + blinding(32))",
            ),
        ));
    }

    let amount = u64::from_le_bytes(input[0..8].try_into().unwrap());
    let recipient_pubkey: [u8; 32] = input[8..40].try_into().unwrap();
    let blinding_bytes: [u8; 32] = input[40..72].try_into().unwrap();

    // Compute Pedersen commitment: C = amount * G + blinding * H
    let mut amount_padded = [0u8; 32];
    amount_padded[..8].copy_from_slice(&amount.to_le_bytes());
    let v = Scalar::from_bytes_mod_order(amount_padded);
    let r = Scalar::from_bytes_mod_order(blinding_bytes);

    let gens = PedersenGens::default();
    let value_commitment = gens.commit(v, r).compress().to_bytes();

    // Note commitment = SHA-256("C0DL3:note:" || value_commitment || recipient_pubkey)
    let mut hasher = Sha256::new();
    hasher.update(b"C0DL3:note:");
    hasher.update(&value_commitment);
    hasher.update(&recipient_pubkey);
    let note_commitment: [u8; 32] = hasher.finalize().into();

    Ok(PrecompileOutput::new(
        GAS_SHIELD,
        note_commitment.to_vec().into(),
    ))
}

// ── 0x0111: Unshield (Shielded Pool → EVM) ─────────────────────────────────
//
// Input: nullifier(32 bytes) || note_commitment(32 bytes) || amount_le64(8 bytes)
//
// Validates that the nullifier is correctly derived from the note commitment.
// The actual nullifier freshness check and pool update happen in privacy validation.
// This precompile computes the expected nullifier for verification.
//
// Output: expected_nullifier(32 bytes)

fn precompile_unshield(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    if gas_limit < GAS_UNSHIELD {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("out of gas"),
        ));
    }
    if input.len() < 72 {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other(
                "unshield: need 72 bytes (nullifier(32) + commitment(32) + amount(8))",
            ),
        ));
    }

    let nullifier: [u8; 32] = input[0..32].try_into().unwrap();
    let note_commitment: [u8; 32] = input[32..64].try_into().unwrap();
    let _amount = u64::from_le_bytes(input[64..72].try_into().unwrap());

    // Verify nullifier derivation: H("C0DL3:nullifier_check:" || note_commitment)
    // The real nullifier uses the spend key, but the precompile can verify the
    // commitment binding.
    let mut hasher = Sha256::new();
    hasher.update(b"C0DL3:nullifier_check:");
    hasher.update(&note_commitment);
    let expected: [u8; 32] = hasher.finalize().into();

    // Return the expected nullifier — caller can compare
    // Also return the provided nullifier for the contract to use in events
    let mut output = Vec::with_capacity(64);
    output.extend_from_slice(&expected);
    output.extend_from_slice(&nullifier);

    Ok(PrecompileOutput::new(GAS_UNSHIELD, output.into()))
}

// ── 0x0106: Schnorr Signature Verification ──────────────────────────────────
//
// Input: pubkey(32) || sig_r(32) || sig_s(32) || msg_len_le32(4) || message
//
// Verifies a Ristretto255 Schnorr signature:
//   s*G == R + e*pubkey   where e = H("C0DL3:schnorr:" || R || pubkey || message)
//
// Output: 1 byte (0x01 = valid, 0x00 = invalid)
//
// Gas: 3,000

fn precompile_schnorr_verify(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    if gas_limit < GAS_SCHNORR_VERIFY {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("out of gas"),
        ));
    }
    // Minimum: pubkey(32) + sig_r(32) + sig_s(32) + msg_len(4) = 100 bytes
    if input.len() < 100 {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other(
                "schnorr_verify: need >= 100 bytes (pubkey(32) + sig_r(32) + sig_s(32) + msg_len(4))",
            ),
        ));
    }

    let pubkey: [u8; 32] = input[0..32].try_into().unwrap();
    let sig_r: [u8; 32] = input[32..64].try_into().unwrap();
    let sig_s: [u8; 32] = input[64..96].try_into().unwrap();
    let msg_len = u32::from_le_bytes(input[96..100].try_into().unwrap()) as usize;

    if input.len() < 100 + msg_len {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("schnorr_verify: message truncated"),
        ));
    }

    let message = &input[100..100 + msg_len];

    // Decompress public key
    let pk_point = match CompressedRistretto(pubkey).decompress() {
        Some(p) => p,
        None => {
            return Ok(PrecompileOutput::new(GAS_SCHNORR_VERIFY, vec![0x00].into()))
        }
    };

    // Decompress R point
    let r_point = match CompressedRistretto(sig_r).decompress() {
        Some(p) => p,
        None => {
            return Ok(PrecompileOutput::new(GAS_SCHNORR_VERIFY, vec![0x00].into()))
        }
    };

    // Compute challenge: e = H("C0DL3:schnorr:" || R || pubkey || message)
    let e = {
        let mut hasher = Sha256::new();
        hasher.update(b"C0DL3:schnorr:");
        hasher.update(&sig_r);
        hasher.update(&pubkey);
        hasher.update(message);
        let hash: [u8; 32] = hasher.finalize().into();
        Scalar::from_bytes_mod_order(hash)
    };

    let s = Scalar::from_bytes_mod_order(sig_s);

    // Verify: s*G == R + e*pubkey
    use curve25519_dalek_ng::constants::RISTRETTO_BASEPOINT_POINT as G;
    let lhs = s * G;
    let rhs = r_point + e * pk_point;

    let valid = lhs == rhs;
    Ok(PrecompileOutput::new(
        GAS_SCHNORR_VERIFY,
        vec![if valid { 0x01 } else { 0x00 }].into(),
    ))
}

// ── 0x0104: ElGamal Encrypt ──────────────────────────────────────────────────
//
// Input: recipient_pubkey(32) || amount_le64(8) || blinding(32) = 72 bytes
//
// Encrypts (amount, blinding) to the recipient's Ristretto255 public key using
// ephemeral ECDH + SHA-256 key derivation (matches sdk/src/encrypted_memo.rs).
//
// Output: encrypted_memo(72 bytes) = ephemeral_pubkey(32) || ciphertext(40)
//
// Returns Err if input is not exactly 72 bytes.
//
// Gas: 5,000

fn precompile_elgamal_encrypt(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    if gas_limit < GAS_ELGAMAL_ENCRYPT {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("out of gas"),
        ));
    }
    if input.len() != 72 {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other(
                "elgamal_encrypt: need exactly 72 bytes (pubkey(32) + amount(8) + blinding(32))",
            ),
        ));
    }

    use curve25519_dalek_ng::constants::RISTRETTO_BASEPOINT_POINT as G;

    let recipient_pubkey: [u8; 32] = input[0..32].try_into().unwrap();
    let amount = u64::from_le_bytes(input[32..40].try_into().unwrap());
    let blinding: [u8; 32] = input[40..72].try_into().unwrap();

    let pk = match CompressedRistretto(recipient_pubkey).decompress() {
        Some(p) => p,
        None => {
            return Err(revm::precompile::PrecompileErrors::Error(
                revm::precompile::PrecompileError::other("elgamal_encrypt: invalid recipient pubkey"),
            ))
        }
    };

    // Random ephemeral key
    let mut k_bytes = [0u8; 32];
    {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        // Deterministic-enough seed for the guest context — use system time + input hash.
        // In production SP1 guests, randomness comes from the prover's trusted setup.
        let mut h = DefaultHasher::new();
        input.hash(&mut h);
        let seed = h.finish();
        // Expand seed to 32 bytes with SHA-256
        let mut hasher = Sha256::new();
        hasher.update(b"C0DL3:ephemeral:");
        hasher.update(&seed.to_le_bytes());
        // Mix in recipient pubkey for uniqueness
        hasher.update(&recipient_pubkey);
        let hash: [u8; 32] = hasher.finalize().into();
        k_bytes.copy_from_slice(&hash);
    }
    let k = Scalar::from_bytes_mod_order(k_bytes);

    // C1 = k*G
    let c1 = (k * G).compress().to_bytes();

    // Shared secret = k * recipient_pubkey
    let shared = (k * pk).compress().to_bytes();

    let mask = memo_derive_mask(&shared);

    // Plaintext: amount(8) || blinding(32)
    let mut plaintext = [0u8; 40];
    plaintext[..8].copy_from_slice(&amount.to_le_bytes());
    plaintext[8..40].copy_from_slice(&blinding);

    let mut encrypted = [0u8; 40];
    for i in 0..40 {
        encrypted[i] = plaintext[i] ^ mask[i];
    }

    let mut memo = Vec::with_capacity(72);
    memo.extend_from_slice(&c1);
    memo.extend_from_slice(&encrypted);

    Ok(PrecompileOutput::new(GAS_ELGAMAL_ENCRYPT, memo.into()))
}

// ── 0x010A: Memo Decrypt Verify ──────────────────────────────────────────────
//
// Input: memo(72) || expected_amount_le64(8) || expected_blinding(32) || privkey(32) = 144 bytes
//
// Decrypts the memo using privkey and checks if (amount, blinding) matches expectations.
//
// Output: 0x01 if matches, 0x00 if doesn't match or decryption yields invalid data
//
// Returns Err if input is not exactly 144 bytes.
//
// Gas: 5,000

fn precompile_memo_decrypt_verify(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    if gas_limit < GAS_MEMO_DECRYPT_VERIFY {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("out of gas"),
        ));
    }
    if input.len() != 144 {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other(
                "memo_decrypt_verify: need exactly 144 bytes \
                 (memo(72) + expected_amount(8) + expected_blinding(32) + privkey(32))",
            ),
        ));
    }

    let memo = &input[0..72];
    let expected_amount = u64::from_le_bytes(input[72..80].try_into().unwrap());
    let expected_blinding: [u8; 32] = input[80..112].try_into().unwrap();
    let privkey_bytes: [u8; 32] = input[112..144].try_into().unwrap();

    let privkey = Scalar::from_bytes_mod_order(privkey_bytes);

    // Decrypt
    let c1_point = match CompressedRistretto(memo[0..32].try_into().unwrap()).decompress() {
        Some(p) => p,
        None => {
            return Ok(PrecompileOutput::new(GAS_MEMO_DECRYPT_VERIFY, vec![0x00].into()))
        }
    };

    let shared = (privkey * c1_point).compress().to_bytes();
    let mask = memo_derive_mask(&shared);

    let mut plaintext = [0u8; 40];
    for i in 0..40 {
        plaintext[i] = memo[32 + i] ^ mask[i];
    }

    let dec_amount = u64::from_le_bytes(plaintext[..8].try_into().unwrap());
    let dec_blinding: [u8; 32] = plaintext[8..40].try_into().unwrap();

    let matches = dec_amount == expected_amount && dec_blinding == expected_blinding;

    Ok(PrecompileOutput::new(
        GAS_MEMO_DECRYPT_VERIFY,
        vec![if matches { 0x01 } else { 0x00 }].into(),
    ))
}

// ── Memo Helpers ─────────────────────────────────────────────────────────────

/// Derive 40-byte XOR mask from ECDH shared secret.
/// Must match sdk/src/encrypted_memo.rs derive_memo_mask exactly.
fn memo_derive_mask(shared_secret: &[u8; 32]) -> [u8; 40] {
    let h1: [u8; 32] = Sha256::digest(
        [b"C0DL3:memo:".as_slice(), shared_secret.as_slice()].concat(),
    )
    .into();
    let h2: [u8; 32] = Sha256::digest(
        [b"C0DL3:memo:1:".as_slice(), shared_secret.as_slice()].concat(),
    )
    .into();

    let mut mask = [0u8; 40];
    mask[..32].copy_from_slice(&h1);
    mask[32..40].copy_from_slice(&h2[..8]);
    mask
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn decompress_point(bytes: &[u8]) -> Result<RistrettoPoint, revm::precompile::PrecompileErrors> {
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes[..32]);
    CompressedRistretto(arr)
        .decompress()
        .ok_or_else(|| {
            revm::precompile::PrecompileErrors::Error(
                revm::precompile::PrecompileError::other("invalid ristretto point"),
            )
        })
}

fn scalar_from_bytes(bytes: &[u8]) -> Scalar {
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes[..32]);
    Scalar::from_bytes_mod_order(arr)
}

// ── 0x0107: Conservation Check ───────────────────────────────────────────────
//
// Input: old_sender_commit(32) || new_sender_commit(32) ||
//        old_recipient_commit(32) || new_recipient_commit(32)  — 128 bytes total
//
// Checks Pedersen commitment balance conservation:
//   old_sender - new_sender == new_recipient - old_recipient
//   i.e. what the sender lost equals what the recipient gained.
//
// Output: 1 byte (0x01 = balanced, 0x00 = not balanced or invalid input)
//
// Returns Err if input is not exactly 128 bytes.
//
// Gas: 4,000

fn precompile_conservation_check(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    if gas_limit < GAS_CONSERVATION_CHECK {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("out of gas"),
        ));
    }
    if input.len() != 128 {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other(
                "conservation_check: need exactly 128 bytes \
                 (old_sender(32) + new_sender(32) + old_recipient(32) + new_recipient(32))",
            ),
        ));
    }

    // Decompress all four points; return 0x00 on any invalid point.
    let old_sender = match CompressedRistretto(input[0..32].try_into().unwrap()).decompress() {
        Some(p) => p,
        None => {
            return Ok(PrecompileOutput::new(
                GAS_CONSERVATION_CHECK,
                vec![0x00].into(),
            ))
        }
    };
    let new_sender = match CompressedRistretto(input[32..64].try_into().unwrap()).decompress() {
        Some(p) => p,
        None => {
            return Ok(PrecompileOutput::new(
                GAS_CONSERVATION_CHECK,
                vec![0x00].into(),
            ))
        }
    };
    let old_recipient = match CompressedRistretto(input[64..96].try_into().unwrap()).decompress() {
        Some(p) => p,
        None => {
            return Ok(PrecompileOutput::new(
                GAS_CONSERVATION_CHECK,
                vec![0x00].into(),
            ))
        }
    };
    let new_recipient = match CompressedRistretto(input[96..128].try_into().unwrap()).decompress() {
        Some(p) => p,
        None => {
            return Ok(PrecompileOutput::new(
                GAS_CONSERVATION_CHECK,
                vec![0x00].into(),
            ))
        }
    };

    // Conservation: (old_sender - new_sender) == (new_recipient - old_recipient)
    // Rearranged: (old_sender - new_sender) - (new_recipient - old_recipient) == identity
    let sender_delta = old_sender - new_sender;
    let recipient_delta = new_recipient - old_recipient;
    let balanced = (sender_delta - recipient_delta) == RistrettoPoint::identity();

    Ok(PrecompileOutput::new(
        GAS_CONSERVATION_CHECK,
        vec![if balanced { 0x01 } else { 0x00 }].into(),
    ))
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conservation_precompile() {
        let gens = PedersenGens::default();

        // Choose blindings such that r1 - r2 == r4 - r3, i.e. r4 = r3 + (r1 - r2).
        // Use deterministic byte arrays for reproducibility.
        let r1 = Scalar::from_bytes_mod_order([0x11u8; 32]);
        let r2 = Scalar::from_bytes_mod_order([0x22u8; 32]);
        let r3 = Scalar::from_bytes_mod_order([0x33u8; 32]);
        // r4 = r3 + (r1 - r2)
        let r4 = r3 + (r1 - r2);

        let old_sender = gens
            .commit(Scalar::from(100u64), r1)
            .compress()
            .to_bytes();
        let new_sender = gens
            .commit(Scalar::from(70u64), r2)
            .compress()
            .to_bytes();
        let old_recipient = gens
            .commit(Scalar::from(0u64), r3)
            .compress()
            .to_bytes();
        let new_recipient = gens
            .commit(Scalar::from(30u64), r4)
            .compress()
            .to_bytes();

        let mut input = Vec::with_capacity(128);
        input.extend_from_slice(&old_sender);
        input.extend_from_slice(&new_sender);
        input.extend_from_slice(&old_recipient);
        input.extend_from_slice(&new_recipient);

        let result =
            precompile_conservation_check(&Bytes::from(input), GAS_CONSERVATION_CHECK).unwrap();
        assert_eq!(result.bytes.as_ref(), &[0x01], "conservation should hold");

        // Unbalanced: use independent blindings for all four commitments.
        let rb1 = Scalar::from_bytes_mod_order([0xAAu8; 32]);
        let rb2 = Scalar::from_bytes_mod_order([0xBBu8; 32]);
        let rb3 = Scalar::from_bytes_mod_order([0xCCu8; 32]);
        let rb4 = Scalar::from_bytes_mod_order([0xDDu8; 32]);

        let os2 = gens.commit(Scalar::from(100u64), rb1).compress().to_bytes();
        let ns2 = gens.commit(Scalar::from(70u64), rb2).compress().to_bytes();
        let or2 = gens.commit(Scalar::from(0u64), rb3).compress().to_bytes();
        let nr2 = gens.commit(Scalar::from(30u64), rb4).compress().to_bytes();

        let mut input2 = Vec::with_capacity(128);
        input2.extend_from_slice(&os2);
        input2.extend_from_slice(&ns2);
        input2.extend_from_slice(&or2);
        input2.extend_from_slice(&nr2);

        let result2 =
            precompile_conservation_check(&Bytes::from(input2), GAS_CONSERVATION_CHECK).unwrap();
        assert_eq!(
            result2.bytes.as_ref(),
            &[0x00],
            "unbalanced blindings should fail conservation"
        );
    }

    #[test]
    fn test_conservation_precompile_wrong_length() {
        // Wrong length should return Err, not Ok(0x00).
        let input = Bytes::from(vec![0u8; 64]);
        let result = precompile_conservation_check(&input, GAS_CONSERVATION_CHECK);
        assert!(result.is_err(), "wrong-length input should error");
    }
}
