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

    // Enable minting for users to mint NFTs by staking 80B HEAT
    console.log("🔓 Enabling minting for user staking...");
    await elderadoCollection.enableMinting();
    console.log("✅ Minting enabled - users can now mint NFTs by staking 80B HEAT tokens!");
    
    // Get collection statistics
    const stats = await elderadoCollection.getCollectionStats();
    console.log("\n📊 Collection Statistics:");
    console.log(`   Total Supply: ${stats.totalSupply} / ${await elderadoCollection.MAX_SUPPLY()}`);
    console.log(`   Minting Enabled: ${stats.mintingEnabled}`);
    console.log(`   Staking Amount Required: ${stats.stakingAmount} HEAT tokens`);

    // Save deployment info
    const [deployer] = await ethers.getSigners();
    const deploymentInfo = {
        contractAddress: elderadoCollection.address,
        deployer: deployer.address,
        collectionName: collectionName,
        collectionSymbol: collectionSymbol,
        maxSupply: await elderadoCollection.MAX_SUPPLY(),
        totalSupply: stats.totalSupply,
        stakingAmount: stats.stakingAmount.toString(),
        mintingEnabled: stats.mintingEnabled,
        deploymentTime: new Date().toISOString(),
        network: await ethers.provider.getNetwork(),
        deploymentType: "On-demand minting with 80B HEAT staking requirement"
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
    console.log(`📊 Max Supply: ${await elderadoCollection.MAX_SUPPLY()} NFTs`);
    console.log(`🔥 Staking Requirement: ${stats.stakingAmount} HEAT tokens per NFT`);
    console.log(`🚀 Users can now mint NFTs by staking 80B HEAT tokens!`);

    return {
        contractAddress: elderadoCollection.address,
        maxSupply: await elderadoCollection.MAX_SUPPLY(),
        stakingAmount: stats.stakingAmount
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