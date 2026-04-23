# Benchmark Suite — 5 Kịch bản đo đạc chuẩn IEEE

## Yêu cầu hệ thống

```bash
# Rust toolchain
rustup update stable

# Node.js dependencies (contracts)
cd contracts && npm install

# Python dependencies
pip install pandas matplotlib seaborn numpy
```

## Thư mục cấu trúc

```
scripts/
├── benchmark/
│   ├── 01_offchain_compute.sh    # Kịch bản 1: VDF scaling + ZK + RAM/CPU
│   ├── 02_gas_metrics.ts         # Kịch bản 2: Gas on-chain
│   ├── 03_latency_breakdown.sh   # Kịch bản 3: E2E latency 5 phases
│   ├── 04_failover_test.sh       # Kịch bản 4: Cross-chain failover
│   ├── 05_mev_censorship.ts      # Kịch bản 5: MEV censorship
│   └── data/                     # CSV output + charts
│       └── charts/               # PNG biểu đồ (300 DPI)
├── plot/
│   ├── ieee_style.py             # Module cấu hình IEEE shared
│   ├── plot_offchain_compute.py  # Fig 1: Dual-axis line chart
│   ├── plot_gas_metrics.py       # Fig 2: Grouped bar (log Y)
│   ├── plot_latency_breakdown.py # Fig 3: Stacked horizontal bar
│   ├── plot_failover.py          # Fig 4: Time-series spike
│   └── plot_mev_censorship.py    # Fig 5: Dual-axis area+line
└── README_benchmark.md           # File này
```

## Chạy từng kịch bản

### Reset dữ liệu benchmark (khuyến nghị trước mỗi đợt đo mới)

```bash
bash scripts/benchmark/00_clean_outputs.sh
```

Lệnh này xóa toàn bộ CSV/PNG benchmark cũ, thư mục temp proof, và Python cache để đảm bảo lần đo tiếp theo bắt đầu từ trạng thái sạch.

### Smoke test (kiểm tra đơn nhanh)

```bash
# Cấp quyền
chmod +x scripts/benchmark/*.sh

# Dọn dữ liệu cũ trước khi chạy mới
bash scripts/benchmark/00_clean_outputs.sh

# Kịch bản 1: VDF sweep (chỉ 2 điểm, ~1 phút)
bash scripts/benchmark/01_offchain_compute.sh --quick

# Kịch bản 2: Gas metrics (Hardhat local node)
cd contracts && npx hardhat run ../scripts/benchmark/02_gas_metrics.ts --network hardhat

# Kịch bản 3: Latency breakdown (2 runs, ~30 giây)
bash scripts/benchmark/03_latency_breakdown.sh --quick

# Kịch bản 4: Failover (10 requests, ~2 phút)
bash scripts/benchmark/04_failover_test.sh --quick

# Kịch bản 5: MEV censorship (Hardhat local)
cd contracts && npx hardhat run ../scripts/benchmark/05_mev_censorship.ts --network hardhat
```

### Chạy đầy đủ (production)

```bash
# Kịch bản 1: VDF sweep 2^16 → 2^24 (9 điểm, ~30-60 phút)
bash scripts/benchmark/01_offchain_compute.sh

# Kịch bản 3: 10 pipeline runs (~5-10 phút)
bash scripts/benchmark/03_latency_breakdown.sh

# Kịch bản 4: 100 requests + failover (~20 phút)
bash scripts/benchmark/04_failover_test.sh
```

## Vẽ biểu đồ

```bash
# Từ thư mục gốc project
python3 scripts/plot/plot_offchain_compute.py
python3 scripts/plot/plot_gas_metrics.py
python3 scripts/plot/plot_latency_breakdown.py
python3 scripts/plot/plot_failover.py
python3 scripts/plot/plot_mev_censorship.py
```

Biểu đồ xuất ra: `scripts/benchmark/data/charts/*.png` (300 DPI, font serif, IEEE Q1 standard).

## MPC Network (Docker)

```bash
# Khởi tạo mạng lưới 3-of-4 MPC
cd docker
VDF_T=262144 ZK_PROVER_ENABLED=true docker-compose up -d

# Kiểm tra trạng thái
docker-compose ps
docker-compose logs -f node-1
```
