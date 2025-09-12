# Fwei : zkC0DL3 Native Gas Unit using Fuego Ξmbers (HEAT)

## Overview

**Fwei** is Fuego's native gas unit, designed specifically for the HEAT ecosystem. It provides a practical and intuitive way to express gas prices and transaction costs in the Fuego blockchain.

## Unit Definitions

### Fwei (Fuego Wei)
- **1 fwei = 0.001 HEAT** (3rd decimal atomic unit of HEAT)
- **1 fwei = 1,000,000,000,000,000 wei** (10^15 wei)
- **1 fwei = 1,000,000 Gwei** (1 Million Gwei)

### HEAT Token
- **1 HEAT = 1,000 fwei**
- **1 HEAT = 1,000,000,000,000,000,000 wei** (10^18 wei)
- **1 HEAT = $0.00001** (based on 1 XFG = $100, 10M HEAT = $100)

## Fwei vs Gwei Comparison

### Conversion Rate
```
1 fwei = 1,000,000 Gwei
1 Gwei = 0.000001 fwei
```

### Examples
- **20 Gwei = 0.00002 fwei**
- **420 fwei = 420,000,000 Gwei**
- **1,000,000 Gwei = 1 fwei**

## Transaction Cost Analysis

### C0DL3 Standard Transaction
- **Gas Price**: 1 fwei
- **Gas Limit**: 21,000 gas (standard transaction)
- **Transaction Cost**: 21 HEAT
- **USD Cost**: $0.00021 (0.00021 cents)

### Gas Price Scenarios

| Gas Price | HEAT Cost | USD Cost | Description |
|-----------|-----------|----------|-------------|
| 0.01 fwei | 0.21 HEAT | $0.000002 | Very Low |
| 0.05 fwei | 1.05 HEAT | $0.000005 | Low |
| 0.10 fwei | 2.10 HEAT | $0.000010 | Medium |
| 0.50 fwei | 10.50 HEAT | $0.000050 | High |
| 1.00 fwei | 21.00 HEAT | $0.000210 | Premium |
| 2.00 fwei | 42.00 HEAT | $0.000420 | Ultra Premium |

## Cost Comparison with ETH

### ETH Transaction Costs (at $3,000/ETH)
- **ETH (20 Gwei)**: $1.26
- **ETH (50 Gwei)**: $3.15
- **ETH (100 Gwei)**: $6.30
- **ETH (200 Gwei)**: $12.60

### C0DL3 Cost Advantage
- **C0DL3 is 6,000x cheaper** than ETH (20 Gwei)
- **C0DL3 is 15,000x cheaper** than ETH (50 Gwei)
- **C0DL3 is 30,000x cheaper** than ETH (100 Gwei)
- **C0DL3 is 60,000x cheaper** than ETH (200 Gwei)

## HEAT Pricing Model

### XFG Relationship
- **1 XFG = $100**
- **10M HEAT = $100**
- **1 HEAT = $0.00001**

### HEAT Value Reference
- **10K HEAT = $0.10**
- **100K HEAT = $1.00**
- **1M HEAT = $10.00**
- **10M HEAT = $100.00**

## Why Fwei Makes Sense

### 1. HEAT Supply Optimization
- **HEAT Supply**: ~69 trillion tokens
- **ETH Supply**: ~120 million tokens
- **Supply Ratio**: ~575,000x
- **Fwei/Gwei Ratio**: 1,000,000x (appropriate for larger supply)

### 2. Practical Usage
- **Intuitive**: 420 fwei = 0.420 HEAT (matching amounts)
- **Human Readable**: Easy to understand and calculate
- **Cost Effective**: Extremely low transaction fees

### 3. Economic Model
- **ETH**: Small supply, high value per unit
- **HEAT**: Large supply, appropriate fwei scale
- **Conversion**: 1 fwei = 1M Gwei reflects economic differences

## Technical Implementation

### FuegoUnit Enum
```rust
pub enum FuegoUnit {
    Heat(u64),    // 18 decimal places
    Fwei(u64),    // 3rd decimal atomic unit of HEAT
    Wei(u64),     // Base unit
}
```

### Conversion Functions
```rust
// Convert fwei to HEAT
pub fn fwei_to_heat(fwei: u64) -> f64 {
    fwei as f64 / 1_000.0
}

// Convert fwei to wei
pub fn fwei_to_wei(fwei: u64) -> u64 {
    fwei * 1_000_000_000_000_000
}

// Convert HEAT to fwei
pub fn heat_to_fwei(heat: f64) -> u64 {
    (heat * 1_000.0) as u64
}
```

### Gas Configuration
```rust
pub struct FuegoGasConfig {
    pub gas_price_fwei: u64,  // Gas price in fwei
    pub gas_limit: u64,       // Gas limit
}

impl Default for FuegoGasConfig {
    fn default() -> Self {
        Self {
            gas_price_fwei: 1,  // 1 fwei (0.001 HEAT per gas unit)
            gas_limit: 21000,   // Standard gas limit
        }
    }
}
```

## Benefits of Fwei

### 1. Cost Efficiency
- **Extremely low fees**: $0.00021 per transaction
- **Predictable pricing**: Fixed fwei-based fees
- **Competitive**: Cheaper than traditional banking

### 2. User Experience
- **Simple math**: 420 fwei = 0.420 HEAT
- **Intuitive**: Easy to understand gas pricing
- **Transparent**: Clear cost structure

### 3. Ecosystem Fit
- **HEAT Integration**: Natural fit for HEAT ecosystem
- **Fuego Branding**: Unique Fuego unit system
- **Scalable**: Appropriate for HEAT's large supply

## Comparison with Traditional Finance

| System | Cost Range |
|--------|------------|
| **C0DL3** | **$0.00021** (0.00021 cents) |
| Bank transfer | $0.25 - $3.00 |
| Credit card | $0.10 - $0.30 + 2-3% |
| Wire transfer | $15 - $50 |
| PayPal | $0.30 + 2.9% |
| ETH (20 Gwei) | $1.26 |
| ETH (100 Gwei) | $6.30 |

## Conclusion

Fwei provides a practical, cost-effective, and intuitive gas unit system for the Fuego ecosystem. With transaction costs of just $0.00021 (0.00021 cents), C0DL3 offers massive cost savings compared to ETH while maintaining the benefits of blockchain technology.

The 1 fwei = 1,000,000 Gwei conversion rate appropriately reflects the economic differences between HEAT and ETH, making Fwei the perfect gas unit for the Fuego ecosystem.

---

*For more information about C0DL3 and the Fuego ecosystem, visit our [main documentation](README.md).*
