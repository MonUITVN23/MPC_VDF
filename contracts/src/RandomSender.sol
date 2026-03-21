// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@axelar-network/axelar-gmp-sdk-solidity/contracts/interfaces/IAxelarGateway.sol";
import "@axelar-network/axelar-gmp-sdk-solidity/contracts/interfaces/IAxelarGasService.sol";

contract RandomSender {
    enum BridgeType {
        None,
        Axelar,
        LayerZero,
        Wormhole
    }

    uint256 public nextRequestId = 1;

    string public constant DESTINATION_CHAIN = "polygon-sepolia";
    string public destinationAddress;

    IAxelarGateway public immutable gateway;
    IAxelarGasService public immutable gasService;

    uint256 private _reentrancyStatus = 1;

    error InvalidBridgeId();
    error InvalidRequestId();
    error InvalidAddress();
    error InvalidDestinationAddress();
    error InsufficientFee();
    error EmptyPayloadPart();
    error ReentrancyGuard();

    modifier nonReentrant() {
        if (_reentrancyStatus != 1) revert ReentrancyGuard();
        _reentrancyStatus = 2;
        _;
        _reentrancyStatus = 1;
    }

    event LogRequest(uint256 indexed requestId, uint256 userSeed, uint256 timestamp);

    event PayloadDispatched(
        uint256 indexed requestId,
        BridgeType indexed bridgeId,
        uint256 feePaid,
        uint256 timestamp
    );

    constructor(address gateway_, address gasService_, string memory destinationAddress_) {
        if (gateway_ == address(0) || gasService_ == address(0)) revert InvalidAddress();
        if (bytes(destinationAddress_).length == 0) revert InvalidDestinationAddress();

        gateway = IAxelarGateway(gateway_);
        gasService = IAxelarGasService(gasService_);
        destinationAddress = destinationAddress_;
    }

    function requestRandomness(uint256 userSeed) external returns (uint256 requestId) {
        requestId = nextRequestId;
        unchecked {
            nextRequestId = requestId + 1;
        }

        emit LogRequest(requestId, userSeed, block.timestamp);
    }

    function relayVDFPayload(
        uint256 requestId,
        bytes calldata y,
        bytes calldata pi,
        bytes calldata seedCollective,
        bytes calldata modulus,
        bytes calldata blsSignature,
        BridgeType bridgeId
    ) external payable nonReentrant {
        if (requestId == 0 || requestId >= nextRequestId) revert InvalidRequestId();
        if (bridgeId == BridgeType.None || bridgeId == BridgeType.Wormhole) revert InvalidBridgeId();
        if (
            y.length == 0 ||
            pi.length == 0 ||
            seedCollective.length == 0 ||
            modulus.length == 0 ||
            blsSignature.length == 0
        ) revert EmptyPayloadPart();

        bytes memory payload = abi.encode(requestId, y, pi, seedCollective, modulus, blsSignature);

        if (bridgeId == BridgeType.Axelar) {
            _dispatchViaAxelar(payload, msg.value);
        } else {
            _dispatchViaLayerZero(payload, msg.value);
        }

        emit PayloadDispatched(requestId, bridgeId, msg.value, block.timestamp);
    }

    function _dispatchViaAxelar(bytes memory payload, uint256 fee) internal {
        if (fee == 0) revert InsufficientFee();

        gasService.payNativeGasForContractCall{value: fee}(
            address(this),
            DESTINATION_CHAIN,
            destinationAddress,
            payload,
            msg.sender
        );
        gateway.callContract(DESTINATION_CHAIN, destinationAddress, payload);
    }

    function _dispatchViaLayerZero(bytes memory, uint256) internal {}
}
