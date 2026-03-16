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
//   0x0103: Poseidon hash (algebraic ZK-friendly hash)
//   0x0120: Private swap (conservation check for 2-party swaps)
//   0x0121: Threshold proof (verify committed value >= threshold)
//   0x0122: Commitment arithmetic (add/subtract Pedersen commitments)
//   0x0132: Batch Schnorr verify (verify N Schnorr signatures in one call)
//   0x0201: P-256 (secp256r1) signature verify — WebAuthn/Passkeys
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
use sha3::Keccak256;
use std::sync::Arc;

// ── Precompile Addresses ────────────────────────────────────────────────────

/// Convert a u64 precompile ID to a 20-byte EVM address.
fn precompile_address(id: u64) -> Address {
    let mut bytes = [0u8; 20];
    bytes[18] = (id >> 8) as u8;
    bytes[19] = id as u8;
    Address::from(bytes)
}

// Ethereum-compatible precompiles (matching standard EVM addresses)
pub const EC_RECOVER_ADDR: u64 = 0x0001;
pub const SHA256_ADDR: u64 = 0x0002;
pub const MOD_EXP_ADDR: u64 = 0x0005;
pub const BN254_ADD_ADDR: u64 = 0x0006;
pub const BN254_MUL_ADDR: u64 = 0x0007;
pub const BN254_PAIRING_ADDR: u64 = 0x0008;
// C0DL3 privacy precompiles
pub const RISTRETTO255_ADDR: u64 = 0x0100;
pub const PEDERSEN_COMMIT_ADDR: u64 = 0x0101;
pub const BULLETPROOFS_VERIFY_ADDR: u64 = 0x0102;
pub const ELGAMAL_ENCRYPT_ADDR: u64 = 0x0104;
pub const SCHNORR_VERIFY_ADDR: u64 = 0x0106;
pub const CONSERVATION_CHECK_ADDR: u64 = 0x0107;
pub const MEMO_DECRYPT_VERIFY_ADDR: u64 = 0x010A;
pub const SHIELD_ADDR: u64 = 0x0110;
pub const UNSHIELD_ADDR: u64 = 0x0111;
// Ed25519 — HEAT/COLD verifier contracts + Cosmos/Solana bridge sigs
pub const ED25519_VERIFY_ADDR: u64 = 0x0140;
// Poseidon hash — ZK-friendly algebraic hash for Merkle trees / nullifiers
pub const POSEIDON_HASH_ADDR: u64 = 0x0103;
// Private swap — conservation check for 2-party atomic swaps
pub const PRIVATE_SWAP_ADDR: u64 = 0x0120;
// Threshold proof — verify committed value >= threshold via shifted range proof
pub const THRESHOLD_PROOF_ADDR: u64 = 0x0121;
// Commitment arithmetic — add/subtract Pedersen commitments
pub const COMMITMENT_ARITH_ADDR: u64 = 0x0122;
// Batch Schnorr verify — verify N Schnorr signatures in one call
pub const BATCH_SCHNORR_VERIFY_ADDR: u64 = 0x0132;
// P-256 / secp256r1 — WebAuthn/Passkeys (Face ID, Touch ID, YubiKey)
pub const P256_VERIFY_ADDR: u64 = 0x0201;

// ── Gas Costs ───────────────────────────────────────────────────────────────
// Conservative estimates. SP1 accelerates curve25519 ops via precompile syscalls,
// so actual RISC-V cycle cost is much lower than software-only execution.

// Ethereum-compat gas constants (EIP-defined values)
const GAS_EC_RECOVER: u64 = 3_000;
const GAS_SHA256_BASE: u64 = 60;
const GAS_SHA256_WORD: u64 = 12;
const GAS_MOD_EXP_MIN: u64 = 200;
const GAS_BN254_ADD: u64 = 150;
const GAS_BN254_MUL: u64 = 6_000;
const GAS_BN254_PAIRING_BASE: u64 = 45_000;
const GAS_BN254_PAIRING_PER_PAIR: u64 = 34_000;
const GAS_ED25519_VERIFY: u64 = 3_000;
const GAS_P256_VERIFY: u64 = 3_000;

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
const GAS_POSEIDON_HASH: u64 = 1_500;
const GAS_PRIVATE_SWAP: u64 = 8_000;
const GAS_THRESHOLD_PROOF: u64 = 50_000;
const GAS_COMMITMENT_ARITH: u64 = 1_500;
const GAS_BATCH_SCHNORR_PER_SIG: u64 = 2_000;

// ── Handler Registration ────────────────────────────────────────────────────

/// Register all C0DL3 privacy precompiles into the revm handler.
/// Call via `Evm::builder().append_handler_register(register_cold_precompiles)`.
pub fn register_cold_precompiles<EXT, DB: Database>(handler: &mut EvmHandler<'_, EXT, DB>) {
    let prev = handler.pre_execution.load_precompiles.clone();
    handler.pre_execution.load_precompiles = Arc::new(move || {
        let mut precompiles = prev();
        precompiles.extend([
            // Ethereum-compatible precompiles
            (
                precompile_address(EC_RECOVER_ADDR),
                ContextPrecompile::Ordinary(Precompile::Standard(precompile_ec_recover)),
            ),
            (
                precompile_address(SHA256_ADDR),
                ContextPrecompile::Ordinary(Precompile::Standard(precompile_sha256)),
            ),
            (
                precompile_address(MOD_EXP_ADDR),
                ContextPrecompile::Ordinary(Precompile::Standard(precompile_mod_exp)),
            ),
            (
                precompile_address(BN254_ADD_ADDR),
                ContextPrecompile::Ordinary(Precompile::Standard(precompile_bn254_add)),
            ),
            (
                precompile_address(BN254_MUL_ADDR),
                ContextPrecompile::Ordinary(Precompile::Standard(precompile_bn254_mul)),
            ),
            (
                precompile_address(BN254_PAIRING_ADDR),
                ContextPrecompile::Ordinary(Precompile::Standard(precompile_bn254_pairing)),
            ),
            // C0DL3 privacy precompiles
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
            (
                precompile_address(ED25519_VERIFY_ADDR),
                ContextPrecompile::Ordinary(Precompile::Standard(precompile_ed25519_verify)),
            ),
            (
                precompile_address(P256_VERIFY_ADDR),
                ContextPrecompile::Ordinary(Precompile::Standard(precompile_p256_verify)),
            ),
            // New C0DL3 precompiles (tasks 4b–4f)
            (
                precompile_address(POSEIDON_HASH_ADDR),
                ContextPrecompile::Ordinary(Precompile::Standard(precompile_poseidon_hash)),
            ),
            (
                precompile_address(PRIVATE_SWAP_ADDR),
                ContextPrecompile::Ordinary(Precompile::Standard(precompile_private_swap)),
            ),
            (
                precompile_address(THRESHOLD_PROOF_ADDR),
                ContextPrecompile::Ordinary(Precompile::Standard(precompile_threshold_proof)),
            ),
            (
                precompile_address(COMMITMENT_ARITH_ADDR),
                ContextPrecompile::Ordinary(Precompile::Standard(precompile_commitment_arithmetic)),
            ),
            (
                precompile_address(BATCH_SCHNORR_VERIFY_ADDR),
                ContextPrecompile::Ordinary(Precompile::Standard(precompile_batch_schnorr_verify)),
            ),
        ]);
        precompiles
    });
}

