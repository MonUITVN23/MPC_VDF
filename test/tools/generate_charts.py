#!/usr/bin/env python3
from __future__ import annotations

import argparse
import math
from pathlib import Path

import pandas as pd
import matplotlib.pyplot as plt
import matplotlib.ticker as mticker
import seaborn as sns


REQUIRED_CRYPTO_COLUMNS = {"T_value", "prover_time_ms", "verify_gas_used"}
REQUIRED_E2E_COLUMNS = {
    "request_id",
    "bridge_id",
    "t2_mpc_ms",
    "t3_vdf_ms",
    "t4_dispatch_ms",
}


def load_csv(path: Path) -> pd.DataFrame:
    if not path.exists():
        raise FileNotFoundError(f"CSV not found: {path}")
    df = pd.read_csv(path)
    if df.empty:
        raise ValueError(f"CSV is empty: {path}")
    return df


def validate_columns(df: pd.DataFrame, required: set[str], name: str) -> None:
    missing = required - set(df.columns)
    if missing:
        missing_txt = ", ".join(sorted(missing))
        raise ValueError(f"Missing required columns in {name}: {missing_txt}")


def prepare_numeric(df: pd.DataFrame, columns: list[str]) -> pd.DataFrame:
    out = df.copy()
    for col in columns:
        out[col] = pd.to_numeric(out[col], errors="coerce")
    out = out.dropna(subset=columns)
    return out


def plot_vdf_computation_cost(crypto_df: pd.DataFrame, out_dir: Path) -> Path:
    grouped = (
        crypto_df.groupby("T_value", as_index=False)
        .agg(
            prover_time_ms=("prover_time_ms", "mean"),
            verify_gas_used=("verify_gas_used", "mean"),
        )
        .sort_values("T_value")
    )

    fig, ax1 = plt.subplots(figsize=(10, 6))
    ax2 = ax1.twinx()

    line1 = ax1.plot(
        grouped["T_value"],
        grouped["prover_time_ms"],
        marker="o",
        linewidth=2,
        color="#1f77b4",
        label="Prover Time (ms)",
    )

    line2 = ax2.plot(
        grouped["T_value"],
        grouped["verify_gas_used"],
        marker="s",
        linewidth=2,
        linestyle="--",
        color="#d62728",
        label="Verify Gas Used",
    )

    ax1.set_xscale("log", base=2)
    ax1.set_xlabel("VDF Delay Parameter T (log2 scale)")
    ax1.set_ylabel("Prover Time (ms)", color="#1f77b4")
    ax2.set_ylabel("Verify Gas Used", color="#d62728")
    gas_min = float(crypto_df["verify_gas_used"].min())
    gas_max = float(crypto_df["verify_gas_used"].max())
    y2_min = int(math.floor((gas_min - 50.0) / 50.0) * 50)
    y2_max = int(math.ceil((gas_max + 50.0) / 50.0) * 50)
    if y2_max - y2_min < 300:
        center = (gas_min + gas_max) / 2.0
        y2_min = int(math.floor((center - 150.0) / 50.0) * 50)
        y2_max = int(math.ceil((center + 150.0) / 50.0) * 50)
    ax2.set_ylim(y2_min, y2_max)
    y2_formatter = mticker.ScalarFormatter(useOffset=False)
    y2_formatter.set_scientific(False)
    ax2.yaxis.set_major_formatter(y2_formatter)
    ax1.set_title("VDF Computation Cost")

    lines = line1 + line2
    labels = [line.get_label() for line in lines]
    ax1.legend(lines, labels, loc="upper left")

    fig.tight_layout()
    out_path = out_dir / "chart1_vdf_computation_cost.png"
    fig.savefig(out_path, dpi=300)
    plt.close(fig)
    return out_path


def plot_e2e_stacked_breakdown(e2e_df: pd.DataFrame, out_dir: Path) -> Path:
    grouped = (
        e2e_df.groupby("bridge_id", as_index=False)
        .agg(
            t2_mpc_ms=("t2_mpc_ms", "mean"),
            t3_vdf_ms=("t3_vdf_ms", "mean"),
            t4_dispatch_ms=("t4_dispatch_ms", "mean"),
        )
        .sort_values("bridge_id")
    )

    if grouped.empty:
        raise ValueError("No valid E2E rows available for chart 2")

    bridge_label = {
        1: "Normal (Bridge 1 - Axelar)",
        2: "Failover (Bridge 2 - LayerZero)",
    }
    grouped["label"] = grouped["bridge_id"].map(bridge_label).fillna(grouped["bridge_id"].astype(str))

    fig, ax = plt.subplots(figsize=(10, 6))

    x = range(len(grouped))
    t2 = grouped["t2_mpc_ms"].values
    t3 = grouped["t3_vdf_ms"].values
    t4 = grouped["t4_dispatch_ms"].values

    ax.bar(x, t2, label="T_MPC (t2)", color="#4C78A8")
    ax.bar(x, t3, bottom=t2, label="T_VDF (t3)", color="#F58518")
    ax.bar(x, t4, bottom=t2 + t3, label="T_Network_Dispatch (t4)", color="#54A24B")

    totals = t2 + t3 + t4
    for idx, total in enumerate(totals):
        ax.text(idx, total + 40, f"{total:.1f} ms", ha="center", va="bottom", fontsize=10)

    ax.set_xticks(list(x))
    ax.set_xticklabels(grouped["label"], rotation=10)
    ax.set_ylabel("Average Latency (ms)")
    ax.set_title("End-to-End Latency Decomposition (Average)")
    ax.legend(loc="upper right")

    fig.tight_layout()
    out_path = out_dir / "chart2_e2e_latency_breakdown.png"
    fig.savefig(out_path, dpi=300)
    plt.close(fig)
    return out_path


