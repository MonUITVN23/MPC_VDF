"""
Scenario 4: Cross-chain Failover — Time-series Line Chart
Shows latency spike at bridge kill point and recovery.
"""
import sys, os
sys.path.insert(0, os.path.dirname(__file__))
from ieee_style import (apply_ieee_style, FIGURE_WIDTH, FIGURE_HEIGHT, 
                        DATA_DIR, plt, savefig, PALETTE_BRIDGES)
import pandas as pd
import numpy as np

apply_ieee_style()

csv_path = os.path.join(DATA_DIR, 'failover_timeseries.csv')
if not os.path.exists(csv_path):
    print(f"ERROR: {csv_path} not found. Run 04_failover_test.sh first.")
    sys.exit(1)

df = pd.read_csv(csv_path)

fig, ax = plt.subplots(figsize=(FIGURE_WIDTH, FIGURE_HEIGHT))

# ── Plot continuous background line and fill ──
ax.plot(df['request_id'], df['latency_ms'], color='#7F8C8D', linewidth=1.0, alpha=0.6, zorder=1)
ax.fill_between(df['request_id'], df['latency_ms'], min(df['latency_ms']) * 0.9, 
                color='#BDC3C7', alpha=0.2, zorder=0)

# ── Plot colored scatter points for each bridge ──
for bridge_name, color in zip(['AXELAR', 'LAYERZERO'], ['#2980B9', '#F39C12']):
    subset = df[df['bridge_used'] == bridge_name]
    if not subset.empty:
        ax.scatter(subset['request_id'], subset['latency_ms'], 
                   color=color, s=25, edgecolor='white', linewidth=0.5, 
                   label=f'Bridge: {bridge_name}', zorder=3)

# ── Failover annotation ──
failover_rows = df[df['failover_occurred'] == True]
if not failover_rows.empty:
    kill_req = failover_rows['request_id'].iloc[0]
    spike_latency = failover_rows['latency_ms'].max()

    # Red vertical line for disaster
    ax.axvline(x=kill_req - 0.5, color='#E74C3C', linestyle='--', linewidth=1.5, alpha=0.8, zorder=2)
    ax.annotate('Axelar Relayer\nKilled',
                xy=(kill_req - 0.5, spike_latency),
                xytext=(kill_req - 15, spike_latency * 1.02),
                fontsize=9, color='#C0392B', fontweight='bold',
                arrowprops=dict(arrowstyle='->', color='#E74C3C', lw=1.5), zorder=4)

    # Shade the transition / recovery zone
    transition_end = df[df['failover_occurred'] == True]['request_id'].max()
    ax.axvspan(kill_req - 0.5, transition_end + 0.5, alpha=0.1, color='#E74C3C',
               label='Failover Recovery Window', zorder=0)

# ── Styling ──
ax.set_xlabel('Protocol Request Number')
ax.set_ylabel('E2E Latency (ms)')
ax.set_title('Cross-chain Resilience: Dynamic Bridge Failover', pad=12, fontweight='bold')
ax.set_ylim(min(df['latency_ms']) * 0.95, max(df['latency_ms']) * 1.08)

# Legend positioning strategy
ax.legend(loc='upper right', framealpha=0.95, edgecolor='#BDC3C7')

# Moving average overlay (Optional, but looks nice)
if len(df) > 5:
    ma = df['latency_ms'].rolling(window=5, center=True).mean()
    ax.plot(df['request_id'], ma, color='#2C3E50', linestyle=':', linewidth=1.5,
            label='5-pt Moving Average', zorder=2)

fig.tight_layout()
savefig(fig, 'fig4_failover_resilience.png')
print("Done: fig4_failover_resilience.png")