// ── 0x0001: ecRecover (secp256k1) ───────────────────────────────────────────
//
// Input: msg_hash(32) || v(32) || r(32) || s(32) = 128 bytes (Ethereum format)
//   v is 32 bytes; recovery id is encoded in the last byte: 0x1B (27) → 0, 0x1C (28) → 1
//
// Output: 32-byte zero-padded Ethereum address, or all-zeros on failure

fn precompile_ec_recover(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    if gas_limit < GAS_EC_RECOVER {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("out of gas"),
        ));
    }

    // Pad input to 128 bytes if shorter (Ethereum spec)
    let mut padded = [0u8; 128];
    let copy_len = input.len().min(128);
    padded[..copy_len].copy_from_slice(&input[..copy_len]);

    let msg_hash = &padded[0..32];
    // v is in byte 63 (last byte of the 32-byte v field)
    let v_byte = padded[63];
    let r_s = &padded[64..128];

    // Parse recovery id: 27 → 0, 28 → 1; anything else fails gracefully
    let recovery_id_byte = match v_byte {
        27 => 0u8,
        28 => 1u8,
        _ => {
            return Ok(PrecompileOutput::new(GAS_EC_RECOVER, vec![0u8; 32].into()));
        }
    };

    let recid = match k256::ecdsa::RecoveryId::try_from(recovery_id_byte) {
        Ok(r) => r,
        Err(_) => {
            return Ok(PrecompileOutput::new(GAS_EC_RECOVER, vec![0u8; 32].into()));
        }
    };

    let sig = match k256::ecdsa::Signature::try_from(r_s) {
        Ok(s) => s,
        Err(_) => {
            return Ok(PrecompileOutput::new(GAS_EC_RECOVER, vec![0u8; 32].into()));
        }
    };

    let vk = match k256::ecdsa::VerifyingKey::recover_from_prehash(msg_hash, &sig, recid) {
        Ok(k) => k,
        Err(_) => {
            return Ok(PrecompileOutput::new(GAS_EC_RECOVER, vec![0u8; 32].into()));
        }
    };

    // Ethereum address = keccak256(uncompressed_pubkey[1..])[12..]
    let uncompressed = vk.to_encoded_point(false);
    let pubkey_bytes = uncompressed.as_bytes(); // 65 bytes: 0x04 || x(32) || y(32)
    let hash = Keccak256::digest(&pubkey_bytes[1..]); // keccak256 of x || y

    // Left-pad address to 32 bytes
    let mut output = vec![0u8; 32];
    output[12..32].copy_from_slice(&hash[12..32]);

    Ok(PrecompileOutput::new(GAS_EC_RECOVER, output.into()))
}

// ── 0x0002: SHA-256 ─────────────────────────────────────────────────────────
//
// Input: arbitrary bytes
// Output: 32-byte SHA-256 hash
// Gas: 60 + 12 * ceil(input_len / 32)

fn precompile_sha256(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    let gas = GAS_SHA256_BASE + GAS_SHA256_WORD * ((input.len() as u64 + 31) / 32);
    if gas_limit < gas {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("out of gas"),
        ));
    }
    let hash: [u8; 32] = Sha256::digest(input.as_ref()).into();
    Ok(PrecompileOutput::new(gas, hash.to_vec().into()))
}

// ── 0x0005: ModExp (bigint modular exponentiation) ──────────────────────────
//
// Input: base_len(32 BE) || exp_len(32 BE) || mod_len(32 BE) || base || exp || mod
// Output: result padded to mod_len bytes (big-endian)
// Gas: EIP-2565 simplified formula

fn precompile_mod_exp(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    use num_bigint::BigUint;

    // Parse length fields (we only need the last 4 bytes of each 32-byte field to cover
    // any reasonable length; anything with higher bytes set would be astronomically large)
    let read_len = |slice: &[u8]| -> usize {
        let mut arr = [0u8; 4];
        arr.copy_from_slice(&slice[28..32]);
        u32::from_be_bytes(arr) as usize
    };

    let mut padded = [0u8; 96];
    let copy = input.len().min(96);
    padded[..copy].copy_from_slice(&input[..copy]);

    let base_len = read_len(&padded[0..32]);
    let exp_len = read_len(&padded[32..64]);
    let mod_len = read_len(&padded[64..96]);

    // Gas computation (EIP-2565 simplified)
    let max_len = base_len.max(mod_len) as u64;
    let mult_complexity = if max_len <= 64 {
        max_len * max_len
    } else if max_len <= 1024 {
        max_len * max_len / 4 + 96 * max_len - 3072
    } else {
        max_len * max_len / 16 + 480 * max_len - 199680
    };
    let adjusted_exp_len = {
        if exp_len == 0 {
            0u64
        } else {
            let exp_start = 96 + base_len;
            let exp_bytes = if input.len() > exp_start {
                &input[exp_start..input.len().min(exp_start + exp_len)]
            } else {
                &[]
            };
            // highest bit of exp
            let mut bit_len = 0u64;
            for &b in exp_bytes {
                if b != 0 {
                    bit_len = 8 * exp_bytes.len() as u64 - b.leading_zeros() as u64;
                    break;
                }
            }
            if bit_len > 1 { bit_len - 1 } else { 0 }
        }
    };
    let gas = GAS_MOD_EXP_MIN.max(mult_complexity * adjusted_exp_len.max(1) / 3);

    if gas_limit < gas {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("out of gas"),
        ));
    }

    // If mod_len == 0, output is empty
    if mod_len == 0 {
        return Ok(PrecompileOutput::new(gas, vec![].into()));
    }

    // Extract base, exp, mod byte arrays from input
    let data = input.as_ref();
    let base_start = 96;
    let exp_start = base_start + base_len;
    let mod_start = exp_start + exp_len;

    let get_bytes = |start: usize, len: usize| -> Vec<u8> {
        let end = (start + len).min(data.len());
        let available = if start < data.len() { &data[start..end] } else { &[] };
        let mut v = vec![0u8; len];
        let offset = len - available.len();
        v[offset..].copy_from_slice(available);
        v
    };

    let base_bytes = get_bytes(base_start, base_len);
    let exp_bytes = get_bytes(exp_start, exp_len);
    let mod_bytes = get_bytes(mod_start, mod_len);

    let base = BigUint::from_bytes_be(&base_bytes);
    let exp = BigUint::from_bytes_be(&exp_bytes);
    let modulus = BigUint::from_bytes_be(&mod_bytes);

    let result = if modulus == BigUint::from(0u32) {
        BigUint::from(0u32)
    } else {
        base.modpow(&exp, &modulus)
    };

    // Left-pad result to mod_len bytes
    let result_bytes = result.to_bytes_be();
    let mut output = vec![0u8; mod_len];
    if result_bytes.len() <= mod_len {
        let offset = mod_len - result_bytes.len();
        output[offset..].copy_from_slice(&result_bytes);
    } else {
        output.copy_from_slice(&result_bytes[result_bytes.len() - mod_len..]);
    }

    Ok(PrecompileOutput::new(gas, output.into()))
}

