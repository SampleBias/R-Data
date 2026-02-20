use anyhow::{Context, Result};
use polars::prelude::*;
use polars::series::Series;

pub struct StatisticalAnalyzer;

impl StatisticalAnalyzer {
    pub fn compute_correlation(df: &DataFrame) -> Result<DataFrame> {
        let numeric_cols: Vec<String> = df
            .get_columns()
            .iter()
            .filter(|col| col.dtype().is_numeric())
            .map(|col| col.name().to_string())
            .collect();

        if numeric_cols.len() < 2 {
            return Err(anyhow::anyhow!("Need at least 2 numeric columns for correlation"));
        }

        let n = numeric_cols.len();
        let mut correlation_matrix: Vec<Vec<f64>> = vec![vec![0.0; n]; n];

        for i in 0..n {
            for j in 0..n {
                if i == j {
                    correlation_matrix[i][j] = 1.0;
                } else {
                    let series_i = df.column(&numeric_cols[i])?;
                    let series_j = df.column(&numeric_cols[j])?;
                    
                    let corr = Self::pearson_corr(series_i, series_j)?;
                    correlation_matrix[i][j] = corr;
                }
            }
        }

        Self::correlation_matrix_to_dataframe(
            numeric_cols.clone(),
            &correlation_matrix,
        )
    }

    fn pearson_corr(col_i: &Column, col_j: &Column) -> Result<f64> {
        let series_i = col_i.f64()?;
        let series_j = col_j.f64()?;
        
        let x: Vec<f64> = series_i.into_no_null_iter().collect();
        let y: Vec<f64> = series_j.into_no_null_iter().collect();
        
        if x.len() != y.len() || x.is_empty() {
            return Ok(0.0);
        }

        let n = x.len() as f64;
        let sum_x: f64 = x.iter().sum();
        let sum_y: f64 = y.iter().sum();
        let sum_xy: f64 = x.iter().zip(y.iter()).map(|(a, b)| a * b).sum();
        let sum_x2: f64 = x.iter().map(|a| a * a).sum();
        let sum_y2: f64 = y.iter().map(|a| a * a).sum();

        let numerator = n * sum_xy - sum_x * sum_y;
        let denominator = ((n * sum_x2 - sum_x * sum_x) * (n * sum_y2 - sum_y * sum_y)).sqrt();
        
        if denominator == 0.0 {
            Ok(0.0)
        } else {
            Ok(numerator / denominator)
        }
    }

    fn correlation_matrix_to_dataframe(
        col_names: Vec<String>,
        matrix: &[Vec<f64>],
    ) -> Result<DataFrame> {
        let n = col_names.len();
        let mut columns: Vec<Column> = Vec::new();

        for j in 0..n {
            let values: Vec<f64> = (0..n).map(|i| matrix[i][j]).collect();
            let series = Series::new(col_names[j].as_str().into(), values.as_slice());
            columns.push(series.into_column());
        }

        DataFrame::new(columns).map_err(|e| anyhow::anyhow!("Failed to create dataframe: {}", e))
    }

    pub fn linear_regression(
        df: &DataFrame,
        x_col: &str,
        y_col: &str,
    ) -> Result<RegressionResult> {
        let x_series = df
            .column(x_col)
            .context(format!("Column '{}' not found", x_col))?;
        let y_series = df
            .column(y_col)
            .context(format!("Column '{}' not found", y_col))?;

        let x: Vec<f64> = x_series
            .f64()
            .context(format!("Column '{}' is not numeric", x_col))?
            .into_no_null_iter()
            .collect();
        let y: Vec<f64> = y_series
            .f64()
            .context(format!("Column '{}' is not numeric", y_col))?
            .into_no_null_iter()
            .collect();

        if x.len() != y.len() || x.is_empty() {
            return Err(anyhow::anyhow!("Invalid data for regression"));
        }

        let n = x.len() as f64;
        let sum_x: f64 = x.iter().sum();
        let sum_y: f64 = y.iter().sum();
        let sum_xy: f64 = x.iter().zip(y.iter()).map(|(a, b)| a * b).sum();
        let sum_x2: f64 = x.iter().map(|a| a * a).sum();
        let _sum_y2: f64 = y.iter().map(|a| a * a).sum();

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x * sum_x);
        let intercept = (sum_y - slope * sum_x) / n;

