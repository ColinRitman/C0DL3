// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "@openzeppelin/contracts/token/ERC721/ERC721.sol";
import "@openzeppelin/contracts/token/ERC721/extensions/ERC721Enumerable.sol";
import "@openzeppelin/contracts/token/ERC721/extensions/ERC721URIStorage.sol";
import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/utils/Counters.sol";
import "@openzeppelin/contracts/utils/Strings.sol";

/**
 * @title ElderadoDualStakingCollection
 * @dev Genesis NFT collection for zkC0DL3 𝝣lderado validators with dual staking options
 * @author zkC0DL3 Foundation
 */
contract ElderadoDualStakingCollection is 
    ERC721, 
    ERC721Enumerable, 
    ERC721URIStorage, 
    Ownable, 
    ReentrancyGuard 
{
    using Counters for Counters.Counter;
    using Strings for uint256;

    // Collection constants
    uint256 public constant MAX_SUPPLY = 21;
    uint256 public constant MINT_PRICE = 0.1 ether;
    uint256 public constant ROYALTY_PERCENTAGE = 500; // 5% in basis points
    
    // Dual staking amounts (equivalent values)
    uint256 public constant HEAT_STAKING_AMOUNT = 80000000000; // 80 billion HEAT tokens
    uint256 public constant XFG_STAKING_AMOUNT = 8000; // 8,000 XFG tokens
    uint256 public constant STAKING_EQUIVALENCE_RATIO = 10000000; // 80B HEAT / 8K XFG = 10M
    
    // Token addresses (to be set during deployment)
    address public heatTokenAddress;
    address public xfgTokenAddress;
    
    // State variables
    Counters.Counter private _tokenIdCounter;
    string private _baseTokenURI;
    string private _contractURI;
    bool public mintingEnabled = false;
    uint256 public mintStartTime;
    
    // Validator data structure
    struct ElderadoValidator {
        uint256 tokenId;
        string name;
        string validatorType;
        uint256 powerLevel;
        uint256 stakeMultiplier;
        string specialAbility;
        uint256 rarityScore;
        bool isGenesis;
        StakingType stakingType;
        uint256 stakedAmount;
        address stakingToken;
    }
    
    // Staking type enum
    enum StakingType {
        HEAT,
        XFG
    }
    
    // Mapping from token ID to validator data
    mapping(uint256 => ElderadoValidator) public validators;
    
    // Mapping from address to staking proof
    mapping(address => StakingProof) public stakingProofs;
    
    // Staking proof structure
    struct StakingProof {
        StakingType stakingType;
        uint256 amount;
        address tokenAddress;
        uint256 blockNumber;
        bytes32 transactionHash;
        bool verified;
        uint256 verificationTimestamp;
    }
    
    // Events
    event ElderadoMinted(
        uint256 indexed tokenId,
        address indexed to,
        string name,
        string validatorType,
        uint256 powerLevel,
        StakingType stakingType
    );
    
    event GenesisTransactionExecuted(
        uint256 indexed blockNumber,
        uint256 totalMinted,
        uint256 totalValue
    );
    
    event StakingProofSubmitted(
        address indexed validator,
        StakingType stakingType,
        uint256 amount,
        bytes32 transactionHash
    );
    
    event StakingProofVerified(
        address indexed validator,
        StakingType stakingType,
        uint256 amount,
        bool verified
    );
    
    constructor(
        string memory name,
        string memory symbol,
        string memory baseURI,
        string memory contractURI_,
        address _heatTokenAddress,
        address _xfgTokenAddress
    ) ERC721(name, symbol) {
        _baseTokenURI = baseURI;
        _contractURI = contractURI_;
        heatTokenAddress = _heatTokenAddress;
        xfgTokenAddress = _xfgTokenAddress;
        mintStartTime = block.timestamp;
    }
    
    /**
     * @dev Enable minting for the genesis collection
     */
    function enableMinting() external onlyOwner {
        mintingEnabled = true;
    }
    
    /**
     * @dev Disable minting
     */
    function disableMinting() external onlyOwner {
        mintingEnabled = false;
    }
    
    /**
     * @dev Submit staking proof for Fuego chain verification
     * @param stakingType The type of staking (HEAT or XFG)
     * @param amount The amount staked
     * @param transactionHash The transaction hash on Fuego chain
     */
    function submitStakingProof(
        StakingType stakingType,
        uint256 amount,
        bytes32 transactionHash
    ) external {
        require(stakingType == StakingType.HEAT || stakingType == StakingType.XFG, "Invalid staking type");
        
        if (stakingType == StakingType.HEAT) {
            require(amount == HEAT_STAKING_AMOUNT, "Invalid HEAT staking amount");
        } else {
            require(amount == XFG_STAKING_AMOUNT, "Invalid XFG staking amount");
        }
        
        stakingProofs[msg.sender] = StakingProof({
            stakingType: stakingType,
            amount: amount,
            tokenAddress: stakingType == StakingType.HEAT ? heatTokenAddress : xfgTokenAddress,
            blockNumber: block.number,
            transactionHash: transactionHash,
            verified: false,
            verificationTimestamp: 0
        });
        
        emit StakingProofSubmitted(msg.sender, stakingType, amount, transactionHash);
    }
    
    /**
     * @dev Verify staking proof (only owner can verify)
     * @param validator The validator address
     * @param verified Whether the proof is verified
     */
    function verifyStakingProof(address validator, bool verified) external onlyOwner {
        require(stakingProofs[validator].amount > 0, "No staking proof found");
        
        stakingProofs[validator].verified = verified;
        stakingProofs[validator].verificationTimestamp = block.timestamp;
        
        emit StakingProofVerified(
            validator,
            stakingProofs[validator].stakingType,
            stakingProofs[validator].amount,
            verified
        );
    }
    
    /**
     * @dev Mint a genesis 𝝣lderado NFT with staking proof
     * @param to The address to mint to
     * @param validatorData The validator data
     */
    function mintElderadoWithStakingProof(
        address to,
        ElderadoValidator memory validatorData
    ) external onlyOwner nonReentrant {
        require(mintingEnabled, "Minting not enabled");
        require(_tokenIdCounter.current() < MAX_SUPPLY, "Max supply reached");
        require(to != address(0), "Invalid recipient");
        require(stakingProofs[to].verified, "Staking proof not verified");
        
        uint256 tokenId = _tokenIdCounter.current();
        _tokenIdCounter.increment();
        
        // Store validator data with staking information
        validators[tokenId] = ElderadoValidator({
            tokenId: tokenId,
            name: validatorData.name,
            validatorType: validatorData.validatorType,
            powerLevel: validatorData.powerLevel,
            stakeMultiplier: validatorData.stakeMultiplier,
            specialAbility: validatorData.specialAbility,
            rarityScore: validatorData.rarityScore,
            isGenesis: true,
            stakingType: stakingProofs[to].stakingType,
            stakedAmount: stakingProofs[to].amount,
            stakingToken: stakingProofs[to].tokenAddress
        });
        
        _safeMint(to, tokenId);
        
        emit ElderadoMinted(
            tokenId,
            to,
            validatorData.name,
            validatorData.validatorType,
            validatorData.powerLevel,
            stakingProofs[to].stakingType
        );
    }
    
    /**
     * @dev Execute the genesis transaction - mint all 21 𝝣lderado NFTs
     * @param recipients Array of recipient addresses
     * @param validatorDataArray Array of validator data
     */
    function executeGenesisTransactionWithStakingProofs(
        address[] calldata recipients,
        ElderadoValidator[] calldata validatorDataArray
    ) external onlyOwner nonReentrant {
        require(mintingEnabled, "Minting not enabled");
        require(recipients.length == MAX_SUPPLY, "Invalid recipients length");
        require(validatorDataArray.length == MAX_SUPPLY, "Invalid validator data length");
        require(_tokenIdCounter.current() == 0, "Genesis already executed");
        
        uint256 totalValue = 0;
        
        for (uint256 i = 0; i < MAX_SUPPLY; i++) {
            require(stakingProofs[recipients[i]].verified, "Staking proof not verified");
            
            uint256 tokenId = _tokenIdCounter.current();
            _tokenIdCounter.increment();
            
            // Store validator data with staking information
            validators[tokenId] = ElderadoValidator({
                tokenId: tokenId,
                name: validatorDataArray[i].name,
                validatorType: validatorDataArray[i].validatorType,
                powerLevel: validatorDataArray[i].powerLevel,
                stakeMultiplier: validatorDataArray[i].stakeMultiplier,
                specialAbility: validatorDataArray[i].specialAbility,
                rarityScore: validatorDataArray[i].rarityScore,
                isGenesis: true,
                stakingType: stakingProofs[recipients[i]].stakingType,
                stakedAmount: stakingProofs[recipients[i]].amount,
                stakingToken: stakingProofs[recipients[i]].tokenAddress
            });
            
            _safeMint(recipients[i], tokenId);
            totalValue += MINT_PRICE;
            
            emit ElderadoMinted(
                tokenId,
                recipients[i],
                validatorDataArray[i].name,
                validatorDataArray[i].validatorType,
                validatorDataArray[i].powerLevel,
                stakingProofs[recipients[i]].stakingType
            );
        }
        
        emit GenesisTransactionExecuted(
            block.number,
            MAX_SUPPLY,
            totalValue
        );
    }
    
    /**
     * @dev Get validator data by token ID
     * @param tokenId The token ID
     * @return validator The validator data
     */
    function getValidatorData(uint256 tokenId) 
        external 
        view 
        returns (ElderadoValidator memory validator) 
    {
        require(_exists(tokenId), "Token does not exist");
        return validators[tokenId];
    }
    
    /**
     * @dev Get staking proof for an address
     * @param validator The validator address
     * @return proof The staking proof
     */
    function getStakingProof(address validator) 
        external 
        view 
        returns (StakingProof memory proof) 
    {
        return stakingProofs[validator];
    }
    
    /**
     * @dev Get validators by staking type
     * @param stakingType The staking type to filter by
     * @return validatorsArray Array of validators with the specified staking type
     */
    function getValidatorsByStakingType(StakingType stakingType) 
        external 
        view 
        returns (ElderadoValidator[] memory validatorsArray) 
    {
        uint256 count = 0;
        uint256 totalSupply = totalSupply();
        
        // Count validators with the specified staking type
        for (uint256 i = 0; i < totalSupply; i++) {
            uint256 tokenId = tokenByIndex(i);
            if (validators[tokenId].stakingType == stakingType) {
                count++;
            }
        }
        
        // Create array with correct size
        validatorsArray = new ElderadoValidator[](count);
        uint256 index = 0;
        
        // Populate array
        for (uint256 i = 0; i < totalSupply; i++) {
            uint256 tokenId = tokenByIndex(i);
            if (validators[tokenId].stakingType == stakingType) {
                validatorsArray[index] = validators[tokenId];
                index++;
            }
        }
        
        return validatorsArray;
    }
    
    /**
     * @dev Get staking equivalence ratio
     * @return The ratio between HEAT and XFG staking amounts
     */
    function getStakingEquivalenceRatio() external pure returns (uint256) {
        return STAKING_EQUIVALENCE_RATIO;
    }
    
    /**
     * @dev Get required staking amount for a specific staking type
     * @param stakingType The staking type
     * @return The required staking amount
     */
    function getRequiredStakingAmount(StakingType stakingType) external pure returns (uint256) {
        if (stakingType == StakingType.HEAT) {
            return HEAT_STAKING_AMOUNT;
        } else {
            return XFG_STAKING_AMOUNT;
        }
    }
    
    /**
     * @dev Override transfer to enforce staking amount restriction
     */
    function _beforeTokenTransfer(
        address from,
        address to,
        uint256 tokenId,
        uint256 batchSize
    ) internal override(ERC721, ERC721Enumerable) {
        // Allow minting (from address(0)) and burning (to address(0))
        if (from == address(0) || to == address(0)) {
            super._beforeTokenTransfer(from, to, tokenId, batchSize);
            return;
        }
        
        // For transfers between users, enforce staking amount restriction
        ElderadoValidator memory validator = validators[tokenId];
        uint256 requiredAmount = validator.stakingType == StakingType.HEAT 
            ? HEAT_STAKING_AMOUNT 
            : XFG_STAKING_AMOUNT;
            
        require(msg.value == requiredAmount, "NFT can only be sold for the staking amount");
        
        super._beforeTokenTransfer(from, to, tokenId, batchSize);
    }
    
    /**
     * @dev Override transferFrom to enforce staking amount restriction
     */
    function transferFrom(
        address from,
        address to,
        uint256 tokenId
    ) public payable override(ERC721, IERC721) {
        // Allow minting (from address(0)) and burning (to address(0))
        if (from == address(0) || to == address(0)) {
            super.transferFrom(from, to, tokenId);
            return;
        }
        
        // For transfers between users, enforce staking amount restriction
        ElderadoValidator memory validator = validators[tokenId];
        uint256 requiredAmount = validator.stakingType == StakingType.HEAT 
            ? HEAT_STAKING_AMOUNT 
            : XFG_STAKING_AMOUNT;
            
        require(msg.value == requiredAmount, "NFT can only be sold for the staking amount");
        
        super.transferFrom(from, to, tokenId);
    }
    
    /**
     * @dev Override safeTransferFrom to enforce staking amount restriction
     */
    function safeTransferFrom(
        address from,
        address to,
        uint256 tokenId,
        bytes memory data
    ) public payable override(ERC721, IERC721) {
        // Allow minting (from address(0)) and burning (to address(0))
        if (from == address(0) || to == address(0)) {
            super.safeTransferFrom(from, to, tokenId, data);
            return;
        }
        
        // For transfers between users, enforce staking amount restriction
        ElderadoValidator memory validator = validators[tokenId];
        uint256 requiredAmount = validator.stakingType == StakingType.HEAT 
            ? HEAT_STAKING_AMOUNT 
            : XFG_STAKING_AMOUNT;
            
        require(msg.value == requiredAmount, "NFT can only be sold for the staking amount");
        
        super.safeTransferFrom(from, to, tokenId, data);
    }
    
    /**
     * @dev Set token addresses
     * @param _heatTokenAddress HEAT token address
     * @param _xfgTokenAddress XFG token address
     */
    function setTokenAddresses(address _heatTokenAddress, address _xfgTokenAddress) external onlyOwner {
        heatTokenAddress = _heatTokenAddress;
        xfgTokenAddress = _xfgTokenAddress;
    }
    
    /**
     * @dev Set base URI for token metadata
     * @param baseURI The new base URI
     */
    function setBaseURI(string memory baseURI) external onlyOwner {
        _baseTokenURI = baseURI;
    }
    
    /**
     * @dev Set contract URI for marketplace metadata
     * @param contractURI_ The new contract URI
     */
    function setContractURI(string memory contractURI_) external onlyOwner {
        _contractURI = contractURI_;
    }
    
    /**
     * @dev Withdraw contract balance
     */
    function withdraw() external onlyOwner {
        uint256 balance = address(this).balance;
        require(balance > 0, "No funds to withdraw");
        
        (bool success, ) = payable(owner()).call{value: balance}("");
        require(success, "Withdrawal failed");
    }
    
    /**
     * @dev Get contract URI for marketplace metadata
     * @return The contract URI
     */
    function contractURI() external view returns (string memory) {
        return _contractURI;
    }
    
    /**
     * @dev Get token URI
     * @param tokenId The token ID
     * @return The token URI
     */
    function tokenURI(uint256 tokenId) 
        public 
        view 
        override(ERC721, ERC721URIStorage) 
        returns (string memory) 
    {
        require(_exists(tokenId), "Token does not exist");
        
        string memory baseURI = _baseURI();
        return bytes(baseURI).length > 0 
            ? string(abi.encodePacked(baseURI, tokenId.toString(), ".json"))
            : "";
    }
    
    /**
     * @dev Get base URI
     * @return The base URI
     */
    function _baseURI() internal view override returns (string memory) {
        return _baseTokenURI;
    }
    
    /**
     * @dev Override required for multiple inheritance
     */
    function _burn(uint256 tokenId) internal override(ERC721, ERC721URIStorage) {
        super._burn(tokenId);
    }
    
    /**
     * @dev Override required for multiple inheritance
     */
    function supportsInterface(bytes4 interfaceId)
        public
        view
        override(ERC721, ERC721Enumerable, ERC721URIStorage)
        returns (bool)
    {
        return super.supportsInterface(interfaceId);
    }
}