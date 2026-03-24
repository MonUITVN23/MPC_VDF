// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

interface IBridgeAdapter {
    function estimateFee(
        string calldata destChain,
        string calldata destAddress,
        bytes calldata payload
    ) external view returns (uint256);

    function dispatchPayload(
        string calldata destChain,
        string calldata destAddress,
        bytes calldata payload
    ) external payable;
}