        let y_pred: Vec<f64> = x.iter().map(|xi| slope * xi + intercept).collect();
        let residuals: Vec<f64> = y.iter().zip(y_pred.iter()).map(|(yi, yp)| yi - yp).collect();
        let ss_res: f64 = residuals.iter().map(|r| r * r).sum();
        let ss_tot: f64 = y
            .iter()
            .map(|yi| yi - sum_y / n)
            .map(|diff| diff * diff)
            .sum();
        let r_squared = if ss_tot == 0.0 { 1.0 } else { 1.0 - (ss_res / ss_tot) };

        Ok(RegressionResult {
            slope,
            intercept,
            r_squared,
            x_col: x_col.to_string(),
            y_col: y_col.to_string(),
            data_points: x.len(),
        })
    }

    pub fn compute_histogram_data(
        df: &DataFrame,
        col_name: &str,
        bins: usize,
    ) -> Result<HistogramData> {
        let series = df.column(col_name)?;
        let values: Vec<f64> = series
            .f64()
            .context(format!("Column '{}' is not numeric", col_name))?
            .into_no_null_iter()
            .collect();

        if values.is_empty() {
            return Err(anyhow::anyhow!("No valid data in column"));
        }

        let min_val = values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_val = values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let bin_width = if min_val == max_val { 1.0 } else { (max_val - min_val) / bins as f64 };

        let mut bin_counts: Vec<usize> = vec![0; bins];
        for val in values {
            let bin_idx = if val == max_val {
                bins.saturating_sub(1)
            } else if bin_width > 0.0 {
                ((val - min_val) / bin_width).floor() as usize
            } else {
                0
            };
            if bin_idx < bins {
                bin_counts[bin_idx] += 1;
            }
        }

        Ok(HistogramData {
            col_name: col_name.to_string(),
            min_val,
            max_val,
            bin_width,
            bin_counts,
        })
    }

    pub fn compute_boxplot_data(
        df: &DataFrame,
        col_name: &str,
    ) -> Result<BoxplotData> {
        let series = df.column(col_name)?;
        let mut values: Vec<f64> = series
            .f64()
            .context(format!("Column '{}' is not numeric", col_name))?
            .into_no_null_iter()
            .collect();

        values.sort_by(|a, b| a.partial_cmp(b).unwrap());

        if values.is_empty() {
            return Err(anyhow::anyhow!("No valid data in column"));
        }

        let q1 = percentile(&values, 25.0);
        let median = percentile(&values, 50.0);
        let q3 = percentile(&values, 75.0);
        let iqr = q3 - q1;

        let lower_fence = q1 - 1.5 * iqr;
        let upper_fence = q3 + 1.5 * iqr;

        let outliers: Vec<f64> = values
            .iter()
            .filter(|v| **v < lower_fence || **v > upper_fence)
            .copied()
            .collect();

        Ok(BoxplotData {
            col_name: col_name.to_string(),
            min: *values.first().unwrap(),
            q1,
            median,
            q3,
            max: *values.last().unwrap(),
            outliers,
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

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RegressionResult {
    pub slope: f64,
    pub intercept: f64,
    pub r_squared: f64,
    pub x_col: String,
    pub y_col: String,
    pub data_points: usize,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct HistogramData {
    pub col_name: String,
    pub min_val: f64,
    pub max_val: f64,
    pub bin_width: f64,
    pub bin_counts: Vec<usize>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct BoxplotData {
    pub col_name: String,
    pub min: f64,
    pub q1: f64,
    pub median: f64,
    pub q3: f64,
    pub max: f64,
    pub outliers: Vec<f64>,
}

/// (age, expression) for expression trend plot.
#[derive(Debug, Clone)]
pub struct ExpressionTrendPoint {
    pub age: f64,
    pub expression: f64,
}

/// Per-gene expression trend: gene_id -> (age, expression) points.
#[derive(Debug, Clone)]
pub struct ExpressionTrendData {
    pub gene_id: String,
    pub points: Vec<ExpressionTrendPoint>,
}

/// Young vs Old scatter: (mean_young, mean_old) per gene.
#[derive(Debug, Clone)]
pub struct YoungVsOldPoint {
    pub gene_id: String,
    pub mean_young: f64,
    pub mean_old: f64,
}

/// Age group box plot: one box per age column.
#[derive(Debug, Clone)]
pub struct AgeGroupBoxData {
    pub age_label: String,
    pub values: Vec<f64>,
}

impl StatisticalAnalyzer {
    /// Expression vs age for selected gene(s). Returns (age, expression) points per gene.
    pub fn expression_trend(
        df: &DataFrame,
        gene_column: &str,
        age_columns: &[String],
        gene_ids: &[String],
    ) -> Result<Vec<ExpressionTrendData>> {
        let gene_series = df.column(gene_column)?;
        let gene_str = gene_series.str()?;
        let mut result = Vec::new();

        for gene_id in gene_ids {
            let row_idx = gene_str
                .into_iter()
                .position(|o| o.map(|s| s == gene_id).unwrap_or(false));
            let Some(row_idx) = row_idx else { continue };

            let mut points = Vec::new();
            for col_name in age_columns {
                let age: f64 = col_name.trim().parse().unwrap_or(0.0);
                if let Ok(col) = df.column(col_name) {
                    if let Ok(f64_col) = col.f64() {
                        if let Some(v) = f64_col.get(row_idx) {
                            points.push(ExpressionTrendPoint { age, expression: v });
                        }
                    }
                }
            }
            points.sort_by(|a, b| a.age.partial_cmp(&b.age).unwrap());
            if !points.is_empty() {
                result.push(ExpressionTrendData {
                    gene_id: gene_id.clone(),
                    points,
                });
            }
        }
        Ok(result)
    }

    /// Mean expression Young vs Old across genes. Splits ages at median.
    pub fn young_vs_old(
        df: &DataFrame,
        gene_column: &str,
        age_columns: &[String],
    ) -> Result<Vec<YoungVsOldPoint>> {
        let ages: Vec<i64> = age_columns
            .iter()
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        if ages.is_empty() {
            return Err(anyhow::anyhow!("No valid age columns"));
        }
        let mut sorted = ages.clone();
        sorted.sort();
        let median_age = sorted[sorted.len() / 2];
        let (young_cols, old_cols): (Vec<_>, Vec<_>) = age_columns
            .iter()
            .partition(|s| s.trim().parse::<i64>().unwrap_or(0) < median_age);

        if young_cols.is_empty() || old_cols.is_empty() {
            return Err(anyhow::anyhow!("Need both young and old age groups"));
        }

        let gene_series = df.column(gene_column)?.str()?;
        let n_rows = df.height();
        let mut result = Vec::new();

        for row in 0..n_rows {
            let gene_id = gene_series.get(row).unwrap_or("").to_string();
            let mut sum_young = 0.0;
            let mut cnt_young = 0usize;
            for c in &young_cols {
                if let Ok(col) = df.column(c) {
                    if let Ok(f64_col) = col.f64() {
                        if let Some(v) = f64_col.get(row) {
                            sum_young += v;
                            cnt_young += 1;
                        }
                    }
                }
            }
            let mean_young = if cnt_young > 0 {
                sum_young / cnt_young as f64
            } else {
                0.0
            };

            let mut sum_old = 0.0;
            let mut cnt_old = 0usize;
            for c in &old_cols {
                if let Ok(col) = df.column(c) {
                    if let Ok(f64_col) = col.f64() {
                        if let Some(v) = f64_col.get(row) {
                            sum_old += v;
                            cnt_old += 1;
                        }
                    }
                }
            }
            let mean_old = if cnt_old > 0 {
                sum_old / cnt_old as f64
            } else {
                0.0
            };

            result.push(YoungVsOldPoint {
                gene_id,
                mean_young,
                mean_old,
            });
        }
        Ok(result)
    }

    /// Box plot by age: one box per age column, values = expression across genes.
    pub fn age_group_box_data(
        df: &DataFrame,
        _gene_column: &str,
        age_columns: &[String],
    ) -> Result<Vec<AgeGroupBoxData>> {
        let mut result = Vec::new();
        for col_name in age_columns {
            let col = df.column(col_name)?;
            let values: Vec<f64> = col.f64()?.into_no_null_iter().collect();
            result.push(AgeGroupBoxData {
                age_label: col_name.clone(),
                values,
            });
        }
        Ok(result)
    }
}
