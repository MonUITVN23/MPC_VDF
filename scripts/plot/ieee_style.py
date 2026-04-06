"""
IEEE Q1 Publication-Quality Matplotlib Configuration
Shared by all 5 benchmark plot scripts.
"""
import matplotlib
matplotlib.use('Agg')  # Non-interactive backend
import matplotlib.pyplot as plt
import matplotlib.ticker as ticker
import seaborn as sns
import os

# ── IEEE Style Constants ──
FIGURE_WIDTH = 7.16        # IEEE double-column width (inches)
FIGURE_HEIGHT = 4.5
DPI = 300
FONT_FAMILY = 'serif'
FONT_SERIF = ['Times New Roman', 'DejaVu Serif', 'Liberation Serif', 'serif']
FONT_SIZE = 10
LABEL_SIZE = 11
TITLE_SIZE = 12

# ── Color Palettes ──
PALETTE_PHASES = ['#2E86C1', '#E67E22', '#8E44AD', '#27AE60', '#E74C3C']
PALETTE_BARS = ['#2980B9', '#E74C3C', '#F39C12', '#27AE60', '#8E44AD', '#7F8C8D']
PALETTE_BRIDGES = {'AXELAR': '#2E86C1', 'LAYERZERO': '#E67E22', 'WORMHOLE': '#27AE60'}
COLOR_AREA_FILL = '#FFB74D'
COLOR_LINE_ACCENT = '#1565C0'

# ── Output paths ──
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_ROOT = os.path.abspath(os.path.join(SCRIPT_DIR, '..', '..'))
DATA_DIR = os.path.join(PROJECT_ROOT, 'scripts', 'benchmark', 'data')
CHART_DIR = os.path.join(DATA_DIR, 'charts')


def apply_ieee_style():
    """Apply IEEE-standard matplotlib/seaborn configuration."""
    os.makedirs(CHART_DIR, exist_ok=True)

    plt.rcParams.update({
        'font.family': FONT_FAMILY,
        'font.serif': FONT_SERIF,
        'font.size': FONT_SIZE,
        'axes.titlesize': TITLE_SIZE,
        'axes.labelsize': LABEL_SIZE,
        'axes.titleweight': 'bold',
        'axes.labelweight': 'bold',
        'axes.grid': True,
        'grid.alpha': 0.3,
        'grid.linestyle': '--',
        'legend.fontsize': 9,
        'legend.framealpha': 0.8,
        'figure.figsize': (FIGURE_WIDTH, FIGURE_HEIGHT),
        'figure.dpi': DPI,
        'savefig.dpi': DPI,
        'savefig.bbox': 'tight',
        'savefig.pad_inches': 0.05,
        'xtick.direction': 'in',
        'ytick.direction': 'in',
        'xtick.major.size': 4,
        'ytick.major.size': 4,
    })

    sns.set_context("paper", font_scale=1.1)
    try:
        sns.set_theme(style="whitegrid", rc={
            "axes.edgecolor": "0.15",
            "xtick.bottom": True,
            "ytick.left": True,
        })
    except Exception:
        pass


def savefig(fig, filename):
    """Save figure to CHART_DIR with IEEE settings."""
    path = os.path.join(CHART_DIR, filename)
    fig.savefig(path, dpi=DPI, bbox_inches='tight', pad_inches=0.05)
    plt.close(fig)
    print(f"  ✅ Saved: {path}")
    return path
