# Next Session Context & Checklist

Date snapshot: 2026-03-24

---

## 🔥 New Checklist (2026-03-25) — Ưu tiên fix destination trước benchmark lớn

> Mục tiêu: chỉ đo E2E nhiều mẫu khi luồng **dispatch + destination execute** đã ổn định.

### P0 — Chặn lỗi destination execute (bắt buộc)

- [ ] Chốt 1 bộ địa chỉ active duy nhất trong `.env`:
  - `RANDOM_ROUTER_ADDRESS`, `RANDOM_RECEIVER_ADDRESS`, `WORMHOLE_RELAYER_SEPOLIA`
  - Không để duplicate key hoặc key legacy trỏ sai contract.
- [ ] Xác minh route on-chain sau mỗi lần deploy:
  - `AXELAR`, `LAYERZERO`, `WORMHOLE` đều `registerAdapter` đúng vào `RandomRouter` mới.
- [ ] Tăng khả năng quan sát lỗi ở destination contract:
  - Thêm/kiểm tra event trước-sau decode payload trong `RandomReceiver`.
  - Bắt được nơi fail cụ thể thay vì chỉ thấy `UNPREDICTABLE_GAS_LIMIT`.
- [ ] Reproduce tối thiểu 1 lỗi Axelar execute và ghi root-cause rõ ràng:
  - Lỗi do gas limit, payload mismatch, trusted sender hay require guard.

### P1 — Ổn định từng bridge độc lập (smoke)

- [ ] Chạy 5 request riêng cho `AXELAR` (`BRIDGE_PRIORITY=AXELAR`).
- [ ] Chạy 5 request riêng cho `LAYERZERO` (`BRIDGE_PRIORITY=LAYERZERO`).
- [ ] Chạy 5 request riêng cho `WORMHOLE` (`BRIDGE_PRIORITY=WORMHOLE`).
- [ ] Mỗi bridge phải đạt tối thiểu 4/5 success ở `dispatch_status=success`.

### P2 — Chuẩn hóa guardrails để không làm sai kết quả benchmark

- [ ] Khóa `CROSS_CHAIN_FEE_CAP_WEI` đủ lớn cho quote hiện tại (đã gặp ~`3.06e16`).
- [ ] Trước khi chạy relayer, clear env override trong shell:
  - `unset CROSS_CHAIN_FEE_CAP_WEI` nếu nghi ngờ đang dính giá trị cũ.
- [ ] Chốt `RELAYER_BRIDGE_TIMEOUT_SECS` và fee buffer theo từng bridge.

### P3 — Chạy benchmark nhiều mẫu (chỉ làm khi P0/P1 pass)

- [ ] Warm-up: 10 request mixed priority.
- [ ] Batch chính: 100 request.
- [ ] Batch mở rộng: 300 request (nếu tỷ lệ fail < 10%).
- [ ] Xuất dữ liệu + chart vào:
  - `test/results/data`
  - `test/results/charts`

### Definition of Done cho đợt benchmark này

- [ ] Có success ổn định ở destination execute cho cả 3 bridge.
- [ ] Không còn lỗi lặp dạng config (`fee cap`, wrong relayer, wrong router).
- [ ] `e2e_metrics_v2.csv` có đủ mẫu để tính p50/p95 theo bridge.
- [ ] Có báo cáo fallback ratio + latency theo timeline request.

### Lệnh vận hành chuẩn (copy nhanh)

```bash
cd /home/xuananh/mpc-vdf/contracts
npx hardhat run scripts/deploy/03_deploy_router_and_adapters.ts --network sepolia
```

```bash
cd /home/xuananh/mpc-vdf/off-chain
BRIDGE_PRIORITY=WORMHOLE,AXELAR,LAYERZERO RELAYER_BRIDGE_TIMEOUT_SECS=40 cargo run -p network_module --bin network_module
```

```bash
cd /home/xuananh/mpc-vdf/contracts
REQUEST_INTERVAL_SECONDS=15 REQUEST_MAX_REQUESTS=10 npm run cron:requests
```

---

## 1) Bối cảnh hiện tại (đã xong)

- On-chain đã chuyển sang kiến trúc Adapter/Router:
  - `contracts/src/RandomRouter.sol`
  - `contracts/src/interfaces/IBridgeAdapter.sol`
  - `contracts/src/adapters/AxelarAdapter.sol`
  - `contracts/src/adapters/LayerZeroAdapter.sol`
  - `contracts/src/adapters/WormholeAdapter.sol`
- Deploy script đã chuyển sang config-driven:
  - `contracts/scripts/deploy/03_deploy_router_and_adapters.ts`
  - `contracts/config/bridges.sepolia.json`
  - `contracts/config/README.md`
- `RandomReceiver` đã bypass BLS để phục vụ đo routing latency:
  - `contracts/src/RandomReceiver.sol`
- Contracts compile OK.

## 2) Điểm nghẽn chính trước benchmark E2E mới

1. Off-chain relayer vẫn bám ABI cũ (`RandomSender`, `uint8 bridge_id`).
2. `network_module` vẫn dùng `LayerZeroMockRelayer`, chưa có relayer thật cho LZ/Wormhole.
3. Pipeline metrics/charts vẫn group theo `bridge_id` số (1/2), chưa theo bridge name/bytes32.
4. Scripts benchmark trong `contracts/scripts/benchmark/*` vẫn gọi tên/address cũ (`RANDOM_SENDER_ADDRESS`).

