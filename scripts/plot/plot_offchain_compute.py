import sys, os
sys.path.insert(0, os.path.dirname(__file__))
from ieee_style import (apply_ieee_style, FIGURE_WIDTH, FIGURE_HEIGHT,
                        DATA_DIR, plt, savefig)
import pandas as pd
import numpy as np

apply_ieee_style()

vdf_csv = os.path.join(DATA_DIR, 'bench_vdf.csv')
zk_csv = os.path.join(DATA_DIR, 'bench_zk.csv')
if not os.path.exists(vdf_csv) or not os.path.exists(zk_csv):
    print(f"ERROR: {vdf_csv} or {zk_csv} not found. Run 01_offchain_compute.sh first.")
    sys.exit(1)

vdf_df = pd.read_csv(vdf_csv)
zk_df = pd.read_csv(zk_csv)

vdf_df['T_exp'] = np.log2(vdf_df['t']).astype(int)
vdf_df = vdf_df.sort_values('T_exp')

zk_mean_ms = float(zk_df['zk_ms'].mean())
zk_peak_rss_mb = float(zk_df['peak_rss_kb'].mean() / 1024.0)

fig, ax1 = plt.subplots(figsize=(FIGURE_WIDTH, FIGURE_HEIGHT))

ln1 = ax1.plot(vdf_df['T_exp'], vdf_df['vdf_ms'], 'o-', color='#2980B9',
               linewidth=2, markersize=6, label='VDF Evaluation Time')
ln2 = ax1.plot(vdf_df['T_exp'], [zk_mean_ms] * len(vdf_df), 's--', color='#E74C3C',
               linewidth=2, markersize=6, label='ZK Proving Time (avg.)')

ax1.set_xlabel(r'VDF Delay Parameter $T$ (exponent, $T = 2^x$)')
ax1.set_ylabel('Computation Time (ms)')
ax1.set_yscale('log')
ax1.set_xticks(vdf_df['T_exp'])
ax1.set_xticklabels([f'$2^{{{int(x)}}}$' for x in vdf_df['T_exp']])

ax2 = ax1.twinx()
ln3 = ax2.plot(vdf_df['T_exp'], [zk_peak_rss_mb] * len(vdf_df), '^-.', color='#27AE60',
               linewidth=1.5, markersize=6, label='Peak RSS (avg. MB)')
ax2.set_ylabel('Peak Memory (MB)')
ax2.spines['right'].set_color('#27AE60')
ax2.yaxis.label.set_color('#27AE60')
ax2.tick_params(axis='y', colors='#27AE60')

lns = ln1 + ln2 + ln3
labs = [l.get_label() for l in lns]
ax1.legend(lns, labs, loc='upper left', fontsize=8)

ax1.set_title('Asymmetric Off-chain Computation:\nVDF Scaling vs. Constant ZK Proving (Halo2 IPA)', pad=12)

ax1.annotate('VDF: inherently sequential\n(time ∝ T)',
             xy=(vdf_df['T_exp'].iloc[-1], vdf_df['vdf_ms'].iloc[-1]),
             xytext=(-80, -30), textcoords='offset points',
             fontsize=7, fontstyle='italic', color='#2980B9',
             arrowprops=dict(arrowstyle='->', color='#2980B9', lw=0.8))

ax1.annotate('ZK: constant proving\n(Halo2 IPA, no trusted setup)',
             xy=(vdf_df['T_exp'].iloc[0], zk_mean_ms),
             xytext=(40, 30), textcoords='offset points',
             fontsize=7, fontstyle='italic', color='#E74C3C',
             arrowprops=dict(arrowstyle='->', color='#E74C3C', lw=0.8))

fig.tight_layout()
savefig(fig, 'fig1_offchain_compute.png')
print("Done: fig1_offchain_compute.png")
