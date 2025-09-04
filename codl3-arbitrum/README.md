# CODL3 Arbitrum Orbit Implementation

A complete implementation of the CODL3 protocol running on Arbitrum Orbit (AnyTrust), featuring dual mining with Fuego, validator staking, and cross-chain bridging.

## 🚀 Features

### ✅ Core Components Implemented

- **Dual Mining System**: Integrated Fuego PoW mining with CODL3 gas fee mining
- **Validator Staking**: 80B HEAT minimum stake requirement with slashing mechanisms
- **Bridge Integration**: Full Arbitrum Orbit bridge for L1 ↔ L3 transfers
- **AnyTrust Consensus**: 21-validator committee with fraud proofs
- **Block Production**: Automated block creation with L1 data availability
- **Transaction Processing**: Complete transaction lifecycle management
- **Fraud Proof System**: Challenge-response mechanism for invalid states

### 🔧 Technical Architecture

```
┌─────────────────────────────────────────────────┐
│                CODL3 Arbitrum Node              │
├─────────────────────────────────────────────────┤
│ • P2P Network Layer                            │
│ • RPC API Server                               │
│ • Block Synchronization                        │
│ • Consensus Engine (AnyTrust)                  │
│ • Bridge Monitoring                            │
│ • Dual Mining Coordinator                      │
│ • L1 Monitoring & Confirmations                │
├─────────────────────────────────────────────────┤
│ Validator Staking System                       │
├─────────────────────────────────────────────────┤
│ • Minimum Stake: 80B HEAT                      │
│ • Max Validators: 21                           │
│ • Slashing: 50% for violations                 │
│ • Reward Distribution                          │
├─────────────────────────────────────────────────┤
│ Arbitrum Bridge Integration                    │
├─────────────────────────────────────────────────┤
│ • L1 → L3 Deposits                             │
│ • L3 → L1 Withdrawals                          │
│ • Gas Price Optimization                       │
│ • Transaction Status Tracking                  │
└─────────────────────────────────────────────────┘
```

## 🏃‍♂️ Quick Start

### Prerequisites

- Rust 1.70+
- Node.js 16+ (for Fuego integration)
- Arbitrum Sepolia RPC access
- Ethereum Sepolia RPC access

### Installation

```bash
# Clone the repository
cd codl3-arbitrum

# Build the project
cargo build --release

# Run with default configuration
cargo run --release -- --help
```

### Configuration

The node supports extensive configuration through command-line arguments, including customizable block time:

```bash
cargo run --release -- \
  --l1-rpc-url https://sepolia.infura.io/v3/YOUR_KEY \
  --l2-rpc-url https://sepolia-rollup.arbitrum.io/rpc \
  --fuego-rpc-url http://localhost:8080 \
  --block-time 30 \
  --validator-address 0x1234... \
  --gas-token-address 0xabcd...
```

## 📊 Validator Staking System

### Staking Requirements

- **Minimum Stake**: 80,000,000,000 HEAT tokens (80B HEAT)
- **Maximum Validators**: 21 active validators
- **Slashing Penalty**: 50% of stake for violations
- **Reward Distribution**: Every 100 blocks

### Validator Operations

```rust
// Stake HEAT tokens to become a validator
node.stake_validator("0x1234...".to_string(), 80_000_000_000).await?;

// Get validator information
let validator = node.get_validator_info("0x1234...").await;

// Distribute rewards to all active validators
node.distribute_validator_rewards(1000000).await?;
```

## 🌉 Bridge Integration

### L1 ↔ L3 Transfers

The bridge system handles secure token transfers between Ethereum L1 and CODL3 L3:

```rust
// Deposit tokens from L1 to L3
let deposit_tx = bridge.deposit_to_l2(U256::from(1000000), recipient_address).await?;

// Withdraw tokens from L3 to L1
let withdraw_tx = bridge.withdraw_to_l1(U256::from(500000), recipient_address).await?;

// Check transaction status
let status = bridge.get_bridge_transaction(&tx_hash).await;
```

### Bridge Features

- **Automatic Confirmation**: Monitors L1 confirmations
- **Gas Optimization**: Estimates optimal gas prices
- **Status Tracking**: Complete transaction lifecycle
- **Error Handling**: Robust failure recovery

## 🔥 HEAT Token Minting System

### Recommended Minting Path: L3 → L1 Bridge

When the HEAT token contract is deployed on L1 (Ethereum), the recommended minting path follows this secure flow:

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   CODL3 L3      │    │ Arbitrum Bridge │    │   HEAT Token    │
│   Activity      │    │   L3 → L1       │    │   L1 Contract   │
│                 │    │                 │    │                 │
│ • Gas Fees      │───▶│ • Batch Proofs  │───▶│ • mintFromCODL3 │
│ • Block Rewards │    │ • Validator Sig  │    │ • Proof Verify  │
│ • Validator Pay │    │ • Bridge Message │    │ • HEAT Mint     │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

### Minting Sources

