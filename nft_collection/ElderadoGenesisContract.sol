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
 * @title ElderadoGenesisCollection
 * @dev Genesis NFT collection for zkC0DL3 𝝣lderado validators
 * @author zkC0DL3 Foundation
 */
contract ElderadoGenesisCollection is 
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
    uint256 public constant STAKING_AMOUNT = 80000000000; // 80,000,000,000 HEAT tokens (staking requirement)
    
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
    }
    
    // Mapping from token ID to validator data
    mapping(uint256 => ElderadoValidator) public validators;
    
    // Events
    event ElderadoMinted(
        uint256 indexed tokenId,
        address indexed to,
        string name,
        string validatorType,
        uint256 powerLevel
    );
    
    event GenesisTransactionExecuted(
        uint256 indexed blockNumber,
        uint256 totalMinted,
        uint256 totalValue
    );
    
    constructor(
        string memory name,
        string memory symbol,
        string memory baseURI,
        string memory contractURI_
    ) ERC721(name, symbol) {
        _baseTokenURI = baseURI;
        _contractURI = contractURI_;
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
     * @dev Mint a genesis 𝝣lderado NFT
     * @param to The address to mint to
     * @param validatorData The validator data
     */
    function mintElderado(
        address to,
        ElderadoValidator memory validatorData
    ) external onlyOwner nonReentrant {
        require(mintingEnabled, "Minting not enabled");
        require(_tokenIdCounter.current() < MAX_SUPPLY, "Max supply reached");
        require(to != address(0), "Invalid recipient");
        
        uint256 tokenId = _tokenIdCounter.current();
        _tokenIdCounter.increment();
        
        // Store validator data
        validators[tokenId] = ElderadoValidator({
            tokenId: tokenId,
            name: validatorData.name,
            validatorType: validatorData.validatorType,
            powerLevel: validatorData.powerLevel,
            stakeMultiplier: validatorData.stakeMultiplier,
            specialAbility: validatorData.specialAbility,
            rarityScore: validatorData.rarityScore,
            isGenesis: true
        });
        
        _safeMint(to, tokenId);
        
        emit ElderadoMinted(
            tokenId,
            to,
            validatorData.name,
            validatorData.validatorType,
            validatorData.powerLevel
        );
    }
    
    /**
     * @dev Execute the genesis transaction - mint all 21 𝝣lderado NFTs
     * @param recipients Array of recipient addresses
     * @param validatorDataArray Array of validator data
     */
    function executeGenesisTransaction(
        address[] calldata recipients,
        ElderadoValidator[] calldata validatorDataArray
    ) external onlyOwner nonReentrant {
        require(mintingEnabled, "Minting not enabled");
        require(recipients.length == MAX_SUPPLY, "Invalid recipients length");
        require(validatorDataArray.length == MAX_SUPPLY, "Invalid validator data length");
        require(_tokenIdCounter.current() == 0, "Genesis already executed");
        
        uint256 totalValue = 0;
        
        for (uint256 i = 0; i < MAX_SUPPLY; i++) {
            uint256 tokenId = _tokenIdCounter.current();
            _tokenIdCounter.increment();
            
            // Store validator data
            validators[tokenId] = ElderadoValidator({
                tokenId: tokenId,
                name: validatorDataArray[i].name,
                validatorType: validatorDataArray[i].validatorType,
                powerLevel: validatorDataArray[i].powerLevel,
                stakeMultiplier: validatorDataArray[i].stakeMultiplier,
                specialAbility: validatorDataArray[i].specialAbility,
                rarityScore: validatorDataArray[i].rarityScore,
                isGenesis: true
            });
            
            _safeMint(recipients[i], tokenId);
            totalValue += MINT_PRICE;
            
            emit ElderadoMinted(
                tokenId,
                recipients[i],
                validatorDataArray[i].name,
                validatorDataArray[i].validatorType,
                validatorDataArray[i].powerLevel
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
     * @dev Get all validator data
     * @return validatorsArray Array of all validator data
     */
    function getAllValidators() 
        external 
        view 
        returns (ElderadoValidator[] memory validatorsArray) 
    {
        uint256 totalSupply = totalSupply();
        validatorsArray = new ElderadoValidator[](totalSupply);
        
        for (uint256 i = 0; i < totalSupply; i++) {
            uint256 tokenId = tokenByIndex(i);
            validatorsArray[i] = validators[tokenId];
        }
        
        return validatorsArray;
    }
    
    /**
     * @dev Get validator by type
     * @param validatorType The validator type to search for
     * @return validatorsArray Array of validators with the specified type
     */
    function getValidatorsByType(string memory validatorType) 
        external 
        view 
        returns (ElderadoValidator[] memory validatorsArray) 
    {
        uint256 count = 0;
        uint256 totalSupply = totalSupply();
        
        // Count validators with the specified type
        for (uint256 i = 0; i < totalSupply; i++) {
            uint256 tokenId = tokenByIndex(i);
            if (keccak256(bytes(validators[tokenId].validatorType)) == keccak256(bytes(validatorType))) {
                count++;
            }
        }
        
        // Create array with correct size
        validatorsArray = new ElderadoValidator[](count);
        uint256 index = 0;
        
        // Populate array
        for (uint256 i = 0; i < totalSupply; i++) {
            uint256 tokenId = tokenByIndex(i);
            if (keccak256(bytes(validators[tokenId].validatorType)) == keccak256(bytes(validatorType))) {
                validatorsArray[index] = validators[tokenId];
                index++;
            }
        }
        
        return validatorsArray;
    }
    
    /**
     * @dev Get validators by rarity score range
     * @param minScore Minimum rarity score
     * @param maxScore Maximum rarity score
     * @return validatorsArray Array of validators within the score range
     */
    function getValidatorsByRarityScore(uint256 minScore, uint256 maxScore) 
        external 
        view 
        returns (ElderadoValidator[] memory validatorsArray) 
    {
        uint256 count = 0;
        uint256 totalSupply = totalSupply();
        
        // Count validators within score range
        for (uint256 i = 0; i < totalSupply; i++) {
            uint256 tokenId = tokenByIndex(i);
            if (validators[tokenId].rarityScore >= minScore && validators[tokenId].rarityScore <= maxScore) {
                count++;
            }
        }
        
        // Create array with correct size
        validatorsArray = new ElderadoValidator[](count);
        uint256 index = 0;
        
        // Populate array
        for (uint256 i = 0; i < totalSupply; i++) {
            uint256 tokenId = tokenByIndex(i);
            if (validators[tokenId].rarityScore >= minScore && validators[tokenId].rarityScore <= maxScore) {
                validatorsArray[index] = validators[tokenId];
                index++;
            }
        }
        
        return validatorsArray;
    }
    
    /**
     * @dev Calculate total rarity score of all validators
     * @return totalScore Total rarity score
     */
    function getTotalRarityScore() external view returns (uint256 totalScore) {
        uint256 totalSupply = totalSupply();
        
        for (uint256 i = 0; i < totalSupply; i++) {
            uint256 tokenId = tokenByIndex(i);
            totalScore += validators[tokenId].rarityScore;
        }
        
        return totalScore;
    }
    
    /**
     * @dev Get average rarity score
     * @return averageScore Average rarity score
     */
    function getAverageRarityScore() external view returns (uint256 averageScore) {
        uint256 totalSupply = totalSupply();
        if (totalSupply == 0) return 0;
        
        uint256 totalScore = this.getTotalRarityScore();
        return totalScore / totalSupply;
    }
    
    /**
     * @dev Get collection statistics
     * @return totalSupply Total number of minted NFTs
     * @return totalRarityScore Total rarity score
     * @return averageRarityScore Average rarity score
     * @return mintingEnabled Current minting status
     * @return stakingAmount Required staking amount for transfers
     */
    function getCollectionStats() 
        external 
        view 
        returns (
            uint256 totalSupply,
            uint256 totalRarityScore,
            uint256 averageRarityScore,
            bool mintingEnabled,
            uint256 stakingAmount
        ) 
    {
        totalSupply = totalSupply();
        totalRarityScore = this.getTotalRarityScore();
        averageRarityScore = this.getAverageRarityScore();
        mintingEnabled = mintingEnabled;
        stakingAmount = STAKING_AMOUNT;
    }
    
    /**
     * @dev Get the required staking amount for NFT transfers
     * @return The staking amount required to transfer NFTs
     */
    function getStakingAmount() external pure returns (uint256) {
        return STAKING_AMOUNT;
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
        require(msg.value == STAKING_AMOUNT, "NFT can only be sold for the staking amount");
        
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
        require(msg.value == STAKING_AMOUNT, "NFT can only be sold for the staking amount");
        
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
        require(msg.value == STAKING_AMOUNT, "NFT can only be sold for the staking amount");
        
        super.safeTransferFrom(from, to, tokenId, data);
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