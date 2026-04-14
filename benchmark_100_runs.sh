#!/bin/bash

source .env

if [ -z "$PRIVATE_KEY" ] || [ -z "$SEPOLIA_RPC_URL" ] || [ -z "$RANDOM_ROUTER_ADDRESS" ]; then
    echo "Lỗi: Không tìm thấy PRIVATE_KEY, SEPOLIA_RPC_URL hoặc RANDOM_ROUTER_ADDRESS trong tệp .env"
    exit 1
fi

ROUTER_ADDRESS=$RANDOM_ROUTER_ADDRESS
TOTAL_RUNS=100
DELAY_SECONDS=25

echo "==========================================================="
echo "BẮT ĐẦU CHẠY BENCHMARK ($TOTAL_RUNS SỰ KIỆN)"
echo "Tự động gửi Request lên Router: $ROUTER_ADDRESS"
echo "Giãn cách thời gian: ${DELAY_SECONDS}s"
echo "Cảnh báo: Xin đảm bảo Relayer đang chạy ở Terminal khác!"
echo "==========================================================="

for ((i=1; i<=TOTAL_RUNS; i++))
do
    SEED=$(date +%s%N)
    
    echo "[$i/$TOTAL_RUNS] Đang chuẩn bị gửi Request với Seed: $SEED..."

    cast send "$ROUTER_ADDRESS" "requestRandomness(uint256)" "$SEED" \
        --rpc-url "$SEPOLIA_RPC_URL" \
        --private-key "$PRIVATE_KEY" \
        --async > /dev/null

    if [ $? -eq 0 ]; then
        echo "[$i/$TOTAL_RUNS] Thành công! Vui lòng kiểm tra Log của Relayer."
    else
        echo "[$i/$TOTAL_RUNS] Lỗi mạng! Revert ở transaction này, tiếp tục đợi..."
    fi

    if [ $i -lt $TOTAL_RUNS ]; then
        echo "Đang đợi ${DELAY_SECONDS}s trạm trước khi gửi yêu cầu tiếp theo..."
        sleep $DELAY_SECONDS
    fi
done

echo "==========================================================="
echo "ĐÃ HOÀN TẤT LÊN LỊCH $TOTAL_RUNS BENCHMARK"
echo "Dữ liệu CSV sẽ được trích xuất tự động tại: ./off-chain/e2e_metrics_v2.csv"
echo "==========================================================="
