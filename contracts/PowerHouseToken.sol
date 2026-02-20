// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import {Ownable} from "@openzeppelin/contracts/access/Ownable.sol";
import {MerkleProof} from "@openzeppelin/contracts/utils/cryptography/MerkleProof.sol";

/// @title PowerHouseToken (JULIAN)
/// @notice ERC-20 migration token with one-time Merkle claims from stake snapshots.
contract PowerHouseToken is ERC20, Ownable {
    bytes32 public migrationRoot;
    uint256 public immutable snapshotHeight;
    uint256 public immutable conversionRatio;

    mapping(bytes32 => bool) public claimed;

    event MigrationRootUpdated(bytes32 indexed oldRoot, bytes32 indexed newRoot);
    event MigrationClaimed(bytes32 indexed claimId, address indexed account, uint256 amount);

    constructor(
        address owner_,
        uint256 snapshotHeight_,
        uint256 conversionRatio_,
        uint256 treasuryMint,
        bytes32 migrationRoot_
    ) ERC20("PowerHouseToken", "JULIAN") Ownable(owner_) {
        snapshotHeight = snapshotHeight_;
        conversionRatio = conversionRatio_ == 0 ? 1 : conversionRatio_;
        migrationRoot = migrationRoot_;
        if (treasuryMint > 0) {
            _mint(owner_, treasuryMint);
        }
    }

    /// @notice Rotate the claim root for a governance-approved migration anchor.
    function setMigrationRoot(bytes32 newRoot) external onlyOwner {
        bytes32 oldRoot = migrationRoot;
        migrationRoot = newRoot;
        emit MigrationRootUpdated(oldRoot, newRoot);
    }

    /// @notice Claim migrated stake allocation.
    /// @dev leaf = keccak256(abi.encodePacked(snapshotHeight, claimId, account, amount)).
    function claim(
        bytes32 claimId,
        address account,
        uint256 amount,
        bytes32[] calldata proof
    ) external {
        require(account != address(0), "invalid account");
        require(!claimed[claimId], "already claimed");

        bytes32 leaf = keccak256(abi.encodePacked(snapshotHeight, claimId, account, amount));
        require(MerkleProof.verify(proof, migrationRoot, leaf), "invalid proof");

        claimed[claimId] = true;
        _mint(account, amount * conversionRatio);
        emit MigrationClaimed(claimId, account, amount * conversionRatio);
    }

    /// @notice Owner burn used by slashing oracle confirmations.
    function burnFromMigration(address account, uint256 amount) external onlyOwner {
        _burn(account, amount);
    }
}
