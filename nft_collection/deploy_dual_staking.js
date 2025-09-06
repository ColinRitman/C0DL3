const { ethers } = require("hardhat");
const fs = require("fs");
const path = require("path");

async function main() {
    console.log("🔥 Deploying 𝝣lderado Dual Staking Collection...");
    
    // Get the contract factory
    const ElderadoDualStakingCollection = await ethers.getContractFactory("ElderadoDualStakingCollection");
    
    // Collection metadata
    const collectionName = "𝝣lderado Genesis Collection";
    const collectionSymbol = "ELDERADO";
    const baseURI = "https://ipfs.io/ipfs/QmElderadoGenesisCollection/";
    const contractURI = "https://ipfs.io/ipfs/QmElderadoGenesisContract/";
    
    // Token addresses (to be set based on network)
    const heatTokenAddress = process.env.HEAT_TOKEN_ADDRESS || "0x0000000000000000000000000000000000000000";
    const xfgTokenAddress = process.env.XFG_TOKEN_ADDRESS || "0x0000000000000000000000000000000000000000";
    
    console.log("📝 Token Addresses:");
    console.log(`   HEAT Token: ${heatTokenAddress}`);
    console.log(`   XFG Token: ${xfgTokenAddress}`);
    
    // Deploy the contract
    console.log("📝 Deploying dual staking contract...");
    const elderadoCollection = await ElderadoDualStakingCollection.deploy(
        collectionName,
        collectionSymbol,
        baseURI,
        contractURI,
        heatTokenAddress,
        xfgTokenAddress
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
        isGenesis: true,
        stakingType: 0, // Default to HEAT, will be updated based on staking proof
        stakedAmount: 0, // Will be set based on staking proof
        stakingToken: "0x0000000000000000000000000000000000000000" // Will be set based on staking proof
    }));
    
    // Enable minting
    console.log("🔓 Enabling minting...");
    await elderadoCollection.enableMinting();
    
    // Get staking amounts
    const heatStakingAmount = await elderadoCollection.getRequiredStakingAmount(0); // HEAT
    const xfgStakingAmount = await elderadoCollection.getRequiredStakingAmount(1); // XFG
    const equivalenceRatio = await elderadoCollection.getStakingEquivalenceRatio();
    
    console.log("💰 Staking Requirements:");
    console.log(`   HEAT Staking: ${heatStakingAmount.toString()} HEAT tokens`);
    console.log(`   XFG Staking: ${xfgStakingAmount.toString()} XFG tokens`);
    console.log(`   Equivalence Ratio: ${equivalenceRatio.toString()}`);
    
    // Create example staking proofs (for demonstration)
    const [deployer] = await ethers.getSigners();
    const recipients = new Array(21).fill(deployer.address);
    
    console.log("📋 Creating example staking proofs...");
    
    // Create mixed staking proofs (some HEAT, some XFG)
    for (let i = 0; i < 21; i++) {
        const stakingType = i % 2; // Alternate between HEAT (0) and XFG (1)
        const amount = stakingType === 0 ? heatStakingAmount : xfgStakingAmount;
        const txHash = ethers.utils.keccak256(ethers.utils.toUtf8Bytes(`example_tx_${i}`));
        
        // Submit staking proof
        await elderadoCollection.submitStakingProof(stakingType, amount, txHash);
        
        // Verify staking proof
        await elderadoCollection.verifyStakingProof(deployer.address, true);
        
        console.log(`   Created proof ${i + 1}/21: ${stakingType === 0 ? 'HEAT' : 'XFG'} staking`);
    }
    
    // Execute genesis transaction
    console.log("⚡ Executing genesis transaction with dual staking...");
    const genesisTx = await elderadoCollection.executeGenesisTransactionWithStakingProofs(
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
    console.log(`   Staking Amount: ${stats.stakingAmount}`);
    
    // Get validators by staking type
    const heatValidators = await elderadoCollection.getValidatorsByStakingType(0);
    const xfgValidators = await elderadoCollection.getValidatorsByStakingType(1);
    
    console.log("\n🔥 Validator Distribution:");
    console.log(`   HEAT Validators: ${heatValidators.length}`);
    console.log(`   XFG Validators: ${xfgValidators.length}`);
    
    // Get all validators
    const allValidators = await elderadoCollection.getAllValidators();
    console.log("\n👑 Genesis 𝝣lderado Validators:");
    allValidators.forEach((validator, index) => {
        const stakingType = validator.stakingType === 0 ? 'HEAT' : 'XFG';
        console.log(`   ${index + 1}. ${validator.name}`);
        console.log(`      Type: ${validator.validatorType}`);
        console.log(`      Power Level: ${validator.powerLevel}`);
        console.log(`      Rarity Score: ${validator.rarityScore}`);
        console.log(`      Staking Type: ${stakingType}`);
        console.log(`      Staked Amount: ${validator.stakedAmount}`);
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
        stakingAmounts: {
            heat: heatStakingAmount.toString(),
            xfg: xfgStakingAmount.toString(),
            equivalenceRatio: equivalenceRatio.toString()
        },
        tokenAddresses: {
            heat: heatTokenAddress,
            xfg: xfgTokenAddress
        },
        validatorDistribution: {
            heatValidators: heatValidators.length,
            xfgValidators: xfgValidators.length
        },
        validators: allValidators.map(v => ({
            tokenId: v.tokenId.toString(),
            name: v.name,
            validatorType: v.validatorType,
            powerLevel: v.powerLevel.toString(),
            stakeMultiplier: v.stakeMultiplier.toString(),
            specialAbility: v.specialAbility,
            rarityScore: v.rarityScore.toString(),
            isGenesis: v.isGenesis,
            stakingType: v.stakingType,
            stakedAmount: v.stakedAmount.toString(),
            stakingToken: v.stakingToken
        }))
    };
    
    const deploymentPath = path.join(__dirname, "dual_staking_deployment_info.json");
    fs.writeFileSync(deploymentPath, JSON.stringify(deploymentInfo, null, 2));
    console.log(`💾 Deployment info saved to: ${deploymentPath}`);
    
    // Create verification script
    const verifyScript = `
// Verification script for 𝝣lderado Dual Staking Collection
// Run with: npx hardhat verify --network <network> <contract_address> "<collection_name>" "<collection_symbol>" "<base_uri>" "<contract_uri>" "<heat_token_address>" "<xfg_token_address>"

const contractAddress = "${elderadoCollection.address}";
const collectionName = "${collectionName}";
const collectionSymbol = "${collectionSymbol}";
const baseURI = "${baseURI}";
const contractURI = "${contractURI}";
const heatTokenAddress = "${heatTokenAddress}";
const xfgTokenAddress = "${xfgTokenAddress}";

console.log("🔍 Verification command:");
console.log(\`npx hardhat verify --network <network> \${contractAddress} "\${collectionName}" "\${collectionSymbol}" "\${baseURI}" "\${contractURI}" "\${heatTokenAddress}" "\${xfgTokenAddress}"\`);
`;
    
    const verifyScriptPath = path.join(__dirname, "verify_dual_staking_contract.js");
    fs.writeFileSync(verifyScriptPath, verifyScript);
    console.log(`🔍 Verification script saved to: ${verifyScriptPath}`);
    
    console.log("\n🎉 𝝣lderado Dual Staking Collection deployment completed successfully!");
    console.log(`📍 Contract Address: ${elderadoCollection.address}`);
    console.log(`🔗 Transaction Hash: ${genesisTx.hash}`);
    console.log(`📊 Total NFTs Minted: ${stats.totalSupply}`);
    console.log(`⭐ Total Rarity Score: ${stats.totalRarityScore}`);
    console.log(`🔥 HEAT Validators: ${heatValidators.length}`);
    console.log(`🔥 XFG Validators: ${xfgValidators.length}`);
    
    return {
        contractAddress: elderadoCollection.address,
        transactionHash: genesisTx.hash,
        blockNumber: genesisTx.blockNumber,
        totalSupply: stats.totalSupply,
        totalRarityScore: stats.totalRarityScore,
        heatValidators: heatValidators.length,
        xfgValidators: xfgValidators.length
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