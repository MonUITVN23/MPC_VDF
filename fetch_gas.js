const fs = require('fs');
const readline = require('readline');
const { ethers } = require('ethers');

// Cấu hình RPC
const provider = new ethers.JsonRpcProvider('https://1rpc.io/sepolia');
const CSV_FILE = 'off-chain/e2e_metrics_v2.csv';
const OUT_CSV_FILE = 'off-chain/e2e_metrics_with_gas.csv';

async function main() {
    const fileStream = fs.createReadStream(CSV_FILE);
    const rl = readline.createInterface({
        input: fileStream,
        crlfDelay: Infinity
    });

    let isHeader = true;
    let outStream = fs.createWriteStream(OUT_CSV_FILE);

    console.log("Bắt đầu truy xuất dữ liệu Gas từ Blockchain...");
    
    for await (const line of rl) {
        if (isHeader) {
            outStream.write(line + ',gas_used,gas_price_gwei,tx_fee_eth\n');
            isHeader = false;
            continue;
        }

        if (!line.trim()) continue;

        const columns = line.split(',');
        // format: request_id,t1_timestamp,t2_mpc_ms,t3_vdf_ms,t4_dispatch_ms,bridge_name,bridge_id_hex,selected_bridge,attempt_count,fallback_hops,dispatch_status,error_reason,tx_hash
        const txHashStr = columns[12];
        const status = columns[10].replace(/"/g, '');

        if (status === 'success' && txHashStr && txHashStr.length > 10) {
            const txHash = txHashStr.replace(/"/g, '');
            try {
                const receipt = await provider.getTransactionReceipt(txHash);
                if (receipt) {
                    const gasUsed = receipt.gasUsed.toString();
                    const gasPrice = receipt.gasPrice;
                    const gasPriceGwei = ethers.formatUnits(gasPrice, 'gwei');
                    const txFeeEth = ethers.formatEther(gasPrice * receipt.gasUsed);
                    
                    console.log(`[OK] Request ${columns[0]} - Bridge: ${columns[5]} - Gas: ${gasUsed}`);
                    outStream.write(`${line},${gasUsed},${gasPriceGwei},${txFeeEth}\n`);
                } else {
                    console.log(`[WARN] Not found receipt for ${txHash}`);
                    outStream.write(`${line},0,0,0\n`);
                }
            } catch (err) {
                console.error(`[ERROR] Failed to fetch ${txHash}: ${err.message}`);
                outStream.write(`${line},0,0,0\n`);
            }
        } else {
            // failed tx
            outStream.write(`${line},0,0,0\n`);
        }
        
        // Tránh bị RPC rate limit
        await new Promise(r => setTimeout(r, 200));
    }

    outStream.close();
    console.log(`\n✅ Đã xuất báo cáo trọn vẹn tại: ${OUT_CSV_FILE}`);
}

main().catch(console.error);
