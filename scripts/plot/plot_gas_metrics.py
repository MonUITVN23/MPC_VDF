"""
Scenario 2: Gas Economics — Stacked Bar Chart (Linear-scale Y)
Compares End-to-End protocol pipelines for CrossRand vs baselines.
"""
import sys, os
sys.path.insert(0, os.path.dirname(__file__))
from ieee_style import (apply_ieee_style, FIGURE_WIDTH, FIGURE_HEIGHT, 
                        DATA_DIR, plt, savefig, PALETTE_PHASES)
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

# Define Pipelines (Bottom to Top)
pipelines = {
    'Chainlink VRF': {
        'Request': gas_dict.get('Chainlink_VRF_Request', 100000),
        'Delivery/Fulfill': gas_dict.get('Chainlink_VRF_Fulfill', 200000),
        'Finalize/Verify': 0
    },
    'Drand': {
        'Request': 0,
        'Delivery/Fulfill': 0,
        'Finalize/Verify': gas_dict.get('Drand_Verify', 150000)
    },
    'CrossRand': {
        'Request': gas_dict.get('requestRandomness', 28000),
        'Delivery/Fulfill': gas_dict.get('submitOptimisticResult_ZK', 756000),
        'Finalize/Verify': gas_dict.get('finalizeRandomness', 87000)
    }
}

labels = list(pipelines.keys())
components = ['Request', 'Delivery/Fulfill', 'Finalize/Verify']
display_labels = ['Request\n(DApp User)', 'Delivery/Verify\n(Relayer)', 'Finalize\n(DApp User)']
colors = ['#34495E', '#3498DB', '#2ECC71']  # Dark slate, Blue, Green

fig, ax = plt.subplots(figsize=(FIGURE_WIDTH + 1, FIGURE_HEIGHT + 0.5))

bottoms = np.zeros(len(labels))

# Draw Stacked Bars
for i, comp in enumerate(components):
    values = [pipelines[label][comp] for label in labels]
    # Filter 0s for text rendering
    bars = ax.bar(labels, values, bottom=bottoms, color=colors[i], 
                  edgecolor='white', linewidth=1.2, label=display_labels[i], width=0.6)
    
    # Add text inside bars if value > 0
    for j, (bar, val) in enumerate(zip(bars, values)):
        if val > 0:
            y_pos = bottoms[j] + val/2
            ax.text(bar.get_x() + bar.get_width()/2, y_pos, 
                    f'{int(val):,}', ha='center', va='center', 
                    color='white', fontweight='bold', fontsize=8)
    
    # Increment bottom
    bottoms += values

# Total label at top of bars
for j, v_total in enumerate(bottoms):
    ax.text(j, v_total + max(bottoms)*0.02, f"Total: {int(v_total):,}", 
            ha='center', va='bottom', fontweight='bold', color='#2C3E50', fontsize=9)

ax.set_ylabel('Total Gas Consumed (Linear)')
ax.set_title('Lifecycle On-chain Verification Gas Comparison', pad=15)
ax.set_ylim(0, max(bottoms) * 1.15)
ax.grid(axis='y', linestyle='--', alpha=0.4)

ax.legend(title="Lifecycle Stage", loc='upper left', framealpha=0.9)

footnote = ("* Note: ZK Verification Gas (~756k) is paid by off-chain Relayers and amortized across "
            "batches.\nDApp users only pay Request (~28k) + Finalize (~87k) = ~115k Gas per randomness delivery.")
fig.text(0.5, -0.05, footnote, ha='center', va='top', fontsize=8.5, 
         style='italic', color='#444444', bbox=dict(facecolor='#F4F6F6', alpha=0.8, edgecolor='#BDC3C7', pad=6))

fig.tight_layout()
savefig(fig, 'fig2_gas_economics.png')
print("Done: fig2_gas_economics.png (Stacked)")
