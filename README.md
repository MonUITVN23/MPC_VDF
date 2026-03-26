# Hệ thống ZK-SNARK Cross-Chain Randomness 🎲🔗

Dự án này là một nguyên mẫu hoàn chỉnh (Proof-of-Concept) cho việc tạo và xác minh số ngẫu nhiên (Randomness) an toàn trên chuỗi khối, áp dụng cơ chế đồng thuận phân tán **MPC (Multi-Party Computation)** kết hợp với **VDF (Verifiable Delay Function)** và **ZK-SNARK (Zero-Knowledge Proof)**. Hệ quả cuối cùng (Số ngẫu nhiên an toàn) được truyền đi xuyên chuỗi (Cross-Chain) đến các mạng lưới đích thông qua các Bridge lớn (Axelar, LayerZero, Wormhole).

## 1. Tổng quan Dự án

Hệ thống cung cấp một phương pháp giải quyết tối ưu **"Trilemma Randomness"** trên Blockchain: Tốc độ - Bảo mật - Phí Gas Rẻ. Thay vì xác minh chữ ký mã hóa nặng nề (như thuật toán BLS12-381) trực tiếp bằng ngôn ngữ Solidity trên On-chain, toàn bộ quá trình xác minh được mã hóa thành mạch **Circom ZK Circuit**.

### 🌟 Những hạng mục ĐÃ hoàn thành
1. **Mạch chứng minh (ZK Circuit):** Hoàn chỉnh quá trình biên dịch `bls_commitment.circom` để kiểm tra các hàm băm PK, Message, và Signature thành một Groth16 Proof (Kéo thời gian Prover xuống còn ~2s).
2. **Cơ sở hạ tầng Relayer Off-chain (Rust):** Viết bằng Rust mạnh mẽ, hỗ trợ pipeline cực độ trễ thấp liên kết trực tiếp với Node.js.
   - Thư viện tự động "độn số" (padding) zero-bytes cho tương thích ZK Circuit (Signals Array).
   - Module bắt lỗi và khắc phục Deadlock Node.js/Snarkjs để luồng chạy mượt mà ngay cả khi có tải nặng.
3. **Thư viện Router Đa cầu nối (Multi-Bridge Router):** Fail-over (chuyển đổi dự phòng) linh hoạt giữa **Axelar** (Phí rẻ, default), **Wormhole** (Trình tiếp dẫn), và **LayerZero** (Endpoint V2).
4. **Hệ thống Smart Contract On-chain:** 
   - Triển khai thành công trên Testnet (Sepolia -> Polygon Amoy).
   - Hỗ trợ Dynamic Fee Estimation và ngăn ngừa các lỗi bảo mật Gas Revert.

### 🚧 Những hạng mục CHƯA hoàn thành (Future Works)
1. **Phi tập trung Mạng Lưới MPC thực tế:** Hiện tại các khoá dkg/shares đang được mô phỏng giả lập cục bộ bằng một node duy nhất (Simulation). Trong môi trường Production, cần kết nối thực sự đến một giao thức TSS (Threshold Signature Scheme).
2. **Tính toán VDF tối ưu phần cứng:** Code VDF hiện được chạy bằng CPU Rust. Trong tương lai cần các bộ tăng tốc ASIC hoặc FPGA để ép thời gian VDF sinh ra độ trễ dưới 2-3 giây ở mức $T$ lớn.
3. **Frontend dApp & Indexer:** Chưa có giao diện tương tác minh hoạ cũng như Subgraph để lập chỉ mục các trạng thái Request từ người dùng cuối. Đang hoàn toàn phải giao tiếp qua CLI / Log terminal.

---

## 2. Hướng dẫn Bước-từng-bước (Step-by-Step Guide)

Để setup và chạy thử nghiệm một luồng hoàn chỉnh, vui lòng làm theo hướng dẫn dưới đây:

### Bước 2.1: Chuẩn bị Môi trường (Prerequisites)
- Vẫn còn ở trên Mạng Testnet Sepolia. Môi trường yêu cầu **Node.js 18+**, **Rust/Cargo**, và **Foundry (cast, forge)**.
- Kiểm tra khoá Private Key trong file `.env` (tại thu mục gốc) và đảm bảo có đủ ETH mạng Sepolia (phục vụ đổ phí bridge).

### Bước 2.2: Build Smart Contract và ZK Circuit
Mọi tệp biên dịch của ZK và Hardhat đều đặt trong thư mục `contracts`:
```bash
cd contracts

# Cài đặt thư viện Nodejs
npm install

# Build mạch ZK và Sinh Trusted Setup (Nếu sửa đổi file circom)
# Quá trình này đã được làm xong, sinh ra các file ptau và zkey
bash circuits/scripts/setup.sh

# Compile hợp đồng thông minh sang ABI
npx hardhat compile
```

### Bước 2.3: Chạy Relayer Off-chain
Relayer đóng vai trò như một bộ não, dùng để quét (listen) các event trên chuỗi, tính toán MPC -> VDF -> ZKP và đẩy lại lên On-chain. Hãy mở một tab Terminal riêng:

```bash
cd off-chain

# Chạy bản Release để Rust tối ưu hoá tốc độ tính toán VDF và API Bridge
RUST_LOG=info cargo run --bin network_module --release
```
*Trạng thái thành công: Bạn sẽ thấy dòng chữ "Bắt đầu lắng nghe tại địa chỉ 0x89ad... (mode=HTTP polling, interval=8s)" hiện lên.*

### Bước 2.4: Gửi một Yêu cầu Randomness On-chain
Trong khi Relayer đang chạy, hãy mở một tab Terminal khác. Gửi một transaction yêu cầu tạo số ngẫu nhiên lên Testnet bằng lệnh Foundry (Ví dụ lấy số seed giả lập cục bộ là `12345`):

```bash
cd ..
source .env

# Gọi hàm requestRandomness() gửi lên Sepolia
cast send $RANDOM_ROUTER_ADDRESS "requestRandomness(uint256)" 12345 \
    --rpc-url $SEPOLIA_RPC_URL \
    --private-key $PRIVATE_KEY
```

### Bước 2.5: Quan sát và Đánh giá (Verification)
1. **Trên Terminal của Relayer:** 
   - Quá trình bắt lấy Request sẽ lập tức diễn ra.
   - Quá trình VDF tiêu tốn ~ 4 giây.
   - Quá trình ZK Prove (SnarkJS) tiêu tốn ~ 2 giây.
   - Giao dịch Dispatch được trích xuất (Ước tính phí linh động).
2. **Theo dõi Chuyển Cầu (Bridge Selection):**
   - Theo thiết lập trong file `.env`, `BRIDGE_PRIORITY=AXELAR,LAYERZERO,WORMHOLE`, Relayer sẽ cố sử dụng Axelar đầu tiên. Nếu có lỗi mạng hoặc ước tính phí (Ex: Gas Revert), Relayer tự động ngắt và chuyển qua cầu LayerZero và tương tự.
   - Nếu thành công, Log sẽ trả lại `tx_hash` (Mã giao dịch) và In ra bảng biểu Final Result trực quan ngay trên Console.
3. **Thống kê Báo cáo CSV:** Đo đạc chi phí độ trễ (Latency Pipeline) giữa thời gian xử lý các bước đều được xuất tự động ra tệp `e2e_metrics_v2.csv` nằm trong thư mục `/off-chain/`. Có thể dùng file này cho báo cáo đánh giá.
