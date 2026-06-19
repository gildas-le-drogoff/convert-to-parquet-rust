// tests/schema_tests.rs
use arrow::datatypes::DataType;
use convert_to_parquet::schema::infer_schema;
use std::io::Write;
use tempfile::NamedTempFile;

fn csv_temp(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "{content}").unwrap();
    f
}

#[test]
fn test_infer_schema_simple() {
    let csv = csv_temp(
        "a,b,c\n\
         1,2,3\n\
         4,5,6\n",
    );
    let schema = infer_schema(csv.path(), b',', true, true).unwrap();
    assert_eq!(schema.fields().len(), 3);
    assert_eq!(schema.fields()[0].data_type(), &DataType::Int64);
    assert_eq!(schema.fields()[1].data_type(), &DataType::Int64);
    assert_eq!(schema.fields()[2].data_type(), &DataType::Int64);
}

#[test]
fn test_infer_schema_mixed_types() {
    let csv = csv_temp(
        "a,b,c\n\
         true,2024-01-01,3.14\n\
         false,2024-01-02,2.71\n",
    );
    let schema = infer_schema(csv.path(), b',', true, true).unwrap();
    assert_eq!(schema.fields()[0].data_type(), &DataType::Boolean);
    assert_eq!(schema.fields()[1].data_type(), &DataType::Date32);
    assert_eq!(schema.fields()[2].data_type(), &DataType::Float64);
}
