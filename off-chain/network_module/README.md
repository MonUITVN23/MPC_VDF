# network_module E2E Relayer

Relayer này thực hiện luồng E2E:
1. Poll `LogRequest` từ `RandomSender` (Sepolia)
2. Chạy `DKG -> VDF` bằng `crypto_engine`
3. Relay payload `{requestId, Y, pi, seed_collective, aggregate_signature}` vào `RandomReceiver` (Amoy)

## Biến môi trường bắt buộc (đọc từ `../.env`)
- `SEPOLIA_RPC_URL`
- `AMOY_RPC_URL`
- `PRIVATE_KEY`
- `RANDOM_SENDER_ADDRESS`
- `RANDOM_RECEIVER_ADDRESS`

## Biến tùy chọn
- `VDF_T_DEFAULT` (mặc định `2^20`)
- `RELAYER_POLL_INTERVAL_SECS` (mặc định `8`)
- `RELAYER_START_LOOKBACK_BLOCKS` (mặc định `500`)

## Chạy relayer
```bash
cd /home/xuananh/mpc-vdf/off-chain
cargo run -p network_module
```

## Ghi chú kiến trúc
- Module hiện relay trực tiếp vào `submitOptimisticResult` của receiver để hoàn thiện E2E.
- Có thể thay lớp relay này bằng Axelar GMP adapter mà không đổi pipeline crypto output.
