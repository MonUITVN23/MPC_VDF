// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "./interfaces/IBridgeAdapter.sol";

contract RandomRouter {
    uint256 public nextRequestId = 1;

    address public owner;
    string public destinationChain;
    string public destinationAddress;

    mapping(bytes32 => IBridgeAdapter) public bridgeAdapters;

    uint256 private _reentrancyStatus = 1;

    error NotOwner();
    error InvalidBridgeId();
    error InvalidRequestId();
    error InvalidAddress();
    error InvalidDestinationAddress();
    error EmptyPayloadPart();
    error ReentrancyGuard();
    error AdapterNotRegistered();

    modifier onlyOwner() {
        if (msg.sender != owner) revert NotOwner();
        _;
    }

    modifier nonReentrant() {
        if (_reentrancyStatus != 1) revert ReentrancyGuard();
        _reentrancyStatus = 2;
        _;
        _reentrancyStatus = 1;
    }

    event LogRequest(uint256 indexed requestId, uint256 userSeed, uint256 timestamp);

    event AdapterRegistered(bytes32 indexed bridgeId, address indexed adapterAddress);

    event DestinationUpdated(string destinationChain, string destinationAddress);

    event PayloadDispatched(
        uint256 indexed requestId,
        bytes32 indexed bridgeId,
        uint256 feePaid,
        uint256 timestamp
    );

    constructor(string memory destinationChain_, string memory destinationAddress_) {
        if (bytes(destinationChain_).length == 0 || bytes(destinationAddress_).length == 0) {
            revert InvalidDestinationAddress();
        }

        owner = msg.sender;
        destinationChain = destinationChain_;
        destinationAddress = destinationAddress_;
    }

    function registerAdapter(bytes32 bridgeId, address adapterAddress) external onlyOwner {
        if (bridgeId == bytes32(0)) revert InvalidBridgeId();
        if (adapterAddress == address(0)) revert InvalidAddress();

        bridgeAdapters[bridgeId] = IBridgeAdapter(adapterAddress);
        emit AdapterRegistered(bridgeId, adapterAddress);
    }

    function setDestination(
        string calldata destinationChain_,
        string calldata destinationAddress_
    ) external onlyOwner {
        if (bytes(destinationChain_).length == 0 || bytes(destinationAddress_).length == 0) {
            revert InvalidDestinationAddress();
        }

        destinationChain = destinationChain_;
        destinationAddress = destinationAddress_;
        emit DestinationUpdated(destinationChain_, destinationAddress_);
    }

    function requestRandomness(uint256 userSeed) external returns (uint256 requestId) {
        requestId = nextRequestId;
        unchecked {
            nextRequestId = requestId + 1;
        }

        emit LogRequest(requestId, userSeed, block.timestamp);
    }

    function estimateBridgeFee(bytes32 bridgeId, bytes calldata payload) external view returns (uint256) {
        IBridgeAdapter adapter = bridgeAdapters[bridgeId];
        if (address(adapter) == address(0)) revert AdapterNotRegistered();
        return adapter.estimateFee(destinationChain, destinationAddress, payload);
    }

    function relayVDFPayload(
        uint256 requestId,
        bytes calldata y,
        bytes calldata pi,
        bytes calldata seedCollective,
        bytes calldata modulus,
        bytes calldata blsSignature,
        bytes32 bridgeId
    ) external payable nonReentrant {
        if (bridgeId == bytes32(0)) revert InvalidBridgeId();

        bytes memory payload = _buildAndValidatePayload(
            requestId,
            y,
            pi,
            seedCollective,
            modulus,
            blsSignature
        );

        IBridgeAdapter adapter = bridgeAdapters[bridgeId];
        if (address(adapter) == address(0)) revert AdapterNotRegistered();

        adapter.dispatchPayload{value: msg.value}(destinationChain, destinationAddress, payload);
        emit PayloadDispatched(requestId, bridgeId, msg.value, block.timestamp);
    }

    function _buildAndValidatePayload(
        uint256 requestId,
        bytes calldata y,
        bytes calldata pi,
        bytes calldata seedCollective,
        bytes calldata modulus,
        bytes calldata blsSignature
    ) internal view returns (bytes memory payload) {
        if (requestId == 0 || requestId >= nextRequestId) revert InvalidRequestId();
        if (
            y.length == 0 ||
            pi.length == 0 ||
            seedCollective.length == 0 ||
            modulus.length == 0 ||
            blsSignature.length == 0
        ) revert EmptyPayloadPart();

        payload = abi.encode(requestId, y, pi, seedCollective, modulus, blsSignature);
    }
}