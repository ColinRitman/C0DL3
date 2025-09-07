# 𝝣lderado Dual Staking System - Comprehensive Guide

## 🔥 Overview

The **𝝣lderado Dual Staking System** provides validators with two equivalent options to stake and become 𝝣lderado validators in the zkC0DL3 network. This system integrates with the Fuego chain's Elderfier verification setup to provide flexible staking options.

## 🎯 Dual Staking Options

### Option 1: HEAT Token Staking
- **Amount**: 80,000,000,000 HEAT tokens
- **Chain**: zkC0DL3 Hyperchain
- **Verification**: Direct on-chain verification

### Option 2: XFG Token Staking (Fuego Chain)
- **Amount**: 8,000 XFG tokens
- **Chain**: Fuego L1 Blockchain
- **Verification**: Elderfier verification setup
- **Equivalence**: 8,000 XFG = 80,000,000,000 HEAT

## 🔗 Staking Equivalence

### Mathematical Relationship
```
80,000,000,000 HEAT = 8,000 XFG
Equivalence Ratio: 10,000,000 HEAT per 1 XFG
```

### Economic Rationale
- **HEAT**: Native token of zkC0DL3 ecosystem
- **XFG**: Native token of Fuego L1 blockchain
- **Cross-Chain**: Enables validators from Fuego ecosystem to participate
- **Flexibility**: Validators can choose their preferred staking asset

## 🛠️ Technical Implementation

### Smart Contract Features

#### Dual Staking Support
```solidity
enum StakingType {
    HEAT,
    XFG
}

struct StakingProof {
    StakingType stakingType;
    uint256 amount;
    address tokenAddress;
    uint256 blockNumber;
    bytes32 transactionHash;
    bool verified;
    uint256 verificationTimestamp;
}
```

#### Key Functions
- `submitStakingProof()`: Submit proof of staking on Fuego chain
- `verifyStakingProof()`: Verify staking proof (owner only)
- `mintElderadoWithStakingProof()`: Mint NFT with verified staking proof
- `getValidatorsByStakingType()`: Filter validators by staking type

### Staking Proof Process

#### Step 1: Stake on Fuego Chain
1. **Stake XFG**: Stake exactly 8,000 XFG tokens on Fuego chain
2. **Record Transaction**: Note the transaction hash and block number
3. **Elderfier Verification**: Use Fuego's Elderfier verification system

#### Step 2: Submit Proof to zkC0DL3
1. **Call Contract**: Submit staking proof to zkC0DL3 contract
2. **Provide Details**: Include transaction hash, amount, and staking type
3. **Wait Verification**: Contract owner verifies the proof

#### Step 3: Mint 𝝣lderado NFT
1. **Verification Complete**: Once proof is verified
2. **Mint NFT**: Receive 𝝣lderado validator NFT
3. **Validator Status**: Become active 𝝣lderado validator

## 🔥 Fuego Chain Integration

### Elderfier Verification Setup

#### Prerequisites
- **Fuego Node**: Running Fuego L1 node
- **XFG Tokens**: 8,000 XFG tokens for staking
- **Elderfier Setup**: Configured Elderfier verification

#### Staking Process on Fuego
```bash
# 1. Start Fuego node
./fuegod --daemon

# 2. Stake XFG tokens
fuego-cli staketoaddress <validator_address> 8000

# 3. Verify staking
fuego-cli getstakinginfo

# 4. Get transaction hash
fuego-cli gettransaction <tx_hash>
```

#### Elderfier Configuration
```json
{
  "elderfier": {
    "enabled": true,
    "stake_amount": 8000,
    "verification_method": "proof_of_stake",
    "cross_chain_enabled": true,
    "target_chain": "zkc0dl3"
  }
}
```

## 🚀 Deployment Guide

### Contract Deployment

#### Prerequisites
```bash
npm install hardhat @openzeppelin/contracts
```

#### Deploy Dual Staking Contract
```javascript
// Deploy with token addresses
const ElderadoDualStakingCollection = await ethers.getContractFactory("ElderadoDualStakingCollection");

const elderadoCollection = await ElderadoDualStakingCollection.deploy(
    "𝝣lderado Genesis Collection",
    "ELDERADO",
    "https://ipfs.io/ipfs/QmElderadoGenesisCollection/",
    "https://ipfs.io/ipfs/QmElderadoGenesisContract/",
    heatTokenAddress,  // HEAT token address
    xfgTokenAddress   // XFG token address
);
```

### Environment Configuration

#### Required Variables
```bash
# Token Addresses
HEAT_TOKEN_ADDRESS=0x...
XFG_TOKEN_ADDRESS=0x...

# Fuego Chain RPC
FUEGO_RPC_URL=https://fuego-rpc.example.com
FUEGO_CHAIN_ID=12345

# zkC0DL3 Configuration
ZKC0DL3_RPC_URL=https://zkc0dl3-rpc.example.com
ZKC0DL3_CHAIN_ID=67890
```

## 📊 Validator Management

### Staking Proof Submission

#### For XFG Stakers
```javascript
// Submit staking proof
await elderadoCollection.submitStakingProof(
    1, // StakingType.XFG
    8000, // Amount in XFG
    "0x..." // Transaction hash from Fuego chain
);
```

