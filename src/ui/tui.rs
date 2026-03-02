use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use futures::StreamExt;
use ratatui::{
    backend::Backend,
    prelude::*,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};
use std::time::Duration;
use tokio::sync::mpsc;

use crate::{
    client::GlmClient,
    conversation::Conversation,
    data::{DataLoader, ColumnInfo, DataLayout, build_filtered_dataframe, coerce_expression_columns},
    runner::{AnalysisRequest, AnalysisRunner},
    tools::{get_all_tools, google_search},
    viz::{VisualizationEngine, available_visualizations},
    config::{Config, ConfigManager},
};
use super::components::{AgentFocus, AgentMessage, AgentStatus, AnalysisStatus, AppTabs, LoadStatus, Tab};
use super::loading::LoadingWidget;

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
    /// Selected genes (when Some, use these; else use all genes).
    selected_genes: Option<std::collections::HashSet<String>>,
    /// User-defined age groups for Young vs Old, e.g. Young=17-30, Old=40-60.
    age_groups: Option<Vec<crate::data::AgeGroupDef>>,
    should_quit: bool,
    input_mode: InputMode,
    file_dialog_state: FileDialogState,
    gene_selection: Option<GeneSelectionState>,
    data_tab_age_selector: Option<DataTabAgeSelectorState>,
    data_tab_gene_selector: Option<DataTabGeneSelectorState>,
    data_tab_focus: DataTabFocus,
    age_group_input: Option<String>,
    pending_analysis: Option<AnalysisRequest>,
    /// For loading animation
    loading_tick: u64,
    /// Pending input to send to Agent (when user presses Enter in Agent tab)
    pending_agent_input: Option<String>,
    /// Persistent conversation history for the AI (full context across turns)
    conversation: Conversation,
    /// Cached visible height of agent chat (for scroll calculations)
    agent_chat_visible_height: u16,
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

/// Focus on Data tab: TabBar (tabs at top), AgeSelector, or GeneSelector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DataTabFocus {
    TabBar,
    AgeSelector,
    GeneSelector,
}

/// Age selector embedded on Data tab: only available ages from layout, [x]:17 format. Scrollable list.
#[derive(Debug, Clone)]
struct DataTabAgeSelectorState {
    ages: Vec<String>,
    cursor: usize,
    scroll_offset: usize,
}

/// Gene selector embedded on Data tab: genes from layout, [x]:ENSG... format. Scrollable list.
#[derive(Debug, Clone)]
struct DataTabGeneSelectorState {
    genes: Vec<String>,
    cursor: usize,
    scroll_offset: usize,
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

    fn effective_genes(&self) -> Option<std::collections::HashSet<String>> {
        self.selected_genes.clone()
    }

