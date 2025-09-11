// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Capped.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Burnable.sol";
import "@openzeppelin/contracts/access/AccessControlEnumerable.sol";
import "@openzeppelin/contracts/utils/math/SafeMath.sol";

/// @title C0LD Token (CD) – DAO-governed capped circulating supply token for COLↁAO
/// @notice Max circulating supply 20 * 10^12 units (decimals = 12). 
/// @notice When tokens are burned, more tokens can be minted to maintain max circulating supply.
/// @notice Mintable only by authorised minter contracts.
contract ColdToken is ERC20Capped, ERC20Burnable, AccessControlEnumerable {
    using SafeMath for uint256;
    
    /// @dev 12-decimals version of 1 token.
    uint256 private constant ONE = 10 ** 12;

    /// @dev Max supply: 20 COLD (20 * 10^12).
    uint256 private constant MAX_SUPPLY = 20 * ONE;

    // --- Roles ---
    bytes32 public constant DAO_ADMIN_ROLE    = keccak256("DAO_ADMIN_ROLE");
    bytes32 public constant YIELD_MINTER_ROLE = keccak256("YIELD_MINTER_ROLE");
    bytes32 public constant LP_MINTER_ROLE    = keccak256("LP_MINTER_ROLE");
    bytes32 public constant STAKE_MINTER_ROLE = keccak256("STAKE_MINTER_ROLE");
    bytes32 public constant COMMUNITY_LISTING_ROLE = keccak256("COMMUNITY_LISTING_ROLE");

    // --- State Variables ---
    uint256 public constant COMMUNITY_LISTING_BURN_AMOUNT = ONE; // 1 COLD token per listing
    uint256 public totalBurned;
    uint256 public communityListingsCount;
    
    // --- Events ---
    event CommunityTokenListed(address indexed tokenAddress, string tokenSymbol, uint256 burnAmount);
    event TokensBurned(address indexed burner, uint256 amount, string reason);

    constructor(address daoTreasury)
        ERC20("C0LD Token", "COLD")
        ERC20Capped(MAX_SUPPLY)
    {
        require(daoTreasury != address(0), "dao zero");
        _setupRole(DAO_ADMIN_ROLE, daoTreasury);
        _setRoleAdmin(YIELD_MINTER_ROLE, DAO_ADMIN_ROLE);
        _setRoleAdmin(LP_MINTER_ROLE, DAO_ADMIN_ROLE);
        _setRoleAdmin(STAKE_MINTER_ROLE, DAO_ADMIN_ROLE);
        _setRoleAdmin(COMMUNITY_LISTING_ROLE, DAO_ADMIN_ROLE);
    }

    /// @dev Override decimals to 12.
    function decimals() public pure override returns (uint8) {
        return 12;
    }

    // --------- Mint functions (only authorised contracts) ---------

    function mintYield(address to, uint256 amount) external onlyRole(YIELD_MINTER_ROLE) {
        _mintChecked(to, amount);
    }

    function mintLpRewards(address to, uint256 amount) external onlyRole(LP_MINTER_ROLE) {
        _mintChecked(to, amount);
    }

    function mintStakingRewards(address to, uint256 amount) external onlyRole(STAKE_MINTER_ROLE) {
        _mintChecked(to, amount);
    }

    // --------- Burn functions ---------

    /// @dev Burn tokens for community token listing (requires 1 COLD token)
    function burnForCommunityListing(address tokenAddress, string calldata tokenSymbol) 
        external 
        onlyRole(COMMUNITY_LISTING_ROLE) 
    {
        require(tokenAddress != address(0), "Token address zero");
        require(bytes(tokenSymbol).length > 0, "Token symbol empty");
        
        // Burn 1 COLD token from the caller
        require(balanceOf(msg.sender) >= COMMUNITY_LISTING_BURN_AMOUNT, "Insufficient COLD balance");
        
        _burn(msg.sender, COMMUNITY_LISTING_BURN_AMOUNT);
        totalBurned = totalBurned.add(COMMUNITY_LISTING_BURN_AMOUNT);
        communityListingsCount = communityListingsCount.add(1);
        
        emit CommunityTokenListed(tokenAddress, tokenSymbol, COMMUNITY_LISTING_BURN_AMOUNT);
        emit TokensBurned(msg.sender, COMMUNITY_LISTING_BURN_AMOUNT, "Community Token Listing");
    }

    /// @dev Burn tokens (standard burn function)
    function burn(uint256 amount) public override {
        super.burn(amount);
        totalBurned = totalBurned.add(amount);
        emit TokensBurned(msg.sender, amount, "Manual Burn");
    }

    /// @dev Burn tokens from a specific account (requires allowance)
    function burnFrom(address account, uint256 amount) public override {
        super.burnFrom(account, amount);
        totalBurned = totalBurned.add(amount);
        emit TokensBurned(account, amount, "Burn From");
    }

    // Internal mint with cap enforcement (accounts for burned tokens)
    function _mintChecked(address to, uint256 amount) internal {
        // Calculate available minting space: cap - (totalSupply - totalBurned)
        // This allows minting up to the cap even after burns occur
        uint256 availableMintingSpace = cap().sub(totalSupply().sub(totalBurned));
        require(amount <= availableMintingSpace, "cap exceeded");
        _mint(to, amount);
    }

    // --------- Utility functions ---------

    /// @dev Get total burned tokens
    function getTotalBurned() external view returns (uint256) {
        return totalBurned;
    }

    /// @dev Get community listings count
    function getCommunityListingsCount() external view returns (uint256) {
        return communityListingsCount;
    }

    /// @dev Get circulating supply (total supply - burned)
    function getCirculatingSupply() external view returns (uint256) {
        return totalSupply().sub(totalBurned);
    }

    /// @dev Get remaining mintable supply (cap - circulating supply)
    function getRemainingMintableSupply() external view returns (uint256) {
        return cap().sub(getCirculatingSupply());
    }

    /// @dev Get effective supply (same as circulating supply for clarity)
    function getEffectiveSupply() external view returns (uint256) {
        return getCirculatingSupply();
    }

    // Disallow transfers before any supply exists (optional)
    function _beforeTokenTransfer(address from, address to, uint256 amount) internal override(ERC20) {
        super._beforeTokenTransfer(from, to, amount);
    }
}