## 3) Checklist làm tiếp ngày mai (ưu tiên theo thứ tự)

### A. Đồng bộ off-chain với Router mới

- [x] Update ABI call trong `off-chain/network_module/src/bridges.rs`:
  - từ `relayVDFPayload(..., uint8 bridgeId)`
  - sang `relayVDFPayload(..., bytes32 bridgeId)`.
- [x] Đổi contract binding event/query từ `RandomSender` sang `RandomRouter` ở:
  - `off-chain/network_module/src/rpc/mod.rs`
  - `off-chain/network_module/src/main.rs`.
- [x] Chuẩn hóa env alias:
  - dùng `RANDOM_ROUTER_ADDRESS` (có thể fallback sang `RANDOM_SENDER_ADDRESS` tạm thời).

### B. Bỏ mock fallback, dùng real bridge relayer

- [x] Thay `LayerZeroMockRelayer` bằng `LayerZeroRelayer` thật.
- [x] Thêm `WormholeRelayer` thật.
- [x] Cập nhật `MultiBridgeRouter` để chạy priority list thật (không fake tx hash).
- [x] Giữ timeout/retry/backoff hiện có, log rõ bridge fail reason.

### C. Chuẩn hóa schema benchmark mới

- [x] Mở rộng schema metrics sang file mới `off-chain/e2e_metrics_v2.csv` (giữ nguyên file cũ):
  - `bridge_name` (AXELAR/LAYERZERO/WORMHOLE)
  - `bridge_id_hex` (bytes32)
  - `fallback_hops` / `attempt_count`
  - `selected_bridge`
  - `dispatch_status` / `error_reason`
- [x] Không dùng `bridge_id` số làm field chính nữa.

Ghi chú mở rộng:
- Runtime failover trong `network_module` đã chuyển sang nhận diện bridge theo `bridge_name + bridge_id_hex`.
- Việc thêm bridge mới sẽ theo hướng plugin: thêm relayer implement `BridgeRelayer` và đăng ký vào danh sách metadata, không cần đổi schema metrics.
- Có thể cấu hình thứ tự thử bridge bằng env `BRIDGE_PRIORITY` (mặc định `AXELAR,LAYERZERO,WORMHOLE`) mà không cần sửa logic failover.
- Đã thêm registry file `off-chain/network_module/config/bridge_registry.json` (có thể override qua `BRIDGE_REGISTRY_PATH`) để quản lý priority/enable bridge bằng config.
- Đã tách `off-chain/network_module/src/relayer_factory.rs` để gom điểm đăng ký relayer plugin vào một nơi, giảm sửa đổi ở `main.rs` khi mở rộng bridge mới.
- Đã thêm scaffold `off-chain/network_module/src/relayers/template.rs` làm mẫu tạo bridge plugin mới theo chuẩn `BridgeRelayer`.
- Đã thêm `DummyBridgeRelayer` + `dummy_relayer_smoke` để test plugin flow local không cần testnet (bật qua `ENABLE_DUMMY_BRIDGE=true` khi cần).

### D. Cập nhật scripts benchmark/charts

- [x] Update `contracts/scripts/benchmark/request_cronjob.ts` dùng router address.
- [x] Update `contracts/scripts/benchmark/e2e_latency.ts` đọc event từ router mới.
- [x] Update `contracts/scripts/benchmark/postprocess_metrics.ts` group theo bridge string.
- [x] Update `test/tools/generate_charts.py` và `test/tools/plot_new_charts.py`:
  - bỏ hardcode bridge 1/2
  - hỗ trợ 3 bridge + fallback ratio chart.

### E. Chạy batch benchmark mới

- [ ] Chạy warm-up 5-10 request.
- [ ] Chạy batch chính (ví dụ 100-300 request).
- [ ] Xuất summary + charts vào `test/results/data` và `test/results/charts`.

## 4) Lệnh bắt đầu nhanh cho ngày mai

```bash
cd /home/xuananh/mpc-vdf/contracts
npm run compile
```

```bash
cd /home/xuananh/mpc-vdf/off-chain
cargo check -p network_module
```

```bash
cd /home/xuananh/mpc-vdf/contracts
npx hardhat run scripts/deploy/03_deploy_router_and_adapters.ts --network sepolia
```

## 5) Definition of Done cho phase benchmark này

- [ ] Relayer gửi thật qua ít nhất 2 bridge (không mock).
- [ ] Fallback trigger thật khi bridge đầu fail/timeout.
- [ ] CSV mới phản ánh bridge theo `bytes32/name`, không còn numeric-only.
- [ ] Charts mới thể hiện:
  - latency theo bridge
  - fallback ratio
  - timeline theo request.
- [ ] Có bộ dữ liệu đủ lớn để dùng cho phần benchmark paper.

## 6) Ghi chú an toàn

- Không commit private key trong `.env`.
- Nếu đã lộ key testnet, rotate key trước batch lớn.
- Giữ `RELAYER_RPC_MAX_RETRIES` + `RELAYER_RPC_RETRY_BASE_MS` để giảm lỗi 429.
