// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@axelar-network/axelar-gmp-sdk-solidity/contracts/executable/AxelarExecutable.sol";
import "./VDFVerifier.sol";
import "./interfaces/IGroth16Verifier.sol";

contract RandomReceiver is AxelarExecutable, VDFVerifier {
	uint256 public challengeWindow = 10 minutes;
	bool public enforceBlsSignature = false;
	bool public enforceZkProof = false;
	IGroth16Verifier public zkVerifier;
	bytes32 public registeredPkHash;  // SHA256(aggregate_pk_bytes)

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
	event BlsVerificationModeUpdated(bool enabled);
	event ZkVerifierUpdated(address verifier);
	event ZkProofModeUpdated(bool enabled);
	event PkHashRegistered(bytes32 pkHash);
	event ZkProofVerified(uint256 indexed requestId);

	error NotOwner();
	error InvalidPayload();
	error ResultAlreadySubmitted();
	error ResultMissing();
	error AlreadyChallenged();
	error ChallengeWindowNotExpired();
	error ChallengeWindowExpired();
	error AlreadyFinalized();
	error InvalidBlsSignature();
	error InvalidZkProof();
	error PayloadHashMismatch();
	error CommitteeKeyMismatch();
	error RequestIdMismatch();
	error ZkVerifierNotSet();
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
			bytes memory blsSignature,
			bytes memory zkProofData,
			uint256[7] memory zkPublicSignals
		) = abi.decode(payload, (uint256, bytes, bytes, bytes, bytes, bytes, bytes, uint256[7]));

		emit DataReceived(requestId, keccak256(payload), sourceChain, sourceAddress);
		submitOptimisticResult(
			requestId, y, pi, seedCollective, modulus,
			blsSignature, zkProofData, zkPublicSignals
		);
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

	function setBlsVerificationMode(bool enabled) external onlyOwner {
		enforceBlsSignature = enabled;
		emit BlsVerificationModeUpdated(enabled);
	}

	function setZkVerifier(address verifier) external onlyOwner {
		zkVerifier = IGroth16Verifier(verifier);
		emit ZkVerifierUpdated(verifier);
	}

	function setZkProofMode(bool enabled) external onlyOwner {
		enforceZkProof = enabled;
		emit ZkProofModeUpdated(enabled);
	}

	function registerPkHash(bytes32 _pkHash) external onlyOwner {
		registeredPkHash = _pkHash;
		emit PkHashRegistered(_pkHash);
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
		bytes memory blsSignature,
		bytes memory zkProofData,
		uint256[7] memory zkPublicSignals
	) public {
		if (queue[requestId].submittedAt != 0) revert ResultAlreadySubmitted();
		if (y.length == 0 || pi.length == 0 || seedCollective.length == 0 || modulus.length == 0) {
			revert InvalidPayload();
		}

		// === ZK Proof Verification ===
		if (zkProofData.length > 0) {
			if (address(zkVerifier) == address(0)) revert ZkVerifierNotSet();

			// Decode Groth16 proof components: (pA[2], pB[2][2], pC[2])
			(uint[2] memory pA, uint[2][2] memory pB, uint[2] memory pC) =
				abi.decode(zkProofData, (uint[2], uint[2][2], uint[2]));

			// Verify the Groth16 proof
			if (!zkVerifier.verifyProof(pA, pB, pC, zkPublicSignals)) {
				revert InvalidZkProof();
			}

			// Recompute payload_hash and verify binding
			bytes32 computedPayloadHash = sha256(
				abi.encodePacked(
					bytes32(requestId),
					y, pi, seedCollective, modulus
				)
			);

			// Check payload_hash binding (public signals [4] = hi, [5] = lo)
			bytes32 proofPayloadHash = bytes32(
				(zkPublicSignals[4] << 128) | zkPublicSignals[5]
			);
			if (computedPayloadHash != proofPayloadHash) {
				revert PayloadHashMismatch();
			}

			// Check pk_hash binding (public signals [2] = hi, [3] = lo)
			if (registeredPkHash != bytes32(0)) {
				bytes32 proofPkHash = bytes32(
					(zkPublicSignals[2] << 128) | zkPublicSignals[3]
				);
				if (registeredPkHash != proofPkHash) {
					revert CommitteeKeyMismatch();
				}
			}

			// Check request_id binding (public signal [6])
			if (zkPublicSignals[6] != requestId) {
				revert RequestIdMismatch();
			}

			emit ZkProofVerified(requestId);
		} else if (enforceZkProof) {
			// ZK mode required but no proof provided
			revert InvalidZkProof();
		}
		// Legacy BLS path (kept for backward compatibility)
		// if (enforceBlsSignature && !_verifyBlsSignature(seedCollective, blsSignature)) revert InvalidBlsSignature();

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