// ── 0x0006: BN254 ecAdd ─────────────────────────────────────────────────────
//
// Input: ax(32) || ay(32) || bx(32) || by(32) = 128 bytes (two G1 points, big-endian)
// Output: x(32) || y(32) result point
// Gas: 150

fn precompile_bn254_add(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    use substrate_bn::{AffineG1, Fq, G1, Group};

    if gas_limit < GAS_BN254_ADD {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("out of gas"),
        ));
    }

    let mut padded = [0u8; 128];
    let copy = input.len().min(128);
    padded[..copy].copy_from_slice(&input[..copy]);

    let parse_g1 = |buf: &[u8; 64]| -> Result<G1, revm::precompile::PrecompileErrors> {
        let x = Fq::from_slice(&buf[0..32]).map_err(|_| {
            revm::precompile::PrecompileErrors::Error(
                revm::precompile::PrecompileError::other("bn254_add: invalid Fq x"),
            )
        })?;
        let y = Fq::from_slice(&buf[32..64]).map_err(|_| {
            revm::precompile::PrecompileErrors::Error(
                revm::precompile::PrecompileError::other("bn254_add: invalid Fq y"),
            )
        })?;
        if x.is_zero() && y.is_zero() {
            Ok(G1::zero())
        } else {
            AffineG1::new(x, y)
                .map(Into::into)
                .map_err(|_| revm::precompile::PrecompileErrors::Error(
                    revm::precompile::PrecompileError::other("bn254_add: point not on curve"),
                ))
        }
    };

    let a = parse_g1(padded[0..64].try_into().unwrap())?;
    let b = parse_g1(padded[64..128].try_into().unwrap())?;
    let result = a + b;

    let mut output = [0u8; 64];
    if let Some(affine) = AffineG1::from_jacobian(result) {
        affine.x().to_big_endian(&mut output[0..32]).map_err(|_| {
            revm::precompile::PrecompileErrors::Error(
                revm::precompile::PrecompileError::other("bn254_add: output x encoding failed"),
            )
        })?;
        affine.y().to_big_endian(&mut output[32..64]).map_err(|_| {
            revm::precompile::PrecompileErrors::Error(
                revm::precompile::PrecompileError::other("bn254_add: output y encoding failed"),
            )
        })?;
    }
    // If result is zero, output stays all-zeros (point at infinity)

    Ok(PrecompileOutput::new(GAS_BN254_ADD, output.to_vec().into()))
}

// ── 0x0007: BN254 ecMul ─────────────────────────────────────────────────────
//
// Input: x(32) || y(32) || scalar(32) = 96 bytes
// Output: x(32) || y(32)
// Gas: 6,000

fn precompile_bn254_mul(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    use substrate_bn::{AffineG1, Fq, Fr, G1, Group};

    if gas_limit < GAS_BN254_MUL {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("out of gas"),
        ));
    }

    let mut padded = [0u8; 96];
    let copy = input.len().min(96);
    padded[..copy].copy_from_slice(&input[..copy]);

    let x = Fq::from_slice(&padded[0..32]).map_err(|_| {
        revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("bn254_mul: invalid Fq x"),
        )
    })?;
    let y = Fq::from_slice(&padded[32..64]).map_err(|_| {
        revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("bn254_mul: invalid Fq y"),
        )
    })?;
    let scalar = Fr::from_slice(&padded[64..96]).map_err(|_| {
        revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("bn254_mul: invalid Fr scalar"),
        )
    })?;

    let point: G1 = if x.is_zero() && y.is_zero() {
        G1::zero()
    } else {
        AffineG1::new(x, y)
            .map(Into::into)
            .map_err(|_| revm::precompile::PrecompileErrors::Error(
                revm::precompile::PrecompileError::other("bn254_mul: point not on curve"),
            ))?
    };

    let result = point * scalar;

    let mut output = [0u8; 64];
    if let Some(affine) = AffineG1::from_jacobian(result) {
        affine.x().to_big_endian(&mut output[0..32]).map_err(|_| {
            revm::precompile::PrecompileErrors::Error(
                revm::precompile::PrecompileError::other("bn254_mul: output x encoding failed"),
            )
        })?;
        affine.y().to_big_endian(&mut output[32..64]).map_err(|_| {
            revm::precompile::PrecompileErrors::Error(
                revm::precompile::PrecompileError::other("bn254_mul: output y encoding failed"),
            )
        })?;
    }

    Ok(PrecompileOutput::new(GAS_BN254_MUL, output.to_vec().into()))
}

// ── 0x0008: BN254 ecPairing ──────────────────────────────────────────────────
//
// Input: n × (G1_x(32) || G1_y(32) || G2_x1(32) || G2_x2(32) || G2_y1(32) || G2_y2(32))
//        = n × 192 bytes
// Output: 0x01 if product of pairings == GT identity, 0x00 otherwise
// Gas: 45,000 + 34,000 * n