#### For HEAT Stakers
```javascript
// Submit staking proof
await elderadoCollection.submitStakingProof(
    0, // StakingType.HEAT
    80000000000, // Amount in HEAT
    "0x..." // Transaction hash from zkC0DL3 chain
);
```

### Verification Process

#### Owner Verification
```javascript
// Verify staking proof
await elderadoCollection.verifyStakingProof(
    validatorAddress,
    true // Verified
);
```

#### Query Staking Proof
```javascript
// Get staking proof
const proof = await elderadoCollection.getStakingProof(validatorAddress);
console.log("Staking Type:", proof.stakingType);
console.log("Amount:", proof.amount);
console.log("Verified:", proof.verified);
```

## 🔒 Security Features

### Staking Proof Validation
- **Transaction Hash**: Verified against blockchain records
- **Amount Validation**: Exact staking amount required
- **Chain Verification**: Cross-chain proof validation
- **Owner Verification**: Manual verification by contract owner

### Transfer Restrictions
- **HEAT NFTs**: Can only be sold for 80B HEAT tokens
- **XFG NFTs**: Can only be sold for 8K XFG tokens
- **Dynamic Pricing**: Transfer price based on staking type
- **Contract Enforcement**: Smart contract prevents invalid transfers

## 📈 Economic Benefits

### For Validators
- **Flexibility**: Choose preferred staking asset
- **Cross-Chain**: Participate from Fuego ecosystem
- **Equal Rights**: Same validator privileges regardless of staking type
- **Liquidity**: Access to both HEAT and XFG ecosystems

### For Network
- **Diversification**: Multiple token ecosystems
- **Security**: Higher total stake across chains
- **Adoption**: Easier onboarding for Fuego users
- **Interoperability**: Cross-chain validator participation

## 🎮 Usage Examples

### Complete Workflow

#### Step 1: Stake on Fuego Chain
```bash
# Stake 8,000 XFG tokens
fuego-cli staketoaddress 0x1234567890123456789012345678901234567890 8000

# Get transaction hash
fuego-cli gettransaction <tx_hash>
```

#### Step 2: Submit Proof to zkC0DL3
```javascript
// Submit staking proof
const tx = await elderadoCollection.submitStakingProof(
    1, // XFG
    8000,
    "0xabc123..." // Fuego transaction hash
);
```

#### Step 3: Wait for Verification
```javascript
// Check verification status
const proof = await elderadoCollection.getStakingProof(validatorAddress);
if (proof.verified) {
    console.log("Proof verified! Ready to mint NFT.");
}
```

#### Step 4: Mint 𝝣lderado NFT
```javascript
// Mint NFT with verified staking proof
await elderadoCollection.mintElderadoWithStakingProof(
    validatorAddress,
    validatorData
);
```

### Query Validators by Staking Type

#### Get XFG Validators
```javascript
const xfgValidators = await elderadoCollection.getValidatorsByStakingType(1);
console.log(`Found ${xfgValidators.length} XFG validators`);
```

#### Get HEAT Validators
```javascript
const heatValidators = await elderadoCollection.getValidatorsByStakingType(0);
console.log(`Found ${heatValidators.length} HEAT validators`);
```

## 🔮 Future Developments

### Planned Features
- **Automated Verification**: Automatic cross-chain proof verification
- **Dynamic Equivalence**: Adjustable equivalence ratios
- **Multi-Chain Support**: Additional blockchain integrations
- **Staking Rewards**: Cross-chain reward distribution
- **Governance Rights**: Voting power based on staking type

### Roadmap
- **Q1 2024**: Dual staking implementation
- **Q2 2024**: Automated verification system
- **Q3 2024**: Multi-chain expansion
- **Q4 2024**: Advanced governance features

## 📚 Additional Resources

### Documentation
- [Fuego Elderfiers Guide](https://github.com/ColinRitman/fuego/blob/master/FUEGO_ELDERFIERS_COMPREHENSIVE_GUIDE.md)
- [zkC0DL3 Validator Guide](../docs/ZKC0DL3_VALIDATOR_GUIDE.md)
- [Smart Contract Documentation](./contract_docs.md)

### Community
- [GitHub Repository](https://github.com/ColinRitman/C0DL3)
- [Discord Community](https://discord.gg/c0dl3)
- [Telegram Group](https://t.me/c0dl3)

---

## 🎉 Quick Start

### For XFG Stakers
```bash
# 1. Stake on Fuego chain
fuego-cli staketoaddress <address> 8000

# 2. Submit proof to zkC0DL3
npx hardhat run scripts/submit_xfg_proof.js --network zkc0dl3

# 3. Wait for verification and mint NFT
npx hardhat run scripts/mint_elderado.js --network zkc0dl3
```

### For HEAT Stakers
```bash
# 1. Stake on zkC0DL3 chain
zkc0dl3-cli stake <address> 80000000000

# 2. Submit proof
npx hardhat run scripts/submit_heat_proof.js --network zkc0dl3

# 3. Mint NFT
npx hardhat run scripts/mint_elderado.js --network zkc0dl3
```

---

**𝝣lderado Dual Staking System** - Enabling flexible validator participation across the zkC0DL3 and Fuego ecosystems with equivalent staking options.

*For the most up-to-date information, always refer to the latest documentation and community resources.*