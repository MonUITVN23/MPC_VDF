"""
Scenario 1: Off-chain Compute — Dual-axis Line Chart
X-axis: VDF delay parameter T (log2 scale)
Left Y: Computation time (ms) for VDF and ZK Proving
Right Y: Peak RSS memory (MB)
"""
import sys, os
sys.path.insert(0, os.path.dirname(__file__))
from ieee_style import (apply_ieee_style, FIGURE_WIDTH, FIGURE_HEIGHT, 
                        DATA_DIR, plt, savefig, PALETTE_BARS)
import pandas as pd
import numpy as np

apply_ieee_style()

csv_path = os.path.join(DATA_DIR, 'offchain_compute.csv')
if not os.path.exists(csv_path):
    print(f"ERROR: {csv_path} not found. Run 01_offchain_compute.sh first.")
    sys.exit(1)

df = pd.read_csv(csv_path)
df['peak_rss_mb'] = df['peak_rss_kb'] / 1024.0

fig, ax1 = plt.subplots(figsize=(FIGURE_WIDTH, FIGURE_HEIGHT))

# ── Left axis: Time (ms) ──
ln1 = ax1.plot(df['T_exp'], df['vdf_ms'], 'o-', color='#2980B9',
               linewidth=2, markersize=6, label='VDF Evaluation Time')
ln2 = ax1.plot(df['T_exp'], df['zk_prove_ms'], 's--', color='#E74C3C',
               linewidth=2, markersize=6, label='ZK Proving Time (const.)')

ax1.set_xlabel(r'VDF Delay Parameter $T$ (exponent, $T = 2^x$)')
ax1.set_ylabel('Computation Time (ms)')
ax1.set_yscale('log')
ax1.set_xticks(df['T_exp'])
ax1.set_xticklabels([f'$2^{{{int(x)}}}$' for x in df['T_exp']])

# ── Right axis: Memory (MB) ──
ax2 = ax1.twinx()
ln3 = ax2.plot(df['T_exp'], df['peak_rss_mb'], '^-.', color='#27AE60',
               linewidth=1.5, markersize=6, label='Peak RSS (MB)')
ax2.set_ylabel('Peak Memory (MB)')
ax2.spines['right'].set_color('#27AE60')
ax2.yaxis.label.set_color('#27AE60')
ax2.tick_params(axis='y', colors='#27AE60')

# ── Combined legend ──
lns = ln1 + ln2 + ln3
labs = [l.get_label() for l in lns]
ax1.legend(lns, labs, loc='upper left', fontsize=8)

ax1.set_title('Asymmetric Off-chain Computation:\nVDF Scaling vs. Constant ZK Proving', pad=12)

# Annotations
ax1.annotate('VDF: inherently sequential\n(time ∝ T)',
             xy=(df['T_exp'].iloc[-1], df['vdf_ms'].iloc[-1]),
             xytext=(-80, -30), textcoords='offset points',
             fontsize=7, fontstyle='italic', color='#2980B9',
             arrowprops=dict(arrowstyle='->', color='#2980B9', lw=0.8))

ax1.annotate('ZK: constant ~3s\n(independent of T)',
             xy=(df['T_exp'].iloc[2], df['zk_prove_ms'].iloc[2]),
             xytext=(40, 30), textcoords='offset points',
             fontsize=7, fontstyle='italic', color='#E74C3C',
             arrowprops=dict(arrowstyle='->', color='#E74C3C', lw=0.8))

fig.tight_layout()
savefig(fig, 'fig1_offchain_compute.png')
print("Done: fig1_offchain_compute.png")
