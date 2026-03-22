#!/usr/bin/env python3
from __future__ import annotations

import argparse
from pathlib import Path

import matplotlib.pyplot as plt
import pandas as pd
import seaborn as sns

REQUIRED_COLUMNS = {
    "request_id",
    "bridge_id",
    "t2_mpc_ms",
    "t3_vdf_ms",
    "t4_dispatch_ms",
}

BRIDGE_LABEL = {
    1: "Axelar (Normal)",
    2: "LayerZero (Fallback)",
}

BRIDGE_COLOR = {
    1: "#1f77b4",
    2: "#d62728",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Plot comparison charts from new 100-row e2e_metrics.csv"
    )
    parser.add_argument(
        "--input",
        type=Path,
        default=Path("/home/xuananh/mpc-vdf/off-chain/e2e_metrics.csv"),
        help="Path to e2e metrics CSV",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=Path("/home/xuananh/mpc-vdf/off-chain/charts"),
        help="Directory for output PNG charts",
    )
    return parser.parse_args()


def load_and_validate(csv_path: Path) -> pd.DataFrame:
    if not csv_path.exists():
        raise FileNotFoundError(f"Input CSV not found: {csv_path}")

    df = pd.read_csv(csv_path)
    if df.empty:
        raise ValueError(f"Input CSV is empty: {csv_path}")

    missing = REQUIRED_COLUMNS - set(df.columns)
    if missing:
        raise ValueError(f"Missing required columns: {', '.join(sorted(missing))}")

    numeric_cols = ["request_id", "bridge_id", "t2_mpc_ms", "t3_vdf_ms", "t4_dispatch_ms"]
    for col in numeric_cols:
        df[col] = pd.to_numeric(df[col], errors="coerce")

    df = df.dropna(subset=numeric_cols).copy()
    df["request_id"] = df["request_id"].astype(int)
    df["bridge_id"] = df["bridge_id"].astype(int)

    return df


def plot_chart2_latency_breakdown(df: pd.DataFrame, out_dir: Path) -> Path:
    grouped = (
        df.groupby("bridge_id", as_index=False)
        .agg(
            t2_mpc_ms=("t2_mpc_ms", "mean"),
            t3_vdf_ms=("t3_vdf_ms", "mean"),
            t4_dispatch_ms=("t4_dispatch_ms", "mean"),
        )
    )

    order = [bridge for bridge in [1, 2] if bridge in set(grouped["bridge_id"]) ]
    grouped = grouped.set_index("bridge_id").reindex(order).reset_index()

    labels = [BRIDGE_LABEL[b] for b in grouped["bridge_id"]]
    x = range(len(grouped))
    t2 = grouped["t2_mpc_ms"].to_numpy()
    t3 = grouped["t3_vdf_ms"].to_numpy()
    t4 = grouped["t4_dispatch_ms"].to_numpy()

    fig, ax = plt.subplots(figsize=(10, 6))
    ax.bar(x, t2, label="T_MPC (t2)", color="#4C78A8")
    ax.bar(x, t3, bottom=t2, label="T_VDF (t3)", color="#F58518")
    ax.bar(x, t4, bottom=t2 + t3, label="T_Network_Dispatch (t4)", color="#54A24B")

    totals = t2 + t3 + t4
    for i, total in enumerate(totals):
        ax.text(i, total + 30, f"Total: {total:.1f} ms", ha="center", va="bottom", fontsize=10)

    ax.set_xticks(list(x))
    ax.set_xticklabels(labels)
    ax.set_ylabel("Latency (ms)")
    ax.set_title("E2E Latency Breakdown by Bridge")
    ax.legend(loc="upper left")

    fig.tight_layout()
    out_path = out_dir / "new_chart2_e2e_latency_breakdown.png"
    fig.savefig(out_path, dpi=300)
    plt.close(fig)
    return out_path


def plot_chart3_network_timeline(df: pd.DataFrame, out_dir: Path) -> Path:
    plot_df = df.sort_values("request_id").copy()

    fig, ax = plt.subplots(figsize=(11, 6))

    for bridge_id in [1, 2]:
        subset = plot_df[plot_df["bridge_id"] == bridge_id]
        if subset.empty:
            continue
        ax.scatter(
            subset["request_id"],
            subset["t4_dispatch_ms"],
            s=40,
            alpha=0.9,
            color=BRIDGE_COLOR[bridge_id],
            label=BRIDGE_LABEL[bridge_id],
        )

    ax.set_xlabel("Request Sequence (request_id)")
    ax.set_ylabel("t4_dispatch_ms")
    ax.set_title("Network Latency Timeline (100 Transactions)")
    ax.legend(loc="best")
    ax.grid(True, alpha=0.25)

    fig.tight_layout()
    out_path = out_dir / "new_chart3_network_latency_timeline.png"
    fig.savefig(out_path, dpi=300)
    plt.close(fig)
    return out_path


def print_bridge_stats(df: pd.DataFrame) -> None:
    print("\n=== Bridge-wise t4_dispatch_ms Statistics ===")
    for bridge_id in [1, 2]:
        subset = df[df["bridge_id"] == bridge_id]["t4_dispatch_ms"]
        if subset.empty:
            print(f"{BRIDGE_LABEL[bridge_id]}: N=0 (no data)")
            continue

        n_val = int(subset.shape[0])
        avg_val = float(subset.mean())
        p50_val = float(subset.quantile(0.50))
        p95_val = float(subset.quantile(0.95))

        print(
            f"{BRIDGE_LABEL[bridge_id]} | N={n_val} | "
            f"Avg={avg_val:.3f} ms | P50={p50_val:.3f} ms | P95={p95_val:.3f} ms"
        )


def main() -> None:
    args = parse_args()
    sns.set_theme(style="whitegrid", context="paper")

    df = load_and_validate(args.input)
    args.out_dir.mkdir(parents=True, exist_ok=True)

    chart2 = plot_chart2_latency_breakdown(df, args.out_dir)
    chart3 = plot_chart3_network_timeline(df, args.out_dir)

    print(f"Saved: {chart2}")
    print(f"Saved: {chart3}")
    print_bridge_stats(df)


if __name__ == "__main__":
    main()
