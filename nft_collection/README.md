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

## 👑 NFT Rarity Distribution

| Rarity | Count | Percentage | Power Level Range |
|--------|-------|------------|-------------------|
| **Legendary** | 1 | 4.8% | 100 |
| **Epic** | 4 | 19.0% | 88-95 |
| **Rare** | 4 | 19.0% | 80-87 |
| **Uncommon** | 6 | 28.6% | 70-78 |
| **Common** | 6 | 28.6% | 58-66 |

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

## 🚀 Genesis NFTs

### Legendary (1/21)
1. **𝝣lderado #001 - The Founder** ⭐
   - Power Level: 100
   - Stake Multiplier: 3.0x
   - Special Ability: Genesis Blessing
   - Rarity Score: 1000

### Epic (4/21)
2. **𝝣lderado #002 - The Guardian**
   - Power Level: 95
   - Stake Multiplier: 2.5x
   - Special Ability: Shield Wall

3. **𝝣lderado #003 - The Architect**
   - Power Level: 90
   - Stake Multiplier: 2.3x
   - Special Ability: Proof Mastery

4. **𝝣lderado #005 - The Oracle**
   - Power Level: 88
   - Stake Multiplier: 2.2x
   - Special Ability: Future Sight

5. **𝝣lderado #021 - The Prodigy**
   - Power Level: 92
   - Stake Multiplier: 2.4x
   - Special Ability: Evolution Boost

### Rare (4/21)
6. **𝝣lderado #004 - The Miner**
7. **𝝣lderado #006 - The Sentinel**
8. **𝝣lderado #007 - The Scholar**
9. **𝝣lderado #008 - The Warrior**

### Uncommon (6/21)
10. **𝝣lderado #009 - The Merchant**
11. **𝝣lderado #010 - The Explorer**
12. **𝝣lderado #011 - The Healer**
13. **𝝣lderado #012 - The Builder**
14. **𝝣lderado #013 - The Navigator**
15. **𝝣lderado #014 - The Librarian**

### Common (6/21)
16. **𝝣lderado #015 - The Messenger**
17. **𝝣lderado #016 - The Farmer**
18. **𝝣lderado #017 - The Scout**
19. **𝝣lderado #018 - The Artisan**
20. **𝝣lderado #019 - The Steward**
21. **𝝣lderado #020 - The Apprentice**

## 📊 Collection Statistics

- **Total Rarity Score**: 12,000
- **Average Rarity Score**: 571.43
- **Genesis Block**: 0x0000000000000000000000000000000000000000000000000000000000000000
- **Genesis Transaction**: 0x0000000000000000000000000000000000000000000000000000000000000001

## 🛠️ Smart Contract Features

### Core Functions
- `mintElderado()`: Mint individual 𝝣lderado NFTs
- `executeGenesisTransaction()`: Execute the complete genesis mint
- `getValidatorData()`: Retrieve validator information
- `getAllValidators()`: Get all validator data
- `getValidatorsByType()`: Filter validators by type
- `getValidatorsByRarityScore()`: Filter by rarity score range

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
nft_collection/
├── README.md                           # This file
├── elderado_genesis_collection.json    # Complete NFT metadata
├── ElderadoGenesisContract.sol         # Smart contract
├── deploy_genesis.js                   # Deployment script
├── verify_contract.js                   # Verification script
└── deployment_info.json                # Deployment results
```

## 🎮 Usage Examples

### Query Validator Data
```javascript
// Get specific validator
const validator = await contract.getValidatorData(0);
console.log(validator.name); // "𝝣lderado #001 - The Founder"

// Get all validators
const allValidators = await contract.getAllValidators();

// Get validators by type
const guardians = await contract.getValidatorsByType("Network Guardian");

// Get validators by rarity
const epicValidators = await contract.getValidatorsByRarityScore(800, 1000);
```

### Collection Statistics
```javascript
const stats = await contract.getCollectionStats();
console.log(`Total Supply: ${stats.totalSupply}`);
console.log(`Total Rarity Score: ${stats.totalRarityScore}`);
console.log(`Average Rarity Score: ${stats.averageRarityScore}`);
```

## 🔒 Security Features

- **ReentrancyGuard**: Prevents reentrancy attacks
- **Ownable**: Access control for admin functions
- **ERC721Enumerable**: Safe enumeration of tokens
- **ERC721URIStorage**: Secure URI management
- **Input Validation**: Comprehensive parameter validation

## 🌐 Metadata

### Token Metadata Structure
```json
{
  "name": "𝝣lderado #001 - The Founder",
  "description": "The first 𝝣lderado validator...",
  "image": "elderado_001_founder.png",
  "attributes": [
    {"trait_type": "Validator Type", "value": "Genesis Founder"},
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
- **Q1 2024**: Genesis collection launch
- **Q2 2024**: Staking integration
- **Q3 2024**: Governance features
- **Q4 2024**: Cross-chain expansion

## 📚 Additional Resources

### Documentation
- [zkC0DL3 Main Documentation](../README.md)
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

**𝝣lderado Genesis Collection** - The foundation of zkC0DL3's validator ecosystem, immortalized as NFTs on the blockchain.

*For the most up-to-date information, always refer to the latest documentation and community resources.*