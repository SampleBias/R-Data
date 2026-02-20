use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    execute,
};
use ratatui::{
    backend::Backend,
    prelude::*,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap, Scrollbar, ScrollbarOrientation},
};
use std::time::Duration;
use tokio::sync::mpsc;

use crate::{
    data::{DataLoader, ColumnInfo},
    ai::{AIAgent, AnalysisRequest},
    viz::{VisualizationEngine},
    config::ConfigManager,
};
use super::components::{AppTabs, LoadStatus, Tab};

pub struct App {
    tabs: AppTabs,
    agent: AIAgent,
    viz_engine: VisualizationEngine,
    dataframe: Option<polars::prelude::DataFrame>,
    column_info: Vec<ColumnInfo>,
    should_quit: bool,
    input_mode: InputMode,
    file_dialog_state: FileDialogState,
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

pub enum AppEvent {
    LoadData(String),
    Analysis(AnalysisRequest),
    AIPrompt(String),
    ToggleViz(bool),
}

impl App {
    pub fn new(api_key: Option<String>) -> Result<Self> {
        let config = ConfigManager::load_config()?;
        let viz_engine = VisualizationEngine::new(config.viz_width, config.viz_height);
        
        Ok(Self {
            tabs: AppTabs::default(),
            agent: AIAgent::new(api_key),
            viz_engine,
            dataframe: None,
            column_info: Vec::new(),
            should_quit: false,
            input_mode: InputMode::Normal,
            file_dialog_state: FileDialogState::None,
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

            match self.tabs.active {
                Tab::Data => self.render_data_tab(f, chunks[1]),
                Tab::Analysis => self.render_analysis_tab(f, chunks[1]),
                Tab::Visualizations => self.render_viz_tab(f, chunks[1]),
                Tab::AI => self.render_ai_tab(f, chunks[1]),
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
                "Supported formats: .csv  .json  .xlsx (Excel)",
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

            let load_hint = "Press L to load a file (CSV, JSON, or Excel .xlsx)";
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
        if self.tabs.analysis.results.is_empty() {
            Paragraph::new("No analysis results yet.\n\nPress keys to run analyses:\n  s - Summary statistics\n  c - Correlation matrix\n  r - Linear regression\n  b - Box plot\n  i - Histogram")
                .block(Block::default().borders(Borders::ALL).title(" Results "))
                .wrap(Wrap { trim: false })
                .render(area, f.buffer_mut());
        } else {
            let items: Vec<ListItem> = self.tabs.analysis
                .results
                .iter()
                .map(|r| ListItem::new(r.clone()))
                .collect();

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(" Results "));

            Widget::render(list, area, f.buffer_mut());
        }
    }

    fn render_viz_tab(&self, f: &mut Frame, area: Rect) {
        if self.tabs.viz.show_viz && !self.tabs.viz.viz_output.is_empty() {
            let viz_text = if self.tabs.viz.viz_output.len() > 10000 {
                format!("{}...(truncated)", &self.tabs.viz.viz_output[..10000])
            } else {
                self.tabs.viz.viz_output.clone()
            };
            
            Paragraph::new(viz_text)
                .block(Block::default().borders(Borders::ALL).title(self.tabs.viz.viz_title.clone()))
                .wrap(Wrap { trim: false })
                .render(area, f.buffer_mut());
        } else {
            Paragraph::new("Press 'Space' to toggle visualization display.\n\nRun analyses from the Analysis tab to generate visualizations.")
                .block(Block::default().borders(Borders::ALL).title(" Visualizations "))
                .wrap(Wrap { trim: false })
                .render(area, f.buffer_mut());
        }
    }

    fn render_ai_tab(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
            .split(area);

        let conversation_text = if self.tabs.ai.conversation.is_empty() {
            "No conversation yet. Type a message and press Enter to send.".to_string()
        } else {
            self.tabs.ai.conversation.join("\n\n")
        };

        Paragraph::new(conversation_text)
            .block(Block::default().borders(Borders::ALL).title(" AI Assistant "))
            .wrap(Wrap { trim: false })
            .render(chunks[0], f.buffer_mut());

        Paragraph::new(self.tabs.ai.input.lines().join("\n"))
            .block(Block::default().borders(Borders::ALL).title(" Input (Enter to send, Esc to clear) "))
            .render(chunks[1], f.buffer_mut());

        if self.tabs.ai.loading {
            Paragraph::new("Loading AI response...")
                .render(Rect::new(area.x + 10, area.y + 10, 30, 3), f.buffer_mut());
        }
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

        match key.code {
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
                    self.tabs.ai.input = tui_textarea::TextArea::default();
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
                        Ok(df) => {
                            let info = DataLoader::get_column_info(&df);
                            let preview = format!("{:.5}", df.head(Some(10)));
                            let row_count = df.height();

                            self.dataframe = Some(df);
                            self.column_info = info;
                            self.tabs.data.file_path = expanded.clone();
                            self.tabs.data.dataframe_info = self.column_info.iter()
                                .map(|c| format!("{}: {} (nulls: {})", c.name, c.dtype, c.null_count))
                                .collect::<Vec<_>>()
                                .join("\n");
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
            KeyCode::Char(c) if self.input_mode == InputMode::Editing => {
                self.tabs.ai.input.insert_char(c);
            }
            KeyCode::Backspace if self.input_mode == InputMode::Editing => {
                self.tabs.ai.input.delete_char();
            }
            KeyCode::Char(' ') if self.tabs.active == Tab::Visualizations && self.input_mode == InputMode::Normal => {
                self.tabs.viz.show_viz = !self.tabs.viz.show_viz;
            }
            _ if self.tabs.active == Tab::Data && self.input_mode == InputMode::Normal => {
                if matches!(self.tabs.data.load_status, LoadStatus::Success(_) | LoadStatus::Error(_)) {
                    self.tabs.data.load_status = LoadStatus::Idle;
                }
            }
            KeyCode::Enter if self.tabs.active == Tab::AI && self.input_mode == InputMode::Normal => {
                self.input_mode = InputMode::Editing;
            }
            KeyCode::Enter if self.tabs.active == Tab::AI && self.input_mode == InputMode::Editing => {
                let prompt = self.tabs.ai.input.lines().join("\n").trim().to_string();
                if !prompt.is_empty() {
                    if prompt == "/help" || prompt.to_lowercase() == "help" {
                        self.tabs.show_help = true;
                        self.tabs.ai.input = tui_textarea::TextArea::default();
                        self.input_mode = InputMode::Normal;
                    } else {
                        self.tabs.ai.conversation.push(format!("You: {}", prompt));
                        self.tabs.ai.input = tui_textarea::TextArea::default();
                        self.input_mode = InputMode::Normal;
                    }
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
                    let result = self.agent.analyze_request(df, request).await?;
                    
                    let output = format!("{}\n\n{}", result.summary, 
                        result.details.as_deref().unwrap_or("No details"));
                    
                    self.tabs.analysis.results.push(output);
                    
                    if let Some(viz_config) = result.viz_config {
                        if let Ok(viz_data) = self.viz_engine.render(df, &viz_config) {
                            self.tabs.viz.viz_output = viz_data.svg_output;
                            self.tabs.viz.viz_title = viz_data.title;
                            self.tabs.viz.show_viz = true;
                        }
                    }
                    
                    self.tabs.active = Tab::Analysis;
                }
            }
            AppEvent::AIPrompt(prompt) => {
                self.tabs.ai.conversation.push(format!("You: {}", prompt));
                
                if self.dataframe.is_some() {
                    let df = self.dataframe.as_ref().unwrap();
                    if let Ok(result) = self.agent.analyze_request(
                        df,
                        AnalysisRequest::CustomInsight { prompt: prompt.clone() }
                    ).await {
                        self.tabs.ai.conversation.push(format!("AI: {}", result.summary));
                    } else {
                        self.tabs.ai.conversation.push("AI: Failed to get response".to_string());
                    }
                } else {
                    self.tabs.ai.conversation.push("AI: Load data first for better insights".to_string());
                }
            }
            AppEvent::ToggleViz(show) => {
                self.tabs.viz.show_viz = show;
            }
        }
        Ok(())
    }
}
