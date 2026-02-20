pub mod ingestion;
pub mod analysis;

pub use ingestion::{DataLoader, ColumnInfo};
pub use analysis::StatisticalAnalyzer;
