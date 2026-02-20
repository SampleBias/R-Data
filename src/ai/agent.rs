use anyhow::{Result};
use std::sync::Arc;
use crate::ai::client::AIClient;
use crate::data::DataLoader;
use crate::viz::{VisualizationEngine, VisualizationConfig};

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum AnalysisRequest {
    SummaryStats,
    Correlation,
    Histogram { column: String, bins: usize },
    BoxPlot { column: String },
    LinearRegression { x_column: String, y_column: String },
    Heatmap,
    CustomInsight { prompt: String },
}

#[derive(Clone, Debug)]
pub struct AnalysisResult {
    pub summary: String,
    pub details: Option<String>,
    pub viz_config: Option<VisualizationConfig>,
}

#[allow(dead_code)]
pub struct AIAgent {
    client: Option<Arc<AIClient>>,
    viz_engine: Arc<VisualizationEngine>,
}

impl AIAgent {
    pub fn new(api_key: Option<String>) -> Self {
        let client = api_key.map(|key| Arc::new(AIClient::new(key)));
        Self {
            client,
            viz_engine: Arc::new(VisualizationEngine::default()),
        }
    }

    #[allow(dead_code)]
    pub fn has_ai(&self) -> bool {
        self.client.is_some()
    }

    pub async fn analyze_request(
        &self,
        df: &polars::prelude::DataFrame,
        request: AnalysisRequest,
    ) -> Result<AnalysisResult> {
        match request {
            AnalysisRequest::SummaryStats => {
                let stats = DataLoader::get_summary_stats(df)?;
                let details = format!("{}", stats);
                let mut summary = "Statistical summary computed:\n".to_string();
                
                if let Some(client) = &self.client {
                    let ai_prompt = format!(
                        "Analyze this statistical summary and provide key insights:\n{}\n\nWhat are the most notable patterns or concerns?",
                        details
                    );
                    
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(10),
                        client.send_message(&ai_prompt)
                    ).await {
                        Ok(Ok(ai_response)) => {
                            summary.push_str(&format!("AI Insights:\n{}", ai_response));
                        }
                        _ => {
                            summary.push_str("(AI insights unavailable - using summary only)");
                        }
                    }
                }
                
                Ok(AnalysisResult {
                    summary,
                    details: Some(details),
                    viz_config: None,
                })
            }
            AnalysisRequest::Correlation => {
                let corr = crate::data::StatisticalAnalyzer::compute_correlation(df)?;
                let details = format!("{}", corr);
                let mut summary = "Correlation matrix computed:\n".to_string();
                
                if let Some(client) = &self.client {
                    let ai_prompt = format!(
                        "Analyze this correlation matrix and identify strong relationships:\n{}\n\nWhich pairs have the strongest correlations and what might that mean?",
                        details
                    );
                    
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(10),
                        client.send_message(&ai_prompt)
                    ).await {
                        Ok(Ok(ai_response)) => {
                            summary.push_str(&format!("AI Insights:\n{}", ai_response));
                        }
                        _ => {
                            summary.push_str("(AI insights unavailable)");
                        }
                    }
                }
                
                let numeric_cols: Vec<String> = df
                    .get_columns()
                    .iter()
                    .filter(|col| col.dtype().is_numeric())
                    .map(|col| col.name().to_string())
                    .collect();
                
                Ok(AnalysisResult {
                    summary,
                    details: Some(details),
                    viz_config: Some(VisualizationConfig::Heatmap(
                        crate::viz::HeatmapConfig { columns: numeric_cols },
                    )),
                })
            }
            AnalysisRequest::Histogram { column, bins } => {
                let hist_data = crate::data::StatisticalAnalyzer::compute_histogram_data(df, &column, bins)?;
                let details = format!(
                    "Histogram for '{}': {} bins, range [{:.2}, {:.2}]",
                    column, bins, hist_data.min_val, hist_data.max_val
                );
                let mut summary = format!("Histogram analysis for '{}':\n", column);
                
                if let Some(client) = &self.client {
                    let ai_prompt = format!(
                        "Analyze this histogram data:\n{}\n\nWhat does the distribution suggest about this variable?",
                        details
                    );
                    
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(10),
                        client.send_message(&ai_prompt)
                    ).await {
                        Ok(Ok(ai_response)) => {
                            summary.push_str(&format!("AI Insights:\n{}", ai_response));
                        }
                        _ => {
                            summary.push_str("(AI insights unavailable)");
                        }
                    }
                }
                
                Ok(AnalysisResult {
                    summary,
                    details: Some(details),
                    viz_config: Some(VisualizationConfig::Histogram(
                        crate::viz::HistogramConfig { column, bins },
                    )),
                })
            }
            AnalysisRequest::BoxPlot { column } => {
                let box_data = crate::data::StatisticalAnalyzer::compute_boxplot_data(df, &column)?;
                let details = format!(
                    "Box plot for '{}': min={:.2}, Q1={:.2}, median={:.2}, Q3={:.2}, max={:.2}, outliers={}",
                    column, box_data.min, box_data.q1, box_data.median, box_data.q3, box_data.max, box_data.outliers.len()
                );
                let mut summary = format!("Box plot analysis for '{}':\n", column);
                
                if let Some(client) = &self.client {
                    let ai_prompt = format!(
                        "Analyze this box plot data:\n{}\n\nWhat does this tell us about the distribution and potential outliers?",
                        details
                    );
                    
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(10),
                        client.send_message(&ai_prompt)
                    ).await {
                        Ok(Ok(ai_response)) => {
                            summary.push_str(&format!("AI Insights:\n{}", ai_response));
                        }
                        _ => {
                            summary.push_str("(AI insights unavailable)");
                        }
                    }
                }
                
                Ok(AnalysisResult {
                    summary,
                    details: Some(details),
                    viz_config: Some(VisualizationConfig::BoxPlot(
                        crate::viz::BoxPlotConfig { column },
                    )),
                })
            }
            AnalysisRequest::LinearRegression { x_column, y_column } => {
                let reg = crate::data::StatisticalAnalyzer::linear_regression(df, &x_column, &y_column)?;
                let details = format!(
                    "Linear regression: y = {:.4}x + {:.4}, R² = {:.4}",
                    reg.slope, reg.intercept, reg.r_squared
                );
                let mut summary = format!("Linear regression analysis ({} vs {}):\n", y_column, x_column);
                
                if let Some(client) = &self.client {
                    let ai_prompt = format!(
                        "Analyze this regression result:\n{}\n\nWhat does the R² value suggest about the relationship?",
                        details
                    );
                    
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(10),
                        client.send_message(&ai_prompt)
                    ).await {
                        Ok(Ok(ai_response)) => {
                            summary.push_str(&format!("AI Insights:\n{}", ai_response));
                        }
                        _ => {
                            summary.push_str("(AI insights unavailable)");
                        }
                    }
                }
                
                Ok(AnalysisResult {
                    summary,
                    details: Some(details),
                    viz_config: Some(VisualizationConfig::LinearRegression(
                        crate::viz::LinearRegressionConfig { x_column, y_column },
                    )),
                })
            }
            AnalysisRequest::Heatmap => {
                Ok(AnalysisResult {
                    summary: "Heatmap visualization available".to_string(),
                    details: None,
                    viz_config: Some(VisualizationConfig::Heatmap(
                        crate::viz::HeatmapConfig {
                            columns: vec![],
                        },
                    )),
                })
            }
            AnalysisRequest::CustomInsight { prompt } => {
                let mut summary = format!("Custom analysis request:\n{}\n", prompt);
                
                if let Some(client) = &self.client {
                    let ai_prompt = format!(
                        "As a data science expert, analyze this data and provide insights based on the following request:\n{}\n\nDataset columns: {:?}",
                        prompt,
                        df.get_columns().iter().map(|c| c.name().to_string()).collect::<Vec<_>>()
                    );
                    
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(15),
                        client.send_message(&ai_prompt)
                    ).await {
                        Ok(Ok(ai_response)) => {
                            summary.push_str(&format!("AI Response:\n{}", ai_response));
                        }
                        _ => {
                            summary.push_str("(AI response unavailable)");
                        }
                    }
                } else {
                    summary.push_str("(AI not configured)");
                }
                
                Ok(AnalysisResult {
                    summary,
                    details: None,
                    viz_config: None,
                })
            }
        }
    }
}
