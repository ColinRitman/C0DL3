# 𝝣lderado Genesis Collection - zkC0DL3 NFT Collection

## 🏛️ Overview

The **𝝣lderado Genesis Collection** is the inaugural NFT collection celebrating the genesis of zkC0DL3's 𝝣lderado validators. This collection consists of 21 unique NFTs representing the founding validators who will secure and validate the zkSync hyperchain implementation.

## 🎯 Collection Details

- **Name**: 𝝣lderado Genesis Collection
- **Symbol**: ELDERADO
- **Total Supply**: 21 NFTs
- **Standard**: ERC-721
- **Blockchain**: zkC0DL3 Hyperchain
- **Mint Price**: 0.1 ETH
- **Royalty**: 5%

- **Transfer Restriction**: NFTs can only be sold for the staking amount (80,000,000,000 HEAT tokens)

## 👑 NFT Rarity Distribution

| Rarity | Count | Percentage | Power Level |
|--------|-------|------------|-------------|
| **Genesis** | 21 | 100% | 100 |

## 🎨 NFT Attributes

Each 𝝣lderado NFT contains the following attributes:

### Core Attributes
- **Validator Type**: The specific role/function of the validator
- **Rarity**: Legendary, Epic, Rare, Uncommon, or Common
- **Power Level**: Numerical power rating (58-100)
- **Stake Multiplier**: Bonus multiplier for staking rewards (1.0x - 3.0x)
- **Special Ability**: Unique validator capability

### Visual Attributes
- **Background**: Color theme representing the validator's nature
- **Crown**: Headpiece indicating the validator's status
- **Aura**: Visual effect surrounding the validator
- **Eyes**: Eye color/style representing the validator's personality
- **Accessory**: Unique item held by the validator

## 🚀 On-Demand Genesis NFTs

### Genesis Collection (0/21 initially)
Users can mint up to 21 𝝣lderado NFTs by staking exactly 80,000,000,000 HEAT tokens each. All minted NFTs are identical and represent founding validators of the zkC0DL3 network.

