use anyhow::Result;
use plotters::prelude::*;
use plotters::style::RGBColor;
use plotters_svg::SVGBackend;
use std::io::Write;
use std::path::PathBuf;
use tempfile::Builder;
use super::types::*;
use crate::data::{
    StatisticalAnalyzer,
    analysis::{BoxplotData, HistogramData, RegressionResult},
};
use polars::prelude::DataFrame;

const TERM_WIDTH: usize = 58;
const TERM_HEIGHT: usize = 16;

// ggplot2-inspired color palette
const BG_LIGHT_GRAY: RGBColor = RGBColor(245, 245, 245);
const GRID_GRAY: RGBColor = RGBColor(230, 230, 230);
const STEEL_BLUE: RGBColor = RGBColor(70, 130, 180);
const CORAL: RGBColor = RGBColor(231, 76, 60);

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

    fn save_svg_to_temp(svg: &str, prefix: &str) -> Result<PathBuf> {
        let mut temp = Builder::new().prefix(prefix).suffix(".svg").tempfile()?;
        temp.write_all(svg.as_bytes())?;
        let path = temp.into_temp_path().keep()?;
        Ok(path.into())
    }

    pub fn render(&self, df: &DataFrame, config: &VisualizationConfig) -> Result<ChartData> {
        match config {
            VisualizationConfig::Histogram(cfg) => self.render_histogram(df, cfg),
            VisualizationConfig::BoxPlot(cfg) => self.render_boxplot(df, cfg),
            VisualizationConfig::LinearRegression(cfg) => self.render_linear_regression(df, cfg),
            VisualizationConfig::Heatmap(cfg) => self.render_heatmap(df, cfg),
            VisualizationConfig::ExpressionTrend(cfg) => self.render_expression_trend(df, cfg),
            VisualizationConfig::YoungVsOldScatter(cfg) => self.render_young_vs_old(df, cfg),
            VisualizationConfig::AgeGroupBoxPlot(cfg) => self.render_age_group_boxplot(df, cfg),
            VisualizationConfig::CorrelationScatter(cfg) => self.render_correlation_scatter(cfg),
            VisualizationConfig::CorrelationBarChart(cfg) => self.render_correlation_bar_chart(cfg),
            VisualizationConfig::VolcanoPlot(cfg) => self.render_volcano_plot(cfg),
            VisualizationConfig::ExpressionVsAgeRegression(cfg) => {
                self.render_expression_vs_age_regression(df, cfg)
            }
        }
    }

    fn render_histogram(&self, df: &DataFrame, config: &HistogramConfig) -> Result<ChartData> {
        let hist_data = StatisticalAnalyzer::compute_histogram_data(df, &config.column, config.bins)?;

        let mut buffer = String::new();
        {
            let root = SVGBackend::with_string(&mut buffer, (self.width, self.height)).into_drawing_area();
            root.fill(&BG_LIGHT_GRAY)?;

            let max_count = *hist_data.bin_counts.iter().max().unwrap_or(&1) as i32;

            let mut chart = ChartBuilder::on(&root)
                .caption(format!("Histogram: {}", config.column), ("sans-serif", 28).into_font().color(&BLACK))
                .x_label_area_size(40)
                .y_label_area_size(40)
                .margin(12)
                .build_cartesian_2d(
                    (hist_data.min_val)..(hist_data.max_val),
                    0i32..max_count,
                )?;

            chart
                .configure_mesh()
                .axis_style(GRID_GRAY.stroke_width(1))
                .x_desc("Value")
                .y_desc("Frequency")
                .draw()?;

            chart.draw_series(
                (0..config.bins).map(|i| {
                    let x_start = hist_data.min_val + (i as f64) * hist_data.bin_width;
                    let x_end = x_start + hist_data.bin_width;
                    let count = hist_data.bin_counts[i] as i32;
                    Rectangle::new([(x_start, 0), (x_end, count)], STEEL_BLUE.filled())
                }),
            )?;
        }

        let terminal_output = Self::render_histogram_ascii(&hist_data, config.bins);
        let svg_file_path = Self::save_svg_to_temp(&buffer, "rdata-histogram").ok();
        Ok(ChartData {
            chart_type: VisualizationType::Histogram,
            svg_output: buffer,
            svg_file_path,
            terminal_output,
            title: format!("Histogram: {}", config.column),
        })
    }

    fn render_histogram_ascii(hist_data: &HistogramData, bins: usize) -> String {
        let max_count = *hist_data.bin_counts.iter().max().unwrap_or(&1) as f64;
        let plot_height = TERM_HEIGHT.saturating_sub(2);
        let plot_width = TERM_WIDTH.saturating_sub(4);

        let display_bins = bins.min(plot_width);
        let mut lines = vec![format!("Histogram: {} (max freq: {:.0})", hist_data.col_name, max_count)];
        for row in (0..plot_height).rev() {
            let threshold = (1.0 - (row as f64 + 0.5) / plot_height as f64) * max_count;
            let mut line = String::from("  ");
            for i in 0..display_bins {
                let bin_idx = (i * bins) / display_bins;
                let count = hist_data.bin_counts.get(bin_idx).copied().unwrap_or(0) as f64;
                let filled = count >= threshold;
                line.push(if filled { '█' } else { ' ' });
            }
            lines.push(line);
        }
        lines.push(format!("  min:{:.1} {}", hist_data.min_val, "─".repeat(plot_width.saturating_sub(10))));
        lines.push(format!("  {}", hist_data.max_val));
        lines.join("\n")
    }

    fn render_boxplot(&self, df: &DataFrame, config: &BoxPlotConfig) -> Result<ChartData> {
        let box_data = StatisticalAnalyzer::compute_boxplot_data(df, &config.column)?;

        let mut buffer = String::new();
        {
            let root = SVGBackend::with_string(&mut buffer, (self.width, self.height)).into_drawing_area();
            root.fill(&BG_LIGHT_GRAY)?;

            let y_min = box_data.min - 0.1 * (box_data.max - box_data.min);
            let y_max = box_data.max + 0.1 * (box_data.max - box_data.min);

            let mut chart = ChartBuilder::on(&root)
                .caption(format!("Box Plot: {}", config.column), ("sans-serif", 28).into_font().color(&BLACK))
                .y_label_area_size(50)
                .margin(12)
                .build_cartesian_2d(0f64..1f64, y_min..y_max)?;

            chart
                .configure_mesh()
                .axis_style(GRID_GRAY.stroke_width(1))
                .y_desc("Value")
                .disable_x_mesh()
                .disable_x_axis()
                .draw()?;

            let x_center = 0.5;
            let box_width = 0.3;
            let whisker_width = 0.05;

            let black = &BLACK;
            let blue = &STEEL_BLUE;

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
                    CORAL.filled(),
                )))?;
            }
        }

        let terminal_output = Self::render_boxplot_ascii(&box_data);
        let svg_file_path = Self::save_svg_to_temp(&buffer, "rdata-boxplot").ok();
        Ok(ChartData {
            chart_type: VisualizationType::BoxPlot,
            svg_output: buffer,
            svg_file_path,
            terminal_output,
            title: format!("Box Plot: {}", config.column),
        })
    }

    fn render_boxplot_ascii(box_data: &BoxplotData) -> String {
        let range = box_data.max - box_data.min;
        let range = if range == 0.0 { 1.0 } else { range };
        let width = 40usize;
        let to_x = |v: f64| -> usize {
            (((v - box_data.min) / range) * (width as f64)).round() as usize
        };
        let mut grid = vec![' '; width + 2];
        grid[to_x(box_data.min)] = '│';
        grid[to_x(box_data.q1)] = '├';
        grid[to_x(box_data.median)] = '┼';
        grid[to_x(box_data.q3)] = '┤';
        grid[to_x(box_data.max)] = '│';
        for i in to_x(box_data.q1)..=to_x(box_data.q3) {
            if grid[i] == ' ' {
                grid[i] = '─';
            }
        }
        let line: String = grid.iter().collect();
        vec![
            format!("Box Plot: {}", box_data.col_name),
            format!("  min={:.2}  q1={:.2}  med={:.2}  q3={:.2}  max={:.2}",
                box_data.min, box_data.q1, box_data.median, box_data.q3, box_data.max),
            format!("  {}", line),
            if box_data.outliers.is_empty() {
                "  No outliers".to_string()
            } else {
                format!("  Outliers ({}): {:?}", box_data.outliers.len(), &box_data.outliers[..box_data.outliers.len().min(5)])
            },
        ].join("\n")
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
            root.fill(&BG_LIGHT_GRAY)?;

            let mut chart = ChartBuilder::on(&root)
                .caption(
                    format!("y = {:.4}x + {:.4}  (R² = {:.4})", reg.slope, reg.intercept, reg.r_squared),
                    ("sans-serif", 28).into_font().color(&BLACK),
                )
                .x_label_area_size(50)
                .y_label_area_size(50)
                .margin(12)
                .build_cartesian_2d(
                    (x_min - 0.1 * x_range)..(x_max + 0.1 * x_range),
                    (y_min - 0.1 * y_range)..(y_max + 0.1 * y_range),
                )?;

            chart
                .configure_mesh()
                .axis_style(GRID_GRAY.stroke_width(1))
                .x_desc(config.x_column.clone())
                .y_desc(config.y_column.clone())
                .draw()?;

            chart.draw_series(
                x_data.iter().zip(y_data.iter()).map(|(&x, &y)| {
                    Circle::new((x, y), 4, STEEL_BLUE.filled())
                }),
            )?;

            let line_start = x_min - 0.1 * x_range;
            let line_end = x_max + 0.1 * x_range;
            chart.draw_series(std::iter::once(PathElement::new(
                vec![
                    (line_start, reg.slope * line_start + reg.intercept),
                    (line_end, reg.slope * line_end + reg.intercept),
                ],
                CORAL.stroke_width(3),
            )))?;
        }

        let terminal_output = Self::render_regression_ascii(
            &reg,
            &x_data,
            &y_data,
            x_min - 0.1 * x_range,
            x_max + 0.1 * x_range,
            y_min - 0.1 * y_range,
            y_max + 0.1 * y_range,
            &config.x_column,
            &config.y_column,
        );
        let svg_file_path = Self::save_svg_to_temp(&buffer, "rdata-regression").ok();
        Ok(ChartData {
            chart_type: VisualizationType::LinearRegression,
            svg_output: buffer,
            svg_file_path,
            terminal_output,
            title: format!("y = {:.4}x + {:.4}  |  {} vs {}  R²={:.4}", reg.slope, reg.intercept, config.y_column, config.x_column, reg.r_squared),
        })
    }

    fn render_regression_ascii(
        reg: &RegressionResult,
        x_data: &[f64],
        y_data: &[f64],
        x_min: f64,
        x_max: f64,
        y_min: f64,
        y_max: f64,
        x_label: &str,
        y_label: &str,
    ) -> String {
        let w = TERM_WIDTH.saturating_sub(6);
        let h = TERM_HEIGHT.saturating_sub(3);
        let x_range = x_max - x_min;
        let y_range = y_max - y_min;
        let x_range = if x_range == 0.0 { 1.0 } else { x_range };
        let y_range = if y_range == 0.0 { 1.0 } else { y_range };

        let to_col = |x: f64| -> usize {
            let col = (((x - x_min) / x_range) * (w as f64)).round() as i32;
            col.clamp(0, (w as i32).saturating_sub(1)) as usize
        };
        let to_row = |y: f64| -> usize {
            let row_float = ((y - y_min) / y_range) * (h as f64);
            let row_from_bottom = row_float.round() as i32;
            let row_from_top = (h as i32 - 1) - row_from_bottom;
            row_from_top.clamp(0, (h as i32).saturating_sub(1)) as usize
        };

        let mut grid: Vec<Vec<char>> = vec![vec![' '; w]; h];

        for (&x, &y) in x_data.iter().zip(y_data.iter()) {
            let c = to_col(x);
            let r = to_row(y);
            if c < w && r < h {
                grid[r][c] = '·';
            }
        }

        let line_x_start = x_min;
        let line_x_end = x_max;
        let line_y_start = reg.slope * line_x_start + reg.intercept;
        let line_y_end = reg.slope * line_x_end + reg.intercept;
        let steps = w * 2;
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let x = line_x_start + t * (line_x_end - line_x_start);
            let y = line_y_start + t * (line_y_end - line_y_start);
            let c = to_col(x);
            let r = to_row(y);
            if c < w && r < h {
                if grid[r][c] == ' ' {
                    grid[r][c] = '─';
                } else {
                    grid[r][c] = '⊕';
                }
            }
        }

        let mut lines = vec![
            format!("Linear Regression: {} vs {}  R²={:.4}", y_label, x_label, reg.r_squared),
            format!("  Formula: y = {:.4}x + {:.4}", reg.slope, reg.intercept),
            format!("  (slope={:.4}, intercept={:.4})", reg.slope, reg.intercept),
        ];
        for row in &grid {
            lines.push(format!("  {}", row.iter().collect::<String>()));
        }
        lines.push(format!("  {} {}", x_label, y_label));
        lines.join("\n")
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
                svg_file_path: None,
                terminal_output: "No numeric columns available".to_string(),
                title: "Correlation Heatmap".to_string(),
            });
        }

        let mut buffer = String::new();
        {
            let root = SVGBackend::with_string(&mut buffer, (self.width, self.height)).into_drawing_area();
            root.fill(&BG_LIGHT_GRAY)?;

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

        let terminal_output = Self::render_heatmap_ascii(&corr_matrix, &cols)?;
        let svg_file_path = Self::save_svg_to_temp(&buffer, "rdata-heatmap").ok();
        Ok(ChartData {
            chart_type: VisualizationType::Heatmap,
            svg_output: buffer,
            svg_file_path,
            terminal_output,
            title: "Correlation Heatmap".to_string(),
        })
    }

    fn render_heatmap_ascii(corr_matrix: &DataFrame, cols: &[String]) -> Result<String> {
        let n = cols.len();
        let mut lines = vec!["Correlation Heatmap (values)".to_string()];
        let cell_w = 7usize;
        let header: String = cols.iter()
            .map(|c| format!("{:>width$}", c.chars().take(5).collect::<String>(), width = cell_w))
            .collect::<Vec<_>>()
            .join(" ");
        lines.push(format!("       {}", header));
        for i in 0..n {
            let mut row_str = format!("{:>5} ", cols[i].chars().take(5).collect::<String>());
            for j in 0..n {
                let series = corr_matrix.column(&cols[j])?;
                let val = series.f64()?.get(i).unwrap_or(0.0);
                row_str.push_str(&format!("{:>width$} ", format!("{:.2}", val), width = cell_w - 1));
            }
            lines.push(row_str);
        }
        Ok(lines.join("\n"))
    }

    fn render_expression_trend(
        &self,
        df: &DataFrame,
        config: &ExpressionTrendConfig,
    ) -> Result<ChartData> {
        let trend_data = StatisticalAnalyzer::expression_trend(
            df,
            &config.gene_column,
            &config.age_columns,
            &config.gene_ids,
        )?;
        if trend_data.is_empty() {
            return Ok(ChartData {
                chart_type: VisualizationType::ExpressionTrend,
                svg_output: "No data".to_string(),
                svg_file_path: None,
                terminal_output: "No expression trend data".to_string(),
                title: "Expression Trend".to_string(),
            });
        }

        let all_x: Vec<f64> = trend_data
            .iter()
            .flat_map(|d| d.points.iter().map(|p| p.age))
            .collect();
        let all_y: Vec<f64> = trend_data
            .iter()
            .flat_map(|d| d.points.iter().map(|p| p.expression))
            .collect();
        let x_min = all_x.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let x_max = all_x.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let y_min = all_y.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let y_max = all_y.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let x_range = (x_max - x_min).max(0.1);
        let y_range = (y_max - y_min).max(0.1);

        let mut buffer = String::new();
        {
            let root = SVGBackend::with_string(&mut buffer, (self.width, self.height))
                .into_drawing_area();
            root.fill(&BG_LIGHT_GRAY)?;

            let gene_labels_str: String = trend_data.iter().map(|d| d.gene_id.as_str()).collect::<Vec<_>>().join(", ");
            let caption = if gene_labels_str.len() > 70 {
                format!("Expression vs Age: {}...", &gene_labels_str[..67])
            } else {
                format!("Expression vs Age: {}", gene_labels_str)
            };
            let mut chart = ChartBuilder::on(&root)
                .caption(
                    caption,
                    ("sans-serif", 22).into_font().color(&BLACK),
                )
                .x_label_area_size(50)
                .y_label_area_size(50)
                .margin(12)
                .build_cartesian_2d(
                    (x_min - 0.05 * x_range)..(x_max + 0.05 * x_range),
                    (y_min - 0.05 * y_range)..(y_max + 0.05 * y_range),
                )?;

            chart
                .configure_mesh()
                .axis_style(GRID_GRAY.stroke_width(1))
                .x_desc("Age")
                .y_desc("Expression")
                .draw()?;

            let colors = [&STEEL_BLUE, &CORAL, &GREEN];
            for (i, data) in trend_data.iter().enumerate() {
                let color = colors[i % colors.len()];
                let points: Vec<(f64, f64)> = data
                    .points
                    .iter()
                    .map(|p| (p.age, p.expression))
                    .collect();
                chart.draw_series(LineSeries::new(
                    points.iter().copied(),
                    color.stroke_width(2),
                ))?;
                chart.draw_series(
                    points
                        .iter()
                        .map(|&(x, y)| Circle::new((x, y), 4, color.filled())),
                )?;
            }
        }

        let gene_labels: Vec<&str> = trend_data.iter().map(|d| d.gene_id.as_str()).collect();
        let terminal_output = format!(
            "Expression Trend: {} gene(s)\n  Genes: {}\n  Ages: {:.0}-{:.0}",
            trend_data.len(),
            gene_labels.join(", "),
            x_min,
            x_max
        );
        let svg_file_path = Self::save_svg_to_temp(&buffer, "rdata-trend").ok();
        Ok(ChartData {
            chart_type: VisualizationType::ExpressionTrend,
            svg_output: buffer,
            svg_file_path,
            terminal_output,
            title: "Expression vs Age".to_string(),
        })
    }

    fn render_young_vs_old(&self, df: &DataFrame, config: &YoungVsOldConfig) -> Result<ChartData> {
        let points = if !config.young_ages.is_empty() && !config.old_ages.is_empty() {
            StatisticalAnalyzer::young_vs_old_with_groups(
                df,
                &config.gene_column,
                &config.young_ages,
                &config.old_ages,
            )?
        } else {
            StatisticalAnalyzer::young_vs_old(
                df,
                &config.gene_column,
                &config.age_columns,
            )?
        };
        if points.is_empty() {
            return Ok(ChartData {
                chart_type: VisualizationType::YoungVsOldScatter,
                svg_output: "No data".to_string(),
                svg_file_path: None,
                terminal_output: "No Young vs Old data".to_string(),
                title: "Young vs Old Scatter".to_string(),
            });
        }

        let x_min = points.iter().map(|p| p.mean_young).fold(f64::INFINITY, f64::min);
        let x_max = points.iter().map(|p| p.mean_young).fold(f64::NEG_INFINITY, f64::max);
        let y_min = points.iter().map(|p| p.mean_old).fold(f64::INFINITY, f64::min);
        let y_max = points.iter().map(|p| p.mean_old).fold(f64::NEG_INFINITY, f64::max);
        let x_range = (x_max - x_min).max(0.1);
        let y_range = (y_max - y_min).max(0.1);

        let mut buffer = String::new();
        {
            let root = SVGBackend::with_string(&mut buffer, (self.width, self.height))
                .into_drawing_area();
            root.fill(&BG_LIGHT_GRAY)?;

            let mut chart = ChartBuilder::on(&root)
                .caption(
                    "Young vs Old Expression",
                    ("sans-serif", 28).into_font().color(&BLACK),
                )
                .x_label_area_size(50)
                .y_label_area_size(50)
                .margin(12)
                .build_cartesian_2d(
                    (x_min - 0.05 * x_range)..(x_max + 0.05 * x_range),
                    (y_min - 0.05 * y_range)..(y_max + 0.05 * y_range),
                )?;

            chart
                .configure_mesh()
                .axis_style(GRID_GRAY.stroke_width(1))
                .x_desc("Mean expression (Young)")
                .y_desc("Mean expression (Old)")
                .draw()?;

            chart.draw_series(
                points
                    .iter()
                    .map(|p| Circle::new((p.mean_young, p.mean_old), 3, STEEL_BLUE.filled())),
            )?;

            let diag_min = x_min.min(y_min);
            let diag_max = x_max.max(y_max);
            chart.draw_series(std::iter::once(PathElement::new(
                vec![(diag_min, diag_min), (diag_max, diag_max)],
                CORAL.stroke_width(1),
            )))?;
        }

        let sample_ids: Vec<&str> = points.iter().take(5).map(|p| p.gene_id.as_str()).collect();
        let terminal_output = format!(
            "Young vs Old: {} genes\n  Sample: {}\n  Diagonal = no change",
            points.len(),
            sample_ids.join(", ")
        );
        let svg_file_path = Self::save_svg_to_temp(&buffer, "rdata-youngold").ok();
        Ok(ChartData {
            chart_type: VisualizationType::YoungVsOldScatter,
            svg_output: buffer,
            svg_file_path,
            terminal_output,
            title: "Young vs Old Scatter".to_string(),
        })
    }

    fn render_age_group_boxplot(
        &self,
        df: &DataFrame,
        config: &AgeGroupBoxPlotConfig,
    ) -> Result<ChartData> {
        let box_data = StatisticalAnalyzer::age_group_box_data(
            df,
            &config.gene_column,
            &config.age_columns,
        )?;
        if box_data.is_empty() {
            return Ok(ChartData {
                chart_type: VisualizationType::AgeGroupBoxPlot,
                svg_output: "No data".to_string(),
                svg_file_path: None,
                terminal_output: "No age group data".to_string(),
                title: "Age Group Box Plot".to_string(),
            });
        }

        let mut all_vals = Vec::new();
        for b in &box_data {
            all_vals.extend(&b.values);
        }
        all_vals.sort_by(|a: &f64, b: &f64| a.partial_cmp(b).unwrap());
        let y_min = all_vals.first().copied().unwrap_or(0.0);
        let y_max = all_vals.last().copied().unwrap_or(1.0);
        let y_range = (y_max - y_min).max(0.1);

        let n = box_data.len();
        let mut buffer = String::new();
        {
            let root = SVGBackend::with_string(&mut buffer, (self.width, self.height))
                .into_drawing_area();
            root.fill(&BG_LIGHT_GRAY)?;

            let mut chart = ChartBuilder::on(&root)
                .caption(
                    "Expression by Age",
                    ("sans-serif", 28).into_font().color(&BLACK),
                )
                .x_label_area_size(60)
                .y_label_area_size(50)
                .margin(12)
                .build_cartesian_2d(
                    0f64..(n as f64 + 1.0),
                    (y_min - 0.05 * y_range)..(y_max + 0.05 * y_range),
                )?;

            chart
                .configure_mesh()
                .axis_style(GRID_GRAY.stroke_width(1))
                .x_desc("Age")
                .y_desc("Expression")
                .draw()?;

            for (i, b) in box_data.iter().enumerate() {
                let mut vals = b.values.clone();
                vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
                if vals.is_empty() {
                    continue;
                }
                let q1 = percentile(&vals, 25.0);
                let _median = percentile(&vals, 50.0);
                let q3 = percentile(&vals, 75.0);
                let x_center = (i + 1) as f64;
                let box_w = 0.3;

                chart.draw_series(std::iter::once(PathElement::new(
                    vec![(x_center, *vals.first().unwrap()), (x_center, q3)],
                    BLACK.stroke_width(2),
                )))?;
                chart.draw_series(std::iter::once(PathElement::new(
                    vec![(x_center, q1), (x_center, *vals.last().unwrap())],
                    BLACK.stroke_width(2),
                )))?;
                chart.draw_series(std::iter::once(Rectangle::new(
                    [
                        (x_center - box_w / 2.0, q1),
                        (x_center + box_w / 2.0, q3),
                    ],
                    STEEL_BLUE.stroke_width(2).filled(),
                )))?;
                chart.draw_series(std::iter::once(PathElement::new(
                    vec![
                        (x_center - 0.05, *vals.last().unwrap()),
                        (x_center + 0.05, *vals.last().unwrap()),
                    ],
                    BLACK.stroke_width(2),
                )))?;
                chart.draw_series(std::iter::once(PathElement::new(
                    vec![
                        (x_center - 0.05, *vals.first().unwrap()),
                        (x_center + 0.05, *vals.first().unwrap()),
                    ],
                    BLACK.stroke_width(2),
                )))?;
            }
        }

        let terminal_output = format!(
            "Age Group Box Plot: {} age columns",
            box_data.len()
        );
        let svg_file_path = Self::save_svg_to_temp(&buffer, "rdata-agebox").ok();
        Ok(ChartData {
            chart_type: VisualizationType::AgeGroupBoxPlot,
            svg_output: buffer,
            svg_file_path,
            terminal_output,
            title: "Expression by Age".to_string(),
        })
    }

    fn render_correlation_scatter(&self, config: &CorrelationScatterConfig) -> Result<ChartData> {
        if config.points.is_empty() {
            return Ok(ChartData {
                chart_type: VisualizationType::CorrelationScatter,
                svg_output: "No data".to_string(),
                svg_file_path: None,
                terminal_output: "No correlation data".to_string(),
                title: "Correlation vs p-value".to_string(),
            });
        }
        let log_p = |p: f64| -(p.max(1e-10)).log10();
        let x: Vec<f64> = config.points.iter().map(|p| p.correlation).collect();
        let y: Vec<f64> = config.points.iter().map(|p| log_p(p.p_value)).collect();
        let x_min = x.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let x_max = x.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let y_min = y.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let y_max = y.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let x_range = (x_max - x_min).max(0.2);
        let y_range = (y_max - y_min).max(0.2);

        let mut buffer = String::new();
        {
            let root = SVGBackend::with_string(&mut buffer, (self.width, self.height))
                .into_drawing_area();
            root.fill(&BG_LIGHT_GRAY)?;

            let mut chart = ChartBuilder::on(&root)
                .caption(
                    "Correlation vs -log10(p-value)",
                    ("sans-serif", 24).into_font().color(&BLACK),
                )
                .x_label_area_size(50)
                .y_label_area_size(50)
                .margin(12)
                .build_cartesian_2d(
                    (x_min - 0.05 * x_range)..(x_max + 0.05 * x_range),
                    (y_min - 0.05 * y_range)..(y_max + 0.05 * y_range),
                )?;

            chart
                .configure_mesh()
                .axis_style(GRID_GRAY.stroke_width(1))
                .x_desc("Correlation")
                .y_desc("-log10(p-value)")
                .draw()?;

            let pos_points: Vec<_> = config.points.iter().filter(|p| p.direction == "positive").collect();
            let neg_points: Vec<_> = config.points.iter().filter(|p| p.direction == "negative").collect();
            chart.draw_series(pos_points.iter().map(|p| {
                Circle::new((p.correlation, log_p(p.p_value)), 4, CORAL.filled())
            }))?;
            chart.draw_series(neg_points.iter().map(|p| {
                Circle::new((p.correlation, log_p(p.p_value)), 4, STEEL_BLUE.filled())
            }))?;
            if config.points.len() <= 50 {
                for p in config.points.iter().filter(|p| p.significant) {
                    chart.draw_series(std::iter::once(Text::new(
                        p.gene_id.chars().take(12).collect::<String>(),
                        (p.correlation, log_p(p.p_value)),
                        ("sans-serif", 10).into_font().color(&BLACK),
                    )))?;
                }
            }
        }

        let terminal_output = format!(
            "Correlation Scatter: {} genes (red=positive, blue=negative)",
            config.points.len()
        );
        let svg_file_path = Self::save_svg_to_temp(&buffer, "rdata-corr-scatter").ok();
        Ok(ChartData {
            chart_type: VisualizationType::CorrelationScatter,
            svg_output: buffer,
            svg_file_path,
            terminal_output,
            title: "Correlation vs -log10(p-value)".to_string(),
        })
    }

    fn render_correlation_bar_chart(&self, config: &CorrelationBarChartConfig) -> Result<ChartData> {
        let top: Vec<_> = config
            .points
            .iter()
            .map(|p| (p.gene_id.clone(), p.correlation.abs()))
            .collect::<Vec<_>>();
        let mut sorted = top.clone();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let take_n = config.top_n.min(sorted.len());
        let bars: Vec<_> = sorted.into_iter().take(take_n).collect();

        if bars.is_empty() {
            return Ok(ChartData {
                chart_type: VisualizationType::CorrelationBarChart,
                svg_output: "No data".to_string(),
                svg_file_path: None,
                terminal_output: "No data".to_string(),
                title: "Top Genes by |Correlation|".to_string(),
            });
        }

        let max_val = bars.iter().map(|(_, v)| *v).fold(0.0f64, f64::max).max(0.1);
        let n = bars.len() as f64;

        let mut buffer = String::new();
        {
            let root = SVGBackend::with_string(&mut buffer, (self.width, self.height))
                .into_drawing_area();
            root.fill(&BG_LIGHT_GRAY)?;

            let mut chart = ChartBuilder::on(&root)
                .caption(
                    "Top Genes by |Correlation|",
                    ("sans-serif", 24).into_font().color(&BLACK),
                )
                .x_label_area_size(80)
                .y_label_area_size(50)
                .margin(12)
                .build_cartesian_2d(0.0..(n + 1.0), 0.0..(max_val * 1.1))?;

            chart
                .configure_mesh()
                .axis_style(GRID_GRAY.stroke_width(1))
                .x_desc("Gene ID (Ensembl)")
                .y_desc("|Correlation|")
                .draw()?;

            for (i, (gene_id, val)) in bars.iter().enumerate() {
                let x = i as f64;
                chart.draw_series(std::iter::once(Rectangle::new(
                    [(x, 0.0), (x + 1.0, *val)],
                    STEEL_BLUE.filled(),
                )))?;
                chart.draw_series(std::iter::once(Text::new(
                    gene_id.chars().take(14).collect::<String>(),
                    (x + 0.5, 0.0),
                    ("sans-serif", 9).into_font().color(&BLACK),
                )))?;
            }
        }

        let terminal_output = format!(
            "Top {} genes by |correlation|\n  {}",
            take_n,
            bars.iter().map(|(g, _)| g.as_str()).take(5).collect::<Vec<_>>().join(", ")
        );
        let svg_file_path = Self::save_svg_to_temp(&buffer, "rdata-corr-bar").ok();
        Ok(ChartData {
            chart_type: VisualizationType::CorrelationBarChart,
            svg_output: buffer,
            svg_file_path,
            terminal_output,
            title: "Top Genes by |Correlation|".to_string(),
        })
    }

    fn render_volcano_plot(&self, config: &VolcanoPlotConfig) -> Result<ChartData> {
        if config.points.is_empty() {
            return Ok(ChartData {
                chart_type: VisualizationType::VolcanoPlot,
                svg_output: "No data".to_string(),
                svg_file_path: None,
                terminal_output: "No data".to_string(),
                title: "Volcano Plot".to_string(),
            });
        }
        let log_p = |p: f64| -(p.max(1e-10)).log10();
        let x: Vec<f64> = config.points.iter().map(|p| p.correlation).collect();
        let y: Vec<f64> = config.points.iter().map(|p| log_p(p.p_value)).collect();
        let x_min = x.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let x_max = x.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let y_min = y.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let y_max = y.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let x_range = (x_max - x_min).max(0.2);
        let y_range = (y_max - y_min).max(0.2);

        let mut buffer = String::new();
        {
            let root = SVGBackend::with_string(&mut buffer, (self.width, self.height))
                .into_drawing_area();
            root.fill(&BG_LIGHT_GRAY)?;

            let mut chart = ChartBuilder::on(&root)
                .caption(
                    "Volcano: -log10(p) vs Correlation",
                    ("sans-serif", 24).into_font().color(&BLACK),
                )
                .x_label_area_size(50)
                .y_label_area_size(50)
                .margin(12)
                .build_cartesian_2d(
                    (x_min - 0.05 * x_range)..(x_max + 0.05 * x_range),
                    (y_min - 0.05 * y_range)..(y_max + 0.05 * y_range),
                )?;

            chart
                .configure_mesh()
                .axis_style(GRID_GRAY.stroke_width(1))
                .x_desc("Correlation")
                .y_desc("-log10(p-value)")
                .draw()?;

            for p in &config.points {
                let color = if p.direction == "positive" {
                    CORAL.filled()
                } else {
                    STEEL_BLUE.filled()
                };
                chart.draw_series(std::iter::once(Circle::new(
                    (p.correlation, log_p(p.p_value)),
                    3,
                    color,
                )))?;
            }
            if config.points.len() <= 80 {
                for p in &config.points {
                    if p.significant {
                        chart.draw_series(std::iter::once(Text::new(
                            p.gene_id.chars().take(12).collect::<String>(),
                            (p.correlation, log_p(p.p_value)),
                            ("sans-serif", 8).into_font().color(&BLACK),
                        )))?;
                    }
                }
            }
        }

        let mut terminal_output = format!(
            "Volcano: {} genes (red=positive, blue=negative)",
            config.points.len()
        );
        if let Some(ref tables) = config.gene_tables {
            terminal_output.push_str("\n\n");
            terminal_output.push_str(tables);
        }
        let svg_file_path = Self::save_svg_to_temp(&buffer, "rdata-volcano").ok();
        Ok(ChartData {
            chart_type: VisualizationType::VolcanoPlot,
            svg_output: buffer,
            svg_file_path,
            terminal_output,
            title: "Volcano Plot".to_string(),
        })
    }

    fn render_expression_vs_age_regression(
        &self,
        df: &DataFrame,
        config: &ExpressionVsAgeRegressionConfig,
    ) -> Result<ChartData> {
        let trend_data = StatisticalAnalyzer::expression_trend(
            df,
            &config.gene_column,
            &config.age_columns,
            &config.gene_ids,
        )?;
        if trend_data.is_empty() {
            return Ok(ChartData {
                chart_type: VisualizationType::ExpressionVsAgeRegression,
                svg_output: "No data".to_string(),
                svg_file_path: None,
                terminal_output: "No data".to_string(),
                title: "Expression vs Age (Regression)".to_string(),
            });
        }

        let mut all_x = Vec::new();
        let mut all_y = Vec::new();
        for d in &trend_data {
            for p in &d.points {
                all_x.push(p.age);
                all_y.push(p.expression);
            }
        }
        let x_min = all_x.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let x_max = all_x.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let y_min = all_y.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let y_max = all_y.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let x_range = (x_max - x_min).max(0.1);
        let y_range = (y_max - y_min).max(0.1);

        let mut buffer = String::new();
        let mut formulas = Vec::new();
        {
            let root = SVGBackend::with_string(&mut buffer, (self.width, self.height))
                .into_drawing_area();
            root.fill(&BG_LIGHT_GRAY)?;

            let mut chart = ChartBuilder::on(&root)
                .caption(
                    "Expression vs Age (Regression)",
                    ("sans-serif", 22).into_font().color(&BLACK),
                )
                .x_label_area_size(50)
                .y_label_area_size(50)
                .margin(12)
                .build_cartesian_2d(
                    (x_min - 0.05 * x_range)..(x_max + 0.05 * x_range),
                    (y_min - 0.05 * y_range)..(y_max + 0.05 * y_range),
                )?;

            chart
                .configure_mesh()
                .axis_style(GRID_GRAY.stroke_width(1))
                .x_desc("Age")
                .y_desc("Expression")
                .draw()?;

            let colors = [&STEEL_BLUE, &CORAL, &GREEN, &RED, &BLUE];
            for (i, data) in trend_data.iter().enumerate() {
                let color = colors[i % colors.len()];
                let x_vals: Vec<f64> = data.points.iter().map(|p| p.age).collect();
                let y_vals: Vec<f64> = data.points.iter().map(|p| p.expression).collect();
                let points: Vec<(f64, f64)> = x_vals.iter().zip(y_vals.iter()).map(|(a, b)| (*a, *b)).collect();
                chart.draw_series(
                    points
                        .iter()
                        .map(|&(x, y)| Circle::new((x, y), 5, color.filled())),
                )?;
                let n = x_vals.len() as f64;
                let sum_x: f64 = x_vals.iter().sum();
                let sum_y: f64 = y_vals.iter().sum();
                let sum_xy: f64 = x_vals.iter().zip(y_vals.iter()).map(|(a, b)| a * b).sum();
                let sum_x2: f64 = x_vals.iter().map(|a| a * a).sum();
                let slope = if (n * sum_x2 - sum_x * sum_x).abs() < 1e-10 {
                    0.0
                } else {
                    (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x * sum_x)
                };
                let intercept = (sum_y - slope * sum_x) / n;
                formulas.push(format!("  {}: y = {:.4}x + {:.4}", data.gene_id, slope, intercept));
                let line_start = x_min - 0.05 * x_range;
                let line_end = x_max + 0.05 * x_range;
                chart.draw_series(std::iter::once(PathElement::new(
                    vec![
                        (line_start, slope * line_start + intercept),
                        (line_end, slope * line_end + intercept),
                    ],
                    color.stroke_width(2),
                )))?;
                let label_x = points.last().map(|p| p.0).unwrap_or(x_max);
                let label_y = points.last().map(|p| p.1).unwrap_or(y_max);
                chart.draw_series(std::iter::once(Text::new(
                    data.gene_id.chars().take(14).collect::<String>(),
                    (label_x, label_y),
                    ("sans-serif", 10).into_font().color(color),
                )))?;
            }
        }

        let gene_labels: String = trend_data.iter().map(|d| d.gene_id.as_str()).collect::<Vec<_>>().join(", ");
        let terminal_output = format!(
            "Expression vs Age (Regression): {}\n  Genes: {}\n\nFormulas:\n{}",
            trend_data.len(),
            gene_labels,
            formulas.join("\n")
        );
        let svg_file_path = Self::save_svg_to_temp(&buffer, "rdata-regression").ok();
        Ok(ChartData {
            chart_type: VisualizationType::ExpressionVsAgeRegression,
            svg_output: buffer,
            svg_file_path,
            terminal_output,
            title: "Expression vs Age (Regression)".to_string(),
        })
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let n = sorted.len() as f64;
    let pos = (p / 100.0) * (n - 1.0);
    let lower = pos.floor() as usize;
    let upper = pos.ceil() as usize;
    if lower == upper {
        return sorted[lower];
    }
    let weight = pos - lower as f64;
    sorted[lower] * (1.0 - weight) + sorted[upper] * weight
}
