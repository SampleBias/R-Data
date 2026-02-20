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
    pub dataframe_info: String,
    pub preview_data: String,
}

impl Default for DataTab {
    fn default() -> Self {
        Self {
            file_path: String::new(),
            dataframe_info: "No data loaded".to_string(),
            preview_data: "Load a CSV or JSON file to begin".to_string(),
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
            (self.active == Tab::Data, Tab::Data.as_str()),
            (self.active == Tab::Analysis, Tab::Analysis.as_str()),
            (self.active == Tab::Visualizations, Tab::Visualizations.as_str()),
            (self.active == Tab::AI, Tab::AI.as_str()),
        ];

        let tabs_block = Block::default()
            .borders(Borders::ALL)
            .title(" Tabs ");

        let tabs_layout = Layout::default()
            .direction(Direction::Horizontal)
            .margin(1)
            .constraints(
                [Constraint::Length(10), Constraint::Length(10), Constraint::Length(10), Constraint::Length(10)]
            )
            .split(tabs_block.inner(area));

        for (i, (is_active, title)) in titles.iter().enumerate() {
            let title = if *is_active {
                format!("> {} <", title)
            } else {
                format!("  {}  ", title)
            };

            Paragraph::new(title.as_str())
                .block(Block::default())
                .alignment(Alignment::Center)
                .render(tabs_layout[i], buf);
        }

        tabs_block.render(area, buf);
    }

    pub fn render_help(&self, area: Rect, buf: &mut Buffer) {
        let help_text = vec![
            "Key Bindings:",
            "",
            "General:",
            "  Tab        - Switch tabs",
            "  q          - Quit",
            "  h          - Toggle help",
            "",
            "Data Tab:",
            "  l          - Load file",
            "",
            "Analysis Tab:",
            "  s          - Summary statistics",
            "  c          - Correlation matrix",
            "  r          - Linear regression",
            "  b          - Box plot",
            "  i          - Histogram",
            "",
            "Visualizations Tab:",
            "  Space      - Toggle visualization display",
            "",
            "AI Tab:",
            "  Enter      - Send message",
            "  Esc        - Exit input mode",
        ];

        Paragraph::new(help_text.join("\n"))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Help "),
            )
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}
