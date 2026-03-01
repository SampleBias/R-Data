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
    CorrelationScatter,
    CorrelationBarChart,
    VolcanoPlot,
    ExpressionVsAgeRegression,
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
            VisualizationType::CorrelationScatter => write!(f, "Correlation vs p-value Scatter"),
            VisualizationType::CorrelationBarChart => write!(f, "Top Genes by |Correlation|"),
            VisualizationType::VolcanoPlot => write!(f, "Volcano Plot"),
            VisualizationType::ExpressionVsAgeRegression => write!(f, "Expression vs Age (Regression)"),
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
    pub young_ages: Vec<String>,
    pub old_ages: Vec<String>,
}

/// Box plot by age category. One box per age column.
#[derive(Debug, Clone)]
pub struct AgeGroupBoxPlotConfig {
    pub gene_column: String,
    pub age_columns: Vec<String>,
}

/// Point for correlation-based charts (scatter, bar, volcano).
#[derive(Debug, Clone)]
pub struct GeneCorrelationPoint {
    pub gene_id: String,
    pub correlation: f64,
    pub p_value: f64,
    pub significant: bool,
    pub direction: String,
}

/// Scatter: correlation vs -log10(p-value), colored by direction.
#[derive(Debug, Clone)]
pub struct CorrelationScatterConfig {
    pub points: Vec<GeneCorrelationPoint>,
}

/// Bar chart: top N genes by |correlation|, Ensembl IDs on axis.
#[derive(Debug, Clone)]
pub struct CorrelationBarChartConfig {
    pub points: Vec<GeneCorrelationPoint>,
    pub top_n: usize,
}

/// Volcano: -log10(p-value) vs correlation.
#[derive(Debug, Clone)]
pub struct VolcanoPlotConfig {
    pub points: Vec<GeneCorrelationPoint>,
}

/// Expression vs age with regression line(s). 1–5 genes.
#[derive(Debug, Clone)]
pub struct ExpressionVsAgeRegressionConfig {
    pub gene_ids: Vec<String>,
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
    CorrelationScatter(CorrelationScatterConfig),
    CorrelationBarChart(CorrelationBarChartConfig),
    VolcanoPlot(VolcanoPlotConfig),
    ExpressionVsAgeRegression(ExpressionVsAgeRegressionConfig),
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
