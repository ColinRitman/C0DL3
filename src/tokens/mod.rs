// zkC0DL3 Token Modules
//
// Two tokens exist in the C0DL3 ecosystem:
//
//   HEAT (ERC-20) — Gas token. Minted by burning XFG on Fuego (Winterfell STARK proof).
//                   Used for gas fees and validator staking on C0DL3 L3.
//                   Block provers earn HEAT (subsidy + gas fees) per proved block.
//
//   CD (ERC-1155)  — COLDAO governance token. Earned by locking XFG on Fuego.
//                   8 tiers (amount × time). Interest-only (XFG principal returns after lock).
//                   Grants DAO voting power over protocol parameters.
//                   CD holders govern prover subsidy levels, proof window, etc.
//
// See: src/economics.rs for HEAT subsidy schedule and prover reward mechanics.

pub mod coldao;

pub use coldao::{
    CdTierInfo, CdDeposit, ColdaoLedger, ColdaoManager,
    CD_TIER_INTEREST, LEGACY_CUTOFF_TIMESTAMP, CD_DECIMALS,
};
