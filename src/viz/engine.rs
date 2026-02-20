use anyhow::Result;
use plotters::prelude::*;
use plotters_svg::SVGBackend;
use super::types::*;
use crate::data::{StatisticalAnalyzer};
use polars::prelude::DataFrame;

pub struct VisualizationEngine {
    width: u32,
    height: u32,
}

impl Default for VisualizationEngine {
    fn default() -> Self {
        Self::new(800, 600)
    }
}

impl VisualizationEngine {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub fn render(&self, df: &DataFrame, config: &VisualizationConfig) -> Result<ChartData> {
        match config {
            VisualizationConfig::Histogram(cfg) => self.render_histogram(df, cfg),
            VisualizationConfig::BoxPlot(cfg) => self.render_boxplot(df, cfg),
            VisualizationConfig::LinearRegression(cfg) => self.render_linear_regression(df, cfg),
            VisualizationConfig::Heatmap(cfg) => self.render_heatmap(df, cfg),
        }
    }

    fn render_histogram(&self, df: &DataFrame, config: &HistogramConfig) -> Result<ChartData> {
        let hist_data = StatisticalAnalyzer::compute_histogram_data(df, &config.column, config.bins)?;

        let mut buffer = String::new();
        {
            let root = SVGBackend::with_string(&mut buffer, (self.width, self.height)).into_drawing_area();
            root.fill(&WHITE)?;

            let max_count = *hist_data.bin_counts.iter().max().unwrap_or(&1) as i32;

            let mut chart = ChartBuilder::on(&root)
                .caption(format!("Histogram: {}", config.column), ("sans-serif", 30))
                .x_label_area_size(40)
                .y_label_area_size(40)
                .margin(10)
                .build_cartesian_2d(
                    (hist_data.min_val)..(hist_data.max_val),
                    0i32..max_count,
                )?;

            chart
                .configure_mesh()
                .x_desc("Value")
                .y_desc("Frequency")
                .draw()?;

            chart.draw_series(
                (0..config.bins).map(|i| {
                    let x_start = hist_data.min_val + (i as f64) * hist_data.bin_width;
                    let x_end = x_start + hist_data.bin_width;
                    let count = hist_data.bin_counts[i] as i32;
                    Rectangle::new([(x_start, 0), (x_end, count)], BLUE.filled())
                }),
            )?;
        }

        Ok(ChartData {
            chart_type: VisualizationType::Histogram,
            svg_output: buffer,
            title: format!("Histogram: {}", config.column),
        })
    }

    fn render_boxplot(&self, df: &DataFrame, config: &BoxPlotConfig) -> Result<ChartData> {
        let box_data = StatisticalAnalyzer::compute_boxplot_data(df, &config.column)?;

        let mut buffer = String::new();
        {
            let root = SVGBackend::with_string(&mut buffer, (self.width, self.height)).into_drawing_area();
            root.fill(&WHITE)?;

            let y_min = box_data.min - 0.1 * (box_data.max - box_data.min);
            let y_max = box_data.max + 0.1 * (box_data.max - box_data.min);

            let mut chart = ChartBuilder::on(&root)
                .caption(format!("Box Plot: {}", config.column), ("sans-serif", 30))
                .y_label_area_size(50)
                .margin(10)
                .build_cartesian_2d(0f64..1f64, y_min..y_max)?;

            chart
                .configure_mesh()
                .y_desc("Value")
                .disable_x_mesh()
                .disable_x_axis()
                .draw()?;

            let x_center = 0.5;
            let box_width = 0.3;
            let whisker_width = 0.05;

            let black = &BLACK;
            let blue = &BLUE;

            chart.draw_series(std::iter::once(PathElement::new(
                vec![
                    (x_center, box_data.max),
                    (x_center, box_data.q3),
                ],
                black.stroke_width(2),
            )))?;

            chart.draw_series(std::iter::once(PathElement::new(
                vec![
                    (x_center, box_data.q1),
                    (x_center, box_data.min),
                ],
                black.stroke_width(2),
            )))?;

            chart.draw_series(std::iter::once(Rectangle::new(
                [(x_center - box_width / 2.0, box_data.q1), (x_center + box_width / 2.0, box_data.q3)],
                blue.stroke_width(2).filled(),
            )))?;

            chart.draw_series(std::iter::once(PathElement::new(
                vec![
                    (x_center - whisker_width / 2.0, box_data.max),
                    (x_center + whisker_width / 2.0, box_data.max),
                ],
                black.stroke_width(2),
            )))?;

            chart.draw_series(std::iter::once(PathElement::new(
                vec![
                    (x_center - whisker_width / 2.0, box_data.min),
                    (x_center + whisker_width / 2.0, box_data.min),
                ],
                black.stroke_width(2),
            )))?;

            for outlier in &box_data.outliers {
                chart.draw_series(std::iter::once(Circle::new(
                    (x_center, *outlier),
                    5,
                    RED.filled(),
                )))?;
            }
        }

        Ok(ChartData {
            chart_type: VisualizationType::BoxPlot,
            svg_output: buffer,
            title: format!("Box Plot: {}", config.column),
        })
    }

