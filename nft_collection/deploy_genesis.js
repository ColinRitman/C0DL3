const { ethers } = require("hardhat");
const fs = require("fs");
const path = require("path");

async function main() {
    console.log("🚀 Deploying 𝝣lderado Genesis Collection...");
    
    // Get the contract factory
    const ElderadoGenesisCollection = await ethers.getContractFactory("ElderadoGenesisCollection");
    
    // Collection metadata
    const collectionName = "𝝣lderado Genesis Collection";
    const collectionSymbol = "ELDERADO";
    const baseURI = "https://ipfs.io/ipfs/QmElderadoGenesisCollection/";
    const contractURI = "https://ipfs.io/ipfs/QmElderadoGenesisContract/";
    
    // Deploy the contract
    console.log("📝 Deploying contract...");
    const elderadoCollection = await ElderadoGenesisCollection.deploy(
        collectionName,
        collectionSymbol,
        baseURI,
        contractURI
    );
    
    await elderadoCollection.deployed();
    console.log(`✅ Contract deployed to: ${elderadoCollection.address}`);
    
    // Load NFT data from JSON file
    const nftDataPath = path.join(__dirname, "elderado_genesis_collection.json");
    const nftData = JSON.parse(fs.readFileSync(nftDataPath, "utf8"));
    
    // Prepare validator data for genesis transaction
    const validatorDataArray = nftData.nfts.map(nft => ({
        tokenId: nft.token_id - 1, // Convert to 0-based indexing
        name: nft.name,
        validatorType: nft.attributes.find(attr => attr.trait_type === "Validator Type").value,
        powerLevel: nft.attributes.find(attr => attr.trait_type === "Power Level").value,
        stakeMultiplier: parseFloat(nft.attributes.find(attr => attr.trait_type === "Stake Multiplier").value),
        specialAbility: nft.attributes.find(attr => attr.trait_type === "Special Ability").value,
        rarityScore: nft.rarity_score,
        isGenesis: true
    }));
    
    // Generate recipient addresses (for demo purposes, using deployer address)
    const [deployer] = await ethers.getSigners();
    const recipients = new Array(21).fill(deployer.address);
    
    // Enable minting
    console.log("🔓 Enabling minting...");
    await elderadoCollection.enableMinting();
    
    // Execute genesis transaction
    console.log("⚡ Executing genesis transaction...");
    const genesisTx = await elderadoCollection.executeGenesisTransaction(
        recipients,
        validatorDataArray
    );
    
    console.log(`📋 Genesis transaction hash: ${genesisTx.hash}`);
    await genesisTx.wait();
    console.log("✅ Genesis transaction confirmed!");
    
    // Get collection statistics
    const stats = await elderadoCollection.getCollectionStats();
    console.log("\n📊 Collection Statistics:");
    console.log(`   Total Supply: ${stats.totalSupply}`);
    console.log(`   Total Rarity Score: ${stats.totalRarityScore}`);
    console.log(`   Average Rarity Score: ${stats.averageRarityScore}`);
    console.log(`   Minting Enabled: ${stats.mintingEnabled}`);
    
    // Get all validators
    const allValidators = await elderadoCollection.getAllValidators();
    console.log("\n👑 Genesis 𝝣lderado Validators:");
    allValidators.forEach((validator, index) => {
        console.log(`   ${index + 1}. ${validator.name}`);
        console.log(`      Type: ${validator.validatorType}`);
        console.log(`      Power Level: ${validator.powerLevel}`);
        console.log(`      Rarity Score: ${validator.rarityScore}`);
        console.log(`      Special Ability: ${validator.specialAbility}`);
        console.log("");
    });
    
    // Save deployment info
    const deploymentInfo = {
        contractAddress: elderadoCollection.address,
        transactionHash: genesisTx.hash,
        blockNumber: genesisTx.blockNumber,
        deployer: deployer.address,
        collectionName: collectionName,
        collectionSymbol: collectionSymbol,
        totalSupply: stats.totalSupply,
        totalRarityScore: stats.totalRarityScore.toString(),
        averageRarityScore: stats.averageRarityScore.toString(),
        deploymentTime: new Date().toISOString(),
        network: await ethers.provider.getNetwork(),
        validators: allValidators.map(v => ({
            tokenId: v.tokenId.toString(),
            name: v.name,
            validatorType: v.validatorType,
            powerLevel: v.powerLevel.toString(),
            stakeMultiplier: v.stakeMultiplier.toString(),
            specialAbility: v.specialAbility,
            rarityScore: v.rarityScore.toString(),
            isGenesis: v.isGenesis
        }))
    };
    
    const deploymentPath = path.join(__dirname, "deployment_info.json");
    fs.writeFileSync(deploymentPath, JSON.stringify(deploymentInfo, null, 2));
    console.log(`💾 Deployment info saved to: ${deploymentPath}`);
    
    // Create verification script
    const verifyScript = `
// Verification script for 𝝣lderado Genesis Collection
// Run with: npx hardhat verify --network <network> <contract_address> "<collection_name>" "<collection_symbol>" "<base_uri>" "<contract_uri>"

const contractAddress = "${elderadoCollection.address}";
const collectionName = "${collectionName}";
const collectionSymbol = "${collectionSymbol}";
const baseURI = "${baseURI}";
const contractURI = "${contractURI}";

console.log("🔍 Verification command:");
console.log(\`npx hardhat verify --network <network> \${contractAddress} "\${collectionName}" "\${collectionSymbol}" "\${baseURI}" "\${contractURI}"\`);
`;
    
    const verifyScriptPath = path.join(__dirname, "verify_contract.js");
    fs.writeFileSync(verifyScriptPath, verifyScript);
    console.log(`🔍 Verification script saved to: ${verifyScriptPath}`);
    
    console.log("\n🎉 𝝣lderado Genesis Collection deployment completed successfully!");
    console.log(`📍 Contract Address: ${elderadoCollection.address}`);
    console.log(`🔗 Transaction Hash: ${genesisTx.hash}`);
    console.log(`📊 Total NFTs Minted: ${stats.totalSupply}`);
    console.log(`⭐ Total Rarity Score: ${stats.totalRarityScore}`);
    
    return {
        contractAddress: elderadoCollection.address,
        transactionHash: genesisTx.hash,
        blockNumber: genesisTx.blockNumber,
        totalSupply: stats.totalSupply,
        totalRarityScore: stats.totalRarityScore
    };
}

// Execute deployment
main()
    .then((result) => {
        console.log("✅ Deployment successful:", result);
        process.exit(0);
    })
    .catch((error) => {
        console.error("❌ Deployment failed:", error);
        process.exit(1);
    });