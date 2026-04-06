"""
Scenario 3: E2E Latency Breakdown — Stacked Horizontal Bar Chart
Each bar = 1 run, colors = 5 pipeline phases.
"""
import sys, os
sys.path.insert(0, os.path.dirname(__file__))
from ieee_style import (apply_ieee_style, FIGURE_WIDTH, FIGURE_HEIGHT, 
                        DATA_DIR, plt, savefig, PALETTE_PHASES)
import pandas as pd
import numpy as np

apply_ieee_style()

csv_path = os.path.join(DATA_DIR, 'latency_breakdown.csv')
if not os.path.exists(csv_path):
    print(f"ERROR: {csv_path} not found. Run 03_latency_breakdown.sh first.")
    sys.exit(1)

df = pd.read_csv(csv_path)

phase_cols = ['t1_mpc_ms', 't2_vdf_ms', 't3_zk_ms', 't4_bridge_ms', 't5_challenge_window_ms']
phase_labels = [
    '① MPC Generation',
    '② VDF Delay',
    '③ ZK Proving',
    '④ Bridge Routing',
    '⑤ Challenge Window',
]

fig, ax = plt.subplots(figsize=(FIGURE_WIDTH, max(FIGURE_HEIGHT, len(df) * 0.5 + 1.5)))

left = np.zeros(len(df))
for i, (col, label) in enumerate(zip(phase_cols, phase_labels)):
    ax.barh(df['run_id'], df[col], left=left, label=label,
            color=PALETTE_PHASES[i], edgecolor='white', linewidth=0.3, height=0.6)

    # Add time labels inside bars (only if wide enough)
    for j, val in enumerate(df[col]):
        if val > df['total_ms'].max() * 0.05:  # Only label if > 5% of total
            ax.text(left[j] + val / 2, df['run_id'].iloc[j],
                    f'{int(val)}ms', ha='center', va='center',
                    fontsize=6, color='white', fontweight='bold')
    left += df[col].values

ax.set_xlabel('Latency (ms)')
ax.set_ylabel('Run #')
ax.set_yticks(df['run_id'])
ax.set_yticklabels([f'Run {r}' for r in df['run_id']])
ax.invert_yaxis()
ax.set_title('End-to-End Latency Breakdown per Pipeline Phase', pad=12)
ax.legend(loc='lower right', fontsize=7, ncol=2)

# Add total latency annotation
for idx, row in df.iterrows():
    ax.text(row['total_ms'] + df['total_ms'].max() * 0.01, row['run_id'],
            f'{int(row["total_ms"])}ms', va='center', fontsize=7, color='#333')

fig.tight_layout()
savefig(fig, 'fig3_latency_breakdown.png')
print("Done: fig3_latency_breakdown.png")
