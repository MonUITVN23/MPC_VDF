"""
Scenario 2: Gas Economics — Grouped Bar Chart
Compares Total Pipeline Gas vs User-Paid Gas across protocols.
Shows CrossRand's advantage: users pay only ~115k gas.
"""
import sys, os
sys.path.insert(0, os.path.dirname(__file__))
from ieee_style import (apply_ieee_style, FIGURE_WIDTH, FIGURE_HEIGHT, 
                        DATA_DIR, plt, savefig, PALETTE_BARS)
import pandas as pd
import numpy as np

apply_ieee_style()

csv_path = os.path.join(DATA_DIR, 'gas_metrics.csv')
if not os.path.exists(csv_path):
    print(f"ERROR: {csv_path} not found. Run 02_gas_metrics.ts first.")
    sys.exit(1)

df = pd.read_csv(csv_path)

# Build a dictionary to lookup gas easily
gas_dict = dict(zip(df['operation'], df['gas_used']))

# ── Define protocols with Total and User-Paid breakdowns ──
protocols = {
    'Chainlink\nVRF v2': {
        'total': gas_dict.get('Chainlink_VRF_Request', 102000) 
               + gas_dict.get('Chainlink_VRF_Fulfill', 203000),
        'user_paid': gas_dict.get('Chainlink_VRF_Request', 102000) 
                   + gas_dict.get('Chainlink_VRF_Fulfill', 203000),
        'note': 'User pays everything',
    },
    'DRAND\n(BLS Verify)': {
        'total': gas_dict.get('Drand_Verify', 182000),
        'user_paid': gas_dict.get('Drand_Verify', 182000),
        'note': 'User pays everything',
    },
    'API3\nQRNG': {
        'total': gas_dict.get('API3_QRNG_Request', 55000) 
               + gas_dict.get('API3_QRNG_Fulfill', 118000),
        'user_paid': gas_dict.get('API3_QRNG_Request', 55000) 
                   + gas_dict.get('API3_QRNG_Fulfill', 118000),
        'note': 'User pays everything',
    },
    'CrossRand\n(Ours)': {
        'total': gas_dict.get('requestRandomness', 28191) 
               + gas_dict.get('submitOptimisticResult_ZK', 756934) 
               + gas_dict.get('finalizeRandomness', 87813),
        'user_paid': gas_dict.get('requestRandomness', 28191) 
                   + gas_dict.get('finalizeRandomness', 87813),
        'note': 'Relayer absorbs ZK verification',
    },
}

labels = list(protocols.keys())
totals = [protocols[k]['total'] for k in labels]
user_paid = [protocols[k]['user_paid'] for k in labels]

x = np.arange(len(labels))
bar_width = 0.35

fig, ax = plt.subplots(figsize=(FIGURE_WIDTH + 1.0, FIGURE_HEIGHT + 0.5))

# ── Draw grouped bars ──
bars_total = ax.bar(x - bar_width/2, totals, bar_width, 
                    color='#34495E', edgecolor='white', linewidth=1.2,
                    label='Total Pipeline Gas', alpha=0.85)
bars_user = ax.bar(x + bar_width/2, user_paid, bar_width, 
                   color='#2ECC71', edgecolor='white', linewidth=1.2,
                   label='User-Paid Gas Only')

# ── Value labels on top of each bar ──
for bar in bars_total:
    height = bar.get_height()
    ax.text(bar.get_x() + bar.get_width()/2, height + max(totals)*0.01,
            f'{int(height):,}', ha='center', va='bottom', 
            fontweight='bold', fontsize=8, color='#34495E')

for bar in bars_user:
    height = bar.get_height()
    ax.text(bar.get_x() + bar.get_width()/2, height + max(totals)*0.01,
            f'{int(height):,}', ha='center', va='bottom', 
            fontweight='bold', fontsize=8.5, color='#1A8F4B')

# ── Highlight CrossRand user-paid advantage ──
# (Removed annotation as requested)

ax.set_ylabel('Gas Consumed')
ax.set_title('On-chain Gas Cost: Total Pipeline vs. User-Paid', pad=15)
ax.set_xticks(x)
ax.set_xticklabels(labels, fontsize=9)
ax.set_ylim(0, max(totals) * 1.25)
ax.grid(axis='y', linestyle='--', alpha=0.4)
ax.legend(loc='upper right', framealpha=0.9, fontsize=9)

# ── Footnote ──
footnote = ("* CrossRand: ZK Verification Gas (~757k) is paid by off-chain Relayers,\n"
            "  amortized across batches. DApp users pay only Request + Finalize.")
fig.text(0.5, -0.04, footnote, ha='center', va='top', fontsize=8.5, 
         style='italic', color='#444444',
         bbox=dict(facecolor='#F4F6F6', alpha=0.8, edgecolor='#BDC3C7', pad=6))

fig.tight_layout()
savefig(fig, 'fig2_gas_economics.png')
print("Done: fig2_gas_economics.png (Grouped)")
