// Automatic shield-transfer-unshield flow.
//
// The auto-shield flow is the SDK's killer feature: user says "send 100 to Bob"
// and behind the scenes, the SDK:
//   1. Shields from AA wallet -> shielded pool (splits amount, adds decoys)
//   2. Private transfer (re-commit to recipient's stealth pubkey)
//   3. Schedules delayed unshield to recipient's AA wallet (random delay)

use curve25519_dalek_ng::scalar::Scalar;
use serde::{Deserialize, Serialize};

use crate::shield::{create_shield_request, ShieldRequest};

/// Configuration for the auto-shield flow.
#[derive(Debug, Clone)]
pub struct AutoShieldConfig {
    pub min_delay_blocks: u64,
    pub max_delay_blocks: u64,
    pub enable_splitting: bool,
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

/// A planned auto-shield operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoShieldPlan {
    pub shield_phase: Vec<ShieldRequest>,
    pub transfer_notes: Vec<TransferNote>,
    pub unshield_at_block: u64,
    pub recipient: String,
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
pub fn plan_auto_shield(
    amount: u64,
    recipient_pubkey: [u8; 32],
    current_block: u64,
    config: &AutoShieldConfig,
) -> anyhow::Result<AutoShieldPlan> {
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

/// Split an amount into 2 random parts.
pub fn split_amount(amount: u64) -> Vec<u64> {
    if amount <= 2 {
        return vec![amount];
    }
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

    fn random_pubkey() -> [u8; 32] {
        let s = random_blinding();
        (s * PedersenGens::default().B).compress().to_bytes()
    }

    #[test]
    fn test_plan_auto_shield_basic() {
        let config = AutoShieldConfig {
            min_delay_blocks: 5,
            max_delay_blocks: 10,
            enable_splitting: false,
            decoy_count: 0,
        };
        let pubkey = random_pubkey();
        let plan = plan_auto_shield(1000, pubkey, 100, &config).unwrap();

        assert_eq!(plan.shield_phase.len(), 1);
        assert_eq!(plan.transfer_notes.len(), 1);
        assert_eq!(plan.total_amount, 1000);
        assert!(plan.unshield_at_block >= 105);
        assert!(plan.unshield_at_block <= 110);
    }

    #[test]
    fn test_plan_auto_shield_with_splitting() {
        let config = AutoShieldConfig {
            min_delay_blocks: 5,
            max_delay_blocks: 20,
            enable_splitting: true,
            decoy_count: 0,
        };
        let pubkey = random_pubkey();
        let plan = plan_auto_shield(1000, pubkey, 100, &config).unwrap();

        let total: u64 = plan.transfer_notes.iter().map(|n| n.amount).sum();
        assert_eq!(total, 1000);
    }

    #[test]
    fn test_plan_auto_shield_with_decoys() {
        let config = AutoShieldConfig {
            min_delay_blocks: 5,
            max_delay_blocks: 20,
            enable_splitting: false,
            decoy_count: 3,
        };
        let pubkey = random_pubkey();
        let plan = plan_auto_shield(1000, pubkey, 100, &config).unwrap();

        // 1 real + 3 decoys
        assert_eq!(plan.shield_phase.len(), 4);
        // Only 1 real transfer note
        assert_eq!(plan.transfer_notes.len(), 1);
    }

    #[test]
    fn test_split_amount() {
        let parts = split_amount(1000);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0] + parts[1], 1000);
        assert!(parts[0] > 0);
        assert!(parts[1] > 0);
    }

    #[test]
    fn test_split_amount_small() {
        assert_eq!(split_amount(1), vec![1]);
        assert_eq!(split_amount(0), vec![0]);
    }
}
