use serde_json::Value;
use std::collections::HashMap;

pub struct QueryContext {
    pub select_fields: String,
    pub sql_clauses: String,
    pub bind_values: Vec<Value>,
    pub join_clauses: String,
    pub group_by: Option<String>,
    pub having_clauses: String,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub fn validate_identifier(name: &str) -> Result<(), String> {
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(format!(
            "Security error: Invalid database identifier '{}'",
            name
        ));
    }
    Ok(())
}

pub fn parse_query_params(
    main_table: &str,
    target_table: &str,
    target_cols: Vec<String>,
    params: &HashMap<String, String>,
) -> Result<QueryContext, String> {
    let mut sql_clauses = String::new();
    let mut bind_values = Vec::new();
    let mut limit = None;
    let mut offset = None;
    let mut sort_by = None;
    let mut order_by = "ASC".to_string();

    let mut select_list = Vec::new();
    let mut group_by = None;
    let mut having_clauses = String::new();
    let mut join_clauses = String::new();

    let mut has_join = false;
    let mut agg_functions_map = HashMap::new();

    if !target_table.is_empty()
        && let Some(join_str) = params.get("_join")
    {
        let join_obj: Value = serde_json::from_str(join_str).map_err(|e| e.to_string())?;
        if let Some(join_map) = join_obj.as_object() {
            let on_column = join_map
                .get("on")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'on' in _join")?;
            validate_identifier(on_column)?;

            let join_type = join_map
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("LEFT");

            join_clauses = format!(
                " {} JOIN `{}` ON `{}`.`{}` = `{}`.`{}`",
                join_type, target_table, main_table, on_column, target_table, on_column
            );

            select_list.push(format!("`{}`.*", main_table));

            if !target_cols.is_empty() {
                for col in target_cols {
                    if validate_identifier(&col).is_ok() {
                        select_list.push(format!(
                            "`{}`.`{}` AS `{}__{}`",
                            target_table, col, target_table, col
                        ));
                    }
                }
            } else {
                select_list.push(format!("`{}`.*", target_table));
            }
            has_join = true;
        }
    }

    if let Some(g_str) = params.get("_group") {
        validate_identifier(g_str)?;
        group_by = Some(g_str.to_string());
        select_list.push(format!("`{}`.`{}`", main_table, g_str));
    }
    if let Some(l_str) = params.get("_limit") {
        limit = l_str.parse::<i64>().ok();
    }
    if let Some(o_str) = params.get("_offset") {
        offset = o_str.parse::<i64>().ok();
    }

    for (key, value) in params {
        if key == "_limit"
            || key == "_offset"
            || key == "_where"
            || key == "_group"
            || key == "_aggregate"
            || key == "_having"
            || key == "_join"
        {
            continue;
        }
        validate_identifier(key)?;
        sql_clauses.push_str(&format!(" AND `{}`.`{}` = ?", main_table, key));
        bind_values.push(Value::String(value.clone()));
    }

    if let Some(where_str) = params.get("_where") {
        let where_obj: Value = serde_json::from_str(where_str)
            .map_err(|e| format!("Invalid JSON inside _where parameter: {}", e))?;

        if let Some(conditions) = where_obj.as_object() {
            for (field, block) in conditions {
                if field == "_sort" {
                    if let Some(s) = block.as_str() {
                        validate_identifier(s)?;
                        sort_by = Some(s.to_string());
                    }
                    continue;
                }
                if field == "_order" {
                    if let Some(o) = block.as_str() {
                        let o_upper = o.to_uppercase();
                        if o_upper == "ASC" || o_upper == "DESC" {
                            order_by = o_upper;
                        } else {
                            return Err("Invalid _order value. Must be 'asc' or 'desc'".to_string());
                        }
                    }
                    continue;
                }

                validate_identifier(field)?;

                match block {
                    Value::String(_) | Value::Number(_) | Value::Bool(_) => {
                        sql_clauses.push_str(&format!(" AND `{}`.`{}` = ?", main_table, field));
                        bind_values.push(block.clone());
                    }
                    Value::Object(inner_map) => {
                        for (op, op_val) in inner_map {
                            let op_sql = match op.as_str() {
                                "$gt" => ">",
                                "$gte" => ">=",
                                "$lt" => "<",
                                "$lte" => "<=",
                                "$neq" => "!=",
                                "$like" => "LIKE",
                                _ => return Err(format!("Unsupported operator: {}", op)),
                            };
                            sql_clauses.push_str(&format!(
                                " AND `{}`.`{}` {} ?",
                                main_table, field, op_sql
                            ));

                            if op.as_str() == "$like" {
                                let raw_str = op_val.as_str().unwrap_or("");
                                bind_values.push(Value::String(format!("%{}%", raw_str)));
                            } else {
                                bind_values.push(op_val.clone());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    if let Some(agg_str) = params.get("_aggregate") {
        let agg_obj: Value = serde_json::from_str(agg_str)
            .map_err(|e| format!("Invalid JSON inside _aggregate parameter: {}", e))?;

        if let Some(agg_map) = agg_obj.as_object() {
            select_list.clear();
            if let Some(ref g) = group_by {
                select_list.push(format!("`{}`.`{}`", main_table, g));
            }

            for (func_type, target_field) in agg_map {
                let field_str = target_field.as_str().unwrap_or("*");
                if field_str != "*" {
                    validate_identifier(field_str)?;
                }
                let table_prefix = if field_str == "*" {
                    "".to_string()
                } else {
                    format!("`{}`.", main_table)
                };

                match func_type.as_str() {
                    "count" => {
                        if field_str == "*" {
                            select_list.push("COUNT(*) AS `count`".to_string());
                            agg_functions_map.insert("count".to_string(), "COUNT(*)".to_string());
                        } else {
                            select_list.push(format!(
                                "COUNT({}`{}`) AS `count_{}`",
                                table_prefix, field_str, field_str
                            ));
                            agg_functions_map.insert(
                                format!("count_{}", field_str),
                                format!("COUNT({}`{})", table_prefix, field_str),
                            );
                        }
                    }
                    "sum" => {
                        select_list.push(format!(
                            "SUM({}`{}`) AS `sum_{}`",
                            table_prefix, field_str, field_str
                        ));
                        agg_functions_map.insert(
                            format!("sum_{}", field_str),
                            format!("SUM({}`{})", table_prefix, field_str),
                        );
                    }
                    "avg" => {
                        select_list.push(format!(
                            "AVG({}`{}`) AS `avg_{}`",
                            table_prefix, field_str, field_str
                        ));
                        agg_functions_map.insert(
                            format!("avg_{}", field_str),
                            format!("AVG({}`{})", table_prefix, field_str),
                        );
                    }
                    _ => return Err(format!("Unsupported aggregate function: {}", func_type)),
                }
            }
        }
    }

    if let Some(having_str) = params.get("_having") {
        let having_obj: Value = serde_json::from_str(having_str).map_err(|e| e.to_string())?;
        if let Some(having_map) = having_obj.as_object() {
            for (alias_key, block) in having_map {
                let physical_agg_func = if agg_functions_map.contains_key(alias_key) {
                    agg_functions_map.get(alias_key).unwrap().clone()
                } else if alias_key == "count" && agg_functions_map.contains_key("count") {
                    "COUNT(*)".to_string()
                } else {
                    return Err(format!(
                        "HAVING error: field '{}' must be declared inside _aggregate first",
                        alias_key
                    ));
                };

                match block {
                    Value::String(_) | Value::Number(_) | Value::Bool(_) => {
                        having_clauses.push_str(&format!(" AND {} = ?", physical_agg_func));
                        bind_values.push(block.clone());
                    }
                    Value::Object(inner_map) => {
                        for (op, op_val) in inner_map {
                            let op_sql = match op.as_str() {
                                "$gt" => ">",
                                "$gte" => ">=",
                                "$lt" => "<",
                                "$lte" => "<=",
                                "$neq" => "!=",
                                _ => return Err(format!("Unsupported operator in HAVING: {}", op)),
                            };
                            having_clauses
                                .push_str(&format!(" AND {} {} ?", physical_agg_func, op_sql));
                            bind_values.push(op_val.clone());
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    let select_fields = if select_list.is_empty() {
        if has_join {
            format!("`{}`.*", main_table)
        } else {
            "*".to_string()
        }
    } else {
        select_list.join(", ")
    };

    if let Some(sort_field) = sort_by {
        sql_clauses.push_str(&format!(
            " ORDER BY `{}`.`{}` {}",
            main_table, sort_field, order_by
        ));
    }

    Ok(QueryContext {
        select_fields,
        sql_clauses,
        bind_values,
        join_clauses,
        group_by,
        having_clauses,
        limit,
        offset,
    })
}
