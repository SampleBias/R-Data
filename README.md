# R-Data Agent

A powerful Rust-based data science agent with a Terminal User Interface (TUI) for statistical analysis and visualization.

## Features

- **Data Loading**: Support for CSV and JSON file formats using Polars
- **Statistical Analysis**:
  - Summary statistics (mean, std dev, min, max, count)
  - Correlation matrices
  - Linear regression
  - Box plots
  - Histograms
- **Visualizations**: Generate SVG-based charts using Plotters
  - Linear regression plots
  - Box plots
  - Histograms
  - Correlation heatmaps
- **AI-Powered Insights**: Integration with Z.ai GLM 4.7 for intelligent data analysis
- **TUI Interface**: Modern terminal interface built with Ratatui

## Installation

```bash
# Build from source
cargo build --release

# Run
cargo run
```

## Usage

### Keyboard Controls

**General:**
- `Tab` - Switch between tabs (Data, Analysis, Visualizations, AI)
- `q` - Quit application
- `h` - Toggle help screen

**Data Tab:**
- `l` - Load CSV or JSON file

**Analysis Tab:**
- `s` - Compute summary statistics
- `c` - Generate correlation matrix
- `r` - Perform linear regression
- `b` - Create box plot
- `i` - Generate histogram

**Visualizations Tab:**
- `Space` - Toggle visualization display

**AI Tab:**
- Type message and press `Enter` to send
- `Esc` - Exit input mode

## Configuration

The application stores configuration in `~/.config/r-data-agent/config.toml`.

**⚠️ Security Note:** The config file is added to `.gitignore` to prevent API keys from being committed to version control.

### Setting API Key

**Method 1: Use the secure setup script (recommended)**

```bash
./setup_api_key.sh "YOUR_API_KEY_HERE"
```

This script:
- Creates the config directory
- Writes the API key with secure permissions
- Sets file permissions to `600` (owner read/write only)

**Method 2: Manually create config file**

```bash
mkdir -p ~/.config/r-data-agent
cat > ~/.config/r-data-agent/config.toml << EOF
api_key = "YOUR_API_KEY"
viz_width = 800
viz_height = 600
default_bins = 20
EOF
chmod 600 ~/.config/r-data-agent/config.toml
```

**Method 3: Environment variable**

```bash
export R_DATA_AGENT_API_KEY="YOUR_API_KEY"
```

### Verifying Installation

```bash
# Check if API key is configured
cat ~/.config/r-data-agent/config.toml | grep api_key

# Run a quick test
cargo run
```

## Project Structure

```
src/
├── data/           # Data loading and statistical analysis
│   ├── ingestion.rs  # CSV/JSON file loading
│   └── analysis.rs   # Statistical computations
├── viz/            # Visualization engine
│   ├── types.rs      # Visualization types
│   └── engine.rs     # Plotters-based rendering
├── ai/             # AI integration
│   ├── client.rs     # API client for Z.ai
│   └── agent.rs      # Analysis orchestration
├── ui/             # TUI components
│   ├── components.rs # Tab components
│   └── tui.rs       # Main application UI
└── config/          # Configuration management
    └── settings.rs   # Config file handling
```

## Example Workflow

1. Load a dataset:
   ```
   Data tab → Press 'l' → Enter file path
   ```

2. Analyze the data:
   ```
   Analysis tab → Press 's' for statistics
   ```

3. View visualizations:
   ```
   Visualizations tab → Press 'Space' to view
   ```

4. Get AI insights:
   ```
   AI tab → Type question → Press Enter
   ```

## Dependencies

- `ratatui` - Terminal UI framework
- `polars` - High-performance data manipulation
- `plotters` - Visualization library
- `reqwest` - HTTP client for AI API
- `tokio` - Async runtime
- `anyhow` - Error handling

## License

MIT
