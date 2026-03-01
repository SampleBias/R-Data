// Analysis runner: executes statistical analyses without AI.
use anyhow::Result;
use crate::data::DataLoader;
use crate::viz::VisualizationConfig;

#[derive(Clone, Debug)]
pub enum AnalysisRequest {
    SummaryStats {
        /// When present (microarray layout), include gene-age correlation summary (R², p-value, correlation).
        gene_age_summary: Option<(String, Vec<String>)>,
        gene_filter: Option<std::collections::HashSet<String>>,
    },
    Correlation,
    Histogram { column: String, bins: usize },
    BoxPlot { column: String },
    LinearRegression { x_column: String, y_column: String },
    #[allow(dead_code)]
    Heatmap,
    ExpressionTrend {
        gene_ids: Vec<String>,
        gene_column: String,
        age_columns: Vec<String>,
    },
    YoungVsOld {
        gene_column: String,
        age_columns: Vec<String>,
        young_cols: Option<Vec<String>>,
        old_cols: Option<Vec<String>>,
    },
    AgeGroupBoxPlot {
        gene_column: String,
        age_columns: Vec<String>,
    },
    /// Expression vs age per gene (microarray). Linear regression: expression ~ age.
    GenesExpressionVsAge {
        gene_column: String,
        age_columns: Vec<String>,
        gene_filter: Option<std::collections::HashSet<String>>,
    },
    /// Genes statistically significant with age (p<0.05), positively or negatively correlated.
    GenesSignificantWithAge {
        gene_column: String,
        age_columns: Vec<String>,
        gene_filter: Option<std::collections::HashSet<String>>,
    },
    GenesCorrelationScatter {
        gene_column: String,
        age_columns: Vec<String>,
        gene_filter: Option<std::collections::HashSet<String>>,
    },
    GenesCorrelationBarChart {
        gene_column: String,
        age_columns: Vec<String>,
        top_n: usize,
        gene_filter: Option<std::collections::HashSet<String>>,
    },
    GenesVolcanoPlot {
        gene_column: String,
        age_columns: Vec<String>,
        gene_filter: Option<std::collections::HashSet<String>>,
    },
    /// Expression vs age regression for 1–5 selected genes.
    ExpressionVsAgeRegression {
        gene_ids: Vec<String>,
        gene_column: String,
        age_columns: Vec<String>,
    },
    /// Expression heatmap: genes × ages (top N genes by |correlation|).
    ExpressionHeatmap {
        gene_column: String,
        age_columns: Vec<String>,
        top_n: usize,
    },
    /// Export gene correlation results (correlation, slope, R², p-value, fold-change) to CSV.
    ExportGeneCorrelation {
        gene_column: String,
        age_columns: Vec<String>,
    },
}

#[derive(Clone, Debug)]
pub struct AnalysisResult {
    pub summary: String,
    pub details: Option<String>,
    pub viz_config: Option<VisualizationConfig>,
}

pub struct AnalysisRunner;

