# R-Data Agent — Longevity Gene Expression

A Rust-based TUI tool for analyzing gene expression microarray data, focused on longevity research and aging markers (expression changes from young → old).

## Features

- **Microarray Data Layout**: Automatic detection of Gene ID (column A) × age (columns B+). Supports replicates (same age in multiple columns).
- **Data Loading**: CSV, JSON, and Excel (.xlsx) via Polars.
- **Statistical Analysis**:
  - Summary statistics (mean, std dev, min, max)
  - Correlation matrix
  - Linear regression
  - Histogram, box plot
- **Gene-Expression Visualizations**:
  - **Expression trend**: Line plot of expression vs age for selected gene(s)
  - **Young vs Old scatter**: Mean expression Young vs Old across genes (identifies aging markers)
  - **Age group box plot**: Box plot by age category
- **General Visualizations**: Correlation heatmap, histograms, box plots, linear regression.
- **Visualization Availability**: All analyses listed in UI; disabled with reason when data doesn't fit (e.g. "no numeric columns").

## Data Format

Expected microarray layout:

| Gene ID    | 17   | 18   | 21   | 24   | ... |
|------------|------|------|------|------|-----|
| ENSG0000001| 6.55 | 6.72 | 7.10 | ... |     |
| ENSG000001 | 8.12 | 8.81 | 8.81 | ... |     |

- **Row 1**: `Gene ID` in column A, ages (17, 18, 21, 24, …) as column headers.
- **Column A**: Ensembl gene IDs (e.g. `ENSG0000001`).
- **Columns B+**: Log-normalised expression values (float) per gene at each age.
- **One value per gene**: When multiple probes map to the same gene, use the highest probe value.
- Same age may appear in multiple columns (replicates).

## Installation

```bash
cargo build --release
cargo run
```

## Usage

### Keyboard Controls

**General:**
- `Tab` / `Shift+Tab` — Switch tabs (Data, Analysis, Visualizations)
- `q` — Quit
- `?` — Help
- `C` — Clear analysis results and visualizations

**Data Tab:**
- `L` — Load file (CSV, JSON, Excel .xlsx)
- Enter path, press Enter to load

**Analysis Tab:**
- `s` — Summary statistics
- `c` — Correlation matrix
- `i` — Histogram
- `b` — Box plot
- `r` — Expression vs age (microarray) or linear regression
- `g` — Genes significant with age, p<0.05 (microarray)
- `t` — Expression trend (select genes, ★ to select)
- `e` — Expression vs age regression (select 1-5 genes)
- `1` — Volcano plot
- `2` — Correlation scatter
- `3` — Top genes bar chart
- `v` — Young vs Old scatter (microarray)
- `a` — Age group box plot (microarray)
- `Enter` — Confirm and run selected analysis
- `Esc` — Cancel pending analysis

**Visualizations Tab:**
- `Space` — Toggle display
- `O` — Open chart in browser (full-quality SVG)

## Configuration

Config file: `~/.config/r-data-agent/config.toml`

```toml
viz_width = 800
viz_height = 600
default_bins = 20
```

## Example Workflow

1. Load microarray data: Data tab → `L` → enter path.
2. Check layout: Data tab shows "Genes: N | Age columns: M (range X–Y)" when layout is detected.
3. Run analyses: Analysis tab → press key (`s`, `c`, `t`, `v`, `a`, etc.) → Enter to confirm.
4. View charts: Visualizations tab → `Space` to toggle, `O` to open SVG.

## Project Structure

```
src/
├── data/           # Data loading and analysis
│   ├── ingestion.rs  # CSV/JSON/XLSX, layout detection
│   └── analysis.rs   # Statistics, expression trend, young vs old
├── viz/             # Visualization engine
│   ├── types.rs      # Viz configs
│   ├── engine.rs     # Plotters rendering
│   └── availability.rs # Viz availability logic
├── runner.rs        # Analysis runner (no AI)
├── ui/              # TUI
│   ├── components.rs # Tabs, help
│   └── tui.rs       # Main app
└── config/          # Config management
```

## License

MIT
