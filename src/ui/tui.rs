use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    backend::Backend,
    prelude::*,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};
use std::time::Duration;
use tokio::sync::mpsc;

use crate::{
    data::{DataLoader, ColumnInfo, DataLayout, coerce_expression_columns},
    runner::{AnalysisRequest, AnalysisRunner},
    viz::{VisualizationEngine, available_visualizations},
    config::{Config, ConfigManager},
};
use super::components::{AnalysisStatus, AppTabs, LoadStatus, Tab};

pub struct App {
    tabs: AppTabs,
    viz_engine: VisualizationEngine,
    dataframe: Option<polars::prelude::DataFrame>,
    column_info: Vec<ColumnInfo>,
    data_layout: Option<DataLayout>,
    should_quit: bool,
    input_mode: InputMode,
    file_dialog_state: FileDialogState,
    gene_selection: Option<GeneSelectionState>,
    pending_analysis: Option<AnalysisRequest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    Normal,
    Editing,
}

#[derive(Debug, Clone)]
enum FileDialogState {
    None,
    AwaitingPath,
}

#[derive(Debug, Clone, Copy)]
enum GeneSelectionAction {
    ExpressionTrend,
    ExpressionVsAgeRegression,
}

#[derive(Debug, Clone)]
struct GeneSelectionState {
    genes: Vec<String>,
    selected: std::collections::HashSet<usize>,
    cursor: usize,
    max_select: usize,
    action: GeneSelectionAction,
}

#[allow(dead_code)]
pub enum AppEvent {
    LoadData(String),
    Analysis(AnalysisRequest),
    ToggleViz(bool),
}

impl App {
    pub fn new(config: Config) -> Result<Self> {
        let viz_engine = VisualizationEngine::new(config.viz_width, config.viz_height);
        Ok(Self {
            tabs: AppTabs::default(),
            viz_engine,
            dataframe: None,
            column_info: Vec::new(),
            data_layout: None,
            should_quit: false,
            input_mode: InputMode::Normal,
            file_dialog_state: FileDialogState::None,
            gene_selection: None,
            pending_analysis: None,
        })
    }

    pub async fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        let (_event_tx, mut event_rx) = mpsc::unbounded_channel::<AppEvent>();