impl AnalysisRunner {
    pub fn run(
        df: &polars::prelude::DataFrame,
        request: AnalysisRequest,
    ) -> Result<AnalysisResult> {
        match request {
            AnalysisRequest::SummaryStats { gene_age_summary, gene_filter } => {
                let stats = DataLoader::get_summary_stats(df)?;
                let mut details = format!("{}", stats);
                if let Some((gene_column, age_columns)) = gene_age_summary {
                    let results =
                        crate::data::StatisticalAnalyzer::genes_expression_vs_age(
                            df, &gene_column, &age_columns, gene_filter.as_ref(),
                        )?;
                    let n = results.len();
                    let mean_corr = results.iter().map(|r| r.correlation).sum::<f64>() / n.max(1) as f64;
                    let mean_r2 = results.iter().map(|r| r.r_squared).sum::<f64>() / n.max(1) as f64;
                    let mean_slope = results.iter().map(|r| r.slope).sum::<f64>() / n.max(1) as f64;
                    let mean_fc = results.iter().map(|r| r.fold_change).sum::<f64>() / n.max(1) as f64;
                    let n_sig = results.iter().filter(|r| r.significant).count();
                    let n_pos = results.iter().filter(|r| r.significant && r.correlation > 0.0).count();
                    let n_neg = results.iter().filter(|r| r.significant && r.correlation < 0.0).count();
                    let summary_section = format!(
                        "\n\n--- Gene-age correlation summary (R², p-value, correlation, slope, fold-change) ---\n\
                        Genes: {} | Mean correlation: {:.4} | Mean R²: {:.4} | Mean slope: {:.4} | Mean fold-change: {:.4}\n\
                        Significant (p<0.05): {} total ({} positive, {} negative)\n\
                        Press [g] for full table • [x] to export CSV",
                        n, mean_corr, mean_r2, mean_slope, mean_fc, n_sig, n_pos, n_neg
                    );
                    details.push_str(&summary_section);
                }
                Ok(AnalysisResult {
                    summary: format!("Statistical summary:\n{}", details),
                    details: Some(details),
                    viz_config: None,
                })
            }
            AnalysisRequest::Correlation => {
                let corr = crate::data::StatisticalAnalyzer::compute_correlation(df)?;
                let details = format!("{}", corr);
                let numeric_cols: Vec<String> = df
                    .get_columns()
                    .iter()
                    .filter(|col| col.dtype().is_numeric())
                    .map(|col| col.name().to_string())
                    .collect();
                Ok(AnalysisResult {
                    summary: format!("Correlation matrix:\n{}", details),
                    details: Some(details),
                    viz_config: Some(VisualizationConfig::Heatmap(
                        crate::viz::HeatmapConfig { columns: numeric_cols },
                    )),
                })
            }
            AnalysisRequest::Histogram { column, bins } => {
                let hist_data =
                    crate::data::StatisticalAnalyzer::compute_histogram_data(df, &column, bins)?;
                let details = format!(
                    "Histogram '{}': {} bins, range [{:.2}, {:.2}]",
                    column, bins, hist_data.min_val, hist_data.max_val
                );
                Ok(AnalysisResult {
                    summary: details.clone(),
                    details: Some(details),
                    viz_config: Some(VisualizationConfig::Histogram(
                        crate::viz::HistogramConfig { column, bins },
                    )),
                })
            }
            AnalysisRequest::BoxPlot { column } => {
                let box_data =
                    crate::data::StatisticalAnalyzer::compute_boxplot_data(df, &column)?;
                let details = format!(
                    "Box plot '{}': min={:.2}, Q1={:.2}, median={:.2}, Q3={:.2}, max={:.2}, outliers={}",
                    column, box_data.min, box_data.q1, box_data.median, box_data.q3, box_data.max,
                    box_data.outliers.len()
                );
                Ok(AnalysisResult {
                    summary: details.clone(),
                    details: Some(details),
                    viz_config: Some(VisualizationConfig::BoxPlot(
                        crate::viz::BoxPlotConfig { column },
                    )),
                })
            }
            AnalysisRequest::LinearRegression { x_column, y_column } => {
                let reg = crate::data::StatisticalAnalyzer::linear_regression(
                    df, &x_column, &y_column,
                )?;
                let details = format!(
                    "Linear regression: y = {:.4}x + {:.4}, R² = {:.4}",
                    reg.slope, reg.intercept, reg.r_squared
                );
                Ok(AnalysisResult {
                    summary: format!("Regression ({} vs {}):\n{}", y_column, x_column, details),
                    details: Some(details),
                    viz_config: Some(VisualizationConfig::LinearRegression(
                        crate::viz::LinearRegressionConfig {
                            x_column,
                            y_column,
                        },
                    )),
                })
            }
            AnalysisRequest::Heatmap => {
                let numeric_cols: Vec<String> = df
                    .get_columns()
                    .iter()
                    .filter(|col| col.dtype().is_numeric())
                    .map(|col| col.name().to_string())
                    .collect();
                Ok(AnalysisResult {
                    summary: "Correlation heatmap".to_string(),
                    details: None,
                    viz_config: Some(VisualizationConfig::Heatmap(
                        crate::viz::HeatmapConfig {
                            columns: numeric_cols,
                        },
                    )),
                })
            }
            AnalysisRequest::ExpressionTrend {
                gene_ids,
                gene_column,
                age_columns,
            } => {
                let trend_data =
                    crate::data::StatisticalAnalyzer::expression_trend(
                        df, &gene_column, &age_columns, &gene_ids,
                    )?;
                let summary = format!(
                    "Expression trend: {} gene(s)",
                    trend_data.len()
                );
                Ok(AnalysisResult {
                    summary: summary.clone(),
                    details: Some(summary),
                    viz_config: Some(VisualizationConfig::ExpressionTrend(
                        crate::viz::ExpressionTrendConfig {
                            gene_ids,
                            gene_column,
                            age_columns,
                        },
                    )),
                })
            }
            AnalysisRequest::ExpressionVsAgeRegression {
                gene_ids,
                gene_column,
                age_columns,
            } => {
                let trend_data =
                    crate::data::StatisticalAnalyzer::expression_trend(
                        df, &gene_column, &age_columns, &gene_ids,
                    )?;
                let summary = format!(
                    "Expression vs age regression: {} gene(s)",
                    trend_data.len()
                );
                Ok(AnalysisResult {
                    summary: summary.clone(),
                    details: Some(summary),
                    viz_config: Some(VisualizationConfig::ExpressionVsAgeRegression(
                        crate::viz::ExpressionVsAgeRegressionConfig {
                            gene_ids,
                            gene_column,
                            age_columns,
                        },
                    )),
                })
            }
            AnalysisRequest::YoungVsOld {
                gene_column,
                age_columns,
                young_cols,
                old_cols,
            } => {
                let (points, young_ages, old_ages) = match (young_cols.as_ref(), old_cols.as_ref()) {
                    (Some(young), Some(old)) => {
                        let pts = crate::data::StatisticalAnalyzer::young_vs_old_with_groups(
                            df, &gene_column, young, old,
                        )?;
                        (pts, young.clone(), old.clone())
                    }
                    _ => {
                        let pts = crate::data::StatisticalAnalyzer::young_vs_old(
                            df, &gene_column, &age_columns,
                        )?;
                        let ages: Vec<i64> = age_columns
                            .iter()
                            .filter_map(|s| s.trim().parse().ok())
                            .collect();
                        let mut sorted = ages.clone();
                        sorted.sort();
                        let median = sorted[sorted.len() / 2];
                        let ya: Vec<String> = age_columns
                            .iter()
                            .filter(|s| s.trim().parse::<i64>().unwrap_or(0) < median)
                            .cloned()
                            .collect();
                        let oa: Vec<String> = age_columns
                            .iter()
                            .filter(|s| s.trim().parse::<i64>().unwrap_or(0) >= median)
                            .cloned()
                            .collect();
                        (pts, ya, oa)
                    }
                };
                let summary = format!(
                    "Young vs Old scatter: {} genes",
                    points.len()
                );
                Ok(AnalysisResult {
                    summary: summary.clone(),
                    details: Some(summary),
                    viz_config: Some(VisualizationConfig::YoungVsOldScatter(
                        crate::viz::YoungVsOldConfig {
                            gene_column,
                            age_columns,
                            young_ages,
                            old_ages,
                        },
                    )),
                })
            }
            AnalysisRequest::AgeGroupBoxPlot {
                gene_column,
                age_columns,
            } => {
                let box_data =
                    crate::data::StatisticalAnalyzer::age_group_box_data(
                        df, &gene_column, &age_columns,
                    )?;
                let summary = format!(
                    "Age group box plot: {} age columns",
                    box_data.len()
                );
                Ok(AnalysisResult {
                    summary: summary.clone(),
                    details: Some(summary),
                    viz_config: Some(VisualizationConfig::AgeGroupBoxPlot(
                        crate::viz::AgeGroupBoxPlotConfig {
                            gene_column,
                            age_columns,
                        },
                    )),
                })
            }
            AnalysisRequest::GenesExpressionVsAge {
                gene_column,
                age_columns,
                gene_filter,
            } => {
                let results =
                    crate::data::StatisticalAnalyzer::genes_expression_vs_age(
                        df, &gene_column, &age_columns, gene_filter.as_ref(),
                    )?;
                let mut sorted = results.clone();
                sorted.sort_by(|a, b| a.p_value.partial_cmp(&b.p_value).unwrap());
                let n_sig_pos = sorted.iter().filter(|r| r.significant && r.correlation > 0.0).count();
                let n_sig_neg = sorted.iter().filter(|r| r.significant && r.correlation < 0.0).count();
                let summary = format!(
                    "Expression vs age: {} genes. Significant (p<0.05): {} positive, {} negative",
                    sorted.len(),
                    n_sig_pos,
                    n_sig_neg
                );
                let table = format_genes_age_table(&sorted, false);
                let points = to_gene_correlation_points(&sorted);
                Ok(AnalysisResult {
                    summary: summary.clone(),
                    details: Some(format!("{}\n\n{}", summary, table)),
                    viz_config: Some(VisualizationConfig::VolcanoPlot(
                        crate::viz::VolcanoPlotConfig {
                            points,
                            gene_tables: None,
                        },
                    )),
                })
            }
            AnalysisRequest::GenesSignificantWithAge {
                gene_column,
                age_columns,
                gene_filter,
            } => {
                let results =
                    crate::data::StatisticalAnalyzer::genes_expression_vs_age(
                        df, &gene_column, &age_columns, gene_filter.as_ref(),
                    )?;
                let mut significant: Vec<_> = results.into_iter().filter(|r| r.significant).collect();
                significant.sort_by(|a, b| a.p_value.partial_cmp(&b.p_value).unwrap());
                let pos: Vec<_> = significant.iter().filter(|r| r.correlation > 0.0).cloned().collect();
                let neg: Vec<_> = significant.iter().filter(|r| r.correlation < 0.0).cloned().collect();
                let n_pos = pos.len();
                let n_neg = neg.len();
                let summary = format!(
                    "Genes significant with age (p<0.05): {} total ({} positive, {} negative)",
                    significant.len(),
                    n_pos,
                    n_neg
                );
                let details = format_genes_significant_pos_neg(&pos, &neg);
                let points = to_gene_correlation_points(&significant);
                Ok(AnalysisResult {
                    summary: summary.clone(),
                    details: Some(format!("{}\n\n{}", summary, details)),
                    viz_config: Some(VisualizationConfig::VolcanoPlot(
                        crate::viz::VolcanoPlotConfig {
                            points,
                            gene_tables: Some(details),
                        },
                    )),
                })
            }
            AnalysisRequest::GenesCorrelationScatter {
                gene_column,
                age_columns,
                gene_filter,
            } => {
                let results =
                    crate::data::StatisticalAnalyzer::genes_expression_vs_age(
                        df, &gene_column, &age_columns, gene_filter.as_ref(),
                    )?;
                let mut sorted = results.clone();
                sorted.sort_by(|a, b| a.p_value.partial_cmp(&b.p_value).unwrap());
                let points = to_gene_correlation_points(&sorted);
                let summary = format!("Correlation scatter: {} genes", points.len());
                Ok(AnalysisResult {
                    summary: summary.clone(),
                    details: Some(summary),
                    viz_config: Some(VisualizationConfig::CorrelationScatter(
                        crate::viz::CorrelationScatterConfig { points },
                    )),
                })
            }
            AnalysisRequest::GenesCorrelationBarChart {
                gene_column,
                age_columns,
                top_n,
                gene_filter,
            } => {
                let results =
                    crate::data::StatisticalAnalyzer::genes_expression_vs_age(
                        df, &gene_column, &age_columns, gene_filter.as_ref(),
                    )?;
                let mut sorted = results.clone();
                sorted.sort_by(|a, b| a.correlation.abs().partial_cmp(&b.correlation.abs()).unwrap());
                sorted.reverse();
                let points = to_gene_correlation_points(&sorted);
                let summary = format!("Top {} genes by |correlation|", top_n.min(points.len()));
                Ok(AnalysisResult {
                    summary: summary.clone(),
                    details: Some(summary),
                    viz_config: Some(VisualizationConfig::CorrelationBarChart(
                        crate::viz::CorrelationBarChartConfig { points, top_n },
                    )),
                })
            }
            AnalysisRequest::GenesVolcanoPlot {
                gene_column,
                age_columns,
                gene_filter,
            } => {
                let results =
                    crate::data::StatisticalAnalyzer::genes_expression_vs_age(
                        df, &gene_column, &age_columns, gene_filter.as_ref(),
                    )?;
                let mut sorted = results.clone();
                sorted.sort_by(|a, b| a.p_value.partial_cmp(&b.p_value).unwrap());
                let points = to_gene_correlation_points(&sorted);
                let summary = format!("Volcano plot: {} genes", points.len());
                Ok(AnalysisResult {
                    summary: summary.clone(),
                    details: Some(summary),
                    viz_config: Some(VisualizationConfig::VolcanoPlot(
                        crate::viz::VolcanoPlotConfig {
                            points,
                            gene_tables: None,
                        },
                    )),
                })
            }
            AnalysisRequest::ExpressionHeatmap {
                gene_column,
                age_columns,
                top_n,
            } => {
                let results =
                    crate::data::StatisticalAnalyzer::genes_expression_vs_age(
                        df, &gene_column, &age_columns, None,
                    )?;
                let mut sorted = results.clone();
                sorted.sort_by(|a, b| a.correlation.abs().partial_cmp(&b.correlation.abs()).unwrap());
                sorted.reverse();
                let gene_ids: Vec<String> = sorted
                    .iter()
                    .take(top_n)
                    .map(|r| r.gene_id.clone())
                    .collect();
                let summary = format!(
                    "Expression heatmap: top {} genes by |correlation|",
                    gene_ids.len()
                );
                Ok(AnalysisResult {
                    summary: summary.clone(),
                    details: Some(summary),
                    viz_config: Some(VisualizationConfig::ExpressionHeatmap(
                        crate::viz::ExpressionHeatmapConfig {
                            gene_ids,
                            gene_column,
                            age_columns,
                        },
                    )),
                })
            }
            AnalysisRequest::ExportGeneCorrelation {
                gene_column,
                age_columns,
            } => {
                let results =
                    crate::data::StatisticalAnalyzer::genes_expression_vs_age(
                        df, &gene_column, &age_columns, None,
                    )?;
                let path = export_gene_correlation_csv(&results)?;
                let summary = format!(
                    "Exported {} genes to CSV:\n{}",
                    results.len(),
                    path.display()
                );
                Ok(AnalysisResult {
                    summary: summary.clone(),
                    details: Some(summary),
                    viz_config: None,
                })
            }
        }
    }
}

