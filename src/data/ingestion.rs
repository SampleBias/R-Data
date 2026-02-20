use anyhow::{Context, Result};
use calamine::{DataType as CalamineDataType, Reader, open_workbook, Data, Xlsx};
use polars::prelude::*;
use std::path::Path;

pub struct DataLoader;

impl DataLoader {
    pub fn load_xlsx<P: AsRef<Path>>(path: P) -> Result<DataFrame> {
        let path = path.as_ref();
        let mut workbook: Xlsx<_> = open_workbook(path).context("Failed to open Excel file")?;
        let sheet_names = workbook.sheet_names().to_vec();
        let sheet_name = sheet_names.first().ok_or_else(|| anyhow::anyhow!("No sheets in Excel file"))?;
        let range = workbook.worksheet_range(sheet_name).context("Failed to read worksheet")?;

        let rows: Vec<Vec<Data>> = range.rows().map(|r| r.to_vec()).collect();
        if rows.is_empty() {
            return Err(anyhow::anyhow!("Excel sheet is empty"));
        }

        let headers: Vec<String> = rows[0]
            .iter()
            .enumerate()
            .map(|(i, d)| {
                d.as_string()
                    .unwrap_or_else(|| format!("Column_{}", i + 1))
            })
            .collect();

        let num_cols = headers.len();
        let mut columns: Vec<Series> = Vec::with_capacity(num_cols);

        for col_idx in 0..num_cols {
            let mut str_values: Vec<Option<String>> = Vec::new();
            for row in rows.iter().skip(1) {
                let cell = row.get(col_idx).unwrap_or(&Data::Empty);
                str_values.push(cell.as_string());
            }
            let series = Series::new(
                headers[col_idx].as_str().into(),
                str_values
                    .into_iter()
                    .map(|o| o.unwrap_or_default())
                    .collect::<Vec<_>>(),
            );
            columns.push(series);
        }

        let cols: Vec<_> = columns.into_iter().map(|s| s.into_column()).collect();
        let df = DataFrame::new(cols).context("Failed to build DataFrame from Excel")?;
        Ok(df)
    }

    pub fn load_csv<P: AsRef<Path>>(path: P) -> Result<DataFrame> {
        let df = CsvReadOptions::default()
            .with_has_header(true)
            .try_into_reader_with_file_path(Some(path.as_ref().to_path_buf()))
            .context("Failed to open CSV file")?
            .finish()
            .context("Failed to load CSV data")?;
        Ok(df)
    }

    pub fn load_json<P: AsRef<Path>>(path: P) -> Result<DataFrame> {
        let file = std::fs::File::open(path.as_ref())
            .context("Failed to open JSON file")?;
        let reader = std::io::BufReader::new(file);
        let df = JsonReader::new(reader)
            .finish()
            .context("Failed to load JSON data")?;
        Ok(df)
    }

    pub fn load_dataframe(path: &str) -> Result<DataFrame> {
        let extension = Path::new(path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        match extension.to_lowercase().as_str() {
            "csv" => Self::load_csv(path),
            "json" => Self::load_json(path),
            "xlsx" => Self::load_xlsx(path),
            _ => Err(anyhow::anyhow!(
                "Unsupported file format: {}. Supported: .csv, .json, .xlsx",
                extension
            )),
        }
    }

    pub fn get_column_info(df: &DataFrame) -> Vec<ColumnInfo> {
        df.get_columns()
            .iter()
            .map(|col| ColumnInfo {
                name: col.name().to_string(),
                dtype: col.dtype().to_string(),
                null_count: col.null_count(),
            })
            .collect()
    }

    pub fn get_summary_stats(df: &DataFrame) -> Result<DataFrame> {
        let numeric_cols: Vec<String> = df
            .get_columns()
            .iter()
            .filter(|col| col.dtype().is_numeric())
            .map(|col| col.name().to_string())
            .collect();

        if numeric_cols.is_empty() {
            return Err(anyhow::anyhow!("No numeric columns found"));
        }

        let mut stats = Vec::new();
        for col_name in &numeric_cols {
            let series = df.column(col_name)?;
            
            let series_f64 = if let Ok(s) = series.f64() {
                s.clone()
            } else if let Ok(s) = series.i32() {
                s.cast(&DataType::Float64)?.f64()?.clone()
            } else if let Ok(s) = series.i64() {
                s.cast(&DataType::Float64)?.f64()?.clone()
            } else if let Ok(s) = series.u32() {
                s.cast(&DataType::Float64)?.f64()?.clone()
            } else {
                continue;
            };

            let mean = series_f64.mean().unwrap_or(0.0);
            let std_dev = series_f64.std(1).unwrap_or(0.0);
            let min_val = series_f64.min().unwrap_or(0.0);
            let max_val = series_f64.max().unwrap_or(0.0);

            stats.push((
                col_name.clone(),
                mean,
                std_dev,
                min_val,
                max_val,
                series.len(),
            ));
        }

        if stats.is_empty() {
            return Err(anyhow::anyhow!("No valid numeric data for statistics"));
        }

        let df_stats = df!(
            "Column" => &stats.iter().map(|s| s.0.clone()).collect::<Vec<_>>(),
            "Mean" => &stats.iter().map(|s| s.1).collect::<Vec<_>>(),
            "StdDev" => &stats.iter().map(|s| s.2).collect::<Vec<_>>(),
            "Min" => &stats.iter().map(|s| s.3).collect::<Vec<_>>(),
            "Max" => &stats.iter().map(|s| s.4).collect::<Vec<_>>(),
            "Count" => &stats.iter().map(|s| s.5 as u32).collect::<Vec<_>>(),
        )?;

        Ok(df_stats)
    }
}

#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub dtype: String,
    pub null_count: usize,
}
