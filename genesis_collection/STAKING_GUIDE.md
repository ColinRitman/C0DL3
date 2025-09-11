# 𝝣lderado Staking System - Comprehensive Guide

## 🔥 Overview

The **𝝣lderado Staking System** provides validators with the opportunity to stake HEAT tokens and become 𝝣lderado validators in the zkC0DL3 network. This system provides a simple and secure staking mechanism for validator participation.

## 🎯 Staking Requirements

### HEAT Token Staking
- **Amount**: 80,000,000,000 HEAT tokens (80B HEAT)
- **Chain**: zkC0DL3 Hyperchain
- **Verification**: Direct on-chain verification
- **Transfer Restriction**: NFTs can only be bought/sold for exactly 80B HEAT tokens

## 🔗 Staking Mechanism

### Core Requirements
- **Fixed Amount**: All Elderado NFTs require exactly 80,000,000,000 HEAT tokens for transfer
- **No Price Variation**: Cannot be sold for more or less than the staking amount
- **Contract Enforcement**: Smart contract prevents invalid transfers
- **Purpose**: Ensures NFTs maintain their validator staking value

## 🛠️ Technical Implementation

### Smart Contract Features

#### Core Staking Constants
```solidity
uint256 public constant STAKING_AMOUNT = 80000000000; // 80,000,000,000 HEAT tokens
```

#### Transfer Restriction Functions
- `transferFrom()`: Requires exactly 80B HEAT tokens as msg.value
- `safeTransferFrom()`: Requires exactly 80B HEAT tokens as msg.value
- `_beforeTokenTransfer()`: Validates staking amount before transfer

### Staking Process

#### Step 1: Mint Genesis NFTs
1. **Contract Owner**: Calls `executeGenesisTransaction()`
2. **Mint All 21 NFTs**: All NFTs minted to DIGM Treasury
3. **Fixed Supply**: No additional minting allowed

#### Step 2: Transfer NFTs
1. **Fixed Price**: NFTs can only be transferred for exactly 80B HEAT tokens
2. **Contract Enforcement**: Smart contract prevents transfers with wrong amount
3. **Validator Status**: NFT holder becomes active 𝝣lderado validator

#### Step 3: Validator Participation
1. **Staking Maintained**: NFT holder maintains 80B HEAT staking requirement
2. **Transfer Restrictions**: Any transfer requires exact staking amount
3. **Validator Privileges**: NFT provides validator rights and benefits

## 🚀 Deployment Guide

### Contract Deployment

#### Prerequisites
```bash
npm install hardhat @openzeppelin/contracts
```

#### Deploy Elderado Genesis Contract
```javascript
// Deploy the on-demand staking contract
const ElderadoGenesisCollection = await ethers.getContractFactory("ElderadoGenesisCollection");

const elderadoCollection = await ElderadoGenesisCollection.deploy(
    "𝝣lderado Genesis Collection",
    "ELDERADO",
    "https://ipfs.io/ipfs/QmElderadoGenesisCollection/",
    "https://ipfs.io/ipfs/QmElderadoGenesisContract/"
);

// Enable minting for users
await elderadoCollection.enableMinting();
```

### Environment Configuration

#### Required Variables
```bash
# zkC0DL3 Configuration
ZKC0DL3_RPC_URL=https://zkc0dl3-rpc.example.com
ZKC0DL3_CHAIN_ID=67890
```

## 📊 Validator Management

### On-Demand Minting Process

#### Mint Individual NFTs
```javascript
// User mints NFT by staking 80B HEAT tokens
await elderadoCollection.mintElderadoToSelf({
    value: "80000000000" // 80,000,000,000 HEAT tokens
});

// Or mint to specific address
await elderadoCollection.mintElderado(
    recipientAddress,
    { value: "80000000000" }
);
```

#### Check Available Supply
```javascript
// Get current supply and max supply
const totalSupply = await elderadoCollection.totalSupply();
const maxSupply = await elderadoCollection.MAX_SUPPLY();
console.log(`Available: ${maxSupply - totalSupply} / ${maxSupply}`);
```

### Transfer Restrictions

#### Fixed Price Transfers
```javascript
// Transfer NFT with exact staking amount
await elderadoCollection.transferFrom(
    fromAddress,
    toAddress,
    tokenId,
    { value: ethers.utils.parseEther("80000000000") } // 80B HEAT tokens
);
```

#### Query Staking Amount
```javascript
// Get required staking amount
const stakingAmount = await elderadoCollection.getStakingAmount();
console.log("Required staking amount:", stakingAmount.toString());
```

## 🔒 Security Features

### Transfer Restrictions
- **Fixed Price**: NFTs can only be sold for exactly 80B HEAT tokens
- **No Price Variation**: Cannot be sold for more or less than the staking amount
- **Contract Enforcement**: Smart contract prevents invalid transfers
- **Validator Integrity**: Ensures NFTs maintain their staking value

