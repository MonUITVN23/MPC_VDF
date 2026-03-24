// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "../interfaces/IBridgeAdapter.sol";

contract WormholeAdapter is IBridgeAdapter {
    bytes4 private constant QUOTE_SELECTOR =
        bytes4(keccak256("quoteEVMDeliveryPrice(uint16,uint256,uint256)"));
    bytes4 private constant SEND_SELECTOR =
        bytes4(keccak256("sendPayloadToEvm(uint16,address,bytes,uint256,uint256,uint16,address)"));

    struct Route {
        uint16 targetChain;
        address targetAddress;
        uint256 gasLimit;
        uint16 refundChain;
        address refundAddress;
        bool exists;
    }

    address public immutable wormholeRelayer;
    address public immutable router;
    address public owner;
    address public feeOracle;
    uint256 public feeHintWei;

    mapping(bytes32 => Route) public routes;

    error NotRouter();
    error NotOwner();
    error InvalidAddress();
    error RouteNotFound();
    error InsufficientFee();
    error DispatchFailed(bytes returnData);

    event RouteConfigured(
        string destChain,
        string destAddress,
        uint16 targetChain,
        address targetAddress,
        uint256 gasLimit,
        uint16 refundChain,
        address refundAddress
    );
    event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);
    event FeeOracleUpdated(address indexed feeOracle);
    event FeeHintUpdated(uint256 feeHintWei);
    event WormholePayloadDispatched(uint64 sequence, uint16 targetChain, address targetAddress, bytes32 payloadHash);

    modifier onlyRouter() {
        if (msg.sender != router) revert NotRouter();
        _;
    }

    modifier onlyOwner() {
        if (msg.sender != owner) revert NotOwner();
        _;
    }

    constructor(address wormholeRelayer_, address router_) {
        if (wormholeRelayer_ == address(0) || router_ == address(0)) revert InvalidAddress();

        wormholeRelayer = wormholeRelayer_;
        router = router_;
        owner = msg.sender;
        feeOracle = wormholeRelayer_;
        feeHintWei = 200_000_000_000_000;
    }

    function transferOwnership(address newOwner) external onlyOwner {
        if (newOwner == address(0)) revert InvalidAddress();
        address previousOwner = owner;
        owner = newOwner;
        emit OwnershipTransferred(previousOwner, newOwner);
    }

    function setFeeOracle(address feeOracle_) external onlyOwner {
        if (feeOracle_ == address(0)) revert InvalidAddress();
        feeOracle = feeOracle_;
        emit FeeOracleUpdated(feeOracle_);
    }

    function setFeeHint(uint256 feeHintWei_) external onlyOwner {
        feeHintWei = feeHintWei_;
        emit FeeHintUpdated(feeHintWei_);
    }

    function setRoute(
        string calldata destChain,
        string calldata destAddress,
        uint16 targetChain,
        address targetAddress,
        uint256 gasLimit,
        uint16 refundChain,
        address refundAddress
    ) external onlyOwner {
        if (targetAddress == address(0) || refundAddress == address(0)) revert InvalidAddress();

        routes[_routeKey(destChain, destAddress)] = Route({
            targetChain: targetChain,
            targetAddress: targetAddress,
            gasLimit: gasLimit,
            refundChain: refundChain,
            refundAddress: refundAddress,
            exists: true
        });

        emit RouteConfigured(destChain, destAddress, targetChain, targetAddress, gasLimit, refundChain, refundAddress);
    }

    function estimateFee(
        string calldata destChain,
        string calldata destAddress,
        bytes calldata
    ) external view override returns (uint256) {
        Route storage route = routes[_routeKey(destChain, destAddress)];
        if (!route.exists) return type(uint256).max;

        uint256 quoted = _tryQuoteFee(route.targetChain, route.gasLimit);
        if (quoted > 0) return quoted;
        return feeHintWei;
    }

    function dispatchPayload(
        string calldata destChain,
        string calldata destAddress,
        bytes calldata payload
    ) external payable override onlyRouter {
        Route storage route = routes[_routeKey(destChain, destAddress)];
        if (!route.exists) revert RouteNotFound();

        uint256 requiredFee = _tryQuoteFee(route.targetChain, route.gasLimit);
        if (requiredFee == 0) requiredFee = feeHintWei;
        if (msg.value < requiredFee) revert InsufficientFee();

        (bool success, bytes memory result) = wormholeRelayer.call{value: msg.value}(
            abi.encodeWithSelector(
                SEND_SELECTOR,
                route.targetChain,
                route.targetAddress,
                payload,
                0,
                route.gasLimit,
                route.refundChain,
                route.refundAddress
            )
        );
        if (!success) revert DispatchFailed(result);

        uint64 sequence = 0;
        if (result.length >= 32) {
            sequence = uint64(uint256(bytes32(result)));
        }

        emit WormholePayloadDispatched(sequence, route.targetChain, route.targetAddress, keccak256(payload));
    }

    function _tryQuoteFee(uint16 targetChain, uint256 gasLimit) internal view returns (uint256) {
        (bool success, bytes memory result) = feeOracle.staticcall(
            abi.encodeWithSelector(QUOTE_SELECTOR, targetChain, 0, gasLimit)
        );
        if (!success || result.length < 32) return 0;
        return abi.decode(result, (uint256));
    }

    function _routeKey(string calldata destChain, string calldata destAddress) internal pure returns (bytes32) {
        return keccak256(abi.encodePacked(destChain, "|", destAddress));
    }
}