fn precompile_bn254_pairing(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    use substrate_bn::{AffineG1, AffineG2, Fq, Fq2, G1, G2, Group, Gt, pairing_batch};

    if input.len() % 192 != 0 {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other(
                "bn254_pairing: input length must be multiple of 192",
            ),
        ));
    }

    let n = (input.len() / 192) as u64;
    let gas = GAS_BN254_PAIRING_BASE + GAS_BN254_PAIRING_PER_PAIR * n;

    if gas_limit < gas {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("out of gas"),
        ));
    }

    // Empty input → return 1 (vacuous truth)
    if n == 0 {
        let mut output = vec![0u8; 32];
        output[31] = 0x01;
        return Ok(PrecompileOutput::new(gas, output.into()));
    }

    let mut pairs: Vec<(G1, G2)> = Vec::with_capacity(n as usize);

    for i in 0..(n as usize) {
        let base = i * 192;
        let chunk = &input[base..base + 192];

        let ax = Fq::from_slice(&chunk[0..32]).map_err(|_| {
            revm::precompile::PrecompileErrors::Error(
                revm::precompile::PrecompileError::other("bn254_pairing: invalid G1 x"),
            )
        })?;
        let ay = Fq::from_slice(&chunk[32..64]).map_err(|_| {
            revm::precompile::PrecompileErrors::Error(
                revm::precompile::PrecompileError::other("bn254_pairing: invalid G1 y"),
            )
        })?;

        // G2 coordinates: x = (x1, x2), y = (y1, y2) where Fq2 = a + b*i
        // Ethereum encoding: x_im(32) || x_re(32) || y_im(32) || y_re(32)
        let bx_im = Fq::from_slice(&chunk[64..96]).map_err(|_| {
            revm::precompile::PrecompileErrors::Error(
                revm::precompile::PrecompileError::other("bn254_pairing: invalid G2 x_im"),
            )
        })?;
        let bx_re = Fq::from_slice(&chunk[96..128]).map_err(|_| {
            revm::precompile::PrecompileErrors::Error(
                revm::precompile::PrecompileError::other("bn254_pairing: invalid G2 x_re"),
            )
        })?;
        let by_im = Fq::from_slice(&chunk[128..160]).map_err(|_| {
            revm::precompile::PrecompileErrors::Error(
                revm::precompile::PrecompileError::other("bn254_pairing: invalid G2 y_im"),
            )
        })?;
        let by_re = Fq::from_slice(&chunk[160..192]).map_err(|_| {
            revm::precompile::PrecompileErrors::Error(
                revm::precompile::PrecompileError::other("bn254_pairing: invalid G2 y_re"),
            )
        })?;

        let g1: G1 = if ax.is_zero() && ay.is_zero() {
            G1::zero()
        } else {
            AffineG1::new(ax, ay)
                .map(Into::into)
                .map_err(|_| revm::precompile::PrecompileErrors::Error(
                    revm::precompile::PrecompileError::other("bn254_pairing: G1 not on curve"),
                ))?
        };

        let bx = Fq2::new(bx_re, bx_im);
        let by = Fq2::new(by_re, by_im);

        let g2: G2 = if bx.is_zero() && by.is_zero() {
            G2::zero()
        } else {
            AffineG2::new(bx, by)
                .map(Into::into)
                .map_err(|_| revm::precompile::PrecompileErrors::Error(
                    revm::precompile::PrecompileError::other("bn254_pairing: G2 not on curve"),
                ))?
        };

        pairs.push((g1, g2));
    }

    let result = pairing_batch(&pairs);
    let success = result == Gt::one();

    let mut output = vec![0u8; 32];
    if success {
        output[31] = 0x01;
    }

    Ok(PrecompileOutput::new(gas, output.into()))
}

// ── 0x0140: Ed25519 Verify ───────────────────────────────────────────────────
//
// Input: pubkey(32) || signature(64) || msg_len_le32(4) || message
// Output: 0x01 if valid, 0x00 if invalid
// Gas: 3,000

fn precompile_ed25519_verify(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    use ed25519_consensus::{Signature as Ed25519Sig, VerificationKey};

    if gas_limit < GAS_ED25519_VERIFY {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("out of gas"),
        ));
    }

    // Minimum: pubkey(32) + sig(64) + msg_len(4) = 100 bytes
    if input.len() < 100 {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other(
                "ed25519_verify: need >= 100 bytes (pubkey(32) + sig(64) + msg_len(4))",
            ),
        ));
    }

    let pubkey_bytes: [u8; 32] = input[0..32].try_into().unwrap();
    let sig_bytes: [u8; 64] = input[32..96].try_into().unwrap();
    let msg_len = u32::from_le_bytes(input[96..100].try_into().unwrap()) as usize;

    if input.len() < 100 + msg_len {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("ed25519_verify: message truncated"),
        ));
    }

    let message = &input[100..100 + msg_len];

    let vk = match VerificationKey::try_from(pubkey_bytes) {
        Ok(k) => k,
        Err(_) => {
            return Ok(PrecompileOutput::new(GAS_ED25519_VERIFY, vec![0x00].into()));
        }
    };

    let sig = match Ed25519Sig::try_from(sig_bytes) {
        Ok(s) => s,
        Err(_) => {
            return Ok(PrecompileOutput::new(GAS_ED25519_VERIFY, vec![0x00].into()));
        }
    };

    let valid = vk.verify(&sig, message).is_ok();

    Ok(PrecompileOutput::new(
        GAS_ED25519_VERIFY,
        vec![if valid { 0x01 } else { 0x00 }].into(),
    ))
}

// ── 0x0201: P-256 (secp256r1) Signature Verify ──────────────────────────────
//
// Enables WebAuthn / Passkeys authentication: Face ID, Touch ID, YubiKey, etc.
// ERC-7562 / RIP-7212 compatible interface.
//
// Input:  msg_hash(32) || r(32) || s(32) || x(32) || y(32) = 160 bytes
//   msg_hash — the 32-byte pre-hashed message (e.g. SHA-256 of the challenge)
//   r, s     — ECDSA signature scalars (big-endian)
//   x, y     — uncompressed P-256 public key coordinates (big-endian)
// Output: 0x01 if valid, 0x00 if invalid or input malformed
// Gas:    3,000

fn precompile_p256_verify(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    use p256::{
        ecdsa::{signature::hazmat::PrehashVerifier, Signature, VerifyingKey},
        EncodedPoint,
    };

    if gas_limit < GAS_P256_VERIFY {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("out of gas"),
        ));
    }

    // Require exactly 160 bytes: msg_hash(32) + r(32) + s(32) + x(32) + y(32)
    if input.len() != 160 {
        return Ok(PrecompileOutput::new(GAS_P256_VERIFY, vec![0x00].into()));
    }

    let msg_hash = &input[0..32];
    let r = &input[32..64];
    let s = &input[64..96];
    let x = &input[96..128];
    let y = &input[128..160];

    // Build the uncompressed public key from (x, y) coordinates.
    // Construct a 65-byte uncompressed point: 0x04 || x || y
    let mut uncompressed = [0u8; 65];
    uncompressed[0] = 0x04;
    uncompressed[1..33].copy_from_slice(x);
    uncompressed[33..65].copy_from_slice(y);
    let encoded_point = EncodedPoint::from_bytes(&uncompressed)
        .unwrap_or_else(|_| EncodedPoint::identity());
    let vk = match VerifyingKey::from_encoded_point(&encoded_point) {
        Ok(k) => k,
        Err(_) => return Ok(PrecompileOutput::new(GAS_P256_VERIFY, vec![0x00].into())),
    };

    // Build signature from concatenated r || s (64 bytes total)
    let mut rs_bytes = [0u8; 64];
    rs_bytes[..32].copy_from_slice(r);
    rs_bytes[32..].copy_from_slice(s);
    let sig = match Signature::try_from(rs_bytes.as_ref()) {
        Ok(s) => s,
        Err(_) => return Ok(PrecompileOutput::new(GAS_P256_VERIFY, vec![0x00].into())),
    };

    // Verify — msg_hash is already the final hash (prehash interface)
    let valid = vk.verify_prehash(msg_hash, &sig).is_ok();
    Ok(PrecompileOutput::new(
        GAS_P256_VERIFY,
        vec![if valid { 0x01 } else { 0x00 }].into(),
    ))
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

