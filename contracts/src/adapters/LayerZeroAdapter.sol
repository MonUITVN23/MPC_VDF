
pragma solidity ^0.8.20;

import "../interfaces/IBridgeAdapter.sol";

interface ILayerZeroEndpointV2 {
    struct MessagingParams {
        uint32 dstEid;
        bytes32 receiver;
        bytes message;
        bytes options;
        bool payInLzToken;
    }

    struct MessagingFee {
        uint256 nativeFee;
        uint256 lzTokenFee;
    }

    function quote(
        MessagingParams calldata params,
        address sender
    ) external view returns (MessagingFee memory fee);

    function send(
        MessagingParams calldata params,
        address payable refundAddress
    ) external payable returns (bytes32 guid, uint64 nonce);
}

contract LayerZeroAdapter is IBridgeAdapter {
    struct Route {
        uint32 dstEid;
        bytes32 receiver;
        bytes options;
        bool exists;
    }

    ILayerZeroEndpointV2 public immutable lzEndpoint;
    address public immutable router;
    address public owner;

    mapping(bytes32 => Route) public routes;

    error NotRouter();
    error NotOwner();
    error InvalidAddress();
    error RouteNotFound();
    error InsufficientFee();

    event RouteConfigured(
        string destChain,
        string destAddress,
        uint32 dstEid,
        bytes32 receiver,
        bytes options
    );
    event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);

    modifier onlyRouter() {
        if (msg.sender != router) revert NotRouter();
        _;
    }

    modifier onlyOwner() {
        if (msg.sender != owner) revert NotOwner();
        _;
    }

    constructor(address endpoint_, address router_) {
        if (endpoint_ == address(0) || router_ == address(0)) revert InvalidAddress();
        lzEndpoint = ILayerZeroEndpointV2(endpoint_);
        router = router_;
        owner = msg.sender;
    }

    function transferOwnership(address newOwner) external onlyOwner {
        if (newOwner == address(0)) revert InvalidAddress();
        address previousOwner = owner;
        owner = newOwner;
        emit OwnershipTransferred(previousOwner, newOwner);
    }

    function setRoute(
        string calldata destChain,
        string calldata destAddress,
        uint32 dstEid,
        bytes32 receiver,
        bytes calldata options
    ) external onlyOwner {
        if (receiver == bytes32(0)) revert InvalidAddress();

        bytes32 routeKey = _routeKey(destChain, destAddress);
        routes[routeKey] = Route({dstEid: dstEid, receiver: receiver, options: options, exists: true});

        emit RouteConfigured(destChain, destAddress, dstEid, receiver, options);
    }

    function estimateFee(
        string calldata destChain,
        string calldata destAddress,
        bytes calldata payload
    ) external view override returns (uint256) {
        Route storage route = routes[_routeKey(destChain, destAddress)];
        if (!route.exists) return type(uint256).max;

        ILayerZeroEndpointV2.MessagingParams memory params = ILayerZeroEndpointV2.MessagingParams({
            dstEid: route.dstEid,
            receiver: route.receiver,
            message: payload,
            options: route.options,
            payInLzToken: false
        });
        ILayerZeroEndpointV2.MessagingFee memory fee = lzEndpoint.quote(params, address(this));
        return fee.nativeFee;
    }

    function dispatchPayload(
        string calldata destChain,
        string calldata destAddress,
        bytes calldata payload
    ) external payable override onlyRouter {
        Route storage route = routes[_routeKey(destChain, destAddress)];
        if (!route.exists) revert RouteNotFound();

        ILayerZeroEndpointV2.MessagingParams memory params = ILayerZeroEndpointV2.MessagingParams({
            dstEid: route.dstEid,
            receiver: route.receiver,
            message: payload,
            options: route.options,
            payInLzToken: false
        });
        ILayerZeroEndpointV2.MessagingFee memory fee = lzEndpoint.quote(params, address(this));
        uint256 nativeFee = fee.nativeFee;
        if (msg.value < nativeFee) revert InsufficientFee();

        lzEndpoint.send{value: msg.value}(params, payable(tx.origin));
    }

    function _routeKey(string calldata destChain, string calldata destAddress) internal pure returns (bytes32) {
        return keccak256(abi.encodePacked(destChain, "|", destAddress));
    }
}