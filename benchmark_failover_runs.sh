#!/bin/bash

source .env

if [ -z "$PRIVATE_KEY" ] || [ -z "$SEPOLIA_RPC_URL" ] || [ -z "$RANDOM_ROUTER_ADDRESS" ]; then
    echo "Lỗi: Không tìm thấy các hằng số môi trường (Env) quan trọng."
    exit 1
fi

ROUTER_ADDRESS=$RANDOM_ROUTER_ADDRESS
DELAY_SECONDS=25

echo "==========================================================="
echo "BẮT ĐẦU CHẠY BENCHMARK FAILOVER (QUAY VÒNG 3 CẦU NỐI)"
echo "==========================================================="

cleanup() {
    echo -e "\nĐang dọn dẹp tiến trình Relayer..."
    pkill -f network_module
    exit 0
}
trap cleanup EXIT INT TERM

send_requests() {
    local start_idx=$1
    local end_idx=$2
    local bridge_name=$3

    echo "-----------------------------------------------------------"
    echo "-> GIAI ĐOẠN: Ép Relayer sử dụng cầu $bridge_name (Từ $start_idx đến $end_idx)"
    
    export BRIDGE_PRIORITY=$bridge_name
    
    pkill -f network_module 2>/dev/null
    sleep 2 
    
    (cd off-chain && RUST_LOG=info cargo run --bin network_module --release > /tmp/relayer.log 2>&1 &)
    
    echo "Đang đợi 5 giây cho Relayer khởi động hoàn tất..."
    sleep 5

    for ((i=start_idx; i<=end_idx; i++))
    do
        SEED=$(date +%s%N)
        echo "[$i/100] [$bridge_name] Gửi ZK Random Request với Seed: $SEED..."

        cast send "$ROUTER_ADDRESS" "requestRandomness(uint256)" "$SEED" \
            --rpc-url "$SEPOLIA_RPC_URL" \
            --private-key "$PRIVATE_KEY" \
            --async > /dev/null

        if [ $? -eq 0 ]; then
            echo "[$i/100] Thành công! Vui lòng kiểm tra Log của Relayer."
        else
            echo "[$i/100] Lỗi mạng! Revert ở transaction này."
        fi

        if [ $i -lt $end_idx ]; then
            echo "Đang đợi ${DELAY_SECONDS}s trước khi gửi yêu cầu tiếp theo..."
            sleep $DELAY_SECONDS
        fi
    done
    
    echo "Hoàn thành gửi lô dữ liệu cho cầu: $bridge_name"
    echo "Đợi 20 giây cho giao dịch cuối cùng được Broadcast..."
    sleep 20
}

send_requests 1 2 "WORMHOLE"

echo "==========================================================="
echo "🎉 ĐÃ HOÀN TẤT LÊN LỊCH QUAY VÒNG 100 BENCHMARK FAILOVER"
echo "Dữ liệu CSV đã được trích xuất xen kẽ tại: ./off-chain/e2e_metrics_v2.csv"
echo "==========================================================="
