# ZK + Optimistic Verification Implementation Checklist

Checklist này bám sát cấu trúc hiện tại của repo `mpc-vdf`, mục tiêu là hoàn thiện pipeline đo `t2 -> t5` cho bài báo khi tích hợp ZK-proof bọc BLS12-381.

---

## 0) Baseline & Scope (Paper-ready)

- [ ] Chốt baseline hiện tại (không ZK) với pipeline: MPC -> VDF -> Axelar -> Receiver.
- [ ] Chốt định nghĩa metric chính:
  - [ ] `t2_mpc_ms`
  - [ ] `t3_vdf_ms`
  - [ ] `t3_5_zkprove_ms` (mới)
  - [ ] `t4_dispatch_ms`
  - [ ] `t5_delivery_ms`
- [ ] Chốt cách tính `t5_delivery_ms` cho paper:
  - [ ] Mode A: chỉ `executed successfully`
  - [ ] Mode B: `delivery-to-failure` (dùng `time_spent.total` bất chấp status)
- [ ] Chốt tập dữ liệu benchmark (số lượng request, bridge_id, khoảng thời gian chạy).

**Deliverable:** 1 đoạn methodology rõ trong paper + schema CSV cố định.

---

## 1) Scaffold zkVM project (SP1)

**Thư mục liên quan:** `off-chain/`

- [ ] Cài SP1 CLI + toolchain (`sp1up`).
- [ ] Tạo project mới: `off-chain/zk_bls_wrapper`.
- [ ] Commit khung thư mục ban đầu.

**Deliverable:** thư mục `off-chain/zk_bls_wrapper/` tồn tại, build template chạy được.

---

## 2) Guest Program: Verify BLS12-381 trong zkVM

**File chính:** `off-chain/zk_bls_wrapper/program/Cargo.toml`, `off-chain/zk_bls_wrapper/program/src/main.rs`

- [ ] Thêm dependency BLS cho guest (`bls-signatures`).
- [ ] Guest đọc private inputs từ host:
  - [ ] `pk_bytes`
  - [ ] `sig_bytes`
  - [ ] `message_bytes`
- [ ] Deserialize `PublicKey`, `Signature` từ bytes.
- [ ] Verify chữ ký BLS trong guest.
- [ ] `assert!(is_valid)` để fail-proof khi chữ ký sai.
- [ ] Commit public values ra proof:
  - [ ] Tối thiểu `message/payload hash`
  - [ ] Khuyến nghị thêm `request_id` hoặc context bytes
- [ ] Build guest sang RISC-V bằng `cargo prove build`.

**Acceptance criteria:** build thành công, không lỗi `no_std`/cross-compile.

---

## 3) Host Program: E2E prove + verify local

**File chính:** `off-chain/zk_bls_wrapper/script/Cargo.toml`, `off-chain/zk_bls_wrapper/script/src/bin/main.rs` (hoặc `src/main.rs` tùy template)

- [ ] Thêm dependencies host:
  - [ ] `sp1-sdk` (đúng version template)
  - [ ] `bls-signatures`
  - [ ] `rand`
- [ ] Load ELF đúng đường dẫn build output của guest.
- [ ] Sinh mock data local:
  - [ ] keypair BLS12-381
  - [ ] message
  - [ ] signature
- [ ] Ghi input vào `SP1Stdin` theo đúng thứ tự guest đọc.
- [ ] Chạy proving (`client.prove(...).run()`).
- [ ] Verify proof local (`client.verify(...)`).
- [ ] Kiểm tra `public_values` khớp message/context.
- [ ] Ghi log `proving_time_ms`.

**Acceptance criteria:** in được `Proof generated successfully` và `ZK Proof verified successfully`.

---

## 4) Tích hợp Off-chain Node với ZK wrapper

**File liên quan:**
- `off-chain/network_module/src/main.rs`
- `off-chain/network_module/src/bridges.rs`
- `off-chain/crypto_engine/src/lib.rs` (+ module phụ trợ)

- [ ] Thêm bước gọi host prover SP1 sau khi có `seed_collective + aggregate_signature`.
- [ ] Đo và lưu `t3_5_zkprove_ms`.
- [ ] Chuẩn hóa output prover:
  - [ ] proof bytes
  - [ ] public values bytes
  - [ ] payload hash/context hash
- [ ] Update struct payload relay để mang ZK artifacts.
- [ ] Đảm bảo retry/error handling khi prove fail (không crash toàn pipeline).

**Acceptance criteria:** node off-chain tạo được payload có ZK artifacts trước bước relay.

---

## 5) Smart Contracts: Receiver dùng ZK verifier + optimistic flow

**File liên quan (gợi ý):**
- `contracts/src/RandomSender.sol`
- `contracts/src/RandomReceiver.sol`
- `contracts/src/interfaces/*.sol`
- `contracts/scripts/deploy/*.ts`

- [ ] Thiết kế ABI payload mới cho relay có chứa:
  - [ ] dữ liệu randomness/VDF cần thiết
  - [ ] `zk_proof`
  - [ ] `public_values`
- [ ] `RandomSender` relay payload mới qua Axelar.
- [ ] `RandomReceiver::_execute`:
  - [ ] verify ZK proof trước
  - [ ] decode/validate public values
  - [ ] chỉ enqueue optimistic result khi proof hợp lệ
- [ ] Giữ nguyên challenge window/finalize flow hiện tại (hoặc chỉnh có kiểm soát).
- [ ] Bỏ dependency verify BLS trực tiếp trên EVM (tránh mismatch curve).

**Acceptance criteria:** tx đích không còn fail vì BLS curve mismatch; flow optimistic chạy end-to-end.

---

## 6) Deploy & Ops scripts

**File liên quan:**
- `contracts/scripts/deploy/01_deploy_sender.ts`
- `contracts/scripts/deploy/02_deploy_receiver.ts`
- `contracts/scripts/ops/01_relay_payload.ts`

- [ ] Cập nhật deploy script để set địa chỉ verifier/config mới (nếu có).
- [ ] Cập nhật relay script để gửi payload có ZK artifacts.
- [ ] In ra link AxelarScan + tx hash + request_id phục vụ tracing benchmark.
- [ ] Cập nhật `.env` keys cần cho chế độ ZK.

**Acceptance criteria:** scripts chạy được trên testnet với payload mới.

---

## 7) Benchmark & Metrics Pipeline (Paper)

**File liên quan:**
- `test/results/data/e2e_metrics.csv`
- `contracts/scripts/benchmark/*.ts`
- `tools/fetch_delivery_time.py` (hoặc script tương đương)

- [ ] Mở rộng CSV schema thêm cột:
  - [ ] `t3_5_zkprove_ms`
  - [ ] `t5_delivery_ms`
  - [ ] `delivery_status`
- [ ] Thu đủ N mẫu cho 2 mode:
  - [ ] baseline không ZK
  - [ ] có ZK wrapper
- [ ] Lấy `t5_delivery_ms` từ Axelarscan API (`time_spent.total` theo mode đã chốt).
- [ ] Tính thống kê cho paper:
  - [ ] mean
  - [ ] median (p50)
  - [ ] p95
  - [ ] min/max
  - [ ] std dev
- [ ] Sinh bảng + biểu đồ so sánh trước/sau ZK.

**Acceptance criteria:** có dataset cuối cùng và số liệu thống kê tái lập được.

---

## 8) Testing Matrix

- [ ] Unit test cho guest input parsing + invalid signature path.
- [ ] Unit/integration test cho host proving + local verify.
- [ ] Contract tests:
  - [ ] proof hợp lệ -> enqueue success
  - [ ] proof sai -> revert đúng lý do
- [ ] E2E test có Axelar path (ít nhất một batch nhỏ).
- [ ] Soak test với số lượng request đủ cho benchmark paper.

**Acceptance criteria:** test pass ổn định, không có false-positive metrics.

---

## 9) Paper Artifacts & Reproducibility

- [ ] Freeze commit hash cho từng experiment.
- [ ] Ghi cấu hình máy benchmark:
  - [ ] CPU / RAM
  - [ ] OS
  - [ ] phiên bản toolchain (Rust, SP1, Node, Hardhat)
- [ ] Chuẩn hóa câu lệnh chạy (copy-paste) cho toàn pipeline.
- [ ] Đóng gói file output:
  - [ ] raw CSV
  - [ ] processed CSV/JSON
  - [ ] chart scripts
- [ ] Viết mục “Implementation Challenges” + “Threats to Validity”.

**Acceptance criteria:** người khác có thể tái chạy và ra kết quả gần tương đương.

---

## 10) Definition of Done (DoD)

- [ ] Không còn lỗi execute do mismatch BLS curve trên EVM path chính.
- [ ] Pipeline cross-chain có thể hoàn tất theo optimistic flow.
- [ ] Thu được đầy đủ `t2 -> t5` (có `t3_5` nếu bật ZK).
- [ ] Có số liệu so sánh baseline vs ZK để đưa vào paper.
- [ ] Có tài liệu tái lập thí nghiệm end-to-end.

---

## Gợi ý thứ tự triển khai nhanh (practical order)

1. Hoàn tất mục **2 + 3** (guest/host local E2E).
2. Tích hợp node off-chain theo mục **4**.
3. Cập nhật contracts theo mục **5**.
4. Update scripts deploy/ops theo mục **6**.
5. Chạy benchmark + thống kê theo mục **7**.
6. Chốt test matrix + reproducibility theo mục **8 + 9**.

> Mẹo cho paper: luôn lưu cả **attempted delivery latency** và **successful execution latency** để phân tách tác động của network routing vs application logic.