    /// Returns the dataframe to use for analysis: filtered by selected genes and ages when layout
    /// is present, otherwise the full dataframe. This is the "locked in" data until cleared/reloaded.
    fn effective_dataframe(&self) -> Option<polars::prelude::DataFrame> {
        let df = self.active_dataframe()?;
        let layout = match self.active_layout() {
            Some(l) => l,
            None => return Some(df.clone()),
        };
        match build_filtered_dataframe(
            df,
            layout,
            self.selected_genes.as_ref(),
            self.selected_age_columns.as_ref(),
        ) {
            Ok(filtered) => Some(filtered),
            Err(_) => None,
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
            selected_genes: None,
            data_tab_gene_selector: None,
            age_groups: None,
            should_quit: false,
            input_mode: InputMode::Normal,
            file_dialog_state: FileDialogState::None,
            gene_selection: None,
            data_tab_age_selector: None,
            data_tab_focus: DataTabFocus::TabBar,
            age_group_input: None,
            pending_analysis: None,
            loading_tick: 0,
            pending_agent_input: None,
            conversation: Conversation::new(Self::system_prompt()),
            agent_chat_visible_height: 20,
        })
    }

    pub async fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        let (_event_tx, mut event_rx) = mpsc::unbounded_channel::<AppEvent>();

        loop {
            self.loading_tick = self.loading_tick.wrapping_add(1);
            self.draw(terminal)?;

            if self.should_quit {
                return Ok(());
            }

            if let Some(input) = self.pending_agent_input.take() {
                if !input.trim().is_empty() {
                    if let Err(e) = self.process_ai_turn(terminal, &input).await {
                        self.tabs.agent.messages.push(AgentMessage {
                            role: "error".to_string(),
                            content: format!("Error: {}", e),
                        });
                        self.tabs.agent.status = AgentStatus::Idle;
                        self.tabs.agent.loading_start = None;
                        self.tabs.agent.focus = AgentFocus::Input;
                    }
                }
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
                let (run_result, filter_err) = match self.effective_dataframe() {
                    Some(df) => (Some(AnalysisRunner::run(&df, request)), None),
                    None if self.active_dataframe().is_some() && self.active_layout().is_some() => {
                        let err = self.active_layout().and_then(|layout| {
                            build_filtered_dataframe(
                                self.active_dataframe().unwrap(),
                                layout,
                                self.selected_genes.as_ref(),
                                self.selected_age_columns.as_ref(),
                            )
                            .err()
                        });
                        (None, Some(err.map(|e| e.to_string()).unwrap_or_else(|| "No genes or age columns selected".to_string())))
                    }
                    None => (None, None),
                };
                if let Some(ref msg) = filter_err {
                    self.tabs.analysis.analysis_status = AnalysisStatus::Error(msg.clone());
                }
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
                            if let Some(df) = self.effective_dataframe() {
                                if let Ok(viz_data) = self.viz_engine.render(&df, viz_config) {
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
                    None if filter_err.is_none() => {
                        self.tabs.analysis.analysis_status =
                            AnalysisStatus::Error("No data loaded".to_string());
                    }
                    _ => {}
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

    fn draw<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        let visible_height: std::cell::RefCell<u16> =
            std::cell::RefCell::new(self.agent_chat_visible_height);
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
                Tab::Agent => {
                    let agent_chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Min(0), Constraint::Length(3)])
                        .split(chunks[1]);
                    let h = agent_chunks[0].height.saturating_sub(2);
                    *visible_height.borrow_mut() = if self.tabs.agent.status == AgentStatus::Processing
                    {
                        let inner = Layout::default()
                            .direction(Direction::Vertical)
                            .constraints([Constraint::Length(3), Constraint::Min(0)])
                            .split(agent_chunks[0]);
                        inner[1].height.saturating_sub(2)
                    } else {
                        h
                    };
                    self.render_agent_tab(f, chunks[1]);
                }
            }
        })?;
        self.agent_chat_visible_height = visible_height.into_inner();
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
                "Type or paste path • Enter to load • Esc to cancel",
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
            let has_gene_selector = self.data_tab_gene_selector.is_some();
            let has_datasets = !self.datasets.is_empty();
            let constraints: &[Constraint] = if has_datasets && has_age_selector && has_gene_selector {
                &[
                    Constraint::Length(3),
                    Constraint::Length(4),
                    Constraint::Length(3),
                    Constraint::Length(8),
                    Constraint::Length(8),
                    Constraint::Length(10),
                    Constraint::Length(12),
                    Constraint::Min(0),
                ]
            } else if has_datasets && (has_age_selector || has_gene_selector) {
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
            } else if has_age_selector || has_gene_selector {
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
            if has_age_selector {
                self.render_data_tab_age_selector(f, chunks[chunk_idx]);
                chunk_idx += 1;
            }
            if has_gene_selector {
                self.render_data_tab_gene_selector(f, chunks[chunk_idx]);
                chunk_idx += 1;
            }
            let bottom_area = chunks[chunk_idx];

            let info_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(10), Constraint::Min(0)].as_ref())
                .split(bottom_area);

            let (mut info_text, preview_text) = self.active_dataset()
                .map(|d| (d.dataframe_info.clone(), d.preview_data.clone()))
                .unwrap_or((
                    self.tabs.data.dataframe_info.clone(),
                    self.tabs.data.preview_data.clone(),
                ));
            if let Some(df) = self.effective_dataframe() {
                let n_genes = df.height();
                let n_cols = df.width().saturating_sub(1);
                if self.selected_genes.is_some() || self.selected_age_columns.is_some() {
                    info_text.push_str(&format!("\n\n--- Active filter (locked in) ---\n{} genes × {} age columns", n_genes, n_cols));
                }
            }
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

    fn render_agent_tab(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(area);

        let msg_area = chunks[0];
        let input_area = chunks[1];

        // Build chat content (messages + streaming) for both Processing and Idle
        let mut lines: Vec<String> = self.tabs.agent.messages
            .iter()
            .flat_map(|m| {
                let prefix = match m.role.as_str() {
                    "user" => "You: ",
                    "assistant" => "Agent: ",
                    _ => "",
                };
                format!("{}{}", prefix, m.content)
                    .lines()
                    .map(|l| format!("  {}", l))
                    .collect::<Vec<_>>()
            })
            .collect();
        if !self.tabs.agent.streaming_content.is_empty() {
            lines.push(format!("  Agent: {}", self.tabs.agent.streaming_content));
        }
        if lines.is_empty() && self.tabs.agent.status != AgentStatus::Processing {
            lines.push("Type your request in natural language. Examples:".to_string());
            lines.push("  • Load sample_data.csv and show summary".to_string());
            lines.push("  • Find genes significant with age".to_string());
            lines.push("  • Run expression trend for ENSG0000001".to_string());
            lines.push("  • Open the visualization in browser".to_string());
        }
        let content = lines.join("\n");
        let content_lines = content.lines().count() as u16;
        let visible_height = msg_area.height.saturating_sub(2);
        let max_scroll = content_lines.saturating_sub(visible_height).max(0);
        let scroll_offset = self.tabs.agent.scroll_offset.min(max_scroll);
        let skip = content_lines.saturating_sub(visible_height).saturating_sub(scroll_offset).max(0);

        if self.tabs.agent.status == AgentStatus::Processing {
            // Show streaming chat with loading indicator at top
            let inner = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(0)])
                .split(msg_area);
            let tick_ms = self.tabs.agent.loading_start
                .map(|s| s.elapsed().as_millis() as u64)
                .unwrap_or(self.loading_tick);
            LoadingWidget::new(tick_ms).render(
                inner[0],
                f.buffer_mut(),
            );
            let chat_title = " Agent │ ↑/↓ scroll ";
            Paragraph::new(content)
                .block(Block::default().borders(Borders::ALL).title(chat_title))
                .wrap(Wrap { trim: false })
                .scroll((0, skip))
                .render(inner[1], f.buffer_mut());
        } else {
            let page_info = if max_scroll > 0 {
                let pg = visible_height.max(1);
                let total_pages = ((content_lines + pg - 1) / pg).max(1);
                let current_page = (scroll_offset / pg + 1).min(total_pages);
                format!(" Page {}/{} ", current_page, total_pages)
            } else {
                String::new()
            };
            let chat_title = match self.tabs.agent.focus {
                AgentFocus::Chat => format!(" Agent │ ↑/↓ page │ Esc: back {}", page_info),
                AgentFocus::Input => " Agent │ Esc: scroll chat ".to_string(),
            };
            let chat_block = Block::default()
                .borders(Borders::ALL)
                .title(chat_title)
                .border_style(match self.tabs.agent.focus {
                    AgentFocus::Chat => Style::default().fg(Color::Cyan),
                    AgentFocus::Input => Style::default(),
                });
            Paragraph::new(content)
                .block(chat_block)
                .wrap(Wrap { trim: false })
                .scroll((0, skip))
                .render(msg_area, f.buffer_mut());
        }

        let input_display = if self.tabs.agent.input.is_empty() {
            "Type here... (Enter to send)"
        } else {
            &self.tabs.agent.input
        };
        let input_title = match self.tabs.agent.focus {
            AgentFocus::Input => " You │ Esc: scroll chat ",
            AgentFocus::Chat => " You ",
        };
        let input_block = Block::default()
            .borders(Borders::ALL)
            .title(input_title)
            .border_style(match self.tabs.agent.focus {
                AgentFocus::Input => Style::default().fg(Color::Cyan),
                AgentFocus::Chat => Style::default(),
            });
        Paragraph::new(input_display)
            .block(input_block)
            .render(input_area, f.buffer_mut());
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
        let visible_height = area.height.saturating_sub(3).max(1) as usize;
        let start = state.scroll_offset.min(state.ages.len().saturating_sub(1));
        let end = (start + visible_height).min(state.ages.len());
        let hint = if self.data_tab_focus == DataTabFocus::AgeSelector {
            "j/k move • g page down • G end • Home/End • X toggle • a=all u=none • Tab/↑/Esc return"
        } else {
            "↓ to enter age selection • Tab switches Data/Analysis/Viz"
        };
        let mut items = vec![ListItem::new(ratatui::text::Line::from(hint))];
        for (i, age) in state.ages[start..end].iter().enumerate() {
            let idx = start + i;
            let mark = if is_selected(age) { "x" } else { " " };
            let is_cursor = idx == state.cursor && self.data_tab_focus == DataTabFocus::AgeSelector;
            let display = format!("[{}]:{} ", mark, age);
            let style = if is_cursor {
                Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else if is_selected(age) {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            items.push(ListItem::new(display).style(style));
        }
        let n_sel = if all_selected {
            state.ages.len()
        } else {
            sel.map(|s| s.len()).unwrap_or(0)
        };
        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Age Selection (affects all analyses) — {} of {} selected ", n_sel, state.ages.len())),
        );
        Widget::render(list, area, f.buffer_mut());
    }

    fn render_data_tab_gene_selector(&self, f: &mut Frame, area: Rect) {
        let Some(ref state) = self.data_tab_gene_selector else { return };
        let all_selected = self.selected_genes.is_none();
        let sel = self.selected_genes.as_ref();
        let is_selected = |gene: &str| {
            if all_selected {
                true
            } else if let Some(s) = sel {
                s.contains(gene)
            } else {
                true
            }
        };
        // Reserve 1 line for hint, 2 for block borders/title
        let visible_height = area.height.saturating_sub(3).max(1) as usize;
        let start = state.scroll_offset.min(state.genes.len().saturating_sub(1));
        let end = (start + visible_height).min(state.genes.len());
        let hint = if self.data_tab_focus == DataTabFocus::GeneSelector {
            "j/k move • g page down • G end • Home/End • X toggle • a=all u=none • Tab/↑/Esc return"
        } else {
            "↓ to enter gene selection • Tab switches Data/Analysis/Viz"
        };
        let mut items = vec![ListItem::new(ratatui::text::Line::from(hint))];
        for (i, gene) in state.genes[start..end].iter().enumerate() {
            let idx = start + i;
            let mark = if is_selected(gene) { "x" } else { " " };
            let is_cursor = idx == state.cursor && self.data_tab_focus == DataTabFocus::GeneSelector;
            let display = format!("[{}] {} ", mark, gene.chars().take(28).collect::<String>());
            let style = if is_cursor {
                Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else if is_selected(gene) {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            items.push(ListItem::new(display).style(style));
        }
        let n_sel = if all_selected {
            state.genes.len()
        } else {
            sel.map(|s| s.len()).unwrap_or(0)
        };
        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Gene Selection (affects all analyses) — {} of {} selected ", n_sel, state.genes.len())),
        );
        Widget::render(list, area, f.buffer_mut());
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
        // Paste into file path input when loading file(s)
        if matches!(self.file_dialog_state, FileDialogState::AwaitingPath) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                self.tabs.data.file_path_input.push_str(trimmed);
            }
            return Ok(());
        }
        // Paste gene ID when in gene selection
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

        if key.code == KeyCode::Char('?') && self.input_mode == InputMode::Normal {
            self.tabs.show_help = !self.tabs.show_help;
            return Ok(());
        }

        if self.tabs.active == Tab::Agent
            && self.input_mode == InputMode::Normal
            && self.gene_selection.is_none()
            && self.age_group_input.is_none()
            && !matches!(self.file_dialog_state, FileDialogState::AwaitingPath)
        {
            let content_lines = self.agent_content_line_count();
            let page_size = self.agent_chat_visible_height.max(1);
            let max_scroll = content_lines.saturating_sub(page_size).max(0);

            match (self.tabs.agent.focus, key.code) {
                // Chat focus: page-based scroll and Esc to exit
                (AgentFocus::Chat, KeyCode::Esc) => {
                    self.tabs.agent.focus = AgentFocus::Input;
                    return Ok(());
                }
                (AgentFocus::Chat, KeyCode::Up | KeyCode::PageUp) => {
                    // Previous page (older content)
                    self.tabs.agent.scroll_offset =
                        (self.tabs.agent.scroll_offset + page_size).min(max_scroll);
                    return Ok(());
                }
                (AgentFocus::Chat, KeyCode::Down | KeyCode::PageDown) => {
                    // Next page (newer content)
                    self.tabs.agent.scroll_offset =
                        self.tabs.agent.scroll_offset.saturating_sub(page_size);
                    return Ok(());
                }
                (AgentFocus::Chat, KeyCode::Home) => {
                    self.tabs.agent.scroll_offset = max_scroll;
                    return Ok(());
                }
                (AgentFocus::Chat, KeyCode::End) => {
                    self.tabs.agent.scroll_offset = 0;
                    return Ok(());
                }
                // Input focus: Esc to enter chat (scroll mode)
                (AgentFocus::Input, KeyCode::Esc) => {
                    self.tabs.agent.focus = AgentFocus::Chat;
                    return Ok(());
                }
                // Input focus: typing
                (AgentFocus::Input, KeyCode::Enter) => {
                    let input = self.tabs.agent.input.clone();
                    self.tabs.agent.input.clear();
                    if !input.trim().is_empty() {
                        self.tabs.agent.messages.push(AgentMessage {
                            role: "user".to_string(),
                            content: input.clone(),
                        });
                        self.tabs.agent.status = AgentStatus::Processing;
                        self.tabs.agent.streaming_content.clear();
                        self.tabs.agent.scroll_offset = 0;
                        self.tabs.agent.loading_start = Some(std::time::Instant::now());
                        self.pending_agent_input = Some(input);
                    }
                    return Ok(());
                }
                (AgentFocus::Input, KeyCode::Backspace) => {
                    self.tabs.agent.input.pop();
                    return Ok(());
                }
                (AgentFocus::Input, KeyCode::Char(c)) => {
                    self.tabs.agent.input.push(c);
                    return Ok(());
                }
                _ => {}
            }
        }

        if self.tabs.show_help {
            if key.code == KeyCode::Esc {
                self.tabs.show_help = false;
            }
            return Ok(());
        }

        if self.tabs.active == Tab::Data
            && (self.data_tab_age_selector.is_some() || self.data_tab_gene_selector.is_some())
            && !matches!(self.file_dialog_state, FileDialogState::AwaitingPath)
        {
            let has_age_selector = self.data_tab_age_selector.is_some();
            let has_gene_selector = self.data_tab_gene_selector.is_some();

            if self.data_tab_focus == DataTabFocus::TabBar {
                if has_age_selector && key.code == KeyCode::Down {
                    self.data_tab_focus = DataTabFocus::AgeSelector;
                    return Ok(());
                }
                if has_gene_selector && !has_age_selector && key.code == KeyCode::Down {
                    self.data_tab_focus = DataTabFocus::GeneSelector;
                    return Ok(());
                }
            }

            if self.data_tab_focus == DataTabFocus::AgeSelector {
                if key.code == KeyCode::Tab || key.code == KeyCode::Up || key.code == KeyCode::Esc {
                    self.data_tab_focus = DataTabFocus::TabBar;
                    return Ok(());
                }
                if key.code == KeyCode::Down && has_gene_selector {
                    self.data_tab_focus = DataTabFocus::GeneSelector;
                    return Ok(());
                }
                if let Some(ref mut state) = self.data_tab_age_selector {
                    const VISIBLE_HEIGHT: usize = 9;
                    let len = state.ages.len();
                    let mut consumed = true;
                    match key.code {
                        KeyCode::Char('k') => {
                            state.cursor = state.cursor.saturating_sub(1);
                            if state.cursor < state.scroll_offset {
                                state.scroll_offset = state.cursor;
                            }
                        }
                        KeyCode::Char('j') => {
                            state.cursor = (state.cursor + 1).min(len.saturating_sub(1));
                            if state.cursor >= state.scroll_offset + VISIBLE_HEIGHT {
                                state.scroll_offset = state.cursor - VISIBLE_HEIGHT + 1;
                            }
                        }
                        KeyCode::Char('g') => {
                            state.cursor = (state.cursor + VISIBLE_HEIGHT).min(len.saturating_sub(1));
                            if state.cursor >= state.scroll_offset + VISIBLE_HEIGHT {
                                state.scroll_offset = state.cursor - VISIBLE_HEIGHT + 1;
                            }
                        }
                        KeyCode::Char('G') => {
                            state.cursor = len.saturating_sub(1);
                            state.scroll_offset = len.saturating_sub(VISIBLE_HEIGHT).max(0);
                        }
                        KeyCode::Home => {
                            state.cursor = 0;
                            state.scroll_offset = 0;
                        }
                        KeyCode::End => {
                            state.cursor = len.saturating_sub(1);
                            state.scroll_offset = len.saturating_sub(VISIBLE_HEIGHT).max(0);
                        }
                        KeyCode::PageDown => {
                            state.cursor = (state.cursor + VISIBLE_HEIGHT).min(len.saturating_sub(1));
                            if state.cursor >= state.scroll_offset + VISIBLE_HEIGHT {
                                state.scroll_offset = state.cursor - VISIBLE_HEIGHT + 1;
                            }
                        }
                        KeyCode::PageUp => {
                            state.cursor = state.cursor.saturating_sub(VISIBLE_HEIGHT);
                            if state.cursor < state.scroll_offset {
                                state.scroll_offset = state.cursor;
                            }
                        }
                        KeyCode::Char('a') => {
                            self.selected_age_columns = None;
                        }
                        KeyCode::Char('u') => {
                            self.selected_age_columns = Some(std::collections::HashSet::new());
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

            if self.data_tab_focus == DataTabFocus::GeneSelector {
                if key.code == KeyCode::Tab || key.code == KeyCode::Esc {
                    self.data_tab_focus = if has_age_selector {
                        DataTabFocus::AgeSelector
                    } else {
                        DataTabFocus::TabBar
                    };
                    return Ok(());
                }
                if key.code == KeyCode::Up {
                    self.data_tab_focus = if has_age_selector {
                        DataTabFocus::AgeSelector
                    } else {
                        DataTabFocus::TabBar
                    };
                    return Ok(());
                }
                if let Some(ref mut state) = self.data_tab_gene_selector {
                    let len = state.genes.len();
                    // Match render: area height 12 - 3 (hint + borders) = 9 visible rows
                    const VISIBLE_HEIGHT: usize = 9;
                    let mut consumed = true;
                    match key.code {
                        KeyCode::Char('k') => {
                            state.cursor = state.cursor.saturating_sub(1);
                            if state.cursor < state.scroll_offset {
                                state.scroll_offset = state.cursor;
                            }
                        }
                        KeyCode::Char('j') => {
                            state.cursor = (state.cursor + 1).min(len.saturating_sub(1));
                            if state.cursor >= state.scroll_offset + VISIBLE_HEIGHT {
                                state.scroll_offset = state.cursor - VISIBLE_HEIGHT + 1;
                            }
                        }
                        KeyCode::Char('g') => {
                            // Page down
                            state.cursor = (state.cursor + VISIBLE_HEIGHT).min(len.saturating_sub(1));
                            if state.cursor >= state.scroll_offset + VISIBLE_HEIGHT {
                                state.scroll_offset = state.cursor - VISIBLE_HEIGHT + 1;
                            }
                        }
                        KeyCode::Char('G') => {
                            // Go to end
                            state.cursor = len.saturating_sub(1);
                            state.scroll_offset = len.saturating_sub(VISIBLE_HEIGHT).max(0);
                        }
                        KeyCode::Home => {
                            state.cursor = 0;
                            state.scroll_offset = 0;
                        }
                        KeyCode::End => {
                            state.cursor = len.saturating_sub(1);
                            state.scroll_offset = len.saturating_sub(VISIBLE_HEIGHT).max(0);
                        }
                        KeyCode::PageDown => {
                            state.cursor = (state.cursor + VISIBLE_HEIGHT).min(len.saturating_sub(1));
                            if state.cursor >= state.scroll_offset + VISIBLE_HEIGHT {
                                state.scroll_offset = state.cursor - VISIBLE_HEIGHT + 1;
                            }
                        }
                        KeyCode::PageUp => {
                            state.cursor = state.cursor.saturating_sub(VISIBLE_HEIGHT);
                            if state.cursor < state.scroll_offset {
                                state.scroll_offset = state.cursor;
                            }
                        }
                        KeyCode::Char('a') => {
                            // Select all: use all genes (no filter)
                            self.selected_genes = None;
                        }
                        KeyCode::Char('u') => {
                            // Unselect all: clear selection (empty = no genes, user can re-select)
                            self.selected_genes = Some(std::collections::HashSet::new());
                        }
                        KeyCode::Char('x') | KeyCode::Char('X') => {
                            if let Some(gene) = state.genes.get(state.cursor).cloned() {
                                let all_genes: std::collections::HashSet<String> =
                                    state.genes.iter().cloned().collect();
                                match &self.selected_genes {
                                    None => {
                                        let mut new_sel = all_genes.clone();
                                        new_sel.remove(&gene);
                                        self.selected_genes = Some(new_sel);
                                    }
                                    Some(s) => {
                                        let mut new_sel = s.clone();
                                        if new_sel.contains(&gene) {
                                            new_sel.remove(&gene);
                                        } else {
                                            new_sel.insert(gene.clone());
                                        }
                                        self.selected_genes = if new_sel == all_genes {
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
                    let age_columns = self.effective_age_columns(&layout);
                    self.gene_selection = None;
                    if !selected.is_empty() {
                        match action {
                            GeneSelectionAction::ExpressionTrend => {
                                self.pending_analysis = Some(AnalysisRequest::ExpressionTrend {
                                    gene_ids: selected,
                                    gene_column: layout.gene_column,
                                    age_columns,
                                });
                                self.tabs.analysis.analysis_status =
                                    AnalysisStatus::Loading;
                            }
                            GeneSelectionAction::ExpressionVsAgeRegression => {
                                self.pending_analysis = Some(AnalysisRequest::ExpressionVsAgeRegression {
                                    gene_ids: selected,
                                    gene_column: layout.gene_column,
                                    age_columns,
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
                    let layout = self.datasets[idx].layout.as_ref();
                    let ages = layout.map(|l| l.age_columns.clone());
                    let genes = layout.and_then(|l| {
                        self.datasets[idx].dataframe.column(&l.gene_column).ok()
                            .and_then(|c| c.str().ok())
                            .map(|s| s.into_iter().filter_map(|o| o.map(str::to_string)).collect::<Vec<_>>())
                    });
                    self.active_dataset_index = idx;
                    self.tabs.data.file_path = path;
                    self.tabs.data.dataframe_info = info;
                    self.tabs.data.preview_data = preview;
                    self.data_tab_age_selector = ages.map(|a| DataTabAgeSelectorState {
                        ages: a,
                        cursor: 0,
                        scroll_offset: 0,
                    });
                    self.data_tab_gene_selector = genes.map(|g| DataTabGeneSelectorState {
                        genes: g,
                        cursor: 0,
                        scroll_offset: 0,
                    });
                    self.selected_genes = None; // reset when switching datasets
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
                        let layout = self.datasets[idx].layout.as_ref();
                        let ages = layout.map(|l| l.age_columns.clone());
                        let genes = layout.and_then(|l| {
                            self.datasets[idx].dataframe.column(&l.gene_column).ok()
                                .and_then(|c| c.str().ok())
                                .map(|s| s.into_iter().filter_map(|o| o.map(str::to_string)).collect::<Vec<_>>())
                        });
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
                            scroll_offset: 0,
                        });
                        self.data_tab_gene_selector = genes.map(|g| DataTabGeneSelectorState {
                            genes: g,
                            cursor: 0,
                            scroll_offset: 0,
                        });
                        self.selected_genes = None;
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
                            gene_filter: None, // df is pre-filtered by effective_dataframe
                        });
                        self.tabs.analysis.analysis_status =
                            AnalysisStatus::PendingConfirm { request: "Summary statistics (mean, median, mode, R², p-value, correlation)".to_string() };
                    }
                    'r' if avail.map(|v| v.available).unwrap_or(false) => {
                        if let Some(layout) = self.active_layout() {
                            self.pending_analysis = Some(AnalysisRequest::GenesExpressionVsAge {
                                gene_column: layout.gene_column.clone(),
                                age_columns: self.effective_age_columns(layout),
                                gene_filter: None, // df is pre-filtered
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
                                gene_filter: None, // df is pre-filtered
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
                                let bins: u32 = ConfigManager::load_config().map(|c| c.default_bins).unwrap_or(20);
                                self.pending_analysis = Some(AnalysisRequest::Histogram {
                                    column: col.clone(),
                                    bins: bins.try_into().unwrap(),
                                });
                                self.tabs.analysis.analysis_status =
                                    AnalysisStatus::PendingConfirm { request: format!("Histogram: {} ({} bins)", col, bins) };
                            }
                        }
                    }
                    '1' if avail.map(|v| v.available).unwrap_or(false) => {
                        if let Some(layout) = self.active_layout() {
                            self.pending_analysis = Some(AnalysisRequest::GenesVolcanoPlot {
                                gene_column: layout.gene_column.clone(),
                                age_columns: self.effective_age_columns(layout),
                                gene_filter: None,
                            });
                            self.tabs.analysis.analysis_status =
                                AnalysisStatus::PendingConfirm { request: "Volcano plot (significance vs effect size)".to_string() };
                        }
                    }
                    't' if avail.map(|v| v.available).unwrap_or(false) => {
                        if let Some(layout) = self.active_layout() {
                            let genes: Vec<String> = self.selected_genes.as_ref()
                                .map(|s| s.iter().cloned().collect())
                                .unwrap_or_else(|| self.data_tab_gene_selector.as_ref()
                                    .map(|g| g.genes.clone())
                                    .unwrap_or_default());
                            if !genes.is_empty() {
                                self.tabs.viz.viz_output.clear();
                                self.tabs.viz.viz_title.clear();
                                self.tabs.viz.viz_svg_path = None;
                                self.gene_selection = Some(GeneSelectionState {
                                    genes,
                                    selected: std::collections::HashSet::new(),
                                    cursor: 0,
                                    max_select: 5,
                                    action: GeneSelectionAction::ExpressionTrend,
                                    search_input: None,
                                });
                            }
                        }
                    }
                    'v' if avail.map(|v| v.available).unwrap_or(false) => {
                        if let Some(layout) = self.active_layout() {
                            let (young_cols, old_cols) = self.age_groups.as_ref()
                                .and_then(|g| {
                                    let age_cols = self.effective_age_columns(layout);
                                    let parts = crate::data::partition_ages_by_groups(&age_cols, g);
                                    if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
                                        Some((Some(parts[0].clone()), Some(parts[1].clone())))
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or((None, None));
                            self.pending_analysis = Some(AnalysisRequest::YoungVsOld {
                                gene_column: layout.gene_column.clone(),
                                age_columns: self.effective_age_columns(layout),
                                young_cols,
                                old_cols,
                            });
                            self.tabs.analysis.analysis_status =
                                AnalysisStatus::PendingConfirm { request: "Young vs Old scatter".to_string() };
                        }
                    }
                    'e' if avail.map(|v| v.available).unwrap_or(false) => {
                        if let Some(layout) = self.active_layout() {
                            self.pending_analysis = Some(AnalysisRequest::ExpressionHeatmap {
                                gene_column: layout.gene_column.clone(),
                                age_columns: self.effective_age_columns(layout),
                                top_n: 50,
                            });
                            self.tabs.analysis.analysis_status =
                                AnalysisStatus::PendingConfirm { request: "Expression heatmap (genes × ages)".to_string() };
                        }
                    }
                    'x' if avail.map(|v| v.available).unwrap_or(false) => {
                        if let Some(layout) = self.active_layout() {
                            self.pending_analysis = Some(AnalysisRequest::ExportGeneCorrelation {
                                gene_column: layout.gene_column.clone(),
                                age_columns: self.effective_age_columns(layout),
                            });
                            self.tabs.analysis.analysis_status =
                                AnalysisStatus::PendingConfirm { request: "Export gene correlation to CSV".to_string() };
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

    async fn process_ai_turn<B: Backend>(
        &mut self,
        terminal: &mut ratatui::Terminal<B>,
        user_input: &str,
    ) -> Result<()> {
        let config = ConfigManager::load_config()?;
        let api_key = config
            .api_key
            .clone()
            .ok_or_else(|| anyhow::anyhow!("ZAI_API_KEY not set. Add it to .env or config."))?;
        let base_url = config.api_base_url.unwrap_or_else(|| {
            "https://api.z.ai/api/coding/paas/v4".to_string()
        });
        let model = config.model.unwrap_or_else(|| "glm-4.7-flash".to_string());

        let client = GlmClient::new(api_key, base_url, model);

        let app_context = self.build_app_context();
        let message_with_context = if app_context.is_empty() {
            user_input.to_string()
        } else {
            format!(
                "[APP CONTEXT — you have full visibility into the application state]\n{}\n\n[USER REQUEST]\n{}",
                app_context,
                user_input
            )
        };
        self.conversation.add_user_message(&message_with_context);

        let tools = get_all_tools();
        self.process_ai_response(terminal, &client, &tools)
            .await
    }

    fn system_prompt() -> String {
        r#"You are the R-Data Agent, the orchestrator of the entire R-Data application. You have full context and access to everything happening in the app.

Your role: Guide users through loading data, running analyses, and visualizing results using natural language. You receive APP CONTEXT with each message describing: loaded datasets, active dataset info, recent analyses, current visualization, and user actions across Data/Analysis/Viz tabs.

You have tools to: load_data, get_data_info, get_app_context, list_available_analyses, run_summary_stats, run_correlation, run_histogram, run_expression_vs_age, run_genes_significant_with_age, run_expression_trend, run_young_vs_old, run_volcano_plot, run_expression_heatmap, export_gene_correlation, open_visualization, google_search.

Workflow: Use the APP CONTEXT to understand what the user has already done. Load data first if needed, then run analyses. Use list_available_analyses or get_app_context to see current state.
Be concise. When you run an analysis, summarize the result for the user.
For microarray data: Gene ID in column A, ages as column headers. Expression values are log-normalised."#
            .to_string()
    }

    /// Count lines in Agent chat content (for scroll)
    fn agent_content_line_count(&self) -> u16 {
        let mut count = 0u16;
        for m in &self.tabs.agent.messages {
            let prefix = match m.role.as_str() {
                "user" => "You: ",
                "assistant" => "Agent: ",
                _ => "",
            };
            count += format!("{}{}", prefix, m.content).lines().count() as u16;
        }
        if !self.tabs.agent.streaming_content.is_empty() {
            count += format!("  Agent: {}", self.tabs.agent.streaming_content).lines().count() as u16;
        }
        if count == 0 {
            count = 5;
        }
        count
    }

    /// Build a string describing current app state for Agent context
    fn build_app_context(&self) -> String {
        let mut lines = Vec::new();

        lines.push(format!("Current tab: {}", self.tabs.active.as_str()));

        if self.datasets.is_empty() {
            lines.push("Datasets: None loaded.".to_string());
        } else {
            lines.push(format!("Datasets loaded: {} (active: #{})", self.datasets.len(), self.active_dataset_index + 1));
            if let Some(ds) = self.active_dataset() {
                let path = std::path::Path::new(&ds.path).file_name().and_then(|n| n.to_str()).unwrap_or(&ds.path);
                lines.push(format!("  Active file: {}", path));
                lines.push(format!("  Rows: {}", ds.dataframe.height()));
                if let Some(ref layout) = ds.layout {
                    lines.push(format!("  Layout: {} genes × {} age columns (range {}-{})", layout.gene_count, layout.age_columns.len(), layout.age_min, layout.age_max));
                }
                lines.push(format!("  Info: {}", ds.dataframe_info.lines().next().unwrap_or("").to_string()));
            }
        }

        if !self.tabs.analysis.results.is_empty() {
            lines.push(format!("Recent analyses: {} result(s)", self.tabs.analysis.results.len()));
            if let Some(last) = self.tabs.analysis.results.last() {
                let preview: String = last.lines().take(2).collect::<Vec<_>>().join(" ");
                lines.push(format!("  Last: {}...", preview.chars().take(60).collect::<String>()));
            }
        }

        if self.tabs.viz.show_viz && !self.tabs.viz.viz_title.is_empty() {
            lines.push(format!("Current visualization: {}", self.tabs.viz.viz_title));
            if self.tabs.viz.viz_svg_path.is_some() {
                lines.push("  (SVG available — use open_visualization to view in browser)".to_string());
            }
        }

        if self.selected_genes.is_some() || self.selected_age_columns.is_some() {
            lines.push("Filters: Gene/age selection active (subset of data in use).".to_string());
        }
        if self.age_groups.is_some() {
            lines.push("Age groups: User-defined Young/Old groups set.".to_string());
        }

        lines.join("\n")
    }

    async fn process_ai_response<B: Backend>(
        &mut self,
        terminal: &mut ratatui::Terminal<B>,
        client: &GlmClient,
        tools: &[crate::client::glm::Tool],
    ) -> Result<()> {
        use crate::client::glm::{Message, ToolCall};

        loop {
            let mut stream = client
                .chat_stream(self.conversation.get_messages(), Some(tools.to_vec()))
                .await?;

            let mut content = String::new();
            let mut tool_calls: Vec<ToolCall> = Vec::new();

            while let Some(chunk_result) = stream.next().await {
                let chunk = chunk_result?;
                if let Some(choice) = chunk.choices.first() {
                    if let Some(c) = &choice.delta.content {
                        content.push_str(c);
                        self.tabs.agent.streaming_content = content.clone();
                        self.tabs.agent.scroll_offset = 0; // Keep at bottom during streaming
                    let _ = terminal.draw(|f| {
                        let chunks = Layout::default()
                            .direction(Direction::Vertical)
                            .constraints([Constraint::Length(5), Constraint::Min(0)])
                            .split(f.area());
                        self.tabs.render_tabs(chunks[0], f.buffer_mut());
                        self.render_agent_tab(f, chunks[1]);
                    });
                    }
                    if let Some(tcs) = &choice.delta.tool_calls {
                        for tc in tcs {
                            while tool_calls.len() <= tc.index {
                                tool_calls.push(ToolCall {
                                    id: String::new(),
                                    call_type: "function".to_string(),
                                    function: crate::client::glm::FunctionCall {
                                        name: String::new(),
                                        arguments: String::new(),
                                    },
                                });
                            }
                            if let Some(id) = &tc.id {
                                tool_calls[tc.index].id.push_str(id);
                            }
                            if let Some(f) = &tc.function {
                                if let Some(n) = &f.name {
                                    tool_calls[tc.index].function.name.push_str(n);
                                }
                                if let Some(a) = &f.arguments {
                                    tool_calls[tc.index].function.arguments.push_str(a);
                                }
                            }
                        }
                    }
                }
            }

            self.tabs.agent.streaming_content.clear();
            let final_content = content;

            let assistant_msg = Message {
                role: "assistant".to_string(),
                content: if final_content.is_empty() {
                    None
                } else {
                    Some(final_content.clone())
                },
                tool_calls: if tool_calls.is_empty() {
                    None
                } else {
                    Some(tool_calls.clone())
                },
                tool_call_id: None,
            };
            self.conversation.add_assistant_message(assistant_msg);

            if !final_content.is_empty() {
                self.tabs.agent.messages.push(AgentMessage {
                    role: "assistant".to_string(),
                    content: final_content,
                });
                self.tabs.agent.scroll_offset = 0; // Scroll to bottom to show new response
            }

            if tool_calls.is_empty() {
                break;
            }

            for tc in &tool_calls {
                if tc.function.name.is_empty() {
                    continue;
                }
                let result = self
                    .execute_tool(&tc.function.name, &tc.function.arguments)
                    .await;
                let result_str = result.unwrap_or_else(|e| format!("Error: {}", e));
                self.conversation.add_tool_result(&tc.id, &result_str);
            }
            self.tabs.agent.streaming_content = "Processing results...".to_string();
            let _ = terminal.draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(5), Constraint::Min(0)])
                    .split(f.area());
                self.tabs.render_tabs(chunks[0], f.buffer_mut());
                self.render_agent_tab(f, chunks[1]);
            });
        }

        self.tabs.agent.status = AgentStatus::Idle;
        self.tabs.agent.loading_start = None;
        self.tabs.agent.focus = AgentFocus::Input;
        Ok(())
    }

    async fn execute_tool(&mut self, name: &str, arguments: &str) -> Result<String> {
        let args: serde_json::Value =
            serde_json::from_str(arguments).unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

        match name {
            "load_data" => {
                let paths: Vec<String> = args["file_paths"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                let expanded: Vec<String> = paths
                    .iter()
                    .map(|p| shellexpand::tilde(p).to_string())
                    .collect();
                let mut loaded = 0;
                let mut errors = Vec::new();
                for path in &expanded {
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
                                    "Microarray: {} genes, {} age columns (range {}-{})",
                                    l.gene_count,
                                    l.age_columns.len(),
                                    l.age_min,
                                    l.age_max
                                )
                            } else {
                                info.iter()
                                    .map(|c| format!("{}: {}", c.name, c.dtype))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            };
                            self.datasets.push(LoadedDataset {
                                path: path.clone(),
                                dataframe: df,
                                layout,
                                column_info: info,
                                dataframe_info: dataframe_info.clone(),
                                preview_data: preview,
                            });
                            loaded += 1;
                        }
                        Err(e) => errors.push(format!("{}: {}", path, e)),
                    }
                }
                self.active_dataset_index = self.datasets.len().saturating_sub(1);
                if loaded > 0 {
                    let idx = self.active_dataset_index;
                    let path = self.datasets[idx].path.clone();
                    let info = self.datasets[idx].dataframe_info.clone();
                    let preview = self.datasets[idx].preview_data.clone();
                    let layout = self.datasets[idx].layout.as_ref();
                    self.tabs.data.file_path = path;
                    self.tabs.data.dataframe_info = info;
                    self.tabs.data.preview_data = preview;
                    self.data_tab_age_selector = layout.map(|l| DataTabAgeSelectorState {
                        ages: l.age_columns.clone(),
                        cursor: 0,
                        scroll_offset: 0,
                    });
                    self.data_tab_gene_selector = layout.and_then(|l| {
                        self.datasets[idx].dataframe
                            .column(&l.gene_column)
                            .ok()
                            .and_then(|c| c.str().ok())
                            .map(|s| {
                                s.into_iter()
                                    .filter_map(|o| o.map(str::to_string))
                                    .collect::<Vec<_>>()
                            })
                            .map(|g| DataTabGeneSelectorState {
                                genes: g,
                                cursor: 0,
                                scroll_offset: 0,
                            })
                    });
                }
                Ok(if errors.is_empty() {
                    format!("Loaded {} file(s) successfully.", loaded)
                } else {
                    format!("Loaded {}. Errors: {}", loaded, errors.join("; "))
                })
            }
            "get_data_info" => {
                let info = self
                    .active_dataset()
                    .map(|d| d.dataframe_info.clone())
                    .unwrap_or_else(|| "No data loaded.".to_string());
                Ok(info)
            }
            "get_app_context" => Ok(self.build_app_context()),
            "list_available_analyses" => {
                let viz_list = available_visualizations(
                    self.active_dataframe(),
                    self.active_layout(),
                );
                let text: String = viz_list
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
                Ok(format!("Available analyses:\n{}", text))
            }
            "run_summary_stats" => {
                let layout = self.active_layout();
                let gene_age = layout.map(|l| {
                    (l.gene_column.clone(), self.effective_age_columns(l))
                });
                let req = AnalysisRequest::SummaryStats {
                    gene_age_summary: gene_age,
                    gene_filter: None,
                };
                self.run_analysis_tool(req).await
            }
            "run_correlation" => self.run_analysis_tool(AnalysisRequest::Correlation).await,
            "run_histogram" => {
                let mut col = args["column"].as_str().unwrap_or("").to_string();
                if col.is_empty() {
                    if let Some(df) = self.active_dataframe() {
                        col = df
                            .get_columns()
                            .iter()
                            .find(|c| c.dtype().is_numeric())
                            .map(|c| c.name().to_string())
                            .unwrap_or_default();
                    }
                }
                let bins = args["bins"].as_u64().unwrap_or(20) as usize;
                self.run_analysis_tool(AnalysisRequest::Histogram {
                    column: col,
                    bins,
                })
                .await
            }
            "run_expression_vs_age" => {
                if let Some(layout) = self.active_layout() {
                    let req = AnalysisRequest::GenesExpressionVsAge {
                        gene_column: layout.gene_column.clone(),
                        age_columns: self.effective_age_columns(layout),
                        gene_filter: None,
                    };
                    self.run_analysis_tool(req).await
                } else {
                    Ok("Need microarray layout (Gene ID × age columns).".to_string())
                }
            }
            "run_genes_significant_with_age" => {
                if let Some(layout) = self.active_layout() {
                    let req = AnalysisRequest::GenesSignificantWithAge {
                        gene_column: layout.gene_column.clone(),
                        age_columns: self.effective_age_columns(layout),
                        gene_filter: None,
                    };
                    self.run_analysis_tool(req).await
                } else {
                    Ok("Need microarray layout.".to_string())
                }
            }
            "run_expression_trend" => {
                let gene_ids: Vec<String> = args["gene_ids"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                if let Some(layout) = self.active_layout() {
                    let req = AnalysisRequest::ExpressionTrend {
                        gene_ids,
                        gene_column: layout.gene_column.clone(),
                        age_columns: self.effective_age_columns(layout),
                    };
                    self.run_analysis_tool(req).await
                } else {
                    Ok("Need microarray layout.".to_string())
                }
            }
            "run_young_vs_old" => {
                if let Some(layout) = self.active_layout() {
                    let young_str = args["young_ages"].as_str().unwrap_or("");
                    let old_str = args["old_ages"].as_str().unwrap_or("");
                    let (young_cols, old_cols) = if !young_str.is_empty() && !old_str.is_empty() {
                        if let Some(groups) =
                            crate::data::parse_age_groups(&format!("Young={},Old={}", young_str, old_str))
                        {
                            let age_cols = self.effective_age_columns(layout);
                            let parts = crate::data::partition_ages_by_groups(&age_cols, &groups);
                            if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
                                (Some(parts[0].clone()), Some(parts[1].clone()))
                            } else {
                                (None, None)
                            }
                        } else {
                            (None, None)
                        }
                    } else {
                        (None, None)
                    };
                    let req = AnalysisRequest::YoungVsOld {
                        gene_column: layout.gene_column.clone(),
                        age_columns: self.effective_age_columns(layout),
                        young_cols,
                        old_cols,
                    };
                    self.run_analysis_tool(req).await
                } else {
                    Ok("Need microarray layout.".to_string())
                }
            }
            "run_volcano_plot" => {
                if let Some(layout) = self.active_layout() {
                    let req = AnalysisRequest::GenesVolcanoPlot {
                        gene_column: layout.gene_column.clone(),
                        age_columns: self.effective_age_columns(layout),
                        gene_filter: None,
                    };
                    self.run_analysis_tool(req).await
                } else {
                    Ok("Need microarray layout.".to_string())
                }
            }
            "run_expression_heatmap" => {
                if let Some(layout) = self.active_layout() {
                    let top_n = args["top_n"].as_u64().unwrap_or(50) as usize;
                    let req = AnalysisRequest::ExpressionHeatmap {
                        gene_column: layout.gene_column.clone(),
                        age_columns: self.effective_age_columns(layout),
                        top_n,
                    };
                    self.run_analysis_tool(req).await
                } else {
                    Ok("Need microarray layout.".to_string())
                }
            }
            "export_gene_correlation" => {
                if let Some(layout) = self.active_layout() {
                    let req = AnalysisRequest::ExportGeneCorrelation {
                        gene_column: layout.gene_column.clone(),
                        age_columns: self.effective_age_columns(layout),
                    };
                    self.run_analysis_tool(req).await
                } else {
                    Ok("Need microarray layout.".to_string())
                }
            }
            "open_visualization" => {
                if let Some(ref path) = self.tabs.viz.viz_svg_path {
                    let _ = opener::open(path);
                    Ok("Opened visualization in browser.".to_string())
                } else {
                    Ok("No visualization to open. Run an analysis first.".to_string())
                }
            }
            "google_search" => {
                let query = args["query"].as_str().unwrap_or("");
                let num = args["num_results"].as_u64().unwrap_or(10) as usize;
                google_search(query, num).await
            }
            _ => Ok(format!("Unknown tool: {}", name)),
        }
    }

    async fn run_analysis_tool(&mut self, request: AnalysisRequest) -> Result<String> {
        let df = match self.effective_dataframe() {
            Some(d) => d,
            None => {
                return Ok("No data loaded or filter produced empty dataset.".to_string());
            }
        };
        match AnalysisRunner::run(&df, request) {
            Ok(result) => {
                let output = format!(
                    "{}\n\n{}",
                    result.summary,
                    result.details.as_deref().unwrap_or("")
                );
                self.tabs.analysis.results.push(output.clone());
                if let Some(ref viz_config) = result.viz_config {
                    if let Ok(viz_data) = self.viz_engine.render(&df, viz_config) {
                        self.tabs.viz.viz_output = viz_data.terminal_output;
                        self.tabs.viz.viz_title = viz_data.title;
                        self.tabs.viz.viz_svg_path = viz_data.svg_file_path;
                        self.tabs.viz.show_viz = true;
                    }
                }
                Ok(output)
            }
            Err(e) => Ok(format!("Analysis error: {}", e)),
        }
    }
}
