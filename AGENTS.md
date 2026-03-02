# R-Data Agent - AI-Powered Longevity Gene Expression Analyzer

This guide helps AI agents work effectively in this Rust-based gene expression analysis codebase.

## Project Overview

**R-Data Agent** is a terminal-based (TUI) data science application for analyzing gene expression microarray data, focused on longevity research and identifying aging markers. It features:

- Natural language control via GLM 4.7 AI model
- Microarray data analysis (genes × age layout)
- Statistical analyses and visualizations
- Optional web search via SerpAPI
- Tool-calling architecture for AI agent control

## Essential Commands

### Building and Running

```bash
# Build the project (release mode)
cargo build --release

# Run the application
cargo run

# Run with test data workflow
cargo run --release
# Then in app: 'L' to load, enter 'sample_data.csv', Tab to Analysis tab, 's' for summary

# Test Z.AI API key
cargo run -- --test-api
# Or use: ./test_api.sh
```

### Development

```bash
# Check if code compiles (fast check without building)
cargo check

# Run tests
cargo test

# Format code
cargo fmt

# Lint code
cargo clippy
```

**Note**: The project has permission issues with `Cargo.lock` - may need to run `sudo chmod 644 Cargo.lock` or delete it and let cargo regenerate.

## Project Structure

```
src/
├── main.rs              # Entry point, terminal setup, API test
├── client/              # GLM 4.7 API client
│   └── glm.rs          # Chat completions, streaming, tool calls
├── conversation.rs      # Chat history management
├── data/               # Data loading and statistical analysis
│   ├── ingestion.rs    # CSV/JSON/XLSX loading, layout detection
│   └── analysis.rs     # Statistics, correlation, regression
├── runner.rs           # Analysis execution (no AI)
├── tools/              # AI tool definitions and search
│   ├── definitions.rs  # Tool schemas for GLM
│   ├── mod.rs
│   └── search.rs       # SerpAPI web search
├── ui/                 # Terminal UI (ratatui)
│   ├── components.rs   # Tabs, agent components
│   ├── loading.rs      # Loading animation widget
│   ├── mod.rs
│   └── tui.rs          # Main app state, event handling (~2300 lines)
├── viz/                # Visualization engine (plotters)
│   ├── availability.rs # Viz availability logic
│   ├── engine.rs       # SVG rendering
│   ├── mod.rs
│   └── types.rs        # Viz config types
└── config/             # **MISSING** - Referenced but not implemented
```

**Important**: The `config` module is referenced in `main.rs` (`mod config;`) and imported in several files, but the implementation is missing. This is a work-in-progress project.

## Configuration

### Environment Variables (.env)

Create `.env` file (copy from `.env.example`):

```
ZAI_API_KEY=your_zai_api_key_here         # Required for AI
SERPAPI_KEY=your_serpapi_key_here         # Optional, for web search
```