fn export_gene_correlation_csv(
    results: &[crate::data::GeneAgeCorrelation],
) -> Result<std::path::PathBuf> {
    use std::io::Write;
    let mut temp = tempfile::Builder::new()
        .prefix("rdata-gene-correlation-")
        .suffix(".csv")
        .tempfile()?;
    writeln!(
        temp,
        "gene_id,correlation,slope,r_squared,p_value,fold_change,significant,direction"
    )?;
    for r in results {
        writeln!(
            temp,
            "{},{:.6},{:.6},{:.6},{:.6},{:.6},{},{}",
            r.gene_id,
            r.correlation,
            r.slope,
            r.r_squared,
            r.p_value,
            r.fold_change,
            r.significant,
            r.direction
        )?;
    }
    temp.flush()?;
    let path = temp.into_temp_path().keep()?;
    Ok(path.into())
}

fn to_gene_correlation_points(
    results: &[crate::data::GeneAgeCorrelation],
) -> Vec<crate::viz::GeneCorrelationPoint> {
    results
        .iter()
        .map(|r| crate::viz::GeneCorrelationPoint {
            gene_id: r.gene_id.clone(),
            correlation: r.correlation,
            p_value: r.p_value,
            significant: r.significant,
            direction: r.direction.to_string(),
        })
        .collect()
}