def plot_failover_timeline(e2e_df: pd.DataFrame, out_dir: Path, failover_marker: int) -> Path:
    fig, ax = plt.subplots(figsize=(11, 6))

    normal = e2e_df[e2e_df["bridge_id"] == 1]
    failover = e2e_df[e2e_df["bridge_id"] == 2]

    ax.scatter(
        normal["request_id"],
        normal["t4_dispatch_ms"],
        color="#1f77b4",
        label="Bridge 1 (Axelar)",
        alpha=0.85,
        s=36,
    )
    ax.scatter(
        failover["request_id"],
        failover["t4_dispatch_ms"],
        color="#d62728",
        label="Bridge 2 (LayerZero Failover)",
        alpha=0.9,
        s=36,
    )

    ax.axvline(
        failover_marker,
        linestyle="--",
        color="black",
        linewidth=1.5,
        label=f"Failover Trigger ~ Request {failover_marker}",
    )

    ax.set_xlabel("Request ID")
    ax.set_ylabel("T_Network_Dispatch (t4) [ms]")
    ax.set_title("Failover Timeline: Dispatch Latency by Request")
    ax.legend(loc="best")

    fig.tight_layout()
    out_path = out_dir / "chart3_failover_timeline.png"
    fig.savefig(out_path, dpi=300)
    plt.close(fig)
    return out_path


def print_bridge1_t4_stats(e2e_df: pd.DataFrame) -> None:
    bridge1 = e2e_df[e2e_df["bridge_id"] == 1]["t4_dispatch_ms"].dropna()
    if bridge1.empty:
        print("[Stats] Bridge 1 has no t4_dispatch_ms data")
        return

    avg_v = bridge1.mean()
    p50_v = bridge1.quantile(0.50)
    p95_v = bridge1.quantile(0.95)

    print("\n=== Bridge 1 (Axelar) T4 Statistics ===")
    print(f"Avg t4_dispatch_ms: {avg_v:.3f}")
    print(f"p50 t4_dispatch_ms: {p50_v:.3f}")
    print(f"p95 t4_dispatch_ms: {p95_v:.3f}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate academic charts from MPC-VDF benchmark CSV files")
    parser.add_argument(
        "--crypto-csv",
        type=Path,
        default=Path("/home/xuananh/mpc-vdf/off-chain/crypto_benchmarks.csv"),
        help="Path to crypto benchmark CSV",
    )
    parser.add_argument(
        "--e2e-csv",
        type=Path,
        default=Path("/home/xuananh/mpc-vdf/off-chain/e2e_metrics.csv"),
        help="Path to E2E metrics CSV",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=Path("/home/xuananh/mpc-vdf/off-chain/charts"),
        help="Directory to write output PNG files",
    )
    parser.add_argument(
        "--failover-marker",
        type=int,
        default=20,
        help="Request ID position to draw failover trigger vertical line",
    )
    args = parser.parse_args()

    sns.set_theme(style="whitegrid", context="paper")

    crypto_df = load_csv(args.crypto_csv)
    e2e_df = load_csv(args.e2e_csv)

    validate_columns(crypto_df, REQUIRED_CRYPTO_COLUMNS, "crypto_benchmarks.csv")
    validate_columns(e2e_df, REQUIRED_E2E_COLUMNS, "e2e_metrics.csv")

    crypto_df = prepare_numeric(crypto_df, ["T_value", "prover_time_ms", "verify_gas_used"])
    e2e_df = prepare_numeric(
        e2e_df,
        ["request_id", "bridge_id", "t2_mpc_ms", "t3_vdf_ms", "t4_dispatch_ms"],
    )

    args.out_dir.mkdir(parents=True, exist_ok=True)

    chart1 = plot_vdf_computation_cost(crypto_df, args.out_dir)
    chart2 = plot_e2e_stacked_breakdown(e2e_df, args.out_dir)
    chart3 = plot_failover_timeline(e2e_df, args.out_dir, args.failover_marker)

    print(f"Saved: {chart1}")
    print(f"Saved: {chart2}")
    print(f"Saved: {chart3}")

    print_bridge1_t4_stats(e2e_df)


if __name__ == "__main__":
    main()