Get API key from [Z.AI](https://z.ai/model-api). Uses the **Coding Plan** endpoint: `https://api.z.ai/api/coding/paas/v4`.

### Config File (not yet implemented)

Intended config location: `~/.config/r-data-agent/config.toml`

Intended fields (based on usage in code):
```toml
api_base_url = "https://api.z.ai/api/coding/paas/v4"
model = "glm-4.7-flash"  # or glm-4.7, glm-5, glm-4.5-air
viz_width = 800
viz_height = 600
default_bins = 20
```

**Current behavior**: Code tries to load config via `ConfigManager::load_config()`, but this fails at compile time due to missing implementation.

## Data Format

### Microarray Layout (Primary Format)

Expected structure:

| Gene ID      | 17   | 18   | 21   | 24   | ... |
|--------------|------|------|------|------|-----|
| ENSG0000001  | 6.55 | 6.72 | 7.10 | ...  |     |
| ENSG000001   | 8.12 | 8.81 | 8.81 | ...  |     |

- **Row 1**: `Gene ID` in column A, ages (17, 18, 21, 24, …) as column headers
- **Column A**: Ensembl gene IDs (e.g. `ENSG0000001`)
- **Columns B+**: Log-normalized expression values (float) per gene at each age
- Ages may repeat (replicates)
- When multiple probes map to same gene, use highest probe value

### Supported File Formats

- **CSV**: Standard CSV with headers (polars `CsvReadOptions`)
- **JSON**: JSON arrays or records
- **Excel (.xlsx)**: Uses `calamine` crate

### Layout Detection

Implemented in `src/data/ingestion.rs` - automatically detects microarray layout:
- First column header = "Gene ID" (or similar)
- Columns 2+ have numeric headers (ages)
- Validates expression columns are numeric

## Architecture Patterns

### Tool-Calling AI Architecture

The app follows a pattern similar to [Vybrid](https://github.com/SampleBias/vybrid):

1. **Tool Definitions** (`src/tools/definitions.rs`): JSON schemas for available operations
2. **Tool Executor** (`src/ui/tui.rs::execute_tool()`): Dispatches to app operations
3. **Streaming Response** (`src/ui/tui.rs::process_ai_response()`): SSE streaming from GLM API
4. **Conversation History** (`src/conversation.rs`): Maintains full context

### Analysis Request Pattern

`src/runner.rs` defines `AnalysisRequest` enum for all analyses:

```rust
pub enum AnalysisRequest {
    SummaryStats { gene_age_summary, gene_filter },
    Correlation,
    Histogram { column, bins },
    BoxPlot { column },
    LinearRegression { x_column, y_column },
    ExpressionTrend { gene_ids, gene_column, age_columns },
    YoungVsOld { gene_column, age_columns, young_cols, old_cols },
    // ... more variants
}
```

`AnalysisRunner::run(&df, request)` executes synchronously and returns `AnalysisResult`.

### Visualization Engine

`src/viz/engine.rs` uses `plotters` and `plotters-svg`:

- Renders to SVG files (temp files for viewing)
- Generates ASCII fallback for terminal display
- Saves to temp directory, opens in browser with `opener` crate
- Uses ggplot2-inspired color palette (light gray background, steel blue, coral)

## Code Conventions

### Error Handling

- Uses `anyhow::Result` as primary error type
- Context with `.context("description")` for error messages
- `anyhow::bail!` for early errors with messages

### Structs and Enums

- `#[derive(Debug, Clone)]` common for data structures
- `#[derive(Serialize, Deserialize)]` for API types (serde)
- Public structs use camelCase or snake_case depending on domain (Rust convention: snake_case)

### Async Patterns

- Uses `tokio` async runtime (multi-threaded scheduler)
- `async fn` for API calls, streaming operations
- `tokio::sync::mpsc` for inter-task communication (if needed)

### UI State Management

Main app state in `src/ui/tui.rs::App`:

```rust
pub struct App {
    tabs: AppTabs,              // Current tab (Agent, Data, Analysis, Viz)
    viz_engine: VisualizationEngine,
    datasets: Vec<LoadedDataset>,
    active_dataset_index: usize,
    should_quit: bool,
    conversation: Conversation,
    // ... many more fields
}
```

- Uses enums for state machines (`InputMode`, `LoadStatus`, `AgentStatus`)
- `Option<T>` for nullable state (e.g., `pending_analysis: Option<AnalysisRequest>`)
- Event-driven with crossterm event loop

### Naming Conventions

- **Modules**: snake_case (`data_ingestion`, `visualization_engine`)
- **Types**: PascalCase (`DataLoader`, `VisualizationEngine`)
- **Functions**: snake_case (`load_dataframe`, `render_histogram`)
- **Constants**: SCREAMING_SNAKE_CASE (`TERM_WIDTH`, `BG_LIGHT_GRAY`)

## Testing Approach

### Test Scripts

- `test.sh`: Builds project and shows quick workflow
- `test_api.sh`: Tests Z.AI API key with curl

### Manual Testing Workflow

1. Load data: Data tab → `L` → enter path
2. Check layout: Data tab shows "Genes: N | Age columns: M (range X–Y)"
3. Run analyses: Analysis tab → press key (`s`, `c`, `t`, `v`, etc.) → Enter to confirm
4. View charts: Visualizations tab → Space to toggle, `O` to open SVG

### Unit Tests

Standard Rust `cargo test`. Tests should use:
- `anyhow::Result` for test functions
- `assert!`, `assert_eq!` for assertions
- Temp files for I/O tests

## Important Gotchas

### Missing Config Module

The `config` module is imported but not implemented. Files reference:
- `ConfigManager::load_config()`
- `Config` struct with fields: `api_key`, `api_base_url`, `model`, `serpapi_key`, `viz_width`, `viz_height`, `default_bins`

**Impact**: Code won't compile without implementing this module.

### Model Selection

If you get "Unknown Model" (error 1211) from Z.AI API:
- Try different model values: `glm-4.7-flash`, `glm-4.7`, `glm-5`, `glm-4.5-air`
- The model is configured via config (once implemented) or hardcoded defaults

### Data Layout Requirements

Many analyses require microarray layout:
- ExpressionTrend, YoungVsOld, GenesSignificantWithAge - require gene × age layout
- Generic analyses (Histogram, Correlation) work on any numeric data

### Visualization Availability

Visualizations check availability and show disabled state with reason:
- Implemented in `src/viz/availability.rs`
- Returns `(available: bool, reason: Option<String>)`
- UI shows all options with availability status

### Terminal Output vs SVG

Visualizations have two outputs:
1. **Terminal**: ASCII fallback for quick preview in TUI
2. **SVG**: Full-quality chart saved to temp file, opened in browser

### Async/Blocking Mixing

- UI event loop is blocking
- AI requests are async (streaming)
- Uses `terminal.draw()` within async loops for UI updates during streaming

### Polars DataFrames

- Heavy use of Polars for data manipulation
- Expression columns may need coercion with `coerce_expression_columns()`
- Use `get_columns().iter().find(|c| c.dtype().is_numeric())` to find numeric columns

## Key Dependencies

- **ratatui** (0.29): Terminal UI framework with all widgets
- **crossterm** (0.28): Terminal handling, keyboard input
- **polars** (0.44): DataFrame library (lazy, csv, json, dtype-full features)
- **plotters** (0.3) + **plotters-svg** (0.3): Visualization rendering
- **tokio** (1.40): Async runtime (multi-threaded, macros, sync)
- **serde** (1.0) + **serde_json** (1.0): Serialization for API
- **reqwest** (0.12): HTTP client for GLM API (json, stream features)
- **calamine** (0.26): Excel file reading
- **anyhow** (1.0) + **thiserror** (2.0): Error handling
- **dotenvy** (0.15): Load .env files
- **opener** (0.7): Open files in default application (browser)

## Adding New Features

### New Analysis

1. Add variant to `AnalysisRequest` enum in `src/runner.rs`
2. Implement execution logic in `AnalysisRunner::run()`
3. Create viz config type in `src/viz/types.rs` (if visualization needed)
4. Implement rendering in `src/viz/engine.rs`
5. Add tool definition in `src/tools/definitions.rs`
6. Add executor case in `src/ui/tui.rs::execute_tool()`
7. Add keyboard shortcut in Analysis tab event handler
8. Add availability check in `src/viz/availability.rs`

### New Visualization

1. Add variant to `VisualizationConfig` enum in `src/viz/types.rs`
2. Add config struct (e.g., `#[derive(Debug, Clone)] pub struct YourVizConfig { ... }`)
3. Implement render method in `src/viz/engine.rs` using plotters
4. Add to `VisualizationEngine::render()` match statement
5. Add availability check in `src/viz/availability.rs`
6. Update UI to show in Analysis tab and Visualizations tab

### New Tool for AI

1. Add `Tool` struct to `get_all_tools()` in `src/tools/definitions.rs`
2. Define JSON schema for parameters
3. Add executor case in `src/ui/tui.rs::execute_tool()`
4. Tool can call existing app methods or new functionality

## Debugging Tips

### Build Issues

- Check `Cargo.lock` permissions if build fails
- Ensure Rust toolchain is up to date: `rustup update`
- Use `cargo check` for faster feedback than full build

### Runtime Issues

- Check `.env` file is present and contains valid keys
- Enable debug logging: `RUST_LOG=debug cargo run`
- Check temp directory for SVG files if visualization doesn't open

### API Issues

- Test API key with `cargo run -- --test-api`
- Check Z.AI endpoint: `https://api.z.ai/api/coding/paas/v4`
- Verify model name if "Unknown Model" error
- Check internet connectivity for SerpAPI

### UI Issues

- Ratatui coordinates: 0-based, use `f.area()` for dimensions
- Event loop: Ensure async operations don't block UI thread
- Terminal size: Ratatui handles resize events automatically

## Common Tasks

### Add a New Keyboard Shortcut

1. Find key handler in `src/ui/tui.rs` (e.g., `match key.code` in Analysis tab)
2. Add case for new key (e.g., `'x' => ...`)
3. Set `pending_analysis` or trigger action directly
4. Update help text in `src/ui/components.rs` if needed

### Load Data Programmatically

Use `DataLoader::load_dataframe(path)`:
- Handles CSV, JSON, XLSX based on extension
- Returns `Result<DataFrame>`
- Then detect layout with `DataLayout::detect(&df)`

### Create Visualization

Use `VisualizationEngine`:
```rust
let engine = VisualizationEngine::default(); // or new(width, height)
let chart_data = engine.render(&df, &viz_config)?;
// chart_data.terminal_output: ASCII for TUI
// chart_data.svg_file_path: SVG file path
```

### Make AI Call

Use `GlmClient`:
```rust
let client = GlmClient::new(api_key, base_url, model);
let response = client.chat(messages, Some(tools)).await?;
// Or for streaming: client.chat_stream(messages, Some(tools)).await?
```

## Performance Considerations

- **Polars LazyFrame**: Use for large datasets (already configured in features)
- **Streaming**: AI responses use SSE streaming to show progress
- **Visualization**: Plotters rendering can be slow for large datasets - consider sampling or aggregation
- **Terminal updates**: Minimize draw calls in tight loops

## Security Notes

- Never log API keys
- `.env` file should be in `.gitignore` (it is)
- API keys loaded from environment or config file only
- SerpAPI is optional, requires explicit key setup
- User data (gene expression) is processed locally, not sent to AI APIs (only analysis results)

## License

MIT License - See `LICENSE` file for details.
