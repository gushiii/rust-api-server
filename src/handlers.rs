use crate::encoder::mysql_row_to_json;
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde_json::{Map, Value};
use std::collections::HashMap;

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::MySqlPool,
}

fn validate_identifier(name: &str) -> Result<(), String> {
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(format!(
            "Security error: Invalid database identifier '{}'",
            name
        ));
    }
    Ok(())
}

async fn get_primary_key(pool: &sqlx::MySqlPool, table_name: &str) -> Result<String, String> {
    let sql = "
        SELECT COLUMN_NAME
        FROM information_schema.KEY_COLUMN_USAGE
        WHERE TABLE_SCHEMA = DATABASE()
          AND CONSTRAINT_NAME = 'PRIMARY'
          AND TABLE_NAME = ?
        LIMIT 1
    ";

    let row_opt = sqlx::query_scalar::<_, String>(sql)
        .bind(table_name)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(row_opt.unwrap_or_else(|| "id".to_string()))
}

pub async fn handle_create(
    State(state): State<AppState>,
    Path(table_name): Path<String>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, String> {
    validate_identifier(&table_name)?;
    let obj = payload.as_object().ok_or("Payload must be a JSON object")?;

    let mut columns = Vec::new();
    let mut placeholders = Vec::new();
    let mut query_values = Vec::new();

    for (key, value) in obj {
        validate_identifier(key)?;
        columns.push(format!("`{}`", key));
        placeholders.push("?");
        query_values.push(value);
    }

    let insert_sql = format!(
        "INSERT INTO `{}` ({}) VALUES ({})",
        table_name,
        columns.join(", "),
        placeholders.join(", ")
    );

    let mut query = sqlx::query(&insert_sql);
    for val in query_values {
        query = match val {
            Value::String(s) => query.bind(s),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    query.bind(i)
                } else {
                    query.bind(n.as_f64().unwrap_or(0.0))
                }
            }
            Value::Bool(b) => query.bind(*b),
            Value::Null => query.bind(None::<String>),
            _ => query.bind(val.to_string()),
        };
    }

    let result = query
        .execute(&state.pool)
        .await
        .map_err(|e| e.to_string())?;

    let pk_column = get_primary_key(&state.pool, &table_name).await?;
    let select_sql = format!("SELECT * FROM `{}` WHERE `{}` = ?", table_name, pk_column);

    let row = if let Some(front_pk_val) = obj.get(&pk_column) {
        match front_pk_val {
            Value::String(s) => sqlx::query(&select_sql).bind(s).fetch_one(&state.pool).await,
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    sqlx::query(&select_sql).bind(i).fetch_one(&state.pool).await
                } else {
                    sqlx::query(&select_sql).bind(n.as_f64().unwrap_or(0.0)).fetch_one(&state.pool).await
                }
            }
            _ => sqlx::query(&select_sql).bind(front_pk_val.to_string()).fetch_one(&state.pool).await,
        }
    } else {
        let new_id = result.last_insert_id();
        sqlx::query(&select_sql).bind(new_id).fetch_one(&state.pool).await
    }.map_err(|e| {
        format!("Error fetching the inserted row: {}. Please check if the primary key is auto-incrementing or provided in payload.", e)
    })?;

    Ok(Json(mysql_row_to_json(&row)))
}

