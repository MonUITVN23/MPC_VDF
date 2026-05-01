import sys, os
sys.path.insert(0, os.path.dirname(__file__))
from ieee_style import (apply_ieee_style, FIGURE_WIDTH, FIGURE_HEIGHT,
                        DATA_DIR, plt, savefig, COLOR_AREA_FILL, COLOR_LINE_ACCENT)
import pandas as pd
import numpy as np

apply_ieee_style()

csv_path = os.path.join(DATA_DIR, 'mev_censorship.csv')
if not os.path.exists(csv_path):
    print(f"ERROR: {csv_path} not found. Run 05_mev_censorship.ts first.")
    sys.exit(1)

df = pd.read_csv(csv_path)
df = df.sort_values('base_fee_gwei')

fig, ax1 = plt.subplots(figsize=(FIGURE_WIDTH, FIGURE_HEIGHT))

ax1.fill_between(df['base_fee_gwei'], df['challenge_window_blocks'],
                 alpha=0.25, color=COLOR_AREA_FILL)
ax1.plot(df['base_fee_gwei'], df['challenge_window_blocks'],
         color=COLOR_LINE_ACCENT, linewidth=2.5, marker='o', markersize=5,
         label='Challenge Window (blocks)')

ax1.set_xlabel('Base Fee (Gwei)')
ax1.set_ylabel('Challenge Window (blocks)', color=COLOR_LINE_ACCENT)
ax1.tick_params(axis='y', labelcolor=COLOR_LINE_ACCENT)

threshold_fee = 100
ax1.axvline(x=threshold_fee, color='#E74C3C', linestyle='--', linewidth=1.5, alpha=0.7)
ax1.annotate('Base Fee Threshold\n(100 Gwei)',
             xy=(threshold_fee, ax1.get_ylim()[1] * 0.5 if ax1.get_ylim()[1] > 0 else 150),
             xytext=(40, 20), textcoords='offset points',
             fontsize=8, color='#E74C3C', fontweight='bold',
             arrowprops=dict(arrowstyle='->', color='#E74C3C', lw=1.0))

cap_blocks = df['challenge_window_blocks'].max()
cap_start = df.loc[df['challenge_window_blocks'] == cap_blocks, 'base_fee_gwei'].iloc[0]
ax1.axhline(y=cap_blocks, color='#27AE60', linestyle=':', linewidth=1.2, alpha=0.6)
ax1.text(df['base_fee_gwei'].iloc[0] + 10, cap_blocks + 8,
         f'Cap: {int(cap_blocks)} blocks', fontsize=8, color='#27AE60', fontstyle='italic')

stable_mask = df['challenge_window_blocks'] == df['challenge_window_blocks'].iloc[0]
stable_end_fee = df.loc[stable_mask, 'base_fee_gwei'].max()
ramp_mask = (df['base_fee_gwei'] > stable_end_fee) & (df['challenge_window_blocks'] < cap_blocks)
cap_mask = df['challenge_window_blocks'] == cap_blocks

if stable_mask.any():
    mid_fee = df.loc[stable_mask, 'base_fee_gwei'].median()
    ax1.text(mid_fee, df['challenge_window_blocks'].iloc[0] - 15,
             'Stable Zone', ha='center', fontsize=8, color='#2980B9',
             fontweight='bold', fontstyle='italic', alpha=0.8)

if ramp_mask.any():
    ramp_fees = df.loc[ramp_mask, 'base_fee_gwei']
    mid_ramp = ramp_fees.median()
    mid_val = df.loc[ramp_mask, 'challenge_window_blocks'].median()
    ax1.text(mid_ramp, mid_val + 15,
             'Linear Ramp', ha='center', fontsize=8, color='#E67E22',
             fontweight='bold', fontstyle='italic', alpha=0.8)

if cap_mask.sum() > 1:
    cap_fees = df.loc[cap_mask, 'base_fee_gwei']
    mid_cap = cap_fees.median()
    ax1.text(mid_cap, cap_blocks - 20,
             'Saturated', ha='center', fontsize=8, color='#27AE60',
             fontweight='bold', fontstyle='italic', alpha=0.8)

ax1.set_title('MEV Censorship Resistance:\nDynamic Challenge Window Response to Base Fee', pad=12)

from matplotlib.lines import Line2D
from matplotlib.patches import Patch
legend_elements = [
    Line2D([0], [0], color=COLOR_LINE_ACCENT, linewidth=2.5, marker='o',
           markersize=5, label='Challenge Window (blocks)'),
    Patch(facecolor=COLOR_AREA_FILL, alpha=0.25, label='Window Area'),
    Line2D([0], [0], color='#E74C3C', linestyle='--', linewidth=1.5,
           label='Fee Threshold (100 Gwei)'),
    Line2D([0], [0], color='#27AE60', linestyle=':', linewidth=1.2,
           label=f'Cap ({int(cap_blocks)} blocks)'),
]
ax1.legend(handles=legend_elements, loc='upper left', fontsize=8, framealpha=0.9)

fig.tight_layout()
savefig(fig, 'fig5_mev_censorship.png')
print("Done: fig5_mev_censorship.png")