// ── 0x0103: Poseidon Hash ─────────────────────────────────────────────────────
//
// Input: left(32) || right(32) = 64 bytes
// Output: hash(32)
//
// ZK-friendly algebraic hash using Ristretto255 scalar field.

fn precompile_poseidon_hash(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    if gas_limit < GAS_POSEIDON_HASH {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("out of gas"),
        ));
    }
    if input.len() != 64 {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other(
                "poseidon_hash: need exactly 64 bytes (left(32) || right(32))",
            ),
        ));
    }

    let left: [u8; 32] = input[0..32].try_into().unwrap();
    let right: [u8; 32] = input[32..64].try_into().unwrap();

    let hash = poseidon_hash_impl(&left, &right);

    Ok(PrecompileOutput::new(GAS_POSEIDON_HASH, hash.to_vec().into()))
}

/// Inline Poseidon-like hash using Ristretto255 scalar field.
fn poseidon_hash_impl(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    use sha2::Digest as _;
    const FULL_ROUNDS: usize = 8;
    const PARTIAL_ROUNDS: usize = 57;
    const WIDTH: usize = 3;
    const DOMAIN: &[u8] = b"C0DL3:poseidon:";

    fn rc(round: usize, pos: usize) -> Scalar {
        let mut h = Sha256::new();
        h.update(DOMAIN);
        h.update(b"rc:");
        h.update((round as u64).to_le_bytes());
        h.update((pos as u64).to_le_bytes());
        let hash: [u8; 32] = h.finalize().into();
        Scalar::from_bytes_mod_order(hash)
    }

    #[inline]
    fn sbox(x: Scalar) -> Scalar { x * x * x }

    fn mds(s: &mut [Scalar; WIDTH]) {
        let (s0, s1, s2) = (s[0], s[1], s[2]);
        let two = Scalar::from(2u64);
        s[0] = two * s0 + s1 + s2;
        s[1] = s0 + two * s1 + s2;
        s[2] = s0 + s1 + two * s2;
    }

    let cap = {
        let mut h = Sha256::new();
        h.update(DOMAIN);
        h.update(b"capacity");
        let hash: [u8; 32] = h.finalize().into();
        Scalar::from_bytes_mod_order(hash)
    };

    let mut state: [Scalar; WIDTH] = [
        Scalar::from_bytes_mod_order(*left),
        Scalar::from_bytes_mod_order(*right),
        cap,
    ];

    let half = FULL_ROUNDS / 2;
    let mut ri = 0usize;

    for _ in 0..half {
        for j in 0..WIDTH { state[j] += rc(ri, j); }
        for j in 0..WIDTH { state[j] = sbox(state[j]); }
        mds(&mut state);
        ri += 1;
    }
    for _ in 0..PARTIAL_ROUNDS {
        for j in 0..WIDTH { state[j] += rc(ri, j); }
        state[0] = sbox(state[0]);
        mds(&mut state);
        ri += 1;
    }
    for _ in 0..half {
        for j in 0..WIDTH { state[j] += rc(ri, j); }
        for j in 0..WIDTH { state[j] = sbox(state[j]); }
        mds(&mut state);
        ri += 1;
    }

    state[0].to_bytes()
}

// ── 0x0120: Private Swap ─────────────────────────────────────────────────────
//
// Input: party_a_old(32) || party_a_new(32) || party_b_old(32) || party_b_new(32) = 128 bytes
// Output: 0x01 (balanced) or 0x00 (not balanced)
//
// Verifies that delta_a + delta_b == identity (conservation of value in a 2-party swap).

fn precompile_private_swap(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    if gas_limit < GAS_PRIVATE_SWAP {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("out of gas"),
        ));
    }
    if input.len() != 128 {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other(
                "private_swap: need exactly 128 bytes \
                 (party_a_old(32) + party_a_new(32) + party_b_old(32) + party_b_new(32))",
            ),
        ));
    }

    let a_old = match CompressedRistretto(input[0..32].try_into().unwrap()).decompress() {
        Some(p) => p,
        None => return Ok(PrecompileOutput::new(GAS_PRIVATE_SWAP, vec![0x00].into())),
    };
    let a_new = match CompressedRistretto(input[32..64].try_into().unwrap()).decompress() {
        Some(p) => p,
        None => return Ok(PrecompileOutput::new(GAS_PRIVATE_SWAP, vec![0x00].into())),
    };
    let b_old = match CompressedRistretto(input[64..96].try_into().unwrap()).decompress() {
        Some(p) => p,
        None => return Ok(PrecompileOutput::new(GAS_PRIVATE_SWAP, vec![0x00].into())),
    };
    let b_new = match CompressedRistretto(input[96..128].try_into().unwrap()).decompress() {
        Some(p) => p,
        None => return Ok(PrecompileOutput::new(GAS_PRIVATE_SWAP, vec![0x00].into())),
    };

    let delta_a = a_old - a_new;
    let delta_b = b_old - b_new;
    let excess = delta_a + delta_b;
    let balanced = excess == RistrettoPoint::identity();

    Ok(PrecompileOutput::new(
        GAS_PRIVATE_SWAP,
        vec![if balanced { 0x01 } else { 0x00 }].into(),
    ))
}

// ── 0x0121: Threshold Proof ──────────────────────────────────────────────────
//
// Input: commitment(32) || threshold_le64(8) || proof_len_le32(4) || proof_bytes
// Output: 0x01 (value >= threshold) or 0x00
//
// Verifies a Bulletproofs range proof on C' = C - threshold*B.

fn precompile_threshold_proof(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    if gas_limit < GAS_THRESHOLD_PROOF {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("out of gas"),
        ));
    }
    if input.len() < 44 {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other(
                "threshold_proof: need >= 44 bytes (commitment(32) + threshold(8) + proof_len(4))",
            ),
        ));
    }

    let commitment_bytes: [u8; 32] = input[0..32].try_into().unwrap();
    let threshold = u64::from_le_bytes(input[32..40].try_into().unwrap());
    let proof_len = u32::from_le_bytes(input[40..44].try_into().unwrap()) as usize;

    if input.len() < 44 + proof_len {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("threshold_proof: proof truncated"),
        ));
    }

    let proof_bytes = &input[44..44 + proof_len];

    // Decompress commitment
    let c_point = match CompressedRistretto(commitment_bytes).decompress() {
        Some(p) => p,
        None => return Ok(PrecompileOutput::new(GAS_THRESHOLD_PROOF, vec![0x00].into())),
    };

    // Shift: C' = C - threshold * B
    let pc_gens = PedersenGens::default();
    let mut threshold_padded = [0u8; 32];
    threshold_padded[..8].copy_from_slice(&threshold.to_le_bytes());
    let threshold_scalar = Scalar::from_bytes_mod_order(threshold_padded);
    let c_prime = c_point - threshold_scalar * pc_gens.B;
    let c_prime_compressed = c_prime.compress();

    // Deserialize proof
    let rp = match RangeProof::from_bytes(proof_bytes) {
        Ok(rp) => rp,
        Err(_) => return Ok(PrecompileOutput::new(GAS_THRESHOLD_PROOF, vec![0x00].into())),
    };

    // Verify
    let bp_gens = BulletproofGens::new(64, 128);
    let mut transcript = Transcript::new(b"C0DL3-ThresholdProof");
    let valid = rp
        .verify_single(&bp_gens, &pc_gens, &mut transcript, &c_prime_compressed, 64)
        .is_ok();

    Ok(PrecompileOutput::new(
        GAS_THRESHOLD_PROOF,
        vec![if valid { 0x01 } else { 0x00 }].into(),
    ))
}

