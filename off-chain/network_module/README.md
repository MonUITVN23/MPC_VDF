# network_module E2E Relayer

Relayer này thực hiện luồng E2E:
1. Poll `LogRequest` từ `RandomRouter` (Sepolia)
2. Chạy `DKG -> VDF` bằng `crypto_engine`
3. Relay payload `{requestId, Y, pi, seed_collective, aggregate_signature}` qua adapter bridge vào `RandomReceiver` (Amoy)

## Biến môi trường bắt buộc (đọc từ `../.env`)
- `SEPOLIA_RPC_URL`
- `AMOY_RPC_URL`
- `PRIVATE_KEY`
- `RANDOM_ROUTER_ADDRESS` (fallback tạm thời: `RANDOM_SENDER_ADDRESS`)
- `RANDOM_RECEIVER_ADDRESS`

## Biến tùy chọn
- `VDF_T_DEFAULT` (mặc định `2^20`)
- `RELAYER_POLL_INTERVAL_SECS` (mặc định `8`)
- `RELAYER_START_LOOKBACK_BLOCKS` (mặc định `500`)
- `E2E_METRICS_V2_PATH` (mặc định `e2e_metrics_v2.csv`)
- `BRIDGE_PRIORITY` (override runtime, ví dụ `AXELAR,LAYERZERO,WORMHOLE`)
- `BRIDGE_REGISTRY_PATH` (mặc định `network_module/config/bridge_registry.json`)

## Bridge plugin registry

- File mặc định: `off-chain/network_module/config/bridge_registry.json`
- Ý tưởng: core failover giữ nguyên, chỉ đổi config thứ tự/enable bridge để thử nghiệm.
- Khi thêm bridge mới trong code, chỉ cần implement `BridgeRelayer` và đăng ký tại `src/relayer_factory.rs`, rồi thêm tên vào file config/env.

## Template cho bridge mới

- File scaffold: `src/relayers/template.rs`
- Quy trình chuẩn:
	1. Copy `TemplateRelayer` thành relayer mới (ví dụ `FooBridgeRelayer`).
	2. Implement `relay_payload(...)` với logic fee + dispatch thật.
	3. Đăng ký plugin name trong `src/relayer_factory.rs`.
	4. Thêm name vào `BRIDGE_PRIORITY` hoặc `config/bridge_registry.json`.

## Local plugin smoke test (không cần testnet)

- Dummy plugin nằm ở `src/relayers/dummy.rs`.
- Enable dummy cho runtime chính bằng env: `ENABLE_DUMMY_BRIDGE=true`.
- Chạy smoke runner:

```bash
cd /home/xuananh/mpc-vdf/off-chain
cargo run -p network_module --bin dummy_relayer_smoke
```

## Chạy relayer
```bash
cd /home/xuananh/mpc-vdf/off-chain
cargo run -p network_module
```

## Ghi chú kiến trúc
- Module hiện relay trực tiếp vào `submitOptimisticResult` của receiver để hoàn thiện E2E.
- Có thể thay lớp relay này bằng Axelar GMP adapter mà không đổi pipeline crypto output.