        loop {
            self.draw(terminal)?;

            if self.should_quit {
                return Ok(());
            }

            match event_rx.try_recv() {
                Ok(event) => {
                    self.handle_app_event(event).await?;
                }
                Err(_) => {}
            }

            if matches!(self.tabs.analysis.analysis_status, AnalysisStatus::Loading)
                && self.pending_analysis.is_some()
            {
                let request = self.pending_analysis.take().unwrap();
                if let Some(df) = &self.dataframe {
                    match AnalysisRunner::run(df, request) {
                        Ok(result) => {
                            let output = format!(
                                "{}\n\n{}",
                                result.summary,
                                result.details.as_deref().unwrap_or("No details")
                            );
                            self.tabs.analysis.results.push(output);
                            self.tabs.analysis.analysis_status =
                                AnalysisStatus::Success("Analysis completed".to_string());
                            if let Some(viz_config) = result.viz_config {
                                if let Ok(viz_data) = self.viz_engine.render(df, &viz_config) {
                                    self.tabs.viz.viz_output = viz_data.terminal_output;
                                    self.tabs.viz.viz_title = viz_data.title;
                                    self.tabs.viz.viz_svg_path = viz_data.svg_file_path;
                                    self.tabs.viz.show_viz = true;
                                }
                            } else {
                                let details = result.details.as_deref().unwrap_or("No details");
                                self.tabs.viz.viz_output = details.to_string();
                                self.tabs.viz.viz_title = result.summary;
                                self.tabs.viz.viz_svg_path = None;
                                self.tabs.viz.show_viz = true;
                                self.tabs.active = Tab::Visualizations;
                            }
                        }
                        Err(e) => {
                            self.tabs.analysis.analysis_status =
                                AnalysisStatus::Error(e.to_string());
                        }
                    }
                } else {
                    self.tabs.analysis.analysis_status =
                        AnalysisStatus::Error("No data loaded".to_string());
                }
            }

            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key_event(key)?;
                }
            }
        }
    }

    fn draw<B: Backend>(&self, terminal: &mut Terminal<B>) -> Result<()> {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints([Constraint::Length(5), Constraint::Min(0)].as_ref())
                .split(f.area());

            self.tabs.render_tabs(chunks[0], f.buffer_mut());

            if self.tabs.show_help {
                self.tabs.render_help(chunks[1], f.buffer_mut());
                return;
            }

            if self.gene_selection.is_some() {
                self.render_gene_selection(f, chunks[1]);
                return;
            }

            match self.tabs.active {
                Tab::Data => self.render_data_tab(f, chunks[1]),
                Tab::Analysis => self.render_analysis_tab(f, chunks[1]),
                Tab::Visualizations => self.render_viz_tab(f, chunks[1]),
            }
        })?;
        Ok(())
    }

    fn render_data_tab(&self, f: &mut Frame, area: Rect) {
        if matches!(self.file_dialog_state, FileDialogState::AwaitingPath) {
            let load_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(5), Constraint::Length(5), Constraint::Min(0)].as_ref())
                .split(area);

            let instructions = vec![
                "LOAD FILE — Enter the full path to your data file",
                "",
                "Microarray format: Gene ID (col A), ages as column headers (17, 18, 21...).",
                "",
                "Examples:",
                "  ./data/sales.csv",
                "  /Users/you/data/report.xlsx",
                "  ~/Downloads/export.json",
                "",
                "Press Enter to load • Esc to cancel",
            ];
            Paragraph::new(instructions.join("\n"))
                .block(Block::default().borders(Borders::ALL).title(" Load File "))
                .wrap(Wrap { trim: false })
                .render(load_chunks[0], f.buffer_mut());

            let path_display = if self.tabs.data.file_path_input.is_empty() {
                "_".to_string()
            } else {
                self.tabs.data.file_path_input.clone()
            };
            Paragraph::new(path_display)
                .block(Block::default().borders(Borders::ALL).title(" Path (type here) "))
                .render(load_chunks[1], f.buffer_mut());

            Paragraph::new(self.tabs.data.preview_data.clone())
                .block(Block::default().borders(Borders::ALL).title(" Preview "))
                .wrap(Wrap { trim: false })
                .render(load_chunks[2], f.buffer_mut());
        } else {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Length(4), Constraint::Length(10), Constraint::Min(0)].as_ref())
                .split(area);

            let (status_text, status_style) = match &self.tabs.data.load_status {
                LoadStatus::Idle => ("Ready".to_string(), Style::default().fg(Color::DarkGray)),
                LoadStatus::Loading => ("⏳ Loading...".to_string(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                LoadStatus::Success(msg) => (format!("✓ {}  (press any key to dismiss)", msg), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                LoadStatus::Error(msg) => (format!("✗ Failed: {}  (press any key to dismiss)", msg), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            };
            Paragraph::new(status_text)
                .style(status_style)
                .block(Block::default().borders(Borders::ALL).title(" Status "))
                .wrap(Wrap { trim: false })
                .render(chunks[0], f.buffer_mut());

            let load_hint = "Press L to load microarray data: genes (rows) × age (columns). CSV, JSON, or Excel .xlsx";
            Paragraph::new(load_hint)
                .block(Block::default().borders(Borders::ALL).title(" Load File "))
                .render(chunks[1], f.buffer_mut());

            Paragraph::new(
                if self.tabs.data.file_path.is_empty() {
                    "No file loaded yet.".to_string()
                } else {
                    self.tabs.data.file_path.clone()
                },
            )
                .block(Block::default().borders(Borders::ALL).title(" Current File "))
                .wrap(Wrap { trim: false })
                .render(chunks[2], f.buffer_mut());

            let info_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(10), Constraint::Min(0)].as_ref())
                .split(chunks[3]);

            Paragraph::new(self.tabs.data.dataframe_info.clone())
                .block(Block::default().borders(Borders::ALL).title(" DataFrame Info "))
                .wrap(Wrap { trim: false })
                .render(info_chunks[0], f.buffer_mut());

            Paragraph::new(self.tabs.data.preview_data.clone())
                .block(Block::default().borders(Borders::ALL).title(" Preview "))
                .wrap(Wrap { trim: false })
                .render(info_chunks[1], f.buffer_mut());
        }
    }

    fn render_analysis_tab(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(4), Constraint::Min(0)].as_ref())
            .split(area);

        let (status_text, status_style) = match &self.tabs.analysis.analysis_status {
            AnalysisStatus::Idle => ("Ready".to_string(), Style::default().fg(Color::DarkGray)),
            AnalysisStatus::PendingConfirm { request } => (
                format!("▶ {} — Press Enter to run, Esc to cancel", request),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            AnalysisStatus::Loading => (
                "⏳ Running analysis...".to_string(),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            AnalysisStatus::Success(msg) => (
                format!("✓ {}  (press any key to dismiss)", msg),
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
            AnalysisStatus::Error(msg) => (
                format!("✗ {}  (press any key to dismiss)", msg),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        };
        Paragraph::new(status_text)
            .style(status_style)
            .block(Block::default().borders(Borders::ALL).title(" Status "))
            .wrap(Wrap { trim: false })
            .render(chunks[0], f.buffer_mut());

        let viz_list = available_visualizations(
            self.dataframe.as_ref(),
            self.data_layout.as_ref(),
        );
        let list_text: String = viz_list
            .iter()
            .map(|v| {
                if v.available {
                    format!("  [{}] {}", v.key, v.label)
                } else {
                    format!(
                        "  [{}] {} (disabled: {})",
                        v.key,
                        v.label,
                        v.reason.as_deref().unwrap_or("?")
                    )
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        if self.tabs.analysis.results.is_empty() {
            Paragraph::new(format!(
                "No analysis results yet.\n\nPress keys to run analyses:\n{}",
                list_text
            ))
                .block(Block::default().borders(Borders::ALL).title(" Results "))
                .wrap(Wrap { trim: false })
                .render(chunks[1], f.buffer_mut());
        } else {
            let items: Vec<ListItem> = self.tabs.analysis
                .results
                .iter()
                .map(|r| ListItem::new(r.clone()))
                .collect();

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(" Results "));

            Widget::render(list, chunks[1], f.buffer_mut());
        }
    }

    fn render_viz_tab(&self, f: &mut Frame, area: Rect) {
        if self.tabs.viz.show_viz && !self.tabs.viz.viz_output.is_empty() {
            let viz_text = self.tabs.viz.viz_output.clone();
            
            Paragraph::new(viz_text)
                .block(Block::default().borders(Borders::ALL).title(self.tabs.viz.viz_title.clone()))
                .wrap(Wrap { trim: false })
                .render(area, f.buffer_mut());
        } else {
            Paragraph::new("Press 'Space' to toggle display • 'O' to open chart in browser/viewer\n\nRun analyses (s, c, r, b, i) from the Analysis tab. Charts use ggplot2-style rendering.")
                .block(Block::default().borders(Borders::ALL).title(" Visualizations "))
                .wrap(Wrap { trim: false })
                .render(area, f.buffer_mut());
        }
    }

    fn render_gene_selection(&self, f: &mut Frame, area: Rect) {
        let Some(ref state) = self.gene_selection else { return };
        let title = match state.action {
            GeneSelectionAction::ExpressionTrend => "Select genes for Expression Trend (★ to select, max 5)",
            GeneSelectionAction::ExpressionVsAgeRegression => "Select genes for Expression vs Age Regression (★ to select, 1-5 genes)",
        };
        let hint = ratatui::text::Line::from("↑/↓ move • Space or * to select ★ • Enter to confirm • Esc to cancel");
        let mut items = vec![ListItem::new(hint), ListItem::new("")];
        for (i, gene) in state.genes.iter().enumerate() {
            let star = if state.selected.contains(&i) { " ★ " } else { "   " };
            let prefix = if i == state.cursor {
                format!("▶{}", star)
            } else {
                format!(" {} ", star)
            };
            let content = format!("{}{}", prefix, gene);
            let style = if i == state.cursor {
                Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else if state.selected.contains(&i) {
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            items.push(ListItem::new(content).style(style));
        }
        let list = List::new(items).block(Block::default().borders(Borders::ALL).title(title));
        Widget::render(list, area, f.buffer_mut());
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Result<()> {
        if key.code == KeyCode::Char('q') && self.input_mode == InputMode::Normal {
            self.should_quit = true;
            return Ok(());
        }

        if (key.code == KeyCode::Char('h') || key.code == KeyCode::Char('?'))
            && self.input_mode == InputMode::Normal
        {
            self.tabs.show_help = !self.tabs.show_help;
            return Ok(());
        }

        if self.tabs.show_help {
            if key.code == KeyCode::Esc {
                self.tabs.show_help = false;
            }
            return Ok(());
        }

        if let Some(ref mut state) = self.gene_selection {
            match key.code {
                KeyCode::Esc => {
                    self.gene_selection = None;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    state.cursor = state.cursor.saturating_sub(1).min(state.genes.len().saturating_sub(1));
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    state.cursor = (state.cursor + 1).min(state.genes.len().saturating_sub(1));
                }
                KeyCode::Char(' ') | KeyCode::Char('*') => {
                    if state.selected.contains(&state.cursor) {
                        state.selected.remove(&state.cursor);
                    } else if state.selected.len() < state.max_select {
                        state.selected.insert(state.cursor);
                    }
                }
                KeyCode::Enter => {
                    let selected: Vec<String> = state
                        .selected
                        .iter()
                        .filter_map(|&i| state.genes.get(i).cloned())
                        .collect();
                    let action = state.action.clone();
                    let layout = self.data_layout.clone().unwrap();
                    self.gene_selection = None;
                    if !selected.is_empty() {
                        match action {
                            GeneSelectionAction::ExpressionTrend => {
                                self.pending_analysis = Some(AnalysisRequest::ExpressionTrend {
                                    gene_ids: selected,
                                    gene_column: layout.gene_column,
                                    age_columns: layout.age_columns,
                                });
                                self.tabs.analysis.analysis_status =
                                    AnalysisStatus::Loading;
                            }
                            GeneSelectionAction::ExpressionVsAgeRegression => {
                                self.pending_analysis = Some(AnalysisRequest::ExpressionVsAgeRegression {
                                    gene_ids: selected,
                                    gene_column: layout.gene_column,
                                    age_columns: layout.age_columns,
                                });
                                self.tabs.analysis.analysis_status =
                                    AnalysisStatus::Loading;
                            }
                        }
                    }
                }
                _ => {}
            }
            return Ok(());
        }

        match key.code {
            KeyCode::Char('C') if self.input_mode == InputMode::Normal
                && (self.tabs.active == Tab::Analysis || self.tabs.active == Tab::Visualizations)
            => {
                self.tabs.analysis.results.clear();
                self.tabs.analysis.analysis_status = AnalysisStatus::Idle;
                self.tabs.viz.viz_output.clear();
                self.tabs.viz.viz_title.clear();
                self.tabs.viz.viz_svg_path = None;
                self.tabs.viz.show_viz = false;
                self.pending_analysis = None;
            }
            KeyCode::Tab => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.tabs.previous_tab();
                } else {
                    self.tabs.next_tab();
                }
            }
            KeyCode::Esc => {
                if self.input_mode == InputMode::Editing {
                    self.input_mode = InputMode::Normal;
                    self.file_dialog_state = FileDialogState::None;
                    self.tabs.data.file_path_input.clear();
                } else if self.tabs.active == Tab::Analysis
                    && matches!(self.tabs.analysis.analysis_status, AnalysisStatus::PendingConfirm { .. })
                {
                    self.tabs.analysis.analysis_status = AnalysisStatus::Idle;
                    self.pending_analysis = None;
                }
            }
            KeyCode::Char('l') | KeyCode::Char('L') if self.tabs.active == Tab::Data && self.input_mode == InputMode::Normal => {
                self.input_mode = InputMode::Editing;
                self.file_dialog_state = FileDialogState::AwaitingPath;
                self.tabs.data.file_path_input.clear();
                self.tabs.data.load_status = LoadStatus::Idle;
            }
            KeyCode::Enter if matches!(self.file_dialog_state, FileDialogState::AwaitingPath) => {
                let path = self.tabs.data.file_path_input.trim().to_string();
                if !path.is_empty() {
                    let expanded = shellexpand::tilde(&path).to_string();
                    self.tabs.data.load_status = LoadStatus::Loading;
                    match DataLoader::load_dataframe(&expanded) {
                        Ok(mut df) => {
                            let layout = DataLayout::detect(&df);
                            if let Some(ref l) = layout {
                                let _ = coerce_expression_columns(&mut df, l);
                            }
                            let info = DataLoader::get_column_info(&df);
                            let preview = format!("{:.5}", df.head(Some(10)));
                            let row_count = df.height();

                            self.dataframe = Some(df);
                            self.column_info = info;
                            self.data_layout = layout;
                            self.tabs.data.file_path = expanded.clone();
                            self.tabs.data.dataframe_info = if let Some(ref l) = self.data_layout {
                                format!(
                                    "Microarray layout detected\nGenes: {} | Age columns: {} (range {}-{})\n\n{}",
                                    l.gene_count,
                                    l.age_columns.len(),
                                    l.age_min,
                                    l.age_max,
                                    self.column_info.iter()
                                        .map(|c| format!("{}: {} (nulls: {})", c.name, c.dtype, c.null_count))
                                        .collect::<Vec<_>>()
                                        .join("\n")
                                )
                            } else {
                                self.column_info.iter()
                                    .map(|c| format!("{}: {} (nulls: {})", c.name, c.dtype, c.null_count))
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            };
                            self.tabs.data.preview_data = preview;
                            self.tabs.data.load_status = LoadStatus::Success(format!(
                                "Loaded {} rows from {}",
                                row_count,
                                std::path::Path::new(&expanded).file_name().and_then(|n| n.to_str()).unwrap_or(&expanded)
                            ));
                        }
                        Err(e) => {
                            self.tabs.data.load_status = LoadStatus::Error(e.to_string());
                        }
                    }
                }
                self.input_mode = InputMode::Normal;
                self.file_dialog_state = FileDialogState::None;
                self.tabs.data.file_path_input.clear();
            }
            KeyCode::Char(c) if self.input_mode == InputMode::Editing && matches!(self.file_dialog_state, FileDialogState::AwaitingPath) => {
                self.tabs.data.file_path_input.push(c);
            }
            KeyCode::Backspace if self.input_mode == InputMode::Editing && matches!(self.file_dialog_state, FileDialogState::AwaitingPath) => {
                self.tabs.data.file_path_input.pop();
            }
            KeyCode::Char(' ') if self.tabs.active == Tab::Visualizations && self.input_mode == InputMode::Normal => {
                self.tabs.viz.show_viz = !self.tabs.viz.show_viz;
            }
            KeyCode::Char('o') | KeyCode::Char('O') if self.tabs.active == Tab::Visualizations && self.input_mode == InputMode::Normal => {
                if let Some(ref path) = self.tabs.viz.viz_svg_path {
                    let _ = opener::open(path);
                }
            }
            KeyCode::Enter if self.tabs.active == Tab::Analysis
                && matches!(self.tabs.analysis.analysis_status, AnalysisStatus::PendingConfirm { .. })
            => {
                self.tabs.analysis.analysis_status = AnalysisStatus::Loading;
            }
            KeyCode::Char(c) if self.tabs.active == Tab::Analysis && self.input_mode == InputMode::Normal => {
                let viz_list = available_visualizations(
                    self.dataframe.as_ref(),
                    self.data_layout.as_ref(),
                );
                let avail = viz_list.iter().find(|v| v.key == c);
                if let Some(v) = avail {
                    if !v.available {
                        return Ok(());
                    }
                }
                match c {
                    's' if avail.map(|v| v.available).unwrap_or(false) => {
                        self.pending_analysis = Some(AnalysisRequest::SummaryStats);
                        self.tabs.analysis.analysis_status =
                            AnalysisStatus::PendingConfirm { request: "Summary statistics".to_string() };
                    }
                    'c' if avail.map(|v| v.available).unwrap_or(false) => {
                        self.pending_analysis = Some(AnalysisRequest::Correlation);
                        self.tabs.analysis.analysis_status =
                            AnalysisStatus::PendingConfirm { request: "Correlation matrix".to_string() };
                    }
                    'r' if avail.map(|v| v.available).unwrap_or(false) => {
                        if let Some(layout) = &self.data_layout {
                            self.pending_analysis = Some(AnalysisRequest::GenesExpressionVsAge {
                                gene_column: layout.gene_column.clone(),
                                age_columns: layout.age_columns.clone(),
                            });
                            self.tabs.analysis.analysis_status =
                                AnalysisStatus::PendingConfirm { request: "Expression vs age (all genes)".to_string() };
                        } else if let Some(df) = &self.dataframe {
                            let numeric_cols: Vec<String> = df.get_columns()
                                .iter()
                                .filter(|c| c.dtype().is_numeric())
                                .map(|c| c.name().to_string())
                                .collect();
                            if numeric_cols.len() >= 2 {
                                let x = numeric_cols[0].clone();
                                let y = numeric_cols[1].clone();
                                self.pending_analysis = Some(AnalysisRequest::LinearRegression {
                                    x_column: x.clone(),
                                    y_column: y.clone(),
                                });
                                self.tabs.analysis.analysis_status =
                                    AnalysisStatus::PendingConfirm { request: format!("Linear regression: {} vs {}", x, y) };
                            }
                        }
                    }
                    'g' if avail.map(|v| v.available).unwrap_or(false) => {
                        if let Some(layout) = &self.data_layout {
                            self.pending_analysis = Some(AnalysisRequest::GenesSignificantWithAge {
                                gene_column: layout.gene_column.clone(),
                                age_columns: layout.age_columns.clone(),
                            });
                            self.tabs.analysis.analysis_status =
                                AnalysisStatus::PendingConfirm { request: "Genes significant with age (p<0.05)".to_string() };
                        }
                    }
                    'b' if avail.map(|v| v.available).unwrap_or(false) => {
                        if let Some(df) = &self.dataframe {
                            if let Some(col) = df.get_columns().iter()
                                .find(|c| c.dtype().is_numeric())
                                .map(|c| c.name().to_string())
                            {
                                self.pending_analysis = Some(AnalysisRequest::BoxPlot { column: col.clone() });
                                self.tabs.analysis.analysis_status =
                                    AnalysisStatus::PendingConfirm { request: format!("Box plot: {}", col) };
                            }
                        }
                    }
                    'h' if avail.map(|v| v.available).unwrap_or(false) => {
                        if let Some(df) = &self.dataframe {
                            if let Some(col) = df.get_columns().iter()
                                .find(|c| c.dtype().is_numeric())
                                .map(|c| c.name().to_string())
                            {
                                let bins = ConfigManager::load_config().map(|c| c.default_bins).unwrap_or(20);
                                self.pending_analysis = Some(AnalysisRequest::Histogram {
                                    column: col.clone(),
                                    bins,
                                });
                                self.tabs.analysis.analysis_status =
                                    AnalysisStatus::PendingConfirm { request: format!("Histogram: {} ({} bins)", col, bins) };
                            }
                        }
                    }
                    't' if avail.map(|v| v.available).unwrap_or(false) => {
                        if let Some(layout) = &self.data_layout {
                            let genes: Vec<String> = self
                                .dataframe
                                .as_ref()
                                .and_then(|df| df.column(&layout.gene_column).ok())
                                .and_then(|c| c.str().ok())
                                .map(|s| s.into_iter().filter_map(|o| o.map(str::to_string)).collect())
                                .unwrap_or_default();
                            if !genes.is_empty() {
                                self.gene_selection = Some(GeneSelectionState {
                                    genes,
                                    selected: std::collections::HashSet::new(),
                                    cursor: 0,
                                    max_select: 5,
                                    action: GeneSelectionAction::ExpressionTrend,
                                });
                            }
                        }
                    }
                    'v' if avail.map(|v| v.available).unwrap_or(false) => {
                        if let Some(layout) = &self.data_layout {
                            self.pending_analysis = Some(AnalysisRequest::YoungVsOld {
                                gene_column: layout.gene_column.clone(),
                                age_columns: layout.age_columns.clone(),
                            });
                            self.tabs.analysis.analysis_status =
                                AnalysisStatus::PendingConfirm { request: "Young vs Old scatter".to_string() };
                        }
                    }
                    'a' if avail.map(|v| v.available).unwrap_or(false) => {
                        if let Some(layout) = &self.data_layout {
                            self.pending_analysis = Some(AnalysisRequest::AgeGroupBoxPlot {
                                gene_column: layout.gene_column.clone(),
                                age_columns: layout.age_columns.clone(),
                            });
                            self.tabs.analysis.analysis_status =
                                AnalysisStatus::PendingConfirm { request: "Age group box plot".to_string() };
                        }
                    }
                    'e' if avail.map(|v| v.available).unwrap_or(false) => {
                        if let Some(layout) = &self.data_layout {
                            let genes: Vec<String> = self
                                .dataframe
                                .as_ref()
                                .and_then(|df| df.column(&layout.gene_column).ok())
                                .and_then(|c| c.str().ok())
                                .map(|s| s.into_iter().filter_map(|o| o.map(str::to_string)).collect())
                                .unwrap_or_default();
                            if !genes.is_empty() {
                                self.gene_selection = Some(GeneSelectionState {
                                    genes,
                                    selected: std::collections::HashSet::new(),
                                    cursor: 0,
                                    max_select: 5,
                                    action: GeneSelectionAction::ExpressionVsAgeRegression,
                                });
                            }
                        }
                    }
                    '1' if avail.map(|v| v.available).unwrap_or(false) => {
                        if let Some(layout) = &self.data_layout {
                            self.pending_analysis = Some(AnalysisRequest::GenesVolcanoPlot {
                                gene_column: layout.gene_column.clone(),
                                age_columns: layout.age_columns.clone(),
                            });
                            self.tabs.analysis.analysis_status =
                                AnalysisStatus::PendingConfirm { request: "Volcano plot".to_string() };
                        }
                    }
                    '2' if avail.map(|v| v.available).unwrap_or(false) => {
                        if let Some(layout) = &self.data_layout {
                            self.pending_analysis = Some(AnalysisRequest::GenesCorrelationScatter {
                                gene_column: layout.gene_column.clone(),
                                age_columns: layout.age_columns.clone(),
                            });
                            self.tabs.analysis.analysis_status =
                                AnalysisStatus::PendingConfirm { request: "Correlation scatter".to_string() };
                        }
                    }
                    '3' if avail.map(|v| v.available).unwrap_or(false) => {
                        if let Some(layout) = &self.data_layout {
                            self.pending_analysis = Some(AnalysisRequest::GenesCorrelationBarChart {
                                gene_column: layout.gene_column.clone(),
                                age_columns: layout.age_columns.clone(),
                                top_n: 30,
                            });
                            self.tabs.analysis.analysis_status =
                                AnalysisStatus::PendingConfirm { request: "Top 30 genes bar chart".to_string() };
                        }
                    }
                    _ => {}
                }
            }
            _ if self.tabs.active == Tab::Analysis && self.input_mode == InputMode::Normal => {
                if matches!(self.tabs.analysis.analysis_status, AnalysisStatus::Success(_) | AnalysisStatus::Error(_)) {
                    self.tabs.analysis.analysis_status = AnalysisStatus::Idle;
                }
            }
            _ if self.tabs.active == Tab::Data && self.input_mode == InputMode::Normal => {
                if matches!(self.tabs.data.load_status, LoadStatus::Success(_) | LoadStatus::Error(_)) {
                    self.tabs.data.load_status = LoadStatus::Idle;
                }
            }
            _ => {}
        }

        Ok(())
    }

    async fn handle_app_event(&mut self, event: AppEvent) -> Result<()> {
        match event {
            AppEvent::LoadData(path) => {
                let df = DataLoader::load_dataframe(&path)?;
                let info = DataLoader::get_column_info(&df);
                let preview = format!("{:.5}", df.head(Some(10)));
                
                self.dataframe = Some(df);
                self.column_info = info;
                self.tabs.data.file_path = path;
                self.tabs.data.dataframe_info = self.column_info.iter()
                    .map(|c| format!("{}: {} (nulls: {})", c.name, c.dtype, c.null_count))
                    .collect::<Vec<_>>()
                    .join("\n");
                self.tabs.data.preview_data = preview;
                self.tabs.active = Tab::Data;
            }
            AppEvent::Analysis(request) => {
                if let Some(df) = &self.dataframe {
                    if let Ok(result) = AnalysisRunner::run(df, request) {
                        let output = format!(
                            "{}\n\n{}",
                            result.summary,
                            result.details.as_deref().unwrap_or("No details")
                        );
                        self.tabs.analysis.results.push(output);
                        if let Some(viz_config) = result.viz_config {
                            if let Ok(viz_data) = self.viz_engine.render(df, &viz_config) {
                                self.tabs.viz.viz_output = viz_data.terminal_output;
                                self.tabs.viz.viz_title = viz_data.title;
                                self.tabs.viz.viz_svg_path = viz_data.svg_file_path;
                                self.tabs.viz.show_viz = true;
                            }
                        }
                        self.tabs.active = Tab::Analysis;
                    }
                }
            }
            AppEvent::ToggleViz(show) => {
                self.tabs.viz.show_viz = show;
            }
        }
        Ok(())
    }
}
