use serde_json::{Map, Value};
use sqlx::mysql::MySqlRow;
use sqlx::types::BigDecimal;
use sqlx::{Column, Row, TypeInfo, ValueRef};

pub fn mysql_row_to_json(row: &MySqlRow, fk_to_exclude: &str) -> Value {
    let mut main_map = Map::new();
    let mut nested_objects: std::collections::HashMap<String, Map<String, Value>> =
        std::collections::HashMap::new();

    for column in row.columns() {
        let col_name = column.name();
        let type_info = column.type_info();
        let type_name = type_info.name();

        if !fk_to_exclude.is_empty() && col_name == fk_to_exclude {
            continue;
        }

        let raw_value = match row.try_get_raw(col_name) {
            Ok(v) => v,
            Err(_) => {
                main_map.insert(col_name.to_string(), Value::Null);
                continue;
            }
        };

        let json_val = if raw_value.is_null() {
            Value::Null
        } else {
            match type_name {
                "TINY" | "TINYINT" => {
                    if let Ok(b) = row.try_get::<bool, _>(col_name) {
                        Value::Bool(b)
                    } else {
                        Value::Number(row.try_get::<i8, _>(col_name).unwrap_or(0).into())
                    }
                }
                "SHORT" | "LONG" | "INT24" | "INT" | "INTEGER" => {
                    Value::Number(row.try_get::<i32, _>(col_name).unwrap_or(0).into())
                }
                "LONGLONG" | "BIGINT" => {
                    Value::Number(row.try_get::<i64, _>(col_name).unwrap_or(0).into())
                }
                "FLOAT" | "DOUBLE" => {
                    serde_json::Number::from_f64(row.try_get::<f64, _>(col_name).unwrap_or(0.0))
                        .map(Value::Number)
                        .unwrap_or(Value::Null)
                }
                "DECIMAL" | "NEWDECIMAL" => {
                    if let Ok(decimal_val) = row.try_get::<BigDecimal, _>(col_name) {
                        let s = decimal_val.to_string();
                        s.parse::<f64>()
                            .ok()
                            .and_then(serde_json::Number::from_f64)
                            .map(Value::Number)
                            .unwrap_or(Value::Null)
                    } else {
                        serde_json::Number::from_f64(row.try_get::<f64, _>(col_name).unwrap_or(0.0))
                            .map(Value::Number)
                            .unwrap_or(Value::Null)
                    }
                }
                "VARCHAR" | "VAR_STRING" | "STRING" | "BLOB" | "TEXT" => {
                    Value::String(row.try_get::<String, _>(col_name).unwrap_or_default())
                }
                "JSON" => row.try_get::<Value, _>(col_name).unwrap_or(Value::Null),
                _ => Value::String(row.try_get::<String, _>(col_name).unwrap_or_default()),
            }
        };

        if let Some(idx) = col_name.find("__") {
            let prefix = &col_name[..idx];
            let field_cleaned = &col_name[idx + 2..];

            if !field_cleaned.is_empty() && !prefix.is_empty() {
                nested_objects
                    .entry(prefix.to_string())
                    .or_default()
                    .insert(field_cleaned.to_string(), json_val);
                continue;
            }
        }

        main_map.insert(col_name.to_string(), json_val);
    }

    for (sub_table, sub_map) in nested_objects {
        main_map.insert(sub_table, Value::Object(sub_map));
    }

    Value::Object(main_map)
}