fn format_genes_significant_pos_neg(
    pos: &[crate::data::GeneAgeCorrelation],
    neg: &[crate::data::GeneAgeCorrelation],
) -> String {
    let header = "Gene ID (Ensembl)     | Corr    | Slope   | R²     | p-value | FoldChg";
    let sep = "---------------------|--------|--------|--------|---------|--------";
    let mut lines = Vec::new();

    lines.push("POSITIVE (expression ↑ with age)".to_string());
    lines.push(header.to_string());
    lines.push(sep.to_string());
    if pos.is_empty() {
        lines.push("  (none)".to_string());
    } else {
        for r in pos.iter().take(200) {
            lines.push(format!(
                "{:<20} | {:>7.3} | {:>7.4} | {:>6.3} | {:>7.4} | {:>7.3}",
                r.gene_id.chars().take(20).collect::<String>(),
                r.correlation,
                r.slope,
                r.r_squared,
                r.p_value,
                r.fold_change,
            ));
        }
        if pos.len() > 200 {
            lines.push(format!("... and {} more positive", pos.len() - 200));
        }
    }
    lines.push("".to_string());
    lines.push("NEGATIVE (expression ↓ with age)".to_string());
    lines.push(header.to_string());
    lines.push(sep.to_string());
    if neg.is_empty() {
        lines.push("  (none)".to_string());
    } else {
        for r in neg.iter().take(200) {
            lines.push(format!(
                "{:<20} | {:>7.3} | {:>7.4} | {:>6.3} | {:>7.4} | {:>7.3}",
                r.gene_id.chars().take(20).collect::<String>(),
                r.correlation,
                r.slope,
                r.r_squared,
                r.p_value,
                r.fold_change,
            ));
        }
        if neg.len() > 200 {
            lines.push(format!("... and {} more negative", neg.len() - 200));
        }
    }
    lines.join("\n")
}

fn format_genes_age_table(
    results: &[crate::data::GeneAgeCorrelation],
    significant_only: bool,
) -> String {
    if results.is_empty() {
        return if significant_only {
            "No genes significant with age (p<0.05).".to_string()
        } else {
            "No results.".to_string()
        };
    }
    let header = "Gene ID (Ensembl)     | Corr    | Slope   | R²     | p-value | FoldChg | Dir";
    let sep = "---------------------|--------|--------|--------|---------|--------|-----";
    let mut lines = vec![header.to_string(), sep.to_string()];
    for r in results.iter().take(100) {
        let sig = if r.significant { "*" } else { " " };
        lines.push(format!(
            "{:<20} | {:>7.3} | {:>7.4} | {:>6.3} | {:>7.4} | {:>7.3} | {} {}",
            r.gene_id.chars().take(20).collect::<String>(),
            r.correlation,
            r.slope,
            r.r_squared,
            r.p_value,
            r.fold_change,
            r.direction.chars().take(3).collect::<String>(),
            sig
        ));
    }
    if results.len() > 100 {
        lines.push(format!("... and {} more", results.len() - 100));
    }
    lines.push("".to_string());
    lines.push("* = significant (p<0.05)".to_string());
    lines.join("\n")
}
