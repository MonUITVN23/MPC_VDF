#!/usr/bin/env python3
from __future__ import annotations

import argparse
from pathlib import Path

import matplotlib.pyplot as plt
import pandas as pd
import seaborn as sns

REQUIRED_COLUMNS = {
    "request_id",
    "bridge_name",
    "bridge_id_hex",
    "selected_bridge",
    "attempt_count",
    "fallback_hops",
    "dispatch_status",
    "t2_mpc_ms",
    "t3_vdf_ms",
    "t4_dispatch_ms",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Plot comparison charts from e2e_metrics_v2.csv"
    )
    parser.add_argument(
        "--input",
        type=Path,
        default=Path("/home/xuananh/mpc-vdf/off-chain/e2e_metrics_v2.csv"),
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

    if "selected_bridge" in df.columns:
        df["bridge"] = df["selected_bridge"].fillna("")
    else:
        df["bridge"] = ""

    if "bridge_name" in df.columns:
        missing = df["bridge"].astype(str).str.strip() == ""
        df.loc[missing, "bridge"] = df.loc[missing, "bridge_name"]

    df["bridge"] = df["bridge"].fillna("UNKNOWN").astype(str).str.strip()
    df.loc[df["bridge"] == "", "bridge"] = "UNKNOWN"

    numeric_cols = [
        "request_id",
        "t2_mpc_ms",
        "t3_vdf_ms",
        "t4_dispatch_ms",
        "attempt_count",
        "fallback_hops",
    ]
    for col in numeric_cols:
        df[col] = pd.to_numeric(df[col], errors="coerce")

    df = df.dropna(subset=numeric_cols).copy()
    df["request_id"] = df["request_id"].astype(int)

    return df


def plot_chart2_latency_breakdown(df: pd.DataFrame, out_dir: Path) -> Path:
    success_df = df[df["dispatch_status"] == "success"]
    grouped = (
        success_df.groupby("bridge", as_index=False)
        .agg(
            t2_mpc_ms=("t2_mpc_ms", "mean"),
            t3_vdf_ms=("t3_vdf_ms", "mean"),
            t4_dispatch_ms=("t4_dispatch_ms", "mean"),
        )
        .sort_values("bridge")
    )

    labels = grouped["bridge"].tolist()
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
    ax.set_title("E2E Latency Breakdown by Bridge (Success Only)")
    ax.legend(loc="upper left")

    fig.tight_layout()
    out_path = out_dir / "new_chart2_e2e_latency_breakdown.png"
    fig.savefig(out_path, dpi=300)
    plt.close(fig)
    return out_path


def plot_chart3_network_timeline(df: pd.DataFrame, out_dir: Path) -> Path:
    plot_df = df[df["dispatch_status"] == "success"].sort_values("request_id").copy()

    fig, ax = plt.subplots(figsize=(11, 6))

    bridges = list(plot_df["bridge"].dropna().unique())
    palette = sns.color_palette("tab10", n_colors=max(len(bridges), 1))

    for idx, bridge in enumerate(bridges):
        subset = plot_df[plot_df["bridge"] == bridge]
        if subset.empty:
            continue
        ax.scatter(
            subset["request_id"],
            subset["t4_dispatch_ms"],
            s=40,
            alpha=0.9,
            color=palette[idx],
            label=bridge,
        )

    ax.set_xlabel("Request Sequence (request_id)")
    ax.set_ylabel("t4_dispatch_ms")
    ax.set_title("Network Latency Timeline (Success Only)")
    ax.legend(loc="best")
    ax.grid(True, alpha=0.25)

    fig.tight_layout()
    out_path = out_dir / "new_chart3_network_latency_timeline.png"
    fig.savefig(out_path, dpi=300)
    plt.close(fig)
    return out_path


def plot_chart4_fallback_ratio(df: pd.DataFrame, out_dir: Path) -> Path:
    grouped = (
        df.groupby("bridge", as_index=False)
        .agg(
            total=("request_id", "count"),
            fallback_count=("fallback_hops", lambda s: int((s > 0).sum())),
        )
        .sort_values("bridge")
    )
    grouped["fallback_ratio_pct"] = grouped["fallback_count"] / grouped["total"] * 100.0

    fig, ax = plt.subplots(figsize=(10, 6))
    bars = ax.bar(grouped["bridge"], grouped["fallback_ratio_pct"], color="#8B5CF6", alpha=0.9)

    for bar, ratio, fallback_count, total in zip(
        bars,
        grouped["fallback_ratio_pct"],
        grouped["fallback_count"],
        grouped["total"],
    ):
        ax.text(
            bar.get_x() + bar.get_width() / 2,
            bar.get_height() + 0.8,
            f"{ratio:.1f}% ({fallback_count}/{total})",
            ha="center",
            va="bottom",
            fontsize=9,
        )

    ax.set_ylim(0, max(100.0, float(grouped["fallback_ratio_pct"].max()) + 8.0))
    ax.set_ylabel("Fallback Ratio (%)")
    ax.set_xlabel("Selected Bridge")
    ax.set_title("Fallback Ratio by Selected Bridge")

    fig.tight_layout()
    out_path = out_dir / "new_chart4_fallback_ratio.png"
    fig.savefig(out_path, dpi=300)
    plt.close(fig)
    return out_path


def print_bridge_stats(df: pd.DataFrame) -> None:
    print("\n=== Bridge-wise t4_dispatch_ms Statistics (Success Only) ===")
    success_df = df[df["dispatch_status"] == "success"]

    for bridge, subset_df in success_df.groupby("bridge"):
        subset = subset_df["t4_dispatch_ms"]
        if subset.empty:
            print(f"{bridge}: N=0 (no data)")
            continue

        n_val = int(subset.shape[0])
        avg_val = float(subset.mean())
        p50_val = float(subset.quantile(0.50))
        p95_val = float(subset.quantile(0.95))

        print(f"{bridge} | N={n_val} | Avg={avg_val:.3f} ms | P50={p50_val:.3f} ms | P95={p95_val:.3f} ms")


def main() -> None:
    args = parse_args()
    sns.set_theme(style="whitegrid", context="paper")

    df = load_and_validate(args.input)
    args.out_dir.mkdir(parents=True, exist_ok=True)

    chart2 = plot_chart2_latency_breakdown(df, args.out_dir)
    chart3 = plot_chart3_network_timeline(df, args.out_dir)
    chart4 = plot_chart4_fallback_ratio(df, args.out_dir)

    print(f"Saved: {chart2}")
    print(f"Saved: {chart3}")
    print(f"Saved: {chart4}")
    print_bridge_stats(df)


if __name__ == "__main__":
    main()