pub async fn handle_list(
    State(state): State<AppState>,
    Path(table_name): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, String> {
    validate_identifier(&table_name)?;

    let mut sql = format!("SELECT * FROM `{}` WHERE 1=1", table_name);
    let mut bind_values: Vec<Value> = Vec::new();

    let mut limit: Option<i64> = None;
    let mut offset: Option<i64> = None;

    for (key, value) in &params {
        if key == "_limit" {
            limit = value.parse::<i64>().ok();
            continue;
        }
        if key == "_offset" {
            offset = value.parse::<i64>().ok();
            continue;
        }
        if key == "_where" {
            continue;
        }

        validate_identifier(key)?;
        sql.push_str(&format!(" AND `{}` = ?", key));
        bind_values.push(Value::String(value.clone()));
    }

    if let Some(where_str) = params.get("_where") {
        let where_obj: Value = serde_json::from_str(where_str)
            .map_err(|e| format!("Invalid JSON inside _where parameter: {}", e))?;

        if let Some(conditions) = where_obj.as_object() {
            for (field, block) in conditions {
                validate_identifier(field)?;

                match block {
                    Value::String(_) | Value::Number(_) | Value::Bool(_) => {
                        sql.push_str(&format!(" AND `{}` = ?", field));
                        bind_values.push(block.clone());
                    }
                    Value::Object(inner_map) => {
                        for (op, op_val) in inner_map {
                            match op.as_str() {
                                "$gt" => {
                                    sql.push_str(&format!(" AND `{}` > ?", field));
                                    bind_values.push(op_val.clone());
                                }
                                "$gte" => {
                                    sql.push_str(&format!(" AND `{}` >= ?", field));
                                    bind_values.push(op_val.clone());
                                }
                                "$lt" => {
                                    sql.push_str(&format!(" AND `{}` < ?", field));
                                    bind_values.push(op_val.clone());
                                }
                                "$lte" => {
                                    sql.push_str(&format!(" AND `{}` <= ?", field));
                                    bind_values.push(op_val.clone());
                                }
                                "$neq" => {
                                    sql.push_str(&format!(" AND `{}` != ?", field));
                                    bind_values.push(op_val.clone());
                                }
                                "$like" => {
                                    sql.push_str(&format!(" AND `{}` LIKE ?", field));
                                    let raw_str = op_val.as_str().unwrap_or("");
                                    bind_values.push(Value::String(format!("%{}%", raw_str)));
                                }
                                _ => return Err(format!("Unsupported operator: {}", op)),
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    if limit.is_some() {
        sql.push_str(" LIMIT ?");
    }
    if offset.is_some() {
        sql.push_str(" OFFSET ?");
    }

    let mut query = sqlx::query(&sql);
    for json_val in bind_values {
        query = match json_val {
            Value::String(s) => {
                if let Ok(i) = s.parse::<i64>() {
                    query.bind(i)
                } else if let Ok(f) = s.parse::<f64>() {
                    query.bind(f)
                } else {
                    query.bind(s)
                }
            }
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    query.bind(i)
                } else {
                    query.bind(n.as_f64().unwrap_or(0.0))
                }
            }
            Value::Bool(b) => query.bind(b),
            Value::Null => query.bind(None::<String>),
            _ => query.bind(json_val.to_string()),
        };
    }

    if let Some(l) = limit {
        query = query.bind(l);
    }
    if let Some(o) = offset {
        query = query.bind(o);
    }

    let rows = query
        .fetch_all(&state.pool)
        .await
        .map_err(|e| e.to_string())?;
    let json_array: Vec<Value> = rows.iter().map(mysql_row_to_json).collect();

    Ok(Json(Value::Array(json_array)))
}

pub async fn handle_get(
    State(state): State<AppState>,
    Path((table_name, id)): Path<(String, String)>,
) -> Result<Json<Value>, String> {
    validate_identifier(&table_name)?;

    let pk_column = get_primary_key(&state.pool, &table_name).await?;
    let select_sql = format!("SELECT * FROM `{}` WHERE `{}` = ?", table_name, pk_column);

    let row = sqlx::query(&select_sql)
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(Json(mysql_row_to_json(&row)))
}

pub async fn handle_update(
    State(state): State<AppState>,
    Path((table_name, id)): Path<(String, String)>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, String> {
    validate_identifier(&table_name)?;
    let obj = payload.as_object().ok_or("Payload must be a JSON object")?;
    let pk_column = get_primary_key(&state.pool, &table_name).await?;

    let mut set_clauses = Vec::new();
    let mut query_values = Vec::new();

    for (key, value) in obj {
        validate_identifier(key)?;
        if key == &pk_column {
            continue;
        }
        set_clauses.push(format!("`{}` = ?", key));
        query_values.push(value);
    }

    let sql = format!(
        "UPDATE `{}` SET {} WHERE `{}` = ?",
        table_name,
        set_clauses.join(", "),
        pk_column
    );

    let mut query = sqlx::query(&sql);
    for val in query_values {
        query = match val {
            Value::String(s) => query.bind(s),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    query.bind(i)
                } else {
                    query.bind(n.as_f64().unwrap_or(0.0))
                }
            }
            Value::Bool(b) => query.bind(*b),
            Value::Null => query.bind(None::<String>),
            _ => query.bind(val.to_string()),
        };
    }
    query = query.bind(id.clone());
    query
        .execute(&state.pool)
        .await
        .map_err(|e| e.to_string())?;

    let select_sql = format!("SELECT * FROM `{}` WHERE `{}` = ?", table_name, pk_column);
    let row = sqlx::query(&select_sql)
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(Json(mysql_row_to_json(&row)))
}

pub async fn handle_delete(
    State(state): State<AppState>,
    Path((table_name, id)): Path<(String, String)>,
) -> Result<Json<Value>, String> {
    validate_identifier(&table_name)?;

    let pk_column = get_primary_key(&state.pool, &table_name).await?;
    let sql = format!("DELETE FROM `{}` WHERE `{}` = ?", table_name, pk_column);

    let result = sqlx::query(&sql)
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(|e| e.to_string())?;

    let mut response = Map::new();
    response.insert("success".to_string(), Value::Bool(true));
    response.insert(
        "rows_affected".to_string(),
        Value::Number(result.rows_affected().into()),
    );

    Ok(Json(Value::Object(response)))
}
