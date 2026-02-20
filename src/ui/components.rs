use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Wrap},
};
use tui_textarea::TextArea;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Data,
    Analysis,
    Visualizations,
    AI,
}

impl Tab {
    pub fn as_str(&self) -> &str {
        match self {
            Tab::Data => "Data",
            Tab::Analysis => "Analysis",
            Tab::Visualizations => "Viz",
            Tab::AI => "AI",
        }
    }
}

pub struct DataTab {
    pub file_path: String,
    pub file_path_input: String,  // Buffer when loading a file
    pub dataframe_info: String,
    pub preview_data: String,
}

impl Default for DataTab {
    fn default() -> Self {
        Self {
            file_path: String::new(),
            file_path_input: String::new(),
            dataframe_info: "No data loaded".to_string(),
            preview_data: "Load a CSV, JSON, or Excel (.xlsx) file to begin".to_string(),
        }
    }
}

pub struct AnalysisTab {
    pub results: Vec<String>,
    pub selected_result: usize,
}

impl Default for AnalysisTab {
    fn default() -> Self {
        Self {
            results: Vec::new(),
            selected_result: 0,
        }
    }
}

pub struct VizTab {
    pub viz_output: String,
    pub viz_title: String,
    pub show_viz: bool,
}

impl Default for VizTab {
    fn default() -> Self {
        Self {
            viz_output: String::new(),
            viz_title: String::new(),
            show_viz: false,
        }
    }
}

pub struct AITab {
    pub input: TextArea<'static>,
    pub conversation: Vec<String>,
    pub loading: bool,
}

impl Default for AITab {
    fn default() -> Self {
        Self {
            input: TextArea::default(),
            conversation: Vec::new(),
            loading: false,
        }
    }
}

pub struct AppTabs {
    pub data: DataTab,
    pub analysis: AnalysisTab,
    pub viz: VizTab,
    pub ai: AITab,
    pub active: Tab,
    pub show_help: bool,
}

impl Default for AppTabs {
    fn default() -> Self {
        Self {
            data: DataTab::default(),
            analysis: AnalysisTab::default(),
            viz: VizTab::default(),
            ai: AITab::default(),
            active: Tab::Data,
            show_help: false,
        }
    }
}

impl AppTabs {
    pub fn next_tab(&mut self) {
        self.active = match self.active {
            Tab::Data => Tab::Analysis,
            Tab::Analysis => Tab::Visualizations,
            Tab::Visualizations => Tab::AI,
            Tab::AI => Tab::Data,
        }
    }

    pub fn previous_tab(&mut self) {
        self.active = match self.active {
            Tab::Data => Tab::AI,
            Tab::Analysis => Tab::Data,
            Tab::Visualizations => Tab::Analysis,
            Tab::AI => Tab::Visualizations,
        }
    }

    pub fn render_tabs(&self, area: Rect, buf: &mut Buffer) {
        let titles: Vec<(bool, &str)> = vec![
            (self.active == Tab::Data, "Data"),
            (self.active == Tab::Analysis, "Analysis"),
            (self.active == Tab::Visualizations, "Visualizations"),
            (self.active == Tab::AI, "AI Assistant"),
        ];

        let header = "  R-Data Agent  │  ? or h = Help  ";
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
            .constraints(
                [Constraint::Length(12), Constraint::Length(12), Constraint::Length(14), Constraint::Length(14)]
            )
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
            "║  R-Data Agent — Keyboard Shortcuts (press ? or h to close)  ║",
            "╚══════════════════════════════════════════════════════════════╝",
            "",
            "  NAVIGATION",
            "  ──────────",
            "    Tab / Shift+Tab    Switch between tabs",
            "    q                  Quit application",
            "    ? or h             Toggle this help screen",
            "",
            "  DATA TAB — Load your dataset",
            "  ─────────────────────────────",
            "    L                 Load file (CSV, JSON, or Excel .xlsx)",
            "    Enter              Confirm file path when loading",
            "    Esc                Cancel file load",
            "",
            "  ANALYSIS TAB — Run statistical analyses",
            "  ────────────────────────────────────────",
            "    s                 Summary statistics",
            "    c                 Correlation matrix",
            "    r                 Linear regression",
            "    b                 Box plot",
            "    i                 Histogram",
            "",
            "  VISUALIZATIONS TAB — View charts",
            "  ─────────────────────────────────────",
            "    Space             Toggle visualization display",
            "",
            "  AI TAB — Chat with AI assistant",
            "  ────────────────────────────────",
            "    Enter             Start typing / Send message",
            "    Esc               Exit input mode / Clear",
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
