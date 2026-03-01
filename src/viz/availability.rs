//! Visualization availability: which analyses/viz are enabled for the current data.

use crate::data::DataLayout;
use polars::prelude::DataFrame;

#[derive(Debug, Clone)]
pub struct VizAvailability {
    pub key: char,
    pub label: String,
    pub available: bool,
    pub reason: Option<String>,
}

/// Returns the full list of visualizations with availability status.
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
    let age_group_count = layout.map(|l| {
        let ages: std::collections::HashSet<_> = l.age_columns.iter().collect();
        ages.len()
    }).unwrap_or(0);

    vec![
        VizAvailability {
            key: 's',
            label: "Summary statistics".to_string(),
            available: numeric_count >= 1,
            reason: if numeric_count < 1 {
                Some("no numeric columns".to_string())
            } else {
                None
            },
        },
        VizAvailability {
            key: 'c',
            label: "Correlation matrix".to_string(),
            available: numeric_count >= 2,
            reason: if numeric_count < 2 {
                Some("need ≥2 numeric columns".to_string())
            } else {
                None
            },
        },
        VizAvailability {
            key: 'i',
            label: "Histogram".to_string(),
            available: numeric_count >= 1,
            reason: if numeric_count < 1 {
                Some("no numeric columns".to_string())
            } else {
                None
            },
        },
        VizAvailability {
            key: 'b',
            label: "Box plot".to_string(),
            available: numeric_count >= 1,
            reason: if numeric_count < 1 {
                Some("no numeric columns".to_string())
            } else {
                None
            },
        },
        VizAvailability {
            key: 'r',
            label: if has_layout {
                "Expression vs age (all genes → volcano plot)".to_string()
            } else {
                "Linear regression (x vs y scatter)".to_string()
            },
            available: has_layout || numeric_count >= 2,
            reason: if has_layout {
                None
            } else if numeric_count < 2 {
                Some("need microarray layout or ≥2 numeric columns".to_string())
            } else {
                None
            },
        },
        VizAvailability {
            key: 'g',
            label: "Genes significant with age (p<0.05)".to_string(),
            available: has_layout,
            reason: if !has_layout {
                Some("no microarray layout".to_string())
            } else {
                None
            },
        },
        VizAvailability {
            key: 't',
            label: "Expression trend".to_string(),
            available: has_layout,
            reason: if !has_layout {
                Some("no microarray layout (Gene ID × age columns)".to_string())
            } else {
                None
            },
        },
        VizAvailability {
            key: 'v',
            label: "Young vs Old scatter".to_string(),
            available: has_layout && age_group_count >= 2,
            reason: if !has_layout {
                Some("no microarray layout".to_string())
            } else if age_group_count < 2 {
                Some("need ≥2 age groups".to_string())
            } else {
                None
            },
        },
        VizAvailability {
            key: 'a',
            label: "Age group box plot".to_string(),
            available: has_layout,
            reason: if !has_layout {
                Some("no microarray layout".to_string())
            } else {
                None
            },
        },
        VizAvailability {
            key: '1',
            label: "Volcano plot".to_string(),
            available: has_layout,
            reason: if !has_layout {
                Some("no microarray layout".to_string())
            } else {
                None
            },
        },
        VizAvailability {
            key: '2',
            label: "Correlation scatter".to_string(),
            available: has_layout,
            reason: if !has_layout {
                Some("no microarray layout".to_string())
            } else {
                None
            },
        },
        VizAvailability {
            key: '3',
            label: "Top genes bar chart".to_string(),
            available: has_layout,
            reason: if !has_layout {
                Some("no microarray layout".to_string())
            } else {
                None
            },
        },
        VizAvailability {
            key: 'e',
            label: "Expression vs age regression (select genes)".to_string(),
            available: has_layout,
            reason: if !has_layout {
                Some("no microarray layout".to_string())
            } else {
                None
            },
        },
    ]
}
