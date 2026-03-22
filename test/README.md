# Test Artifacts Workspace

Thư mục này gom toàn bộ dữ liệu/thành phẩm phục vụ benchmark và thử nghiệm để tách khỏi mã nguồn chính.

## Cấu trúc

- `test/results/charts/`: ảnh biểu đồ và ảnh kết quả monitor.
- `test/results/data/`: CSV/JSON dữ liệu benchmark và e2e.
- `test/tools/`: script Python phục vụ vẽ/đánh giá dữ liệu thử nghiệm.

## Gợi ý sử dụng

- Khi chạy benchmark mới, ghi/đẩy output vào `test/results/`.
- Mã nguồn chính giữ ở `contracts/` và `off-chain/` để repo sạch, dễ review.