**Minting Process:**
1. User sends exactly 80B HEAT tokens to `mintElderadoToSelf()`
2. Contract mints NFT with sequential numbering (𝝣lderado #001, #002, etc.)
3. NFT provides validator status and benefits

**All NFTs have identical attributes:**
- Power Level: 100
- Stake Multiplier: 3.0x
- Special Ability: Genesis Blessing
- Rarity Score: 1000

## 📊 Collection Statistics

- **Total Rarity Score**: 21,000
- **Average Rarity Score**: 1000
- **Genesis Block**: 0x0000000000000000000000000000000000000000000000000000000000000000
- **Genesis Transaction**: 0x0000000000000000000000000000000000000000000000000000000000000001

## 🔒 Transfer Restrictions

### Staking Amount Requirement
- **Fixed Price**: NFTs can only be sold for exactly 80,000,000,000 HEAT tokens
- **No Price Variation**: Cannot be sold for more or less than the staking amount
- **Enforced by Contract**: Transfer functions require exact staking amount
- **Purpose**: Ensures NFTs maintain their validator staking value

### Transfer Functions
- `transferFrom()`: Requires 80,000,000,000 HEAT tokens as msg.value
- `safeTransferFrom()`: Requires 80,000,000,000 HEAT tokens as msg.value
- `_beforeTokenTransfer()`: Validates staking amount before transfer

## 🛠️ Smart Contract Features

### Core Functions
- `mintElderado()`: Mint NFT to specific address (requires 80B HEAT)
- `mintElderadoToSelf()`: Mint NFT to caller (requires 80B HEAT)
- `getValidatorData()`: Retrieve validator information
- `getAllValidators()`: Get all validator data
- `getValidatorsByType()`: Filter validators by type
- `getValidatorsByRarityScore()`: Filter by rarity score range
- `getStakingAmount()`: Get required staking amount for transfers

### Utility Functions
- `getTotalRarityScore()`: Calculate total collection rarity
- `getAverageRarityScore()`: Calculate average rarity
- `getCollectionStats()`: Get comprehensive statistics
- `supportsInterface()`: ERC-721 compatibility

### Access Control
- `enableMinting()`: Enable/disable minting
- `setBaseURI()`: Update metadata URI
- `setContractURI()`: Update contract metadata
- `withdraw()`: Withdraw contract balance

## 🚀 Deployment

### Prerequisites
```bash
npm install hardhat @openzeppelin/contracts
```

### Deploy Contract
```bash
# Deploy the genesis collection
npx hardhat run deploy_genesis.js --network <network>

# Verify contract
npx hardhat verify --network <network> <contract_address> "<collection_name>" "<collection_symbol>" "<base_uri>" "<contract_uri>"
```

### Environment Variables
```bash
# Required for deployment
PRIVATE_KEY=your_private_key
RPC_URL=your_rpc_url
ETHERSCAN_API_KEY=your_etherscan_key
```

## 📁 File Structure

```
genesis_collection/
├── README.md                           # This file
├── STAKING_GUIDE.md                    # Staking system guide
├── elderado_genesis_collection.json    # Complete NFT metadata
├── ElderadoGenesisContract.sol         # Smart contract
├── deploy_genesis.js                   # Deployment script
├── verify_contract.js                   # Verification script
└── deployment_info.json                # Deployment results
```

## 🎮 Usage Examples

### Mint NFTs by Staking HEAT
```javascript
// Mint NFT to yourself by staking 80B HEAT tokens
await contract.mintElderadoToSelf({
    value: "80000000000" // Exactly 80,000,000,000 HEAT tokens
});

// Mint NFT to specific address
await contract.mintElderado(recipientAddress, {
    value: "80000000000"
});
```

### Query Validator Data
```javascript
// Get specific validator
const validator = await contract.getValidatorData(0);
console.log(validator.name); // "𝝣lderado #001"

// Get all validators
const allValidators = await contract.getAllValidators();

// Get validators by type
const genesisValidators = await contract.getValidatorsByType("Genesis Validator");

// Get validators by rarity
const legendaryValidators = await contract.getValidatorsByRarityScore(900, 1000);
```

### Collection Statistics
```javascript
const stats = await contract.getCollectionStats();
console.log(`Total Supply: ${stats.totalSupply} / 21`);
console.log(`Staking Amount Required: ${stats.stakingAmount} HEAT`);
console.log(`Minting Enabled: ${stats.mintingEnabled}`);
```

## 🔒 Security Features

- **ReentrancyGuard**: Prevents reentrancy attacks
- **Ownable**: Access control for admin functions (enable/disable minting)
- **ERC721Enumerable**: Safe enumeration of tokens
- **ERC721URIStorage**: Secure URI management
- **Staking Validation**: Exact 80B HEAT token requirement for minting
- **Transfer Restrictions**: Fixed price enforcement for NFT transfers
- **Supply Cap**: Maximum 21 NFTs can be minted
- **Input Validation**: Comprehensive parameter validation

## 🌐 Metadata

### Token Metadata Structure
```json
{
  "name": "𝝣lderado #001",
  "description": "Genesis validator NFT for zkC0DL3 network. Stake exactly 80B HEAT tokens to become a validator.",
  "image": "elderado_001.png",
  "attributes": [
    {"trait_type": "Validator Type", "value": "Genesis Validator"},
    {"trait_type": "Rarity", "value": "Legendary"},
    {"trait_type": "Power Level", "value": 100},
    {"trait_type": "Stake Multiplier", "value": "3.0x"},
    {"trait_type": "Special Ability", "value": "Genesis Blessing"}
  ]
}
```

### Contract Metadata
```json
{
  "name": "𝝣lderado Genesis Collection",
  "description": "The inaugural NFT collection celebrating the genesis of zkC0DL3's 𝝣lderado validators.",
  "image": "collection_image.png",
  "external_link": "https://zkc0dl3.com",
  "seller_fee_basis_points": 500
}
```

## 🎯 Future Developments

### Planned Features
- **Validator Staking**: Use NFTs for validator staking bonuses
- **Governance Rights**: Voting power based on NFT rarity
- **Cross-Chain Support**: Bridge NFTs to other chains
- **Dynamic Metadata**: Updateable validator performance data
- **Rental System**: Rent validator NFTs for staking

### Roadmap
- **Q1 2024**: On-demand minting system launch
- **Q2 2024**: User staking and validator activation
- **Q3 2024**: Governance features
- **Q4 2024**: Advanced validator features

## 📚 Additional Resources

### Documentation
- [zkC0DL3 Main Documentation](../README.md)
- [Staking Guide](./STAKING_GUIDE.md)
- [Validator Guide](../docs/ZKC0DL3_VALIDATOR_GUIDE.md)
- [Smart Contract Documentation](./contract_docs.md)

### Community
- [GitHub Repository](https://github.com/ColinRitman/C0DL3)
- [Discord Community](https://discord.gg/c0dl3)
- [Telegram Group](https://t.me/c0dl3)

### Support
- Create an issue in the repository
- Join the C0DL3 community Discord
- Check the documentation wiki

---

## 🎉 Quick Start

```bash
# Clone repository
git clone https://github.com/ColinRitman/C0DL3.git
cd C0DL3/nft_collection

# Install dependencies
npm install

# Deploy genesis collection
npx hardhat run deploy_genesis.js --network localhost

# Verify deployment
npx hardhat verify --network localhost <contract_address> "𝝣lderado Genesis Collection" "ELDERADO" "https://ipfs.io/ipfs/QmElderadoGenesisCollection/" "https://ipfs.io/ipfs/QmElderadoGenesisContract/"
```

---

**𝝣lderado Genesis Collection** - On-demand NFT minting system where users become zkC0DL3 validators by staking exactly 80B HEAT tokens per NFT.

*For the most up-to-date information, always refer to the latest documentation and community resources.*