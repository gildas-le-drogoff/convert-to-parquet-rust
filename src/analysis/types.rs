// ============================================================
use arrow::record_batch::RecordBatch;
use std::sync::atomic::AtomicUsize;

const ERROR_SAMPLE_LIMIT: usize = 10;

#[derive(Clone, Debug)]
pub struct ErrorSample {
    pub values: Vec<String>,
    pub limit: usize,
}

impl ErrorSample {
    pub fn new(limit: usize) -> Self {
        Self {
            values: Vec::new(),
            limit,
        }
    }

    pub fn add(&mut self, value: String) {
        if self.values.len() < self.limit {
            self.values.push(value);
        }
    }
}

#[derive(Clone, Debug)]
pub struct ColumnMetrics {
    pub name: String,
    pub total_values: usize,
    pub total_null_text: usize,
    pub total_conversion_errors: usize,
    pub total_valid_values: usize,
    pub error_samples: ErrorSample,
}

impl ColumnMetrics {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            total_values: 0,
            total_null_text: 0,
            total_conversion_errors: 0,
            total_valid_values: 0,
            error_samples: ErrorSample::new(ERROR_SAMPLE_LIMIT),
        }
    }
}

pub enum ConversionResult<T> {
    Valid(T),
    ExplicitNull,
    ConversionError(String),
}

pub struct BlockResult {
    pub batch: RecordBatch,
    pub metrics: Vec<ColumnMetrics>,
}

#[derive(Default)]
pub struct ErrorCounters {
    pub parse_errors: AtomicUsize,
}
