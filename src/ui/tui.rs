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

/// A single loaded dataset with its metadata.
#[derive(Clone)]
struct LoadedDataset {
    path: String,
    dataframe: polars::prelude::DataFrame,
    layout: Option<DataLayout>,
    column_info: Vec<ColumnInfo>,
    dataframe_info: String,
    preview_data: String,
}

pub struct App {
    tabs: AppTabs,
    viz_engine: VisualizationEngine,
    datasets: Vec<LoadedDataset>,
    active_dataset_index: usize,
    /// Selected age columns (when Some, use these; else use all from layout).
    selected_age_columns: Option<std::collections::HashSet<String>>,
    /// User-defined age groups for Young vs Old, e.g. Young=17-30, Old=40-60.
    age_groups: Option<Vec<crate::data::AgeGroupDef>>,
    should_quit: bool,
    input_mode: InputMode,
    file_dialog_state: FileDialogState,
    gene_selection: Option<GeneSelectionState>,
    data_tab_age_selector: Option<DataTabAgeSelectorState>,
    data_tab_focus: DataTabFocus,
    age_group_input: Option<String>,
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
    /// Search input: type / or paste gene ID, Enter to find and select.
    search_input: Option<String>,
}

/// Focus on Data tab: TabBar (tabs at top) or AgeSelector (age range field).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DataTabFocus {
    TabBar,
    AgeSelector,
}

/// Age selector embedded on Data tab: only available ages from layout, [x]:17 format.
#[derive(Debug, Clone)]
struct DataTabAgeSelectorState {
    ages: Vec<String>,
    cursor: usize,
}

#[allow(dead_code)]
pub enum AppEvent {
    LoadData(String),
    Analysis(AnalysisRequest),
    ToggleViz(bool),
}

impl App {
    fn active_dataset(&self) -> Option<&LoadedDataset> {
        self.datasets.get(self.active_dataset_index)
    }

    fn active_dataframe(&self) -> Option<&polars::prelude::DataFrame> {
        self.active_dataset().map(|d| &d.dataframe)
    }

    fn active_layout(&self) -> Option<&DataLayout> {
        self.active_dataset().and_then(|d| d.layout.as_ref())
    }

    fn effective_age_columns(&self, layout: &DataLayout) -> Vec<String> {
        match &self.selected_age_columns {
            Some(sel) => layout
                .age_columns
                .iter()
                .filter(|c| sel.contains(*c))
                .cloned()
                .collect(),
            None => layout.age_columns.clone(),
        }
    }