    fn render_linear_regression(
        &self,
        df: &DataFrame,
        config: &LinearRegressionConfig,
    ) -> Result<ChartData> {
        let reg = crate::data::StatisticalAnalyzer::linear_regression(df, &config.x_column, &config.y_column)?;

        let x_series = df.column(&config.x_column)?;
        let y_series = df.column(&config.y_column)?;

        let x_data: Vec<f64> = x_series.f64()?.into_no_null_iter().collect();
        let y_data: Vec<f64> = y_series.f64()?.into_no_null_iter().collect();

        let x_min = x_data.iter().fold(f64::INFINITY, |a, b| a.min(*b));
        let x_max = x_data.iter().fold(f64::NEG_INFINITY, |a, b| a.max(*b));
        let y_min = y_data.iter().fold(f64::INFINITY, |a, b| a.min(*b));
        let y_max = y_data.iter().fold(f64::NEG_INFINITY, |a, b| a.max(*b));

        let x_range = x_max - x_min;
        let y_range = y_max - y_min;

        let mut buffer = String::new();
        {
            let root = SVGBackend::with_string(&mut buffer, (self.width, self.height)).into_drawing_area();
            root.fill(&WHITE)?;

            let mut chart = ChartBuilder::on(&root)
                .caption(
                    format!(
                        "Linear Regression: R² = {:.4}",
                        reg.r_squared
                    ),
                    ("sans-serif", 30),
                )
                .x_label_area_size(50)
                .y_label_area_size(50)
                .margin(10)
                .build_cartesian_2d(
                    (x_min - 0.1 * x_range)..(x_max + 0.1 * x_range),
                    (y_min - 0.1 * y_range)..(y_max + 0.1 * y_range),
                )?;

            chart
                .configure_mesh()
                .x_desc(config.x_column.clone())
                .y_desc(config.y_column.clone())
                .draw()?;

            chart.draw_series(
                x_data.iter().zip(y_data.iter()).map(|(&x, &y)| {
                    Circle::new((x, y), 3, BLUE.filled())
                }),
            )?;

            let line_start = x_min - 0.1 * x_range;
            let line_end = x_max + 0.1 * x_range;
            chart.draw_series(std::iter::once(PathElement::new(
                vec![
                    (
                        line_start,
                        reg.slope * line_start + reg.intercept,
                    ),
                    (
                        line_end,
                        reg.slope * line_end + reg.intercept,
                    ),
                ],
                RED.stroke_width(2),
            )))?;
        }

        Ok(ChartData {
            chart_type: VisualizationType::LinearRegression,
            svg_output: buffer,
            title: format!("Linear Regression: {} vs {}", config.y_column, config.x_column),
        })
    }

    fn render_heatmap(&self, df: &DataFrame, config: &HeatmapConfig) -> Result<ChartData> {
        let corr_matrix = crate::data::StatisticalAnalyzer::compute_correlation(df)?;

        let cols: Vec<String> = if config.columns.is_empty() {
            df.get_columns()
                .iter()
                .filter(|col| col.dtype().is_numeric())
                .map(|col| col.name().to_string())
                .collect()
        } else {
            config.columns.clone()
        };

        let n = cols.len();
        if n == 0 {
            return Ok(ChartData {
                chart_type: VisualizationType::Heatmap,
                svg_output: "No numeric columns available".to_string(),
                title: "Correlation Heatmap".to_string(),
            });
        }

        let mut buffer = String::new();
        {
            let root = SVGBackend::with_string(&mut buffer, (self.width, self.height)).into_drawing_area();
            root.fill(&WHITE)?;

            let cell_size = 60;
            let offset_x: i32 = 60;
            let offset_y: i32 = 60;

            root.draw(&Text::new(
                "Correlation Heatmap",
                ((self.width / 2) as i32, 20),
                ("sans-serif", 30).into_font().color(&BLACK),
            ))?;

            for i in 0..n {
                for j in 0..n {
                    let series = corr_matrix.column(&cols[j])?;
                    let corr_val = series.f64()?.get(i).unwrap_or(0.0);

                    let color = if corr_val >= 0.0 {
                        RED.mix(corr_val as f64)
                    } else {
                        BLUE.mix((-corr_val) as f64)
                    };

                    let x = offset_x + (j * cell_size) as i32;
                    let y = offset_y + (i * cell_size) as i32;

                    root.draw(&Rectangle::new(
                        [(x, y), (x + cell_size as i32, y + cell_size as i32)],
                        color.filled(),
                    ))?;

                    let text_color = if corr_val.abs() > 0.5 { WHITE } else { BLACK };
                    root.draw(&Text::new(
                        format!("{:.2}", corr_val),
                        (x + cell_size as i32 / 2, y + cell_size as i32 / 2),
                        ("sans-serif", 14).into_font().color(&text_color),
                    ))?;
                }

                root.draw(&Text::new(
                    cols[i].clone(),
                    (offset_x - 10, offset_y + (i * cell_size) as i32 + cell_size as i32 / 2),
                    ("sans-serif", 12).into_font().color(&BLACK),
                ))?;

                root.draw(&Text::new(
                    cols[i].clone(),
                    (offset_x + (i * cell_size) as i32 + cell_size as i32 / 2, offset_y - 10),
                    ("sans-serif", 12).into_font().color(&BLACK),
                ))?;
            }
        }

        Ok(ChartData {
            chart_type: VisualizationType::Heatmap,
            svg_output: buffer,
            title: "Correlation Heatmap".to_string(),
        })
    }
}
