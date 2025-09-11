const { ethers } = require("hardhat");
const fs = require("fs");
const path = require("path");

async function main() {
    console.log("🚀 Deploying Genesis Token Contracts...");
    
    const [deployer] = await ethers.getSigners();
    console.log(`📝 Deploying contracts with account: ${deployer.address}`);
    
    // Get treasury addresses from environment or use deployer as fallback
    const digmTreasury = process.env.DIGM_TREASURY_ADDRESS || deployer.address;
    const daoTreasury = process.env.DAO_TREASURY_ADDRESS || deployer.address;
    
    console.log(`🏛️ DIGM Treasury: ${digmTreasury}`);
    console.log(`🏛️ DAO Treasury: ${daoTreasury}`);
    
    const deploymentResults = {};
    
    // 1. Deploy Para Token (1.0 supply, 18 decimals)
    console.log("\n📦 Deploying Para Token...");
    const ParaToken = await ethers.getContractFactory("ParaToken");
    const paraToken = await ParaToken.deploy(daoTreasury);
    await paraToken.deployed();
    
    const paraSupply = await paraToken.totalSupply();
    const paraDecimals = await paraToken.decimals();
    
    console.log(`✅ Para Token deployed to: ${paraToken.address}`);
    console.log(`   Supply: ${ethers.utils.formatUnits(paraSupply, paraDecimals)} PARA`);
    console.log(`   Decimals: ${paraDecimals}`);
    
    deploymentResults.paraToken = {
        address: paraToken.address,
        supply: paraSupply.toString(),
        decimals: paraDecimals,
        treasury: daoTreasury
    };
    
    // 2. Deploy C0LD Token (DAO-governed, capped supply)
    console.log("\n❄️ Deploying C0LD Token...");
    const ColdToken = await ethers.getContractFactory("ColdToken");
    const coldToken = await ColdToken.deploy(daoTreasury);
    await coldToken.deployed();
    
    const coldCap = await coldToken.cap();
    const coldDecimals = await coldToken.decimals();
    const coldSymbol = await coldToken.symbol();
    const coldName = await coldToken.name();
    
    console.log(`✅ C0LD Token deployed to: ${coldToken.address}`);
    console.log(`   Name: ${coldName}`);
    console.log(`   Symbol: ${coldSymbol}`);
    console.log(`   Cap: ${ethers.utils.formatUnits(coldCap, coldDecimals)} ${coldSymbol}`);
    console.log(`   Decimals: ${coldDecimals}`);
    console.log(`   DAO Treasury: ${daoTreasury}`);
    
    // Check roles
    const daoAdminRole = await coldToken.DAO_ADMIN_ROLE();
    const hasDaoAdminRole = await coldToken.hasRole(daoAdminRole, daoTreasury);
    console.log(`   DAO Admin Role: ${hasDaoAdminRole ? '✅ Granted' : '❌ Not granted'}`);
    
    deploymentResults.coldToken = {
        address: coldToken.address,
        name: coldName,
        symbol: coldSymbol,
        cap: coldCap.toString(),
        decimals: coldDecimals,
        daoAdmin: daoTreasury,
        roles: {
            DAO_ADMIN_ROLE: daoAdminRole,
            YIELD_MINTER_ROLE: await coldToken.YIELD_MINTER_ROLE(),
            LP_MINTER_ROLE: await coldToken.LP_MINTER_ROLE(),
            STAKE_MINTER_ROLE: await coldToken.STAKE_MINTER_ROLE()
        }
    };
    
    // 3. Deploy Elderado Genesis Collection (21 NFTs to DIGM Treasury)
    console.log("\n👑 Deploying Elderado Genesis Collection...");
    const ElderadoGenesisCollection = await ethers.getContractFactory("ElderadoGenesisCollection");
    
    const collectionName = "𝝣lderado Genesis Collection";
    const collectionSymbol = "ELDERADO";
    const baseURI = "https://ipfs.io/ipfs/QmElderadoGenesisCollection/";
    const contractURI = "https://ipfs.io/ipfs/QmElderadoGenesisContract/";
    
    const elderadoCollection = await ElderadoGenesisCollection.deploy(
        collectionName,
        collectionSymbol,
        baseURI,
        contractURI
    );
    await elderadoCollection.deployed();
    
    console.log(`✅ Elderado Collection deployed to: ${elderadoCollection.address}`);
    console.log(`   Name: ${collectionName}`);
    console.log(`   Symbol: ${collectionSymbol}`);
    console.log(`   Max Supply: 21 NFTs`);
    console.log(`   Target Treasury: ${digmTreasury}`);
    
    // Load NFT data and execute genesis transaction
    const nftDataPath = path.join(__dirname, "elderado_genesis_collection.json");
    if (fs.existsSync(nftDataPath)) {
        console.log("\n📋 Loading NFT data and executing genesis transaction...");
        
        const nftData = JSON.parse(fs.readFileSync(nftDataPath, "utf8"));
        
        const validatorDataArray = nftData.nfts.map(nft => ({
            tokenId: nft.token_id - 1,
            name: nft.name,
            validatorType: nft.attributes.find(attr => attr.trait_type === "Validator Type").value,
            powerLevel: nft.attributes.find(attr => attr.trait_type === "Power Level").value,
            stakeMultiplier: parseFloat(nft.attributes.find(attr => attr.trait_type === "Stake Multiplier").value),
            specialAbility: nft.attributes.find(attr => attr.trait_type === "Special Ability").value,
            rarityScore: nft.rarity_score,
            isGenesis: true
        }));
        
        const recipients = new Array(21).fill(digmTreasury);
        
        // Enable minting and execute genesis transaction
        await elderadoCollection.enableMinting();
        const genesisTx = await elderadoCollection.executeGenesisTransaction(
            recipients,
            validatorDataArray
        );
        
        await genesisTx.wait();
        console.log(`✅ Genesis transaction confirmed: ${genesisTx.hash}`);
        
        const stats = await elderadoCollection.getCollectionStats();
        console.log(`   Total Supply: ${stats.totalSupply}`);
        console.log(`   Total Rarity Score: ${stats.totalRarityScore}`);
        
        deploymentResults.elderadoCollection = {
            address: elderadoCollection.address,
            name: collectionName,
            symbol: collectionSymbol,
            maxSupply: 21,
            totalSupply: stats.totalSupply.toString(),
            totalRarityScore: stats.totalRarityScore.toString(),
            genesisTxHash: genesisTx.hash,
            treasury: digmTreasury
        };
    } else {
        console.log("⚠️ NFT data file not found, skipping genesis transaction");
        deploymentResults.elderadoCollection = {
            address: elderadoCollection.address,
            name: collectionName,
            symbol: collectionSymbol,
            maxSupply: 21,
            treasury: digmTreasury,
            note: "Genesis transaction not executed - NFT data file missing"
        };
    }
    
    // Save comprehensive deployment info
    const deploymentInfo = {
        timestamp: new Date().toISOString(),
        deployer: deployer.address,
        network: await ethers.provider.getNetwork(),
        contracts: deploymentResults,
        environment: {
            DIGM_TREASURY_ADDRESS: digmTreasury,
            DAO_TREASURY_ADDRESS: daoTreasury
        }
    };
    
    const deploymentPath = path.join(__dirname, "genesis_deployment_info.json");
    fs.writeFileSync(deploymentPath, JSON.stringify(deploymentInfo, null, 2));
    console.log(`\n💾 Deployment info saved to: ${deploymentPath}`);
    
    // Summary
    console.log("\n🎉 Genesis Token Contracts Deployment Summary:");
    console.log("=" .repeat(60));
    console.log(`📦 Para Token (PARA): ${deploymentResults.paraToken.address}`);
    console.log(`   Supply: 1.0 PARA (18 decimals)`);
    console.log(`   Treasury: ${daoTreasury}`);
    console.log("");
    console.log(`❄️ C0LD Token (CD): ${deploymentResults.coldToken.address}`);
    console.log(`   Cap: 20 COLD (12 decimals)`);
    console.log(`   DAO Admin: ${daoTreasury}`);
    console.log(`   Minting Roles: YIELD_MINTER, LP_MINTER, STAKE_MINTER`);
    console.log("");
    console.log(`👑 Elderado Collection: ${deploymentResults.elderadoCollection.address}`);
    console.log(`   Supply: 21 NFTs`);
    console.log(`   Treasury: ${digmTreasury}`);
    console.log("=" .repeat(60));
    
    return deploymentResults;
}

// Execute deployment
main()
    .then((result) => {
        console.log("\n✅ All genesis contracts deployed successfully!");
        process.exit(0);
    })
    .catch((error) => {
        console.error("\n❌ Deployment failed:", error);
        process.exit(1);
    });
