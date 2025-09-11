// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/AccessControlEnumerable.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "./ColdToken.sol";

/// @title Community Token Listing Example – Demonstrates C0LD token burning for community listings
/// @notice Example contract showing how to integrate with C0LD token burning mechanism
contract CommunityTokenListingExample is AccessControlEnumerable, ReentrancyGuard {
    
    // --- Roles ---
    bytes32 public constant DAO_ADMIN_ROLE = keccak256("DAO_ADMIN_ROLE");
    bytes32 public constant COMMUNITY_LISTING_ROLE = keccak256("COMMUNITY_LISTING_ROLE");
    
    // --- State Variables ---
    ColdToken public immutable coldToken;
    
    struct CommunityToken {
        address tokenAddress;
        string symbol;
        string name;
        uint256 listingTimestamp;
        address listedBy;
        bool isActive;
    }
    
    mapping(address => CommunityToken) public communityTokens;
    address[] public listedTokens;
    
    // --- Events ---
    event CommunityTokenListed(
        address indexed tokenAddress,
        string symbol,
        string name,
        address indexed listedBy,
        uint256 burnAmount
    );
    
    event CommunityTokenDelisted(
        address indexed tokenAddress,
        string symbol,
        address indexed delistedBy
    );
    
    constructor(address _coldToken, address daoTreasury) {
        require(_coldToken != address(0), "Cold token zero address");
        require(daoTreasury != address(0), "DAO treasury zero address");
        
        coldToken = ColdToken(_coldToken);
        
        _setupRole(DAO_ADMIN_ROLE, daoTreasury);
        _setRoleAdmin(COMMUNITY_LISTING_ROLE, DAO_ADMIN_ROLE);
    }
    
    /// @dev List a new community token (requires burning 1 COLD token)
    function listCommunityToken(
        address tokenAddress,
        string calldata symbol,
        string calldata name
    ) external onlyRole(COMMUNITY_LISTING_ROLE) nonReentrant {
        require(tokenAddress != address(0), "Token address zero");
        require(bytes(symbol).length > 0, "Symbol empty");
        require(bytes(name).length > 0, "Name empty");
        require(!communityTokens[tokenAddress].isActive, "Token already listed");
        
        // Burn 1 COLD token for the listing
        coldToken.burnForCommunityListing(tokenAddress, symbol);
        
        // Create community token record
        CommunityToken memory newToken = CommunityToken({
            tokenAddress: tokenAddress,
            symbol: symbol,
            name: name,
            listingTimestamp: block.timestamp,
            listedBy: msg.sender,
            isActive: true
        });
        
        communityTokens[tokenAddress] = newToken;
        listedTokens.push(tokenAddress);
        
        emit CommunityTokenListed(tokenAddress, symbol, name, msg.sender, coldToken.COMMUNITY_LISTING_BURN_AMOUNT());
    }
    
    /// @dev Delist a community token (DAO only)
    function delistCommunityToken(address tokenAddress) external onlyRole(DAO_ADMIN_ROLE) {
        require(communityTokens[tokenAddress].isActive, "Token not listed");
        
        CommunityToken storage token = communityTokens[tokenAddress];
        token.isActive = false;
        
        emit CommunityTokenDelisted(tokenAddress, token.symbol, msg.sender);
    }
    
    /// @dev Get all listed community tokens
    function getAllListedTokens() external view returns (address[] memory) {
        return listedTokens;
    }
    
    /// @dev Get active community tokens count
    function getActiveTokensCount() external view returns (uint256) {
        uint256 count = 0;
        for (uint256 i = 0; i < listedTokens.length; i++) {
            if (communityTokens[listedTokens[i]].isActive) {
                count++;
            }
        }
        return count;
    }
    
    /// @dev Get community token details
    function getCommunityToken(address tokenAddress) 
        external 
        view 
        returns (
            address tokenAddr,
            string memory symbol,
            string memory name,
            uint256 listingTimestamp,
            address listedBy,
            bool isActive
        ) 
    {
        CommunityToken memory token = communityTokens[tokenAddress];
        return (
            token.tokenAddress,
            token.symbol,
            token.name,
            token.listingTimestamp,
            token.listedBy,
            token.isActive
        );
    }
    
    /// @dev Get C0LD token burn statistics
    function getBurnStatistics() 
        external 
        view 
        returns (
            uint256 totalBurned,
            uint256 communityListingsCount,
            uint256 effectiveSupply,
            uint256 remainingMintableSupply
        ) 
    {
        return (
            coldToken.getTotalBurned(),
            coldToken.getCommunityListingsCount(),
            coldToken.getEffectiveSupply(),
            coldToken.getRemainingMintableSupply()
        );
    }
    
    /// @dev Check if a token is listed and active
    function isTokenListed(address tokenAddress) external view returns (bool) {
        return communityTokens[tokenAddress].isActive;
    }
    
    /// @dev Get listing cost (always 1 COLD token)
    function getListingCost() external pure returns (uint256) {
        return 1e12; // 1 COLD token (12 decimals)
    }
}