### Access Control
- **Owner Only Minting**: Only contract owner can mint genesis NFTs
- **Treasury Minting**: All 21 NFTs minted to DIGM Treasury initially
- **ReentrancyGuard**: Prevents reentrancy attacks
- **Input Validation**: Comprehensive parameter validation

## 📈 Economic Benefits

### For Validators
- **Fixed Value**: NFTs maintain consistent 80B HEAT staking requirement
- **Validator Status**: NFT ownership provides validator privileges
- **Transfer Security**: Contract-enforced fair pricing
- **Network Participation**: Active role in zkC0DL3 ecosystem

### For Network
- **Validator Stability**: Consistent staking requirements
- **Economic Integrity**: Fixed NFT values prevent speculation
- **Security**: Enforced staking maintains network security
- **Simplicity**: Straightforward participation model

## 🎮 Usage Examples

### Complete Workflow

#### Step 1: Deploy Contract
```javascript
const ElderadoGenesisCollection = await ethers.getContractFactory("ElderadoGenesisCollection");
const elderadoCollection = await ElderadoGenesisCollection.deploy(
    "𝝣lderado Genesis Collection",
    "ELDERADO",
    "https://ipfs.io/ipfs/QmElderadoGenesisCollection/",
    "https://ipfs.io/ipfs/QmElderadoGenesisContract/"
);
```

#### Step 2: Enable Minting & Users Mint NFTs
```javascript
// Contract owner enables minting
await elderadoCollection.enableMinting();

// Users can now mint NFTs individually by staking 80B HEAT
await elderadoCollection.mintElderadoToSelf({
    value: "80000000000" // Exact staking amount required
});

// Contract automatically generates validator data:
// - Name: "𝝣lderado #1", "𝝣lderado #2", etc.
// - Type: "Genesis Validator"
// - Power Level: 100
// - Stake Multiplier: 3.0x
// - Special Ability: "Genesis Blessing"
// - Rarity Score: 1000
```

#### Step 3: Transfer NFTs
```javascript
// Transfer NFT with exact staking amount (80B HEAT)
await elderadoCollection.transferFrom(
    fromAddress,
    toAddress,
    tokenId,
    { value: "80000000000" }
);
```

#### Step 4: Query Collection Data
```javascript
// Get collection statistics
const stats = await elderadoCollection.getCollectionStats();
console.log(`Total Supply: ${stats.totalSupply}`);
console.log(`Staking Amount: ${stats.stakingAmount}`);

// Get all validators
const allValidators = await elderadoCollection.getAllValidators();
console.log(`Found ${allValidators.length} validators`);
```

## 🔮 Future Developments

### Planned Features
- **Staking Rewards**: Reward distribution for NFT holders
- **Governance Rights**: Voting power for validator NFTs
- **Validator Performance**: Performance tracking and bonuses
- **Secondary Markets**: Enhanced marketplace features

### Roadmap
- **Q1 2024**: Simplified staking system launch
- **Q2 2024**: Reward distribution implementation
- **Q3 2024**: Governance features
- **Q4 2024**: Advanced validator features

## 📚 Additional Resources

### Documentation
- [zkC0DL3 Validator Guide](../docs/ZKC0DL3_VALIDATOR_GUIDE.md)
- [Smart Contract Documentation](./contract_docs.md)
- [Genesis Collection README](./README.md)

### Community
- [GitHub Repository](https://github.com/ColinRitman/C0DL3)
- [Discord Community](https://discord.gg/c0dl3)
- [Telegram Group](https://t.me/c0dl3)

---

## 🎉 Quick Start

### Deploy and Mint Genesis Collection
```bash
# 1. Install dependencies
npm install hardhat @openzeppelin/contracts

# 2. Set environment variables
export DIGM_TREASURY_ADDRESS="0x..."

# 3. Deploy genesis collection
npx hardhat run deploy_genesis.js --network zkc0dl3

# 4. Verify contract
npx hardhat verify --network zkc0dl3 <contract_address> "𝝣lderado Genesis Collection" "ELDERADO" "https://ipfs.io/ipfs/QmElderadoGenesisCollection/" "https://ipfs.io/ipfs/QmElderadoGenesisContract/"
```

### Transfer NFTs (Fixed Price)
```javascript
// Transfer NFT with exact 80B HEAT tokens
await elderadoCollection.transferFrom(
    fromAddress,
    toAddress,
    tokenId,
    { value: "80000000000" } // 80,000,000,000 HEAT tokens
);
```

---

**𝝣lderado Staking System** - Simplified NFT staking for zkC0DL3 validators with fixed 80B HEAT token requirements.

*For the most up-to-date information, always refer to the latest documentation and community resources.*