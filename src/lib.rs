// ============================================================
pub mod analysis;
pub mod conversion;
pub mod export;
pub mod inspect;
pub mod json;
pub mod json_arrow;
pub mod schema;
pub mod to_csv;
pub mod to_json;
pub mod to_jsonl;
pub mod to_xlsx;
pub mod tui;
pub mod utils;
pub mod xlsx;
pub const BLOCK_SIZE: usize = 100_000;
