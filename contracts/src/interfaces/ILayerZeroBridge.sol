// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @title ILayerZeroBridge
/// @notice Placeholder interface for LayerZero Multi-Bridge Failover (future implementation)
interface ILayerZeroBridge {
    function sendRandomnessResult(
        uint16 dstChainId,
        bytes calldata payload,
        address payable refundAddress
    ) external payable;

    function receiveRandomnessResult(
        uint16 srcChainId,
        bytes calldata srcAddress,
        uint64 nonce,
        bytes calldata payload
    ) external;

    function estimateFees(
        uint16 dstChainId,
        bytes calldata payload
    ) external view returns (uint256 nativeFee, uint256 zroFee);
}
