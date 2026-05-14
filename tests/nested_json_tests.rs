// ============================================================
// tests/nested_json_tests.rs
//
// End-to-end coverage for nested JSON/JSONL handling:
//   - inference of Arrow Struct/List columns
//   - JSON/JSONL → Parquet (native path, nesting preserved)
//   - Parquet → JSON (inverse, nesting reconstructed)
// ============================================================

use csv_to_parquet::json_arrow::{convert_json_to_parquet, json_has_nested};
use csv_to_parquet::to_json::convert_parquet_to_json;
use serde_json::Value;
use std::io::Write;
use tempfile::{Builder, NamedTempFile};

fn write_input(content: &str, ext: &str) -> NamedTempFile {
    let mut temp = Builder::new()
        .suffix(&format!(".{ext}"))
        .tempfile()
        .unwrap();
    temp.write_all(content.as_bytes()).unwrap();
    temp.flush().unwrap();
    temp
}

fn parquet_temp() -> NamedTempFile {
    Builder::new().suffix(".parquet").tempfile().unwrap()
}

fn roundtrip_to_json(input: &NamedTempFile) -> Vec<Value> {
    let parquet = parquet_temp();
    convert_json_to_parquet(input.path(), parquet.path()).unwrap();
    let json_out = Builder::new().suffix(".json").tempfile().unwrap();
    convert_parquet_to_json(parquet.path(), json_out.path()).unwrap();
    let text = std::fs::read_to_string(json_out.path()).unwrap();
    serde_json::from_str::<Value>(&text)
        .unwrap()
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn json_array_nested_object_roundtrip() {
    let input = write_input(
        r#"[
            {"id":1,"meta":{"a":10,"b":"x","ok":true}},
            {"id":2,"meta":{"a":20,"b":"y","ok":false}}
        ]"#,
        "json",
    );
    let rows = roundtrip_to_json(&input);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["id"], 1);
    assert_eq!(rows[0]["meta"]["a"], 10);
    assert_eq!(rows[0]["meta"]["b"], "x");
    assert_eq!(rows[0]["meta"]["ok"], true);
    assert_eq!(rows[1]["meta"]["a"], 20);
    assert_eq!(rows[1]["meta"]["ok"], false);
}

#[test]
fn jsonl_nested_object_roundtrip() {
    let input = write_input(
        "{\"id\":1,\"addr\":{\"city\":\"Paris\",\"zip\":75001}}\n\
         {\"id\":2,\"addr\":{\"city\":\"Lyon\",\"zip\":69001}}\n",
        "jsonl",
    );
    let rows = roundtrip_to_json(&input);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["addr"]["city"], "Paris");
    assert_eq!(rows[0]["addr"]["zip"], 75001);
    assert_eq!(rows[1]["addr"]["city"], "Lyon");
}

#[test]
fn nested_array_of_scalars_roundtrip() {
    let input = write_input(r#"[{"id":1,"tags":[1,2,3]},{"id":2,"tags":[4,5]}]"#, "json");
    let rows = roundtrip_to_json(&input);
    assert_eq!(rows[0]["tags"], serde_json::json!([1, 2, 3]));
    assert_eq!(rows[1]["tags"], serde_json::json!([4, 5]));
}

#[test]
fn deeply_nested_roundtrip() {
    let input = write_input(
        r#"[{"id":1,"a":{"b":{"c":{"d":42}}}}]"#,
        "json",
    );
    let rows = roundtrip_to_json(&input);
    assert_eq!(rows[0]["a"]["b"]["c"]["d"], 42);
}

#[test]
fn array_of_objects_column_roundtrip() {
    let input = write_input(
        r#"[{"id":1,"items":[{"k":"a","v":1},{"k":"b","v":2}]}]"#,
        "json",
    );
    let rows = roundtrip_to_json(&input);
    let items = rows[0]["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["k"], "a");
    assert_eq!(items[1]["v"], 2);
}

#[test]
fn missing_nested_key_yields_null_object() {
    let input = write_input(
        "{\"id\":1,\"meta\":{\"a\":1}}\n{\"id\":2}\n",
        "jsonl",
    );
    let rows = roundtrip_to_json(&input);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["meta"]["a"], 1);
    // arrow-json omits null fields: the absent struct is not emitted.
    assert!(rows[1].get("meta").is_none() || rows[1]["meta"].is_null());
}

#[test]
fn scalar_only_json_not_routed_as_nested() {
    let input = write_input(r#"[{"id":1,"name":"x"}]"#, "json");
    assert!(!json_has_nested(input.path()).unwrap());
}
