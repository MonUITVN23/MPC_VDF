"""
Scenario 5: MEV Censorship — Dual-axis Area + Line Chart
Area fill: BaseFee (Gwei), Line: Challenge Window Duration (seconds)
"""
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

fig, ax1 = plt.subplots(figsize=(FIGURE_WIDTH, FIGURE_HEIGHT))

# ── Left axis: BaseFee (Area fill) ──
ax1.fill_between(df['block_number'], df['base_fee_gwei'],
                 alpha=0.35, color=COLOR_AREA_FILL, label='Base Fee (Gwei)')
ax1.plot(df['block_number'], df['base_fee_gwei'],
         color='#E65100', linewidth=1, alpha=0.7)
ax1.set_xlabel('Block Number')
ax1.set_ylabel('Base Fee (Gwei)', color='#E65100')
ax1.tick_params(axis='y', labelcolor='#E65100')

# ── Spam zone shading ──
spam_mask = df['spam_active'] == True
if spam_mask.any():
    spam_start = df.loc[spam_mask, 'block_number'].iloc[0]
    spam_end = df.loc[spam_mask, 'block_number'].iloc[-1]
    ax1.axvspan(spam_start, spam_end, alpha=0.06, color='#E74C3C')
    ax1.axvline(x=spam_start, color='#E74C3C', linestyle=':', linewidth=1, alpha=0.5)
    ax1.axvline(x=spam_end, color='#E74C3C', linestyle=':', linewidth=1, alpha=0.5)

    mid_block = (spam_start + spam_end) / 2
    ax1.text(mid_block, ax1.get_ylim()[1] * 0.92, 'MEV Spam Active',
             ha='center', fontsize=8, color='#E74C3C', fontweight='bold',
             fontstyle='italic', alpha=0.7)

# ── Right axis: Challenge Window (Line) ──
ax2 = ax1.twinx()
ax2.plot(df['block_number'], df['challenge_window_sec'],
         color=COLOR_LINE_ACCENT, linewidth=2.5, label='Challenge Window Δt (sec)')
ax2.set_ylabel('Challenge Window $\\Delta_t$ (seconds)', color=COLOR_LINE_ACCENT)
ax2.tick_params(axis='y', labelcolor=COLOR_LINE_ACCENT)
ax2.spines['right'].set_color(COLOR_LINE_ACCENT)

# ── Threshold line ──
ax1.axhline(y=100, color='#999', linestyle='--', linewidth=0.8, alpha=0.5)
ax1.text(df['block_number'].iloc[0], 110, 'baseFee threshold (100 Gwei)',
         fontsize=7, color='#999', fontstyle='italic')

ax1.set_title('MEV Censorship Resistance:\nDynamic Challenge Window Response to Base Fee Spikes', pad=12)

# ── Combined legend ──
from matplotlib.lines import Line2D
from matplotlib.patches import Patch
legend_elements = [
    Patch(facecolor=COLOR_AREA_FILL, alpha=0.35, label='Base Fee (Gwei)'),
    Line2D([0], [0], color=COLOR_LINE_ACCENT, linewidth=2.5, label='Challenge Window $\\Delta_t$'),
    Patch(facecolor='#E74C3C', alpha=0.1, label='MEV Spam Period'),
]
ax1.legend(handles=legend_elements, loc='upper left', fontsize=8)

fig.tight_layout()
savefig(fig, 'fig5_mev_censorship.png')
print("Done: fig5_mev_censorship.png")
