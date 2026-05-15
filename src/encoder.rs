use serde_json::{Map, Value};
use sqlx::mysql::MySqlRow;
use sqlx::{Column, Row, TypeInfo, ValueRef};

pub fn mysql_row_to_json(row: &MySqlRow) -> Value {
    let mut map = Map::new();

    for column in row.columns() {
        let col_name = column.name();
        let type_info = column.type_info();
        let type_name = type_info.name();

        let raw_value = match row.try_get_raw(col_name) {
            Ok(v) => v,
            Err(_) => {
                map.insert(col_name.to_string(), Value::Null);
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
                        let val: i8 = row.try_get(col_name).unwrap_or(0);
                        Value::Number(val.into())
                    }
                }
                "SHORT" | "LONG" | "INT24" | "INT" | "INTEGER" => {
                    let val: i32 = row.try_get(col_name).unwrap_or(0);
                    Value::Number(val.into())
                }
                "LONGLONG" | "BIGINT" => {
                    let val: i64 = row.try_get(col_name).unwrap_or(0);
                    Value::Number(val.into())
                }
                "FLOAT" | "DOUBLE" => {
                    let val: f64 = row.try_get(col_name).unwrap_or(0.0);
                    serde_json::Number::from_f64(val)
                        .map(Value::Number)
                        .unwrap_or(Value::Null)
                }
                "VARCHAR" | "VAR_STRING" | "STRING" | "BLOB" | "TEXT" => {
                    let val: String = row.try_get(col_name).unwrap_or_default();
                    Value::String(val)
                }
                "JSON" => {
                    let val: Value = row.try_get(col_name).unwrap_or(Value::Null);
                    val
                }
                _ => {
                    let val: String = row.try_get(col_name).unwrap_or_default();
                    Value::String(val)
                }
            }
        };

        map.insert(col_name.to_string(), json_val);
    }

    Value::Object(map)
}
