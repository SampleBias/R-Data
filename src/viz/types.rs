use std::fmt;

#[derive(Debug, Clone)]
pub enum VisualizationType {
    Histogram,
    BoxPlot,
    LinearRegression,
    Heatmap,
}

impl fmt::Display for VisualizationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VisualizationType::Histogram => write!(f, "Histogram"),
            VisualizationType::BoxPlot => write!(f, "Box Plot"),
            VisualizationType::LinearRegression => write!(f, "Linear Regression"),
            VisualizationType::Heatmap => write!(f, "Heatmap"),
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

#[derive(Debug, Clone)]
pub enum VisualizationConfig {
    Histogram(HistogramConfig),
    BoxPlot(BoxPlotConfig),
    LinearRegression(LinearRegressionConfig),
    Heatmap(HeatmapConfig),
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
