//! Visualization availability: which analyses/viz are enabled for the current data.
//! Simplified menu for microarray data (Gene ID × age columns).

use crate::data::DataLayout;
use polars::prelude::DataFrame;

#[derive(Debug, Clone)]
pub struct VizAvailability {
    pub key: char,
    pub label: String,
    pub available: bool,
    pub reason: Option<String>,
}

/// Returns the simplified analysis menu for microarray data.
/// Format: (key, label, available, reason_if_disabled).
pub fn available_visualizations(
    df: Option<&DataFrame>,
    layout: Option<&DataLayout>,
) -> Vec<VizAvailability> {
    let numeric_cols: Vec<_> = df
        .map(|d| {
            d.get_columns()
                .iter()
                .filter(|c| c.dtype().is_numeric())
                .map(|c| c.name().to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let numeric_count = numeric_cols.len();
    let has_layout = layout.is_some();

    vec![
        VizAvailability {
            key: 's',
            label: "Summary statistics (mean, median, mode, R², p-value, correlation)".to_string(),
            available: numeric_count >= 1 || has_layout,
            reason: if numeric_count < 1 && !has_layout {
                Some("load data first".to_string())
            } else {
                None
            },
        },
        VizAvailability {
            key: 'i',
            label: "Histogram".to_string(),
            available: numeric_count >= 1 || has_layout,
            reason: if numeric_count < 1 && !has_layout {
                Some("load data first".to_string())
            } else {
                None
            },
        },
        VizAvailability {
            key: 'r',
            label: "Linear regression (expression vs age)".to_string(),
            available: has_layout || numeric_count >= 2,
            reason: if !has_layout && numeric_count < 2 {
                Some("need microarray layout or ≥2 numeric columns".to_string())
            } else {
                None
            },
        },
        VizAvailability {
            key: 'h',
            label: "Heatmap (correlation matrix)".to_string(),
            available: numeric_count >= 2 || has_layout,
            reason: if numeric_count < 2 && !has_layout {
                Some("need ≥2 numeric columns".to_string())
            } else {
                None
            },
        },
        VizAvailability {
            key: 'g',
            label: "Gene correlation with aging (positive/negative, p<0.05)".to_string(),
            available: has_layout,
            reason: if !has_layout {
                Some("need microarray layout (Gene ID × age columns)".to_string())
            } else {
                None
            },
        },
    ]
}
