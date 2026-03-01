use crate::client::glm::{FunctionDef, Tool};
use serde_json::json;

/// Get all data science tools for R-Data agent
pub fn get_all_tools() -> Vec<Tool> {
    vec![
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "load_data".to_string(),
                description: "Load microarray or tabular data from CSV, JSON, or Excel (.xlsx). Use before running analyses.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "file_paths": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Paths to data files (supports ~ for home)"
                        }
                    },
                    "required": ["file_paths"]
                }),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "get_data_info".to_string(),
                description: "Get info about the currently loaded dataset: genes count, age columns, layout, column types.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "get_app_context".to_string(),
                description: "Get full application context: loaded datasets, active file, recent analyses, current visualization, filters. Use to understand what the user has done across Data/Analysis/Viz tabs.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "list_available_analyses".to_string(),
                description: "List which analyses are available for the current data and why others might be disabled.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "run_summary_stats".to_string(),
                description: "Run summary statistics (mean, std, min, max) and gene-age correlation summary for microarray data.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "run_correlation".to_string(),
                description: "Run correlation matrix and show heatmap.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "run_histogram".to_string(),
                description: "Create histogram for a numeric column.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "column": { "type": "string", "description": "Column name" },
                        "bins": { "type": "integer", "description": "Number of bins (default 20)" }
                    },
                    "required": ["column"]
                }),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "run_expression_vs_age".to_string(),
                description: "Expression vs age for all genes (microarray). Produces volcano-style results.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "run_genes_significant_with_age".to_string(),
                description: "Find genes significant with age (p<0.05), positive and negative correlation. Microarray only.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "run_expression_trend".to_string(),
                description: "Expression trend line plot for selected genes (1-5 genes). Microarray only.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "gene_ids": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Ensembl gene IDs (e.g. ENSG0000001)"
                        }
                    },
                    "required": ["gene_ids"]
                }),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "run_young_vs_old".to_string(),
                description: "Young vs Old scatter: mean expression young vs old across genes. Identifies aging markers. Microarray only.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "young_ages": { "type": "string", "description": "e.g. 17-30" },
                        "old_ages": { "type": "string", "description": "e.g. 40-60" }
                    },
                    "required": []
                }),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "run_volcano_plot".to_string(),
                description: "Volcano plot: significance vs effect size for genes. Microarray only.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "run_expression_heatmap".to_string(),
                description: "Expression heatmap: genes × ages (top genes by correlation). Microarray only.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "top_n": { "type": "integer", "description": "Top N genes (default 50)" }
                    },
                    "required": []
                }),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "export_gene_correlation".to_string(),
                description: "Export gene correlation results (correlation, slope, R², p-value) to CSV. Microarray only.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "open_visualization".to_string(),
                description: "Open the current chart/visualization in the system browser (full-quality SVG).".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "google_search".to_string(),
                description: "Search the web for information (longevity research, gene names, methods). Uses SerpAPI.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search query" },
                        "num_results": { "type": "integer", "description": "Max results (default 10)" }
                    },
                    "required": ["query"]
                }),
            },
        },
    ]
}
