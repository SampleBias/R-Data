use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Wrap},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Agent,
    Data,
    Analysis,
    Visualizations,
}

impl Tab {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &str {
        match self {
            Tab::Agent => "Agent",
            Tab::Data => "Data",
            Tab::Analysis => "Analysis",
            Tab::Visualizations => "Viz",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    Idle,
    Processing,
}

/// Focus within the Agent tab: Chat (scroll) or Input (type)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentFocus {
    /// Scrolling the conversation (↑/↓ scroll, Esc back to input)
    Chat,
    /// Typing in the input box (Enter send, Tab to focus chat)
    Input,
}

#[derive(Debug, Clone)]
pub struct AgentMessage {
    pub role: String,
    pub content: String,
}

pub struct AgentTab {
    pub input: String,
    pub messages: Vec<AgentMessage>,
    pub status: AgentStatus,
    pub streaming_content: String,
    /// Scroll offset: lines from bottom (0 = at bottom, showing latest)
    pub scroll_offset: u16,
    /// When loading started (for animation)
    pub loading_start: Option<std::time::Instant>,
    /// Focus: Chat (scroll) or Input (type)
    pub focus: AgentFocus,
}

impl Default for AgentTab {
    fn default() -> Self {
        Self {
            input: String::new(),
            messages: Vec::new(),
            status: AgentStatus::Idle,
            streaming_content: String::new(),
            scroll_offset: 0,
            loading_start: None,
            focus: AgentFocus::Input,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LoadStatus {
    Idle,
    Loading,
    Success(String),
    Error(String),
}

pub struct DataTab {
    pub file_path: String,
    pub file_path_input: String,  // Buffer when loading a file
    pub dataframe_info: String,
    pub preview_data: String,
    pub load_status: LoadStatus,
}

impl Default for DataTab {
    fn default() -> Self {
        Self {
            file_path: String::new(),
            file_path_input: String::new(),
            dataframe_info: "No data loaded".to_string(),
            preview_data: "Load a CSV, JSON, or Excel (.xlsx) file to begin".to_string(),
            load_status: LoadStatus::Idle,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AnalysisStatus {
    Idle,
    PendingConfirm { request: String },
    Loading,
    Success(String),
    Error(String),
}

#[allow(dead_code)]
pub struct AnalysisTab {
    pub results: Vec<String>,
    pub selected_result: usize,
    pub analysis_status: AnalysisStatus,
}

impl Default for AnalysisTab {
    fn default() -> Self {
        Self {
            results: Vec::new(),
            selected_result: 0,
            analysis_status: AnalysisStatus::Idle,
        }
    }
}

pub struct VizTab {
    pub viz_output: String,
    pub viz_title: String,
    pub viz_svg_path: Option<std::path::PathBuf>,
    pub show_viz: bool,
}

impl Default for VizTab {
    fn default() -> Self {
        Self {
            viz_output: String::new(),
            viz_title: String::new(),
            viz_svg_path: None,
            show_viz: false,
        }
    }
}

pub struct AppTabs {
    pub agent: AgentTab,
    pub data: DataTab,
    pub analysis: AnalysisTab,
    pub viz: VizTab,
    pub active: Tab,
    pub show_help: bool,
}

impl Default for AppTabs {
    fn default() -> Self {
        Self {
            agent: AgentTab::default(),
            data: DataTab::default(),
            analysis: AnalysisTab::default(),
            viz: VizTab::default(),
            active: Tab::Agent,
            show_help: false,
        }
    }
}

impl AppTabs {
    pub fn next_tab(&mut self) {
        self.active = match self.active {
            Tab::Agent => Tab::Data,
            Tab::Data => Tab::Analysis,
            Tab::Analysis => Tab::Visualizations,
            Tab::Visualizations => Tab::Agent,
        }
    }

    pub fn previous_tab(&mut self) {
        self.active = match self.active {
            Tab::Agent => Tab::Visualizations,
            Tab::Data => Tab::Agent,
            Tab::Analysis => Tab::Data,
            Tab::Visualizations => Tab::Analysis,
        }
    }

    pub fn render_tabs(&self, area: Rect, buf: &mut Buffer) {
        let titles: Vec<(bool, &str)> = vec![
            (self.active == Tab::Agent, "Agent"),
            (self.active == Tab::Data, "Data"),
            (self.active == Tab::Analysis, "Analysis"),
            (self.active == Tab::Visualizations, "Viz"),
        ];

        let header = "  R-Data Agent — Longevity Gene Expression  │  ? = Help  ";
        let tabs_block = Block::default()
            .borders(Borders::ALL)
            .title(header);

        let inner = tabs_block.inner(area);
        let inner_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)].as_ref())
            .split(inner);
        let indicator_area = inner_chunks[0];
        let tabs_area = inner_chunks[1];

        let current = titles.iter().find(|(active, _)| *active).map(|(_, name)| *name).unwrap_or("");
        Paragraph::new(format!("▶ Current: {} ◀", current))
            .style(ratatui::style::Style::default().add_modifier(ratatui::style::Modifier::BOLD))
            .render(indicator_area, buf);

        let tabs_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(12),
                Constraint::Length(10),
                Constraint::Length(12),
                Constraint::Length(10),
            ])
            .split(tabs_area);

        for (i, (is_active, name)) in titles.iter().enumerate() {
            let label = if *is_active {
                format!(" [ {} ] ", name)
            } else {
                format!("   {}   ", name)
            };

            Paragraph::new(label.as_str())
                .block(Block::default())
                .alignment(Alignment::Center)
                .style(if *is_active {
                    ratatui::style::Style::default()
                        .fg(ratatui::style::Color::Black)
                        .bg(ratatui::style::Color::Cyan)
                        .add_modifier(ratatui::style::Modifier::BOLD)
                } else {
                    ratatui::style::Style::default()
                })
                .render(tabs_layout[i], buf);
        }

        tabs_block.render(area, buf);
    }

    pub fn render_help(&self, area: Rect, buf: &mut Buffer) {
        let help_text = vec![
            "╔══════════════════════════════════════════════════════════════╗",
            "║  Longevity Gene Expression — Keyboard Shortcuts (? or h to close)  ║",
            "╚══════════════════════════════════════════════════════════════╝",
            "",
            "  NAVIGATION",
            "  ──────────",
            "    Tab / Shift+Tab    Switch between tabs",
            "    q                  Quit application",
            "    ?                 Toggle this help screen",
            "    C                  Clear results and visualizations (any tab)",
            "",
            "  DATA TAB — Load your dataset(s)",
            "  ─────────────────────────────",
            "    L                 Load file(s). Multiple: comma or semicolon separated",
            "    Enter              Confirm path(s) when loading",
            "    Esc                Cancel file load",
            "    1-9                Select active dataset (when multiple loaded)",
            "    Age selection      ↓ to enter • Tab, ↑ or Esc to return to tabs • X to toggle",
            "    4                 Define age groups (Young=17-30,Old=40-60)",
            "",
            "  ANALYSIS TAB — Run statistical analyses",
            "  ────────────────────────────────────────",
            "    s                 Summary statistics",
            "    c                 Correlation matrix",
            "    i                 Histogram",
            "    b                 Box plot",
            "    r                 Microarray: volcano plot (all genes). Other: linear regression scatter",
            "    g                 Genes significant with age, p<0.05 (microarray)",
            "    t                 Expression trend (select genes)",
            "    e                 Expression vs age scatter (select 1-5 genes, / to search, paste gene ID)",
            "    v                 Young vs Old scatter (microarray)",
            "    a                 Age group box plot (microarray)",
            "    1                 Volcano plot",
            "    2                 Correlation scatter",
            "    3                 Top genes bar chart",
            "    Enter             Confirm and run selected analysis",
            "    Esc               Cancel pending analysis",
            "",
            "  VISUALIZATIONS TAB — View charts",
            "  ─────────────────────────────────────",
            "    Space             Toggle display",
            "    O                 Open chart in browser (full-quality SVG)",
            "",
            "  AGENT TAB — Orchestrates the app (AI-powered)",
            "  ────────────────────────────────────────────────",
            "    Type your request  e.g. \"Load data.csv and show genes significant with age\"",
            "    Enter              Send message to Agent (has full app context)",
            "    Esc                Toggle: input ↔ chat (cyan border = scroll mode)",
            "    ↑/↓ PageUp/Down    Scroll conversation (when chat focused)",
            "    PageUp/PageDown    Scroll by page",
            "    Home/End           Jump to top/bottom",
            "    /tools            List available tools",
            "",
            "  Supported file formats: .csv  .json  .xlsx",
        ];

        Paragraph::new(help_text.join("\n"))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" /help — Key Commands "),
            )
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}