1. **GasFees**: Transaction gas fees collected on L3
2. **BlockRewards**: Consensus block production rewards
3. **ValidatorRewards**: Staking and validation incentives
4. **XfgBurn**: XFG token burns converted to HEAT

### Implementation Flow

```rust
// 1. Queue minting request on L3
let request_id = node.queue_heat_mint(
    recipient_address,
    amount,
    MintSource::GasFees
).await?;

// 2. Process minting batch via bridge
node.process_heat_mint_batch().await?;

// 3. Monitor L1 for confirmation
// Bridge automatically submits proof to L1 HEAT contract
```

### Security Features

- **Batch Processing**: Multiple mints batched for efficiency
- **Validator Consensus**: Multi-signature validation required
- **Bridge Security**: Arbitrum's native bridge security
- **Proof Verification**: Cryptographic proof validation on L1
- **Rate Limiting**: Controlled minting to prevent abuse

## ⚡ Dual Mining Implementation

### Mining Components

1. **Fuego Mining**: PoW mining for XFG rewards
2. **CODL3 Mining**: Gas fee collection and block production
3. **Merge Mining**: Unified mining coordination

### Mining Rewards

- **Fuego Rewards**: XFG block rewards + transaction fees
- **CODL3 Rewards**: HEAT gas fees + eldernode fees
- **L1 Fee Shares**: Portion of L1 data availability costs
- **Validator Shares**: Distributed among active validators

## 🔒 Security Features

### Fraud Proof System

- **Challenge Period**: 7 days for fraud challenges
- **Multi-validator Consensus**: 21 validator committee
- **Economic Incentives**: Rewards for valid proofs, penalties for invalid
- **State Verification**: Complete state transition validation

### Slashing Conditions

1. **Double Signing**: 50% stake slashed
2. **Invalid State**: 25% stake slashed
3. **Inactivity**: 10% stake slashed per day

## 📈 Performance Metrics

### Block Production

- **Block Time**: 30 seconds (configurable via --block-time)
- **Gas Limit**: 30M gas per block
- **L1 Confirmations**: 12 blocks required

### Network Statistics

- **Throughput**: 200+ TPS (with 30s blocks)
- **Finality**: ~1 minute
- **Data Availability**: Celestia integration
- **Cross-chain Latency**: ~5-10 minutes

## 🔧 API Reference

### Node Operations

```rust
// Start the node
node.start().await?;

// Add transaction to mempool
node.add_transaction(transaction).await?;

// Get current node state
let state = node.get_state().await;

// Get mining rewards
let rewards = node.get_mining_rewards().await;
```

### Validator Management

```rust
// Stake validator
node.stake_validator(address, amount).await?;

// Unstake validator
node.unstake_validator(&address, amount).await?;

// Slash validator
node.slash_validator(&address, "violation_reason").await?;
```

### Bridge Operations

```rust
// Bridge deposit
let tx_hash = bridge.deposit_to_l2(amount, recipient).await?;

// Bridge withdrawal
let tx_hash = bridge.withdraw_to_l1(amount, recipient).await?;

// Get gas estimate
let gas_price = bridge.get_gas_price_estimate(true).await?;
```

## 🚀 Deployment

### Production Setup

1. **Configure RPC Endpoints**
   ```bash
   export L1_RPC_URL="https://mainnet.infura.io/v3/YOUR_KEY"
   export L2_RPC_URL="https://arb1.arbitrum.io/rpc"
   export FUEGO_RPC_URL="http://your-fuego-node:8080"
   ```

2. **Initialize Validator**
   ```bash
   ./codl3-arbitrum --validator-address YOUR_ADDRESS --gas-token-address HEAT_ADDRESS
   ```

3. **Start Mining**
   ```bash
   ./codl3-arbitrum --start-mining
   ```

### Monitoring

The node provides comprehensive monitoring through:

- **Prometheus Metrics**: `/metrics` endpoint
- **Health Checks**: `/health` endpoint
- **Block Explorer**: Web interface for transaction tracking
- **Log Aggregation**: Structured logging with tracing

## 🤝 Contributing

### Development Setup

```bash
# Clone and setup
git clone https://github.com/codl3/codl3-arbitrum.git
cd codl3-arbitrum

# Install dependencies
cargo build

# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run
```

### Code Structure

```
src/
├── main.rs              # Main node implementation
├── lib.rs               # Library exports
├── staking.rs           # Validator staking system
├── bridge.rs            # Arbitrum bridge integration
├── consensus.rs         # AnyTrust consensus
├── mining.rs            # Dual mining coordination
└── config.rs            # Configuration management
```

## 📄 License

MIT License - see LICENSE file for details.

## 🔗 Links

- [CODL3 Documentation](https://docs.codl3.io)
- [Arbitrum Orbit Documentation](https://docs.arbitrum.io/launch-orbit-chain/orbit-gentle-introduction)
- [Fuego Blockchain](https://fuego.io)

---

**CODL3 Arbitrum Orbit** - Secure, scalable, and efficient Layer 3 blockchain infrastructure.