    pub fn new(config: Config) -> Result<Self> {
        let viz_engine = VisualizationEngine::new(config.viz_width, config.viz_height);
        Ok(Self {
            tabs: AppTabs::default(),
            viz_engine,
            datasets: Vec::new(),
            active_dataset_index: 0,
            selected_age_columns: None,
            age_groups: None,
            should_quit: false,
            input_mode: InputMode::Normal,
            file_dialog_state: FileDialogState::None,
            gene_selection: None,
            data_tab_age_selector: None,
            data_tab_focus: DataTabFocus::TabBar,
            age_group_input: None,
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
                let run_result = self.active_dataframe().map(|df| AnalysisRunner::run(df, request));
                match run_result {
                    Some(Ok(result)) => {
                        let output = format!(
                            "{}\n\n{}",
                            result.summary,
                            result.details.as_deref().unwrap_or("No details")
                        );
                        self.tabs.analysis.results.push(output);
                        self.tabs.analysis.analysis_status =
                            AnalysisStatus::Success("Analysis completed".to_string());
                        if let Some(ref viz_config) = result.viz_config {
                            if let Some(df) = self.active_dataframe() {
                                if let Ok(viz_data) = self.viz_engine.render(df, viz_config) {
                                    self.tabs.viz.viz_output = viz_data.terminal_output;
                                    self.tabs.viz.viz_title = viz_data.title;
                                    self.tabs.viz.viz_svg_path = viz_data.svg_file_path;
                                    self.tabs.viz.show_viz = true;
                                }
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
                    Some(Err(e)) => {
                        self.tabs.analysis.analysis_status =
                            AnalysisStatus::Error(e.to_string());
                    }
                    None => {
                        self.tabs.analysis.analysis_status =
                            AnalysisStatus::Error("No data loaded".to_string());
                    }
                }
            }

            if event::poll(Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(key) => self.handle_key_event(key)?,
                    Event::Paste(s) => self.handle_paste(&s)?,
                    _ => {}
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

            if self.age_group_input.is_some() {
                self.render_age_group_input(f, chunks[1]);
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
                "LOAD FILE(S) — Enter path(s) to your data file(s)",
                "",
                "Multiple files: separate with comma or semicolon",
                "  ./data/a.csv, ./data/b.csv",
                "",
                "Microarray: log-normalised expression, Gene ID (col A) × age (cols B+).",
                "One value per gene (highest probe when multiple map to same gene).",
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
            let has_age_selector = self.data_tab_age_selector.is_some();
            let has_datasets = !self.datasets.is_empty();
            let constraints: &[Constraint] = if has_datasets && has_age_selector {
                &[
                    Constraint::Length(3),
                    Constraint::Length(4),
                    Constraint::Length(3),
                    Constraint::Length(8),
                    Constraint::Length(8),
                    Constraint::Length(10),
                    Constraint::Min(0),
                ]
            } else if has_datasets {
                &[
                    Constraint::Length(3),
                    Constraint::Length(4),
                    Constraint::Length(3),
                    Constraint::Length(8),
                    Constraint::Length(10),
                    Constraint::Min(0),
                ]
            } else if has_age_selector {
                &[
                    Constraint::Length(3),
                    Constraint::Length(4),
                    Constraint::Length(10),
                    Constraint::Length(8),
                    Constraint::Length(10),
                    Constraint::Min(0),
                ]
            } else {
                &[
                    Constraint::Length(3),
                    Constraint::Length(4),
                    Constraint::Length(10),
                    Constraint::Length(10),
                    Constraint::Min(0),
                ]
            };
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(constraints)
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

            let load_hint = "Press L to load file(s). Microarray: log-normalised expression, gene ID × age. CSV, JSON, or Excel .xlsx";
            Paragraph::new(load_hint)
                .block(Block::default().borders(Borders::ALL).title(" Load File "))
                .render(chunks[1], f.buffer_mut());

            let file_display = if !self.datasets.is_empty() {
                let active = self.active_dataset().unwrap();
                format!("{} ({} of {})", active.path, self.active_dataset_index + 1, self.datasets.len())
            } else {
                "No file loaded yet.".to_string()
            };
            Paragraph::new(file_display)
                .block(Block::default().borders(Borders::ALL).title(" Active Dataset "))
                .wrap(Wrap { trim: false })
                .render(chunks[2], f.buffer_mut());

            let mut chunk_idx = 3;
            if has_datasets {
                self.render_datasets_list(f, chunks[chunk_idx]);
                chunk_idx += 1;
            }
            let bottom_area = if has_age_selector {
                self.render_data_tab_age_selector(f, chunks[chunk_idx]);
                chunk_idx += 1;
                chunks[chunk_idx]
            } else {
                chunks[chunk_idx]
            };

            let info_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(10), Constraint::Min(0)].as_ref())
                .split(bottom_area);

            let (info_text, preview_text) = self.active_dataset()
                .map(|d| (d.dataframe_info.clone(), d.preview_data.clone()))
                .unwrap_or((
                    self.tabs.data.dataframe_info.clone(),
                    self.tabs.data.preview_data.clone(),
                ));
            Paragraph::new(info_text)
                .block(Block::default().borders(Borders::ALL).title(" DataFrame Info "))
                .wrap(Wrap { trim: false })
                .render(info_chunks[0], f.buffer_mut());

            Paragraph::new(preview_text)
                .block(Block::default().borders(Borders::ALL).title(" Preview "))
                .wrap(Wrap { trim: false })
                .render(info_chunks[1], f.buffer_mut());
        }
    }

    fn render_datasets_list(&self, f: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self.datasets
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let name = std::path::Path::new(&d.path).file_name().and_then(|n| n.to_str()).unwrap_or(&d.path);
                let prefix = if i == self.active_dataset_index {
                    "▶ "
                } else {
                    "  "
                };
                let key = format!("[{}] ", i + 1);
                let content = format!("{}{}{} ({} rows)", prefix, key, name, d.dataframe.height());
                let style = if i == self.active_dataset_index {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(content).style(style)
            })
            .collect();
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(" Datasets (1-9 to select) "));
        Widget::render(list, area, f.buffer_mut());
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
            self.active_dataframe(),
            self.active_layout(),
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
            Paragraph::new("Press 'Space' to toggle display • 'O' to open chart in browser/viewer\n\nRun analyses (s, i, r, h, g) from the Analysis tab. Charts use ggplot2-style rendering.")
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
        let hint = if state.search_input.is_some() {
            ratatui::text::Line::from("Type gene ID, Enter to find & select • Esc to cancel search")
        } else {
            ratatui::text::Line::from("↑/↓ move • Space or * select ★ • / to search • Paste gene ID • Enter confirm • Esc cancel")
        };
        let mut items = vec![ListItem::new(hint), ListItem::new("")];
        if let Some(ref search) = state.search_input {
            items.push(ListItem::new(format!("Search: {}_", search)));
            items.push(ListItem::new(""));
        }
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

    fn render_data_tab_age_selector(&self, f: &mut Frame, area: Rect) {
        let Some(ref state) = self.data_tab_age_selector else { return };
        let all_selected = self.selected_age_columns.is_none();
        let sel = self.selected_age_columns.as_ref();
        let is_selected = |age: &str| {
            if all_selected {
                true
            } else if let Some(s) = sel {
                s.contains(age)
            } else {
                true
            }
        };
        const COLS: usize = 12;
        let hint = if self.data_tab_focus == DataTabFocus::AgeSelector {
            "Tab, ↑ or Esc to return to tabs • k/j/h/l to move • X to toggle"
        } else {
            "↓ to enter age selection • Tab switches Data/Analysis/Viz"
        };
        let mut lines: Vec<ratatui::text::Line> = vec![ratatui::text::Line::from(hint)];
        for (i, age) in state.ages.iter().enumerate() {
            let mark = if is_selected(age) { "x" } else { " " };
            let is_cursor = i == state.cursor && self.data_tab_focus == DataTabFocus::AgeSelector;
            let span = ratatui::text::Span::styled(
                format!("[{}]:{} ", mark, age),
                if is_cursor {
                    Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else if is_selected(age) {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            );
            let col = i % COLS;
            if col == 0 {
                lines.push(ratatui::text::Line::from(vec![span]));
            } else {
                let last = lines.len() - 1;
                lines[last].spans.push(span);
            }
        }
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Age Selection (affects all analyses) ");
        let inner = block.inner(area);
        block.render(area, f.buffer_mut());
        let para = Paragraph::new(ratatui::text::Text::from(lines)).wrap(Wrap { trim: false });
        para.render(inner, f.buffer_mut());
    }

    fn render_age_group_input(&self, f: &mut Frame, area: Rect) {
        let input = self.age_group_input.as_deref().unwrap_or("");
        let instructions = vec![
            "Define age groups for Young vs Old (e.g. Young=17-30,Old=40-60)",
            "",
            "Format: Name1=min1-max1,Name2=min2-max2",
            "Example: Young=17-30,Old=40-60",
            "",
            "Press Enter to save • Esc to cancel",
        ];
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(6), Constraint::Min(0)].as_ref())
            .split(area);
        Paragraph::new(instructions.join("\n"))
            .block(Block::default().borders(Borders::ALL).title(" Age Groups "))
            .wrap(Wrap { trim: false })
            .render(chunks[0], f.buffer_mut());
        let display = if input.is_empty() { "_" } else { input };
        Paragraph::new(display)
            .block(Block::default().borders(Borders::ALL).title(" Input "))
            .render(chunks[1], f.buffer_mut());
    }

    fn handle_paste(&mut self, text: &str) -> Result<()> {
        if let Some(ref mut state) = self.gene_selection {
            let query = text.trim();
            if !query.is_empty() {
                let idx = state.genes.iter().position(|g| {
                    g.eq_ignore_ascii_case(query) || g.contains(query)
                });
                if let Some(i) = idx {
                    state.cursor = i;
                    if state.selected.len() < state.max_select {
                        state.selected.insert(i);
                    }
                }
            }
        }
        Ok(())
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

        if self.tabs.active == Tab::Data
            && self.data_tab_age_selector.is_some()
            && !matches!(self.file_dialog_state, FileDialogState::AwaitingPath)
        {
            let has_age_selector = self.data_tab_age_selector.is_some();

            if self.data_tab_focus == DataTabFocus::TabBar {
                if has_age_selector && key.code == KeyCode::Down {
                    self.data_tab_focus = DataTabFocus::AgeSelector;
                    return Ok(());
                }
            }

            if self.data_tab_focus == DataTabFocus::AgeSelector {
                if key.code == KeyCode::Tab || key.code == KeyCode::Up || key.code == KeyCode::Esc {
                    self.data_tab_focus = DataTabFocus::TabBar;
                    return Ok(());
                }
                if let Some(ref mut state) = self.data_tab_age_selector {
                    const COLS: usize = 12;
                    let len = state.ages.len();
                    let mut consumed = true;
                    match key.code {
                        KeyCode::Char('k') => {
                            state.cursor = state.cursor.saturating_sub(COLS).min(len.saturating_sub(1));
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            state.cursor = (state.cursor + COLS).min(len.saturating_sub(1));
                        }
                        KeyCode::Left | KeyCode::Char('h') => {
                            state.cursor = state.cursor.saturating_sub(1).min(len.saturating_sub(1));
                        }
                        KeyCode::Right | KeyCode::Char('l') => {
                            state.cursor = (state.cursor + 1).min(len.saturating_sub(1));
                        }
                        KeyCode::Char('x') | KeyCode::Char('X') => {
                            if let Some(age) = state.ages.get(state.cursor).cloned() {
                                let all_ages: std::collections::HashSet<String> =
                                    state.ages.iter().cloned().collect();
                                match &self.selected_age_columns {
                                    None => {
                                        let mut new_sel = all_ages.clone();
                                        new_sel.remove(&age);
                                        self.selected_age_columns = Some(new_sel);
                                    }
                                    Some(s) => {
                                        let mut new_sel = s.clone();
                                        if new_sel.contains(&age) {
                                            new_sel.remove(&age);
                                        } else {
                                            new_sel.insert(age.clone());
                                        }
                                        self.selected_age_columns = if new_sel == all_ages {
                                            None
                                        } else {
                                            Some(new_sel)
                                        };
                                    }
                                }
                            }
                        }
                        _ => consumed = false,
                    }
                    if consumed {
                        return Ok(());
                    }
                }
            }
        }

        if let Some(ref mut input) = self.age_group_input {
            match key.code {
                KeyCode::Esc => {
                    self.age_group_input = None;
                }
                KeyCode::Enter => {
                    if let Some(groups) = crate::data::parse_age_groups(input) {
                        self.age_groups = Some(groups);
                    }
                    self.age_group_input = None;
                }
                KeyCode::Char(c) => {
                    input.push(c);
                }
                KeyCode::Backspace => {
                    input.pop();
                }
                _ => {}
            }
            return Ok(());
        }

        if let Some(ref mut state) = self.gene_selection {
            if let Some(ref mut search) = state.search_input {
                match key.code {
                    KeyCode::Esc => {
                        state.search_input = None;
                    }
                    KeyCode::Enter => {
                        let query = search.clone();
                        state.search_input = None;
                        let query = query.trim();
                        if !query.is_empty() {
                            let idx = state.genes.iter().position(|g| {
                                g.eq_ignore_ascii_case(query) || g.contains(query)
                            });
                            if let Some(i) = idx {
                                state.cursor = i;
                                if state.selected.len() < state.max_select {
                                    state.selected.insert(i);
                                }
                            }
                        }
                    }
                    KeyCode::Backspace => {
                        search.pop();
                    }
                    KeyCode::Char(c) => {
                        search.push(c);
                    }
                    _ => {}
                }
                return Ok(());
            }
            match key.code {
                KeyCode::Esc => {
                    self.gene_selection = None;
                }
                KeyCode::Char('/') => {
                    state.search_input = Some(String::new());
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
                    let layout = self.active_layout().cloned().unwrap();
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
            KeyCode::Char('4') if self.input_mode == InputMode::Normal
                && self.active_layout().is_some()
                && self.gene_selection.is_none()
                && self.age_group_input.is_none()
            => {
                self.age_group_input = Some(String::new());
            }
            KeyCode::Char('C') if self.input_mode == InputMode::Normal => {
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
            KeyCode::Char(c) if self.tabs.active == Tab::Data
                && self.input_mode == InputMode::Normal
                && !self.datasets.is_empty()
                && c >= '1' && c <= '9'
            => {
                let idx = (c as usize) - ('1' as usize);
                if idx < self.datasets.len() {
                    let path = self.datasets[idx].path.clone();
                    let info = self.datasets[idx].dataframe_info.clone();
                    let preview = self.datasets[idx].preview_data.clone();
                    let ages = self.datasets[idx].layout.as_ref().map(|l| l.age_columns.clone());
                    self.active_dataset_index = idx;
                    self.tabs.data.file_path = path;
                    self.tabs.data.dataframe_info = info;
                    self.tabs.data.preview_data = preview;
                    self.data_tab_age_selector = ages.map(|a| DataTabAgeSelectorState {
                        ages: a,
                        cursor: 0,
                    });
                }
            }
            KeyCode::Char('l') | KeyCode::Char('L') if self.tabs.active == Tab::Data && self.input_mode == InputMode::Normal => {
                self.input_mode = InputMode::Editing;
                self.file_dialog_state = FileDialogState::AwaitingPath;
                self.tabs.data.file_path_input.clear();
                self.tabs.data.load_status = LoadStatus::Idle;
            }
            KeyCode::Enter if matches!(self.file_dialog_state, FileDialogState::AwaitingPath) => {
                let input = self.tabs.data.file_path_input.trim();
                if !input.is_empty() {
                    let paths: Vec<String> = input
                        .split([',', ';', '\n'])
                        .map(|s| shellexpand::tilde(s.trim()).to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    self.tabs.data.load_status = LoadStatus::Loading;
                    let mut loaded = 0;
                    let mut errors = Vec::new();
                    for path in &paths {
                        match DataLoader::load_dataframe(path) {
                            Ok(mut df) => {
                                let layout = DataLayout::detect(&df);
                                if let Some(ref l) = layout {
                                    let _ = coerce_expression_columns(&mut df, l);
                                }
                                let info = DataLoader::get_column_info(&df);
                                let preview = format!("{:.5}", df.head(Some(10)));
                                let dataframe_info = if let Some(ref l) = layout {
                                    format!(
                                        "Microarray layout detected (log-normalised expression)\nGenes: {} | Age columns: {} (range {}-{})\n\n{}",
                                        l.gene_count,
                                        l.age_columns.len(),
                                        l.age_min,
                                        l.age_max,
                                        info.iter()
                                            .map(|c| format!("{}: {} (nulls: {})", c.name, c.dtype, c.null_count))
                                            .collect::<Vec<_>>()
                                            .join("\n")
                                    )
                                } else {
                                    info.iter()
                                        .map(|c| format!("{}: {} (nulls: {})", c.name, c.dtype, c.null_count))
                                        .collect::<Vec<_>>()
                                        .join("\n")
                                };
                                self.datasets.push(LoadedDataset {
                                    path: path.clone(),
                                    dataframe: df,
                                    layout,
                                    column_info: info,
                                    dataframe_info,
                                    preview_data: preview,
                                });
                                loaded += 1;
                            }
                            Err(e) => {
                                let name = std::path::Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or(path);
                                errors.push(format!("{}: {}", name, e));
                            }
                        }
                    }
                    self.active_dataset_index = self.datasets.len().saturating_sub(1);
                    if loaded > 0 {
                        let idx = self.active_dataset_index;
                        let path = self.datasets[idx].path.clone();
                        let info = self.datasets[idx].dataframe_info.clone();
                        let preview = self.datasets[idx].preview_data.clone();
                        let ages = self.datasets[idx].layout.as_ref().map(|l| l.age_columns.clone());
                        self.tabs.data.file_path = path;
                        self.tabs.data.dataframe_info = info;
                        self.tabs.data.preview_data = preview;
                        let msg = if errors.is_empty() {
                            format!("Loaded {} dataset(s)", loaded)
                        } else {
                            format!("Loaded {} dataset(s). Failed: {}", loaded, errors.join("; "))
                        };
                        self.tabs.data.load_status = LoadStatus::Success(msg);
                        self.data_tab_age_selector = ages.map(|a| DataTabAgeSelectorState {
                            ages: a,
                            cursor: 0,
                        });
                    } else {
                        self.tabs.data.load_status = LoadStatus::Error(
                            errors.first().cloned().unwrap_or_else(|| "Failed to load".to_string())
                        );
                    }
                    self.data_tab_focus = DataTabFocus::TabBar;
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
                    self.active_dataframe(),
                    self.active_layout(),
                );
                let avail = viz_list.iter().find(|v| v.key == c);
                if let Some(v) = avail {
                    if !v.available {
                        return Ok(());
                    }
                }
                match c {
                    's' if avail.map(|v| v.available).unwrap_or(false) => {
                        let gene_age = self.active_layout().map(|l| {
                            (l.gene_column.clone(), self.effective_age_columns(l))
                        });
                        self.pending_analysis = Some(AnalysisRequest::SummaryStats {
                            gene_age_summary: gene_age,
                        });
                        self.tabs.analysis.analysis_status =
                            AnalysisStatus::PendingConfirm { request: "Summary statistics (mean, median, mode, R², p-value, correlation)".to_string() };
                    }
                    'r' if avail.map(|v| v.available).unwrap_or(false) => {
                        if let Some(layout) = self.active_layout() {
                            self.pending_analysis = Some(AnalysisRequest::GenesExpressionVsAge {
                                gene_column: layout.gene_column.clone(),
                                age_columns: self.effective_age_columns(layout),
                            });
                            self.tabs.analysis.analysis_status =
                                AnalysisStatus::PendingConfirm { request: "Expression vs age (all genes)".to_string() };
                        } else if let Some(df) = self.active_dataframe() {
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
                        if let Some(layout) = self.active_layout() {
                            self.pending_analysis = Some(AnalysisRequest::GenesSignificantWithAge {
                                gene_column: layout.gene_column.clone(),
                                age_columns: self.effective_age_columns(layout),
                            });
                            self.tabs.analysis.analysis_status =
                                AnalysisStatus::PendingConfirm { request: "Gene correlation with aging (positive/negative, p<0.05)".to_string() };
                        }
                    }
                    'h' if avail.map(|v| v.available).unwrap_or(false) => {
                        self.pending_analysis = Some(AnalysisRequest::Heatmap);
                        self.tabs.analysis.analysis_status =
                            AnalysisStatus::PendingConfirm { request: "Heatmap (correlation matrix)".to_string() };
                    }
                    'i' if avail.map(|v| v.available).unwrap_or(false) => {
                        if let Some(df) = self.active_dataframe() {
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
                if let Ok(mut df) = DataLoader::load_dataframe(&path) {
                    let layout = DataLayout::detect(&df);
                    if let Some(ref l) = layout {
                        let _ = coerce_expression_columns(&mut df, l);
                    }
                    let info = DataLoader::get_column_info(&df);
                    let preview = format!("{:.5}", df.head(Some(10)));
                    let dataframe_info = info.iter()
                        .map(|c| format!("{}: {} (nulls: {})", c.name, c.dtype, c.null_count))
                        .collect::<Vec<_>>()
                        .join("\n");
                    self.datasets.push(LoadedDataset {
                        path: path.clone(),
                        dataframe: df,
                        layout,
                        column_info: info,
                        dataframe_info: dataframe_info.clone(),
                        preview_data: preview.clone(),
                    });
                    self.active_dataset_index = self.datasets.len() - 1;
                    self.tabs.data.file_path = path;
                    self.tabs.data.dataframe_info = dataframe_info;
                    self.tabs.data.preview_data = preview;
                }
                self.tabs.active = Tab::Data;
            }
            AppEvent::Analysis(request) => {
                let run_result = self.active_dataframe().map(|df| AnalysisRunner::run(df, request));
                if let Some(Ok(result)) = run_result {
                    let output = format!(
                        "{}\n\n{}",
                        result.summary,
                        result.details.as_deref().unwrap_or("No details")
                    );
                    self.tabs.analysis.results.push(output);
                    if let Some(ref viz_config) = result.viz_config {
                        if let Some(df) = self.active_dataframe() {
                            if let Ok(viz_data) = self.viz_engine.render(df, viz_config) {
                                self.tabs.viz.viz_output = viz_data.terminal_output;
                                self.tabs.viz.viz_title = viz_data.title;
                                self.tabs.viz.viz_svg_path = viz_data.svg_file_path;
                                self.tabs.viz.show_viz = true;
                            }
                        }
                    }
                    self.tabs.active = Tab::Analysis;
                }
            }
            AppEvent::ToggleViz(show) => {
                self.tabs.viz.show_viz = show;
            }
        }
        Ok(())
    }
}
