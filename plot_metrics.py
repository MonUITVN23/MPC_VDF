import pandas as pd
import matplotlib.pyplot as plt
import seaborn as sns
import numpy as np
import os


csv_file = 'off-chain/e2e_metrics_with_gas.csv'
out_dir = 'test/results/charts'



if not os.path.exists(out_dir):
    os.makedirs(out_dir, exist_ok=True)

if not os.path.exists(csv_file):
    print(f"Error: {csv_file} not found!")
    exit(1)


df = pd.read_csv(csv_file)


success_df = df[df['dispatch_status'] == 'success'].copy()



success_df['t3_vdf_sec'] = success_df['t3_vdf_ms'] / 1000
success_df['t4_dispatch_sec'] = success_df['t4_dispatch_ms'] / 1000
success_df['total_latency_sec'] = success_df['t3_vdf_sec'] + success_df['t4_dispatch_sec']


bridge_colors = {'AXELAR': '#2E86C1', 'LAYERZERO': '#E67E22', 'WORMHOLE': '#27AE60'}


import warnings
warnings.filterwarnings('ignore')

plt.style.use('ggplot')
sns.set_context("paper", font_scale=1.2)
try:
    sns.set_theme(style="whitegrid", rc={"axes.edgecolor": "0.15", "xtick.bottom": True, "ytick.left": True})
except:
    pass




plt.figure(figsize=(10, 6))
sns.boxplot(x='selected_bridge', y='total_latency_sec', data=success_df, palette=bridge_colors)
sns.stripplot(x='selected_bridge', y='total_latency_sec', data=success_df, color='black', alpha=0.3, jitter=True)

plt.title('End-to-End Total Latency (MPC + VDF + Relay) by Bridge', fontweight='bold', pad=15)
plt.ylabel('Total Latency (Seconds)', fontweight='bold')
plt.xlabel('Cross-Chain Bridge', fontweight='bold')
plt.grid(axis='y', linestyle='--', alpha=0.7)
plt.tight_layout()
plt.savefig(f"{out_dir}/latency_boxplot.png", dpi=300)
plt.close()





avg_latency = success_df.groupby('selected_bridge')[['t3_vdf_sec', 't4_dispatch_sec']].mean()

order = [b for b in ['AXELAR', 'LAYERZERO', 'WORMHOLE'] if b in avg_latency.index]
avg_latency = avg_latency.loc[order]

ax = avg_latency.plot(kind='bar', stacked=True, figsize=(10, 6), 
                      color=['#8E44AD', '#3498DB'], alpha=0.85)

plt.title('Average Latency Breakdown per Bridge (No MPC)', fontweight='bold', pad=15)
plt.ylabel('Average Latency (Seconds)', fontweight='bold')
plt.xlabel('Cross-Chain Bridge', fontweight='bold')
plt.legend(['VDF Computation', 'Cross-Chain Dispatch'], loc='upper center', bbox_to_anchor=(0.5, -0.15), ncol=2)
plt.xticks(rotation=0)


for c in ax.containers:
    ax.bar_label(c, label_type='center', fmt='%.1fs', color='white', fontweight='bold')

plt.tight_layout()
plt.savefig(f"{out_dir}/latency_breakdown.png", dpi=300)
plt.close()




valid_gas = success_df[success_df['gas_used'] > 0]
if not valid_gas.empty:
    avg_gas = valid_gas.groupby('selected_bridge')['gas_used'].mean().reindex(order)
    
    plt.figure(figsize=(9, 6))
    ax = sns.barplot(x=avg_gas.index, y=avg_gas.values, palette=bridge_colors)
    
    plt.title('Average Sepolia On-Chain Gas Cost per Bridge Verification', fontweight='bold', pad=15)
    plt.ylabel('Gas Used', fontweight='bold')
    plt.xlabel('Cross-Chain Bridge', fontweight='bold')
    plt.grid(axis='y', linestyle='--', alpha=0.7)
    
    
    for i, v in enumerate(avg_gas.values):
        if not np.isnan(v):
            ax.text(i, v + (v*0.02), f"{int(v):,}", ha='center', va='bottom', fontweight='bold', fontsize=12)
            
    plt.ylim(0, max(avg_gas.values) * 1.15)
    plt.tight_layout()
    plt.savefig(f"{out_dir}/gas_cost_bar.png", dpi=300)
    plt.close()
else:
    print("Gas data empty, skipping gas plot.")





total_requests = len(df)


direct_success = len(df[(df['attempt_count'] == 1) & (df['dispatch_status'] == 'success')])


failover_success = len(df[(df['attempt_count'] > 1) & (df['dispatch_status'] == 'success')])


total_failures = len(df[df['dispatch_status'] == 'failed'])

labels = ['Direct Success (1st Attempt)', 'Failover Success (Rescued)', 'Permanent Failure']
sizes = [direct_success, failover_success, total_failures]
colors = ['#2ECC71', '#F1C40F', '#E74C3C']
explode = (0.05, 0.05, 0.1)  

plt.figure(figsize=(8, 8))
plt.pie(sizes, explode=explode, labels=labels, colors=colors, autopct='%1.1f%%',
        shadow=True, startangle=140, textprops={'fontsize': 12, 'fontweight': 'bold'})
plt.title(f'E2E Reliability & Failover Routing (Total Requests: {total_requests})', fontweight='bold', pad=20, fontsize=14)
plt.tight_layout()
plt.savefig(f"{out_dir}/reliability_pie.png", dpi=300)
plt.close()

print(f"✅ Đã tạo thành công 4 biểu đồ báo cáo khoa học tại thư mục {out_dir}/")