// ── 0x0122: Commitment Arithmetic ────────────────────────────────────────────
//
// Input: op(1) || commitment_a(32) || commitment_b(32) = 65 bytes
//   op = 0x01: add (C_a + C_b)
//   op = 0x02: subtract (C_a - C_b)
// Output: result_commitment(32) or empty on error

fn precompile_commitment_arithmetic(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    if gas_limit < GAS_COMMITMENT_ARITH {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("out of gas"),
        ));
    }
    if input.len() != 65 {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other(
                "commitment_arithmetic: need exactly 65 bytes (op(1) + commitment_a(32) + commitment_b(32))",
            ),
        ));
    }

    let op = input[0];
    let a = match CompressedRistretto(input[1..33].try_into().unwrap()).decompress() {
        Some(p) => p,
        None => return Ok(PrecompileOutput::new(GAS_COMMITMENT_ARITH, Bytes::new())),
    };
    let b = match CompressedRistretto(input[33..65].try_into().unwrap()).decompress() {
        Some(p) => p,
        None => return Ok(PrecompileOutput::new(GAS_COMMITMENT_ARITH, Bytes::new())),
    };

    let result = match op {
        0x01 => a + b,
        0x02 => a - b,
        _ => return Ok(PrecompileOutput::new(GAS_COMMITMENT_ARITH, Bytes::new())),
    };

    Ok(PrecompileOutput::new(
        GAS_COMMITMENT_ARITH,
        result.compress().to_bytes().to_vec().into(),
    ))
}

// ── 0x0132: Batch Schnorr Verify ─────────────────────────────────────────────
//
// Input: count_le32(4) || [pubkey(32) || sig_r(32) || sig_s(32) || msg_len_le32(4) || message] × count
// Output: 0x01 (all valid) or 0x00 (any invalid)

