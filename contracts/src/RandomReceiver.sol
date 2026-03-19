// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@axelar-network/axelar-gmp-sdk-solidity/contracts/executable/AxelarExecutable.sol";
import "./VDFVerifier.sol";

contract RandomReceiver is AxelarExecutable, VDFVerifier {
	uint256 public challengeWindow = 10 minutes;

	uint256 private constant BN254_Q =
		21888242871839275222246405745257275088548364400416034343698204186575808495617;
	uint256 private constant BN254_R =
		21888242871839275222246405745257275088548364400416034343698204186575808495617;

	address public owner;

	struct G1Point {
		uint256 x;
		uint256 y;
	}

	struct G2Point {
		uint256[2] x;
		uint256[2] y;
	}

	struct ResultItem {
		uint256 requestId;
		bytes y;
		bytes pi;
		bytes seedCollective;
		bytes modulus;
		bytes blsSignature;
		uint256 submittedAt;
		uint256 challengeDeadline;
		bool challenged;
		bool finalized;
	}

	mapping(uint256 => ResultItem) public queue;
	mapping(uint256 => bytes32) public finalRandomnessByRequest;

	G2Point private aggregatePublicKey;

	event DataReceived(uint256 indexed requestId, bytes32 payloadHash, string sourceChain, string sourceAddress);
	event OptimisticResultSubmitted(uint256 indexed requestId, uint256 challengeDeadline);
	event ResultChallenged(uint256 indexed requestId, address indexed challenger, bytes computedY);
	event RandomnessFinalized(uint256 indexed requestId, bytes32 finalRandomness);
	event AggregatePublicKeyUpdated(uint256[2] x, uint256[2] y);
	event ChallengeWindowUpdated(uint256 challengeWindow);

	error NotOwner();
	error InvalidPayload();
	error ResultAlreadySubmitted();
	error ResultMissing();
	error AlreadyChallenged();
	error ChallengeWindowNotExpired();
	error ChallengeWindowExpired();
	error AlreadyFinalized();
	error InvalidBlsSignature();
	error ChallengeFailed();
	error InvalidChallengeWindow();

	modifier onlyOwner() {
		if (msg.sender != owner) revert NotOwner();
		_;
	}

	constructor(address gateway) AxelarExecutable(gateway) {
		owner = msg.sender;
	}

	function _execute(
		string calldata sourceChain,
		string calldata sourceAddress,
		bytes calldata payload
	) internal override {
		(
			uint256 requestId,
			bytes memory y,
			bytes memory pi,
			bytes memory seedCollective,
			bytes memory modulus,
			bytes memory blsSignature
		) = abi.decode(payload, (uint256, bytes, bytes, bytes, bytes, bytes));

		emit DataReceived(requestId, keccak256(payload), sourceChain, sourceAddress);
		submitOptimisticResult(requestId, y, pi, seedCollective, modulus, blsSignature);
	}

	function setAggregatePublicKey(
		uint256[2] calldata x,
		uint256[2] calldata y
	) external onlyOwner {
		aggregatePublicKey = G2Point({x: x, y: y});
		emit AggregatePublicKeyUpdated(x, y);
	}

	function setChallengeWindow(uint256 newWindow) external onlyOwner {
		if (newWindow == 0) revert InvalidChallengeWindow();
		challengeWindow = newWindow;
		emit ChallengeWindowUpdated(newWindow);
	}

	function getAggregatePublicKey() external view returns (uint256[2] memory x, uint256[2] memory y) {
		return (aggregatePublicKey.x, aggregatePublicKey.y);
	}

	function submitOptimisticResult(
		uint256 requestId,
		bytes memory y,
		bytes memory pi,
		bytes memory seedCollective,
		bytes memory modulus,
		bytes memory blsSignature
	) public {
		if (queue[requestId].submittedAt != 0) revert ResultAlreadySubmitted();
		if (y.length == 0 || pi.length == 0 || seedCollective.length == 0 || modulus.length == 0) {
			revert InvalidPayload();
		}
		if (!_verifyBlsSignature(seedCollective, blsSignature)) revert InvalidBlsSignature();

		uint256 deadline = block.timestamp + challengeWindow;
		queue[requestId] = ResultItem({
			requestId: requestId,
			y: y,
			pi: pi,
			seedCollective: seedCollective,
			modulus: modulus,
			blsSignature: blsSignature,
			submittedAt: block.timestamp,
			challengeDeadline: deadline,
			challenged: false,
			finalized: false
		});

		emit OptimisticResultSubmitted(requestId, deadline);
	}

	function challengeResult(uint256 requestId) external {
		ResultItem storage item = queue[requestId];
		if (item.submittedAt == 0) revert ResultMissing();
		if (item.finalized) revert AlreadyFinalized();
		if (item.challenged) revert AlreadyChallenged();
		if (block.timestamp > item.challengeDeadline) revert ChallengeWindowExpired();

		bytes memory computedY = verifyVDF(item.seedCollective, item.pi, item.modulus);
		bool isInvalid = keccak256(computedY) != keccak256(item.y);
		if (!isInvalid) revert ChallengeFailed();

		item.challenged = true;
		emit ResultChallenged(requestId, msg.sender, computedY);
	}

	function finalizeRandomness(uint256 requestId) external returns (bytes32 finalRandomness) {
		ResultItem storage item = queue[requestId];
		if (item.submittedAt == 0) revert ResultMissing();
		if (item.finalized) revert AlreadyFinalized();
		if (item.challenged) revert AlreadyChallenged();
		if (block.timestamp < item.challengeDeadline) revert ChallengeWindowNotExpired();

		item.finalized = true;
		finalRandomness = keccak256(abi.encodePacked(item.y, item.seedCollective));
		finalRandomnessByRequest[requestId] = finalRandomness;

		emit RandomnessFinalized(requestId, finalRandomness);
	}

	function _verifyBlsSignature(
		bytes memory message,
		bytes memory signatureBytes
	) internal view returns (bool) {
		if (
			aggregatePublicKey.x[0] == 0 &&
			aggregatePublicKey.x[1] == 0 &&
			aggregatePublicKey.y[0] == 0 &&
			aggregatePublicKey.y[1] == 0
		) {
			return false;
		}

		G1Point memory signature = _decodeG1(signatureBytes);
		G1Point memory hashPoint = _hashToG1(message);

		return _pairingCheck(
			signature,
			_generatorG2(),
			_negate(hashPoint),
			aggregatePublicKey
		);
	}

	function _decodeG1(bytes memory encoded) internal pure returns (G1Point memory point) {
		if (encoded.length != 64) revert InvalidPayload();

		uint256 x;
		uint256 y;
		assembly {
			x := mload(add(encoded, 32))
			y := mload(add(encoded, 64))
		}

		if (x == 0 || x >= BN254_Q || y == 0 || y >= BN254_Q) revert InvalidPayload();
		point = G1Point({x: x, y: y});
	}

	function _hashToG1(bytes memory message) internal view returns (G1Point memory point) {
		uint256 scalar = uint256(keccak256(message)) % BN254_R;
		if (scalar == 0) scalar = 1;

		bool success;
		uint256[3] memory input = [uint256(1), uint256(2), scalar];
		uint256[2] memory output;

		assembly {
			success := staticcall(gas(), 0x07, input, 96, output, 64)
		}
		if (!success || output[0] == 0 || output[1] == 0) revert InvalidPayload();

		point = G1Point({x: output[0], y: output[1]});
	}

	function _generatorG2() internal pure returns (G2Point memory) {
		return
			G2Point({
				x: [
					uint256(11559732032986387107991004021392285783925812861821192530917403151452391805634),
					uint256(10857046999023057135944570762232829481370756359578518086990519993285655852781)
				],
				y: [
					uint256(4082367875863433681332203403145435568316851327593401208105741076214120093531),
					uint256(8495653923123431417604973247489272438418190587263600148770280649306958101930)
				]
			});
	}

	function _negate(G1Point memory point) internal pure returns (G1Point memory) {
		if (point.x == 0 && point.y == 0) {
			return G1Point(0, 0);
		}
		return G1Point(point.x, BN254_Q - (point.y % BN254_Q));
	}

	function _pairingCheck(
		G1Point memory p1,
		G2Point memory p2,
		G1Point memory q1,
		G2Point memory q2
	) internal view returns (bool) {
		uint256[12] memory input = [
			p1.x,
			p1.y,
			p2.x[0],
			p2.x[1],
			p2.y[0],
			p2.y[1],
			q1.x,
			q1.y,
			q2.x[0],
			q2.x[1],
			q2.y[0],
			q2.y[1]
		];

		uint256[1] memory out;
		bool success;
		assembly {
			success := staticcall(gas(), 0x08, input, 384, out, 32)
		}

		return success && out[0] == 1;
	}
}
