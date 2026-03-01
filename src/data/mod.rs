pub mod ingestion;
pub mod analysis;

pub use ingestion::{
    DataLoader, ColumnInfo, DataLayout, AgeGroupDef, build_filtered_dataframe, coerce_expression_columns,
    parse_age_groups, partition_ages_by_groups,
};
pub use analysis::{StatisticalAnalyzer, GeneAgeCorrelation};