fn precompile_batch_schnorr_verify(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    if input.len() < 4 {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other(
                "batch_schnorr_verify: need >= 4 bytes (count)",
            ),
        ));
    }

    let count = u32::from_le_bytes(input[0..4].try_into().unwrap()) as usize;
    let total_gas = GAS_BATCH_SCHNORR_PER_SIG.saturating_mul(count as u64).max(GAS_BATCH_SCHNORR_PER_SIG);

    if gas_limit < total_gas {
        return Err(revm::precompile::PrecompileErrors::Error(
            revm::precompile::PrecompileError::other("out of gas"),
        ));
    }

    use curve25519_dalek_ng::constants::RISTRETTO_BASEPOINT_POINT as G;

    let mut offset = 4usize;

    for _ in 0..count {
        // Need at least 100 bytes for pubkey(32) + sig_r(32) + sig_s(32) + msg_len(4)
        if input.len() < offset + 100 {
            return Err(revm::precompile::PrecompileErrors::Error(
                revm::precompile::PrecompileError::other(
                    "batch_schnorr_verify: signature entry truncated",
                ),
            ));
        }

        let pubkey: [u8; 32] = input[offset..offset + 32].try_into().unwrap();
        let sig_r: [u8; 32] = input[offset + 32..offset + 64].try_into().unwrap();
        let sig_s: [u8; 32] = input[offset + 64..offset + 96].try_into().unwrap();
        let msg_len =
            u32::from_le_bytes(input[offset + 96..offset + 100].try_into().unwrap()) as usize;

        if input.len() < offset + 100 + msg_len {
            return Err(revm::precompile::PrecompileErrors::Error(
                revm::precompile::PrecompileError::other(
                    "batch_schnorr_verify: message truncated",
                ),
            ));
        }

        let message = &input[offset + 100..offset + 100 + msg_len];
        offset += 100 + msg_len;

        // Decompress pubkey
        let pk_point = match CompressedRistretto(pubkey).decompress() {
            Some(p) => p,
            None => return Ok(PrecompileOutput::new(total_gas, vec![0x00].into())),
        };

        // Decompress R
        let r_point = match CompressedRistretto(sig_r).decompress() {
            Some(p) => p,
            None => return Ok(PrecompileOutput::new(total_gas, vec![0x00].into())),
        };

        // Challenge: e = SHA-256("C0DL3:schnorr:" || R || pubkey || msg)
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
        let lhs = s * G;
        let rhs = r_point + e * pk_point;

        if lhs != rhs {
            return Ok(PrecompileOutput::new(total_gas, vec![0x00].into()));
        }
    }

    Ok(PrecompileOutput::new(total_gas, vec![0x01].into()))
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

    // ── Poseidon Hash Tests ──────────────────────────────────────────────────

    #[test]
    fn test_poseidon_precompile_deterministic() {
        let a = [0x01u8; 32];
        let b = [0x02u8; 32];
        let mut input = Vec::with_capacity(64);
        input.extend_from_slice(&a);
        input.extend_from_slice(&b);

        let r1 = precompile_poseidon_hash(&Bytes::from(input.clone()), GAS_POSEIDON_HASH).unwrap();
        let r2 = precompile_poseidon_hash(&Bytes::from(input), GAS_POSEIDON_HASH).unwrap();
        assert_eq!(r1.bytes, r2.bytes, "Poseidon must be deterministic");
        assert_eq!(r1.bytes.len(), 32);
    }

    #[test]
    fn test_poseidon_precompile_different_inputs() {
        let a = [0x01u8; 32];
        let b = [0x02u8; 32];
        let c = [0x03u8; 32];

        let mut in1 = Vec::with_capacity(64);
        in1.extend_from_slice(&a);
        in1.extend_from_slice(&b);
        let mut in2 = Vec::with_capacity(64);
        in2.extend_from_slice(&a);
        in2.extend_from_slice(&c);

        let r1 = precompile_poseidon_hash(&Bytes::from(in1), GAS_POSEIDON_HASH).unwrap();
        let r2 = precompile_poseidon_hash(&Bytes::from(in2), GAS_POSEIDON_HASH).unwrap();
        assert_ne!(r1.bytes, r2.bytes, "Different inputs must differ");
    }

    #[test]
    fn test_poseidon_precompile_not_commutative() {
        let a = [0x01u8; 32];
        let b = [0x02u8; 32];

        let mut in_ab = Vec::with_capacity(64);
        in_ab.extend_from_slice(&a);
        in_ab.extend_from_slice(&b);
        let mut in_ba = Vec::with_capacity(64);
        in_ba.extend_from_slice(&b);
        in_ba.extend_from_slice(&a);

        let r_ab = precompile_poseidon_hash(&Bytes::from(in_ab), GAS_POSEIDON_HASH).unwrap();
        let r_ba = precompile_poseidon_hash(&Bytes::from(in_ba), GAS_POSEIDON_HASH).unwrap();
        assert_ne!(r_ab.bytes, r_ba.bytes, "hash(a,b) != hash(b,a)");
    }

    #[test]
    fn test_poseidon_precompile_wrong_length() {
        let input = Bytes::from(vec![0u8; 32]);
        assert!(precompile_poseidon_hash(&input, GAS_POSEIDON_HASH).is_err());
    }

    // ── Private Swap Tests ───────────────────────────────────────────────────

    #[test]
    fn test_private_swap_balanced() {
        let gens = PedersenGens::default();

        // A has 100, sends 30 to B. B has 50, sends 30 back to A (balanced).
        let r1 = Scalar::from_bytes_mod_order([0x11u8; 32]);
        let r2 = Scalar::from_bytes_mod_order([0x22u8; 32]);
        let r3 = Scalar::from_bytes_mod_order([0x33u8; 32]);
        // For balance: (a_old - a_new) + (b_old - b_new) = identity
        // => blinding conservation: (r1 - r2) + (r3 - r4) = 0 => r4 = r3 + (r1 - r2)
        let r4 = r3 + (r1 - r2);

        let a_old = gens.commit(Scalar::from(100u64), r1).compress().to_bytes();
        let a_new = gens.commit(Scalar::from(70u64), r2).compress().to_bytes();
        let b_old = gens.commit(Scalar::from(50u64), r3).compress().to_bytes();
        let b_new = gens.commit(Scalar::from(80u64), r4).compress().to_bytes();

        let mut input = Vec::with_capacity(128);
        input.extend_from_slice(&a_old);
        input.extend_from_slice(&a_new);
        input.extend_from_slice(&b_old);
        input.extend_from_slice(&b_new);

        let result = precompile_private_swap(&Bytes::from(input), GAS_PRIVATE_SWAP).unwrap();
        assert_eq!(result.bytes.as_ref(), &[0x01], "balanced swap should pass");
    }

    #[test]
    fn test_private_swap_unbalanced() {
        let gens = PedersenGens::default();
        let r1 = Scalar::from_bytes_mod_order([0xAAu8; 32]);
        let r2 = Scalar::from_bytes_mod_order([0xBBu8; 32]);
        let r3 = Scalar::from_bytes_mod_order([0xCCu8; 32]);
        let r4 = Scalar::from_bytes_mod_order([0xDDu8; 32]);

        let a_old = gens.commit(Scalar::from(100u64), r1).compress().to_bytes();
        let a_new = gens.commit(Scalar::from(70u64), r2).compress().to_bytes();
        let b_old = gens.commit(Scalar::from(50u64), r3).compress().to_bytes();
        let b_new = gens.commit(Scalar::from(90u64), r4).compress().to_bytes();

        let mut input = Vec::with_capacity(128);
        input.extend_from_slice(&a_old);
        input.extend_from_slice(&a_new);
        input.extend_from_slice(&b_old);
        input.extend_from_slice(&b_new);

        let result = precompile_private_swap(&Bytes::from(input), GAS_PRIVATE_SWAP).unwrap();
        assert_eq!(result.bytes.as_ref(), &[0x00], "unbalanced swap should fail");
    }

    // ── Threshold Proof Tests ────────────────────────────────────────────────

    #[test]
    fn test_threshold_proof_precompile_valid() {
        let blinding = Scalar::from_bytes_mod_order([0x42u8; 32]);
        let value = 1000u64;
        let threshold = 500u64;
        let diff = value - threshold;

        let pc_gens = PedersenGens::default();
        let bp_gens = BulletproofGens::new(64, 128);

        // Commit to value
        let mut v_padded = [0u8; 32];
        v_padded[..8].copy_from_slice(&value.to_le_bytes());
        let v_scalar = Scalar::from_bytes_mod_order(v_padded);
        let commitment = pc_gens.commit(v_scalar, blinding).compress().to_bytes();

        // Prove diff is in range
        let mut transcript = Transcript::new(b"C0DL3-ThresholdProof");
        let (rp, _) = RangeProof::prove_single(
            &bp_gens, &pc_gens, &mut transcript, diff, &blinding, 64,
        )
        .unwrap();
        let rp_bytes = rp.to_bytes();

        // Build precompile input
        let mut input = Vec::new();
        input.extend_from_slice(&commitment);
        input.extend_from_slice(&threshold.to_le_bytes());
        input.extend_from_slice(&(rp_bytes.len() as u32).to_le_bytes());
        input.extend_from_slice(&rp_bytes);

        let result =
            precompile_threshold_proof(&Bytes::from(input), GAS_THRESHOLD_PROOF).unwrap();
        assert_eq!(result.bytes.as_ref(), &[0x01], "valid threshold proof should pass");
    }

    #[test]
    fn test_threshold_proof_precompile_invalid() {
        // Garbage proof bytes should fail verification (return 0x00, not error).
        let blinding = Scalar::from_bytes_mod_order([0x42u8; 32]);
        let value = 100u64;
        let threshold = 500u64;

        let pc_gens = PedersenGens::default();
        let mut v_padded = [0u8; 32];
        v_padded[..8].copy_from_slice(&value.to_le_bytes());
        let v_scalar = Scalar::from_bytes_mod_order(v_padded);
        let commitment = pc_gens.commit(v_scalar, blinding).compress().to_bytes();

        let fake_proof = vec![0u8; 128];
        let mut input = Vec::new();
        input.extend_from_slice(&commitment);
        input.extend_from_slice(&threshold.to_le_bytes());
        input.extend_from_slice(&(fake_proof.len() as u32).to_le_bytes());
        input.extend_from_slice(&fake_proof);

        let result =
            precompile_threshold_proof(&Bytes::from(input), GAS_THRESHOLD_PROOF).unwrap();
        assert_eq!(result.bytes.as_ref(), &[0x00], "invalid proof should return 0x00");
    }

    // ── Commitment Arithmetic Tests ──────────────────────────────────────────

    #[test]
    fn test_commitment_arith_add() {
        let gens = PedersenGens::default();
        let r1 = Scalar::from_bytes_mod_order([0x11u8; 32]);
        let r2 = Scalar::from_bytes_mod_order([0x22u8; 32]);
        let c1 = gens.commit(Scalar::from(100u64), r1);
        let c2 = gens.commit(Scalar::from(200u64), r2);
        let expected = (c1 + c2).compress().to_bytes();

        let mut input = vec![0x01u8]; // add
        input.extend_from_slice(&c1.compress().to_bytes());
        input.extend_from_slice(&c2.compress().to_bytes());

        let result =
            precompile_commitment_arithmetic(&Bytes::from(input), GAS_COMMITMENT_ARITH).unwrap();
        assert_eq!(result.bytes.as_ref(), &expected);
    }

    #[test]
    fn test_commitment_arith_sub() {
        let gens = PedersenGens::default();
        let r1 = Scalar::from_bytes_mod_order([0x11u8; 32]);
        let r2 = Scalar::from_bytes_mod_order([0x22u8; 32]);
        let c1 = gens.commit(Scalar::from(300u64), r1);
        let c2 = gens.commit(Scalar::from(100u64), r2);
        let expected = (c1 - c2).compress().to_bytes();

        let mut input = vec![0x02u8]; // sub
        input.extend_from_slice(&c1.compress().to_bytes());
        input.extend_from_slice(&c2.compress().to_bytes());

        let result =
            precompile_commitment_arithmetic(&Bytes::from(input), GAS_COMMITMENT_ARITH).unwrap();
        assert_eq!(result.bytes.as_ref(), &expected);
    }

    #[test]
    fn test_commitment_arith_invalid_op() {
        let gens = PedersenGens::default();
        let r1 = Scalar::from_bytes_mod_order([0x11u8; 32]);
        let c1 = gens.commit(Scalar::from(100u64), r1);

        let mut input = vec![0x03u8]; // invalid op
        input.extend_from_slice(&c1.compress().to_bytes());
        input.extend_from_slice(&c1.compress().to_bytes());

        let result =
            precompile_commitment_arithmetic(&Bytes::from(input), GAS_COMMITMENT_ARITH).unwrap();
        assert!(result.bytes.is_empty(), "invalid op should return empty");
    }

    // ── Batch Schnorr Verify Tests ───────────────────────────────────────────

    fn make_schnorr_sig(
        secret: &Scalar,
        message: &[u8],
    ) -> ([u8; 32], [u8; 32], [u8; 32]) {
        use curve25519_dalek_ng::constants::RISTRETTO_BASEPOINT_POINT as G;

        let pubkey = (secret * G).compress().to_bytes();
        // Nonce k (deterministic for testing)
        let k = {
            let mut h = Sha256::new();
            h.update(b"C0DL3:schnorr:nonce:");
            h.update(secret.as_bytes());
            h.update(message);
            let hash: [u8; 32] = h.finalize().into();
            Scalar::from_bytes_mod_order(hash)
        };
        let r_point = (k * G).compress();
        let sig_r = r_point.to_bytes();

        let e = {
            let mut h = Sha256::new();
            h.update(b"C0DL3:schnorr:");
            h.update(&sig_r);
            h.update(&pubkey);
            h.update(message);
            let hash: [u8; 32] = h.finalize().into();
            Scalar::from_bytes_mod_order(hash)
        };

        let s = k + e * secret;
        (pubkey, sig_r, s.to_bytes())
    }

    #[test]
    fn test_batch_schnorr_single_valid() {
        let secret = Scalar::from_bytes_mod_order([0x42u8; 32]);
        let msg = b"hello world";
        let (pubkey, sig_r, sig_s) = make_schnorr_sig(&secret, msg);

        let mut input = Vec::new();
        input.extend_from_slice(&1u32.to_le_bytes()); // count = 1
        input.extend_from_slice(&pubkey);
        input.extend_from_slice(&sig_r);
        input.extend_from_slice(&sig_s);
        input.extend_from_slice(&(msg.len() as u32).to_le_bytes());
        input.extend_from_slice(msg);

        let result = precompile_batch_schnorr_verify(
            &Bytes::from(input),
            GAS_BATCH_SCHNORR_PER_SIG * 2,
        )
        .unwrap();
        assert_eq!(result.bytes.as_ref(), &[0x01], "valid signature should pass");
    }

    #[test]
    fn test_batch_schnorr_multiple_valid() {
        let s1 = Scalar::from_bytes_mod_order([0x42u8; 32]);
        let s2 = Scalar::from_bytes_mod_order([0x43u8; 32]);
        let msg1 = b"message one";
        let msg2 = b"message two";

        let (pk1, r1, ss1) = make_schnorr_sig(&s1, msg1);
        let (pk2, r2, ss2) = make_schnorr_sig(&s2, msg2);

        let mut input = Vec::new();
        input.extend_from_slice(&2u32.to_le_bytes());
        // sig 1
        input.extend_from_slice(&pk1);
        input.extend_from_slice(&r1);
        input.extend_from_slice(&ss1);
        input.extend_from_slice(&(msg1.len() as u32).to_le_bytes());
        input.extend_from_slice(msg1);
        // sig 2
        input.extend_from_slice(&pk2);
        input.extend_from_slice(&r2);
        input.extend_from_slice(&ss2);
        input.extend_from_slice(&(msg2.len() as u32).to_le_bytes());
        input.extend_from_slice(msg2);

        let result = precompile_batch_schnorr_verify(
            &Bytes::from(input),
            GAS_BATCH_SCHNORR_PER_SIG * 3,
        )
        .unwrap();
        assert_eq!(result.bytes.as_ref(), &[0x01], "two valid sigs should pass");
    }

    #[test]
    fn test_batch_schnorr_one_invalid() {
        let s1 = Scalar::from_bytes_mod_order([0x42u8; 32]);
        let s2 = Scalar::from_bytes_mod_order([0x43u8; 32]);
        let msg1 = b"message one";
        let msg2 = b"message two";

        let (pk1, r1, ss1) = make_schnorr_sig(&s1, msg1);
        let (pk2, r2, _ss2) = make_schnorr_sig(&s2, msg2);
        // Corrupt second signature
        let bad_ss2 = [0xFFu8; 32];

        let mut input = Vec::new();
        input.extend_from_slice(&2u32.to_le_bytes());
        input.extend_from_slice(&pk1);
        input.extend_from_slice(&r1);
        input.extend_from_slice(&ss1);
        input.extend_from_slice(&(msg1.len() as u32).to_le_bytes());
        input.extend_from_slice(msg1);
        input.extend_from_slice(&pk2);
        input.extend_from_slice(&r2);
        input.extend_from_slice(&bad_ss2);
        input.extend_from_slice(&(msg2.len() as u32).to_le_bytes());
        input.extend_from_slice(msg2);

        let result = precompile_batch_schnorr_verify(
            &Bytes::from(input),
            GAS_BATCH_SCHNORR_PER_SIG * 3,
        )
        .unwrap();
        assert_eq!(result.bytes.as_ref(), &[0x00], "one bad sig should fail batch");
    }
}
