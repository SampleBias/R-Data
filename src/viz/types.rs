use std::fmt;

#[derive(Debug, Clone)]
pub enum VisualizationType {
    Histogram,
    BoxPlot,
    LinearRegression,
    Heatmap,
    ExpressionTrend,
    YoungVsOldScatter,
    AgeGroupBoxPlot,
}

impl fmt::Display for VisualizationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VisualizationType::Histogram => write!(f, "Histogram"),
            VisualizationType::BoxPlot => write!(f, "Box Plot"),
            VisualizationType::LinearRegression => write!(f, "Linear Regression"),
            VisualizationType::Heatmap => write!(f, "Heatmap"),
            VisualizationType::ExpressionTrend => write!(f, "Expression Trend"),
            VisualizationType::YoungVsOldScatter => write!(f, "Young vs Old Scatter"),
            VisualizationType::AgeGroupBoxPlot => write!(f, "Age Group Box Plot"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HistogramConfig {
    pub column: String,
    pub bins: usize,
}

#[derive(Debug, Clone)]
pub struct BoxPlotConfig {
    pub column: String,
}

#[derive(Debug, Clone)]
pub struct LinearRegressionConfig {
    pub x_column: String,
    pub y_column: String,
}

#[derive(Debug, Clone)]
pub struct HeatmapConfig {
    pub columns: Vec<String>,
}

/// Expression vs age for selected gene(s). Requires microarray layout.
#[derive(Debug, Clone)]
pub struct ExpressionTrendConfig {
    pub gene_ids: Vec<String>,
    pub gene_column: String,
    pub age_columns: Vec<String>,
}

/// Mean expression Young vs Old across genes. Requires microarray layout.
#[derive(Debug, Clone)]
pub struct YoungVsOldConfig {
    pub gene_column: String,
    pub age_columns: Vec<String>,
    #[allow(dead_code)]
    pub young_ages: Vec<String>,
    #[allow(dead_code)]
    pub old_ages: Vec<String>,
}

/// Box plot by age category. One box per age column.
#[derive(Debug, Clone)]
pub struct AgeGroupBoxPlotConfig {
    pub gene_column: String,
    pub age_columns: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum VisualizationConfig {
    Histogram(HistogramConfig),
    BoxPlot(BoxPlotConfig),
    LinearRegression(LinearRegressionConfig),
    Heatmap(HeatmapConfig),
    ExpressionTrend(ExpressionTrendConfig),
    YoungVsOldScatter(YoungVsOldConfig),
    AgeGroupBoxPlot(AgeGroupBoxPlotConfig),
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ChartData {
    pub chart_type: VisualizationType,
    pub svg_output: String,
    pub svg_file_path: Option<std::path::PathBuf>,
    pub terminal_output: String,
    pub title: String,
}
