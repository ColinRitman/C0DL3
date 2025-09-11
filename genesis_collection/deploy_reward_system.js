const { ethers } = require("hardhat");
const fs = require("fs");
const path = require("path");

async function main() {
    console.log("🚀 Deploying C0LD Token Reward Distribution System...");
    
    const [deployer] = await ethers.getSigners();
    console.log(`📝 Deploying contracts with account: ${deployer.address}`);
    
    // Get treasury addresses from environment or use deployer as fallback
    const digmTreasury = process.env.DIGM_TREASURY_ADDRESS || deployer.address;
    const daoTreasury = process.env.DAO_TREASURY_ADDRESS || deployer.address;
    
    console.log(`🏛️ DIGM Treasury: ${digmTreasury}`);
    console.log(`🏛️ DAO Treasury: ${daoTreasury}`);
    
    const deploymentResults = {};
    
    // 1. Deploy PARA Token and Distributor
    console.log("\n📦 Deploying PARA Token and Distributor...");
    
    const ParaToken = await ethers.getContractFactory("ParaToken");
    const paraToken = await ParaToken.deploy(daoTreasury);
    await paraToken.deployed();
    
    const ParaDistributorContract = await ethers.getContractFactory("ParaDistributorContract");
    const paraDistributor = await ParaDistributorContract.deploy(paraToken.address, daoTreasury);
    await paraDistributor.deployed();
    
    // Transfer PARA tokens to distributor
    await paraToken.transfer(paraDistributor.address, await paraToken.totalSupply());
    
    const paraSupply = await paraToken.totalSupply();
    const paraDecimals = await paraToken.decimals();
    
    console.log(`✅ PARA Token deployed to: ${paraToken.address}`);
    console.log(`✅ PARA Distributor deployed to: ${paraDistributor.address}`);
    console.log(`   Supply: ${ethers.utils.formatUnits(paraSupply, paraDecimals)} PARA`);
    console.log(`   Decimals: ${paraDecimals}`);
    
    deploymentResults.paraToken = {
        address: paraToken.address,
        supply: paraSupply.toString(),
        decimals: paraDecimals,
        treasury: daoTreasury
    };
    
    deploymentResults.paraDistributor = {
        address: paraDistributor.address,
        paraToken: paraToken.address,
        treasury: daoTreasury
    };
    
    // 2. Deploy C0LD Token
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
    console.log(`   Max Circulating Supply: ${ethers.utils.formatUnits(coldCap, coldDecimals)} ${coldSymbol}`);
    console.log(`   Decimals: ${coldDecimals}`);
    console.log(`   DAO Treasury: ${daoTreasury}`);
    console.log(`   Circulating Supply Model: Burns free up minting space`);
    
    deploymentResults.coldToken = {
        address: coldToken.address,
        name: coldName,
        symbol: coldSymbol,
        cap: coldCap.toString(),
        decimals: coldDecimals,
        daoAdmin: daoTreasury,
        roles: {
            DAO_ADMIN_ROLE: await coldToken.DAO_ADMIN_ROLE(),
            YIELD_MINTER_ROLE: await coldToken.YIELD_MINTER_ROLE(),
            LP_MINTER_ROLE: await coldToken.LP_MINTER_ROLE(),
            STAKE_MINTER_ROLE: await coldToken.STAKE_MINTER_ROLE(),
            COMMUNITY_LISTING_ROLE: await coldToken.COMMUNITY_LISTING_ROLE()
        },
        burning: {
            communityListingBurnAmount: await coldToken.COMMUNITY_LISTING_BURN_AMOUNT(),
            totalBurned: await coldToken.getTotalBurned(),
            communityListingsCount: await coldToken.getCommunityListingsCount()
        }
    };
    
    // 3. Deploy CD Yield Reward Contract
    console.log("\n💰 Deploying CD Yield Reward Contract...");
    
    const CDYieldRewardContract = await ethers.getContractFactory("CDYieldRewardContract");
    const cdYieldReward = await CDYieldRewardContract.deploy(coldToken.address, daoTreasury);
    await cdYieldReward.deployed();
    
    // Grant YIELD_MINTER_ROLE to CD Yield Reward Contract
    await coldToken.grantRole(await coldToken.YIELD_MINTER_ROLE(), cdYieldReward.address);
    
    console.log(`✅ CD Yield Reward Contract deployed to: ${cdYieldReward.address}`);
    console.log(`   Allocation: 8 COLD tokens over 7 years`);
    console.log(`   Trigger: XFG CDyield deposits (txextra tag 0x09)`);
    
    deploymentResults.cdYieldReward = {
        address: cdYieldReward.address,
        coldToken: coldToken.address,
        allocation: "8 COLD tokens",
        duration: "7 years",
        trigger: "XFG CDyield deposits (txextra tag 0x09)"
    };
    
    // 4. Deploy LP Liquidity Reward Contract
    console.log("\n💧 Deploying LP Liquidity Reward Contract...");
    
    // Token addresses (to be set based on network)
    const heatTokenAddress = process.env.HEAT_TOKEN_ADDRESS || "0x0000000000000000000000000000000000000000";
    const ethTokenAddress = process.env.ETH_TOKEN_ADDRESS || "0x0000000000000000000000000000000000000000";
    const lpTokenAddress = process.env.LP_TOKEN_ADDRESS || "0x0000000000000000000000000000000000000000";
    
    const LPLiquidityRewardContract = await ethers.getContractFactory("LPLiquidityRewardContract");
    const lpReward = await LPLiquidityRewardContract.deploy(
        coldToken.address,
        heatTokenAddress,
        ethTokenAddress,
        lpTokenAddress,
        daoTreasury
    );
    await lpReward.deployed();
    
    // Grant LP_MINTER_ROLE to LP Reward Contract
    await coldToken.grantRole(await coldToken.LP_MINTER_ROLE(), lpReward.address);
    
    console.log(`✅ LP Liquidity Reward Contract deployed to: ${lpReward.address}`);
    console.log(`   Allocation: 7 COLD tokens over 8 years`);
    console.log(`   Trigger: L1 HEAT/ETH Liquidity Providers`);
    console.log(`   HEAT Token: ${heatTokenAddress}`);
    console.log(`   ETH Token: ${ethTokenAddress}`);
    console.log(`   LP Token: ${lpTokenAddress}`);
    
    deploymentResults.lpReward = {
        address: lpReward.address,
        coldToken: coldToken.address,
        heatToken: heatTokenAddress,
        ethToken: ethTokenAddress,
        lpToken: lpTokenAddress,
        allocation: "7 COLD tokens",
        duration: "8 years",
        trigger: "L1 HEAT/ETH Liquidity Providers"
    };
    
    // 5. Deploy Elderado Staking Reward Contract
    console.log("\n👑 Deploying Elderado Staking Reward Contract...");
    
    const elderadoNFTAddress = process.env.ELDERADO_NFT_ADDRESS || "0x0000000000000000000000000000000000000000";
    
    const ElderadoStakingRewardContract = await ethers.getContractFactory("ElderadoStakingRewardContract");
    const elderadoStaking = await ElderadoStakingRewardContract.deploy(
        coldToken.address,
        elderadoNFTAddress,
        daoTreasury
    );
    await elderadoStaking.deployed();
    
    // Grant STAKE_MINTER_ROLE to Elderado Staking Contract
    await coldToken.grantRole(await coldToken.STAKE_MINTER_ROLE(), elderadoStaking.address);
    
    console.log(`✅ Elderado Staking Reward Contract deployed to: ${elderadoStaking.address}`);
    console.log(`   Allocation: 5 COLD tokens over 10 years`);
    console.log(`   Trigger: Elderado NFT Staking`);
    console.log(`   Elderado NFT: ${elderadoNFTAddress}`);
    
    deploymentResults.elderadoStaking = {
        address: elderadoStaking.address,
        coldToken: coldToken.address,
        elderadoNFT: elderadoNFTAddress,
        allocation: "5 COLD tokens",
        duration: "10 years",
        trigger: "Elderado NFT Staking"
    };
    
    // 6. Deploy Elderado Genesis Collection (if not already deployed)
    console.log("\n🎨 Deploying Elderado Genesis Collection...");
    
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
    
    deploymentResults.elderadoCollection = {
        address: elderadoCollection.address,
        name: collectionName,
        symbol: collectionSymbol,
        maxSupply: 21,
        treasury: digmTreasury
    };
    
    // Save comprehensive deployment info
    const deploymentInfo = {
        timestamp: new Date().toISOString(),
        deployer: deployer.address,
        network: await ethers.provider.getNetwork(),
        contracts: deploymentResults,
        environment: {
            DIGM_TREASURY_ADDRESS: digmTreasury,
            DAO_TREASURY_ADDRESS: daoTreasury,
            HEAT_TOKEN_ADDRESS: heatTokenAddress,
            ETH_TOKEN_ADDRESS: ethTokenAddress,
            LP_TOKEN_ADDRESS: lpTokenAddress,
            ELDERADO_NFT_ADDRESS: elderadoNFTAddress
        },
        rewardAllocation: {
            totalColdTokens: "20 COLD",
            cdYieldRewards: "8 COLD (40%) - 7 years",
            lpRewards: "7 COLD (35%) - 8 years", 
            elderadoStaking: "5 COLD (25%) - 10 years",
            paraStreaming: "1.0 PARA - 10 years",
            burning: "1 COLD per community token listing"
        }
    };
    
    const deploymentPath = path.join(__dirname, "reward_system_deployment_info.json");
    fs.writeFileSync(deploymentPath, JSON.stringify(deploymentInfo, null, 2));
    console.log(`\n💾 Deployment info saved to: ${deploymentPath}`);
    
    // Summary
    console.log("\n🎉 C0LD Token Reward Distribution System Deployment Summary:");
    console.log("=" .repeat(80));
    console.log(`📦 PARA Token: ${deploymentResults.paraToken.address}`);
    console.log(`   Supply: 1.0 PARA (18 decimals) → Streamed over 10 years`);
    console.log(`   Distributor: ${deploymentResults.paraDistributor.address}`);
    console.log("");
    console.log(`❄️ C0LD Token: ${deploymentResults.coldToken.address}`);
    console.log(`   Max Circulating Supply: 20 COLD (12 decimals) - None minted at launch`);
    console.log(`   DAO Admin: ${daoTreasury}`);
    console.log(`   Burning: 1 COLD per community token listing`);
    console.log(`   Model: Burns free up minting space to maintain max circulating supply`);
    console.log("");
    console.log(`💰 CD Yield Rewards: ${deploymentResults.cdYieldReward.address}`);
    console.log(`   Allocation: 8 COLD tokens over 7 years`);
    console.log(`   Trigger: XFG CDyield deposits (txextra tag 0x09)`);
    console.log("");
    console.log(`💧 LP Liquidity Rewards: ${deploymentResults.lpReward.address}`);
    console.log(`   Allocation: 7 COLD tokens over 8 years`);
    console.log(`   Trigger: L1 HEAT/ETH Liquidity Providers`);
    console.log("");
    console.log(`👑 Elderado Staking Rewards: ${deploymentResults.elderadoStaking.address}`);
    console.log(`   Allocation: 5 COLD tokens over 10 years`);
    console.log(`   Trigger: Elderado NFT Staking`);
    console.log("");
    console.log(`🎨 Elderado Collection: ${deploymentResults.elderadoCollection.address}`);
    console.log(`   Supply: 21 NFTs → Minted to ${digmTreasury}`);
    console.log("=" .repeat(80));
    
    return deploymentResults;
}

// Execute deployment
main()
    .then((result) => {
        console.log("\n✅ All reward system contracts deployed successfully!");
        console.log("🚀 Ready for 5-10 year reward distribution!");
        process.exit(0);
    })
    .catch((error) => {
        console.error("\n❌ Deployment failed:", error);
        process.exit(1);
    });
