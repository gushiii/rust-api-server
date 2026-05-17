use axum::response::IntoResponse;
use axum::{
    extract::{Json, Path, State},
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde_json::Value;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::binder::bind_json_value;
use crate::encoder::mysql_row_to_json;
use crate::handlers::AppState;
use crate::parser::validate_identifier;
use crate::response::ApiResponse;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: u64,
    pub token_type: String,
}

pub fn sign_token(
    username: &str,
    token_type: &str,
    expiration_secs: u64,
) -> Result<String, String> {
    let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string());
    let current_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let claims = Claims {
        sub: username.to_string(),
        exp: current_timestamp + expiration_secs,
        token_type: token_type.to_string(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| e.to_string())
}

pub async fn jwt_middleware(
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, Response> {
    let auth_header = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok());

    if let Some(auth_value) = auth_header
        && auth_value.starts_with("Bearer ")
    {
        let token = &auth_value[7..];
        let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string());

        if let Ok(token_data) = decode::<Claims>(
            token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::new(Algorithm::HS256),
        ) {
            let current_timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            if token_data.claims.token_type != "access" {
                return Err(build_unauthorized_response(
                    "Unauthorized: Invalid token type for business API.",
                ));
            }

            if token_data.claims.exp > current_timestamp {
                req.extensions_mut().insert(token_data.claims.sub);
                return Ok(next.run(req).await);
            } else {
                return Err(build_unauthorized_response(
                    "Unauthorized: Access token has expired. Please refresh.",
                ));
            }
        }
    }

    Err(build_unauthorized_response(
        "Unauthorized: Invalid or missing Bearer token",
    ))
}

fn build_unauthorized_response(err_msg: &str) -> Response {
    let mut response_map = serde_json::Map::new();
    response_map.insert("success".to_string(), Value::Bool(false));
    response_map.insert("status".to_string(), Value::Number(401.into()));
    response_map.insert("error".to_string(), Value::String(err_msg.to_string()));

    let mut http_res = Json(Value::Object(response_map)).into_response();
    *http_res.status_mut() = StatusCode::UNAUTHORIZED;
    http_res
}

pub async fn handle_login(
    State(state): State<AppState>,
    Path(table_name): Path<String>,
    Json(payload): Json<Value>,
) -> Result<ApiResponse<Value>, ApiResponse<Value>> {
    let start = Instant::now();
    validate_identifier(&table_name).map_err(ApiResponse::bad_request)?;

    let user_col = std::env::var("AUTH_USERNAME_COL").unwrap_or_else(|_| "username".to_string());
    let pwd_col = std::env::var("AUTH_PASSWORD_COL").unwrap_or_else(|_| "password".to_string());

    let obj = payload
        .as_object()
        .ok_or_else(|| ApiResponse::bad_request("Payload must be a JSON object"))?;
    let username = obj
        .get(&user_col)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiResponse::bad_request(format!("Missing '{}'", user_col)))?;
    let password = obj
        .get(&pwd_col)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiResponse::bad_request(format!("Missing '{}'", pwd_col)))?;

    let sql = format!(
        "SELECT `{}` FROM `{}` WHERE `{}` = ?",
        pwd_col, table_name, user_col
    );
    let hashed_password_opt = sqlx::query_scalar::<_, String>(&sql)
        .bind(username)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| ApiResponse::internal_error(e.to_string()))?;
    let hashed_password = hashed_password_opt
        .ok_or_else(|| ApiResponse::bad_request("Invalid username or password"))?;

    let is_valid = bcrypt::verify(password, &hashed_password)
        .map_err(|e| ApiResponse::internal_error(e.to_string()))?;
    if !is_valid {
        return Err(ApiResponse::bad_request("Invalid username or password"));
    }

    let access_exp = std::env::var("JWT_ACCESS_EXPIRATION_SECS")
        .unwrap_or_else(|_| "900".to_string())
        .parse::<u64>()
        .unwrap_or(900);
    let refresh_exp = std::env::var("JWT_REFRESH_EXPIRATION_SECS")
        .unwrap_or_else(|_| "604800".to_string())
        .parse::<u64>()
        .unwrap_or(604800);

    let final_access_token =
        sign_token(username, "access", access_exp).map_err(ApiResponse::internal_error)?;
    let final_refresh_token =
        sign_token(username, "refresh", refresh_exp).map_err(ApiResponse::internal_error)?;

    let mut data = serde_json::Map::new();
    data.insert(
        "access_token".to_string(),
        Value::String(final_access_token),
    );
    data.insert(
        "refresh_token".to_string(),
        Value::String(final_refresh_token),
    );
    data.insert(
        "token_type".to_string(),
        Value::String("Bearer".to_string()),
    );

    Ok(ApiResponse::success(Value::Object(data), start))
}

pub async fn handle_refresh(
    State(_state): State<AppState>,
    Path(table_name): Path<String>,
    Json(payload): Json<Value>,
) -> Result<ApiResponse<Value>, ApiResponse<Value>> {
    let start = Instant::now();
    validate_identifier(&table_name).map_err(ApiResponse::bad_request)?;

    let obj = payload
        .as_object()
        .ok_or_else(|| ApiResponse::bad_request("Payload must be a JSON object"))?;
    let refresh_token_input = obj
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            ApiResponse::bad_request("Missing 'refresh_token' parameter in JSON body")
        })?;

    println!("Received refresh token request for table '{}'", table_name);
    println!("Provided refresh token: {}", refresh_token_input);

    let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string());

    let token_data = decode::<Claims>(
        refresh_token_input,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|e| ApiResponse::bad_request(format!("Invalid refresh token: {}", e)))?;

    if token_data.claims.token_type != "refresh" {
        return Err(ApiResponse::bad_request(
            "Prohibited: Target token is not a valid refresh token.",
        ));
    }

    let current_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    if token_data.claims.exp <= current_timestamp {
        return Err(ApiResponse::bad_request(
            "Session expired: Refresh token has expired. Please log in again.",
        ));
    }

    let access_exp = std::env::var("JWT_ACCESS_EXPIRATION_SECS")
        .unwrap_or_else(|_| "900".to_string())
        .parse::<u64>()
        .unwrap_or(900);
    let new_access_token = sign_token(&token_data.claims.sub, "access", access_exp)
        .map_err(ApiResponse::internal_error)?;

    let mut data = serde_json::Map::new();
    data.insert("access_token".to_string(), Value::String(new_access_token));
    data.insert(
        "refresh_token".to_string(),
        Value::String(refresh_token_input.to_string()),
    );
    data.insert(
        "token_type".to_string(),
        Value::String("Bearer".to_string()),
    );

    Ok(ApiResponse::success(Value::Object(data), start))
}

pub async fn handle_register(
    State(state): State<AppState>,
    Path(table_name): Path<String>,
    Json(payload): Json<Value>,
) -> Result<ApiResponse<Value>, ApiResponse<Value>> {
    let start = Instant::now();
    validate_identifier(&table_name).map_err(ApiResponse::bad_request)?;
    let pwd_col = std::env::var("AUTH_PASSWORD_COL").unwrap_or_else(|_| "password".to_string());
    let mut obj = payload
        .as_object()
        .ok_or_else(|| ApiResponse::bad_request("Payload must be a JSON object"))?
        .clone();

    if let Some(Value::String(raw_pwd)) = obj.get(&pwd_col) {
        let hashed_pwd = bcrypt::hash(raw_pwd, bcrypt::DEFAULT_COST)
            .map_err(|e| ApiResponse::internal_error(e.to_string()))?;
        obj.insert(pwd_col.clone(), Value::String(hashed_pwd));
    }

    let mut columns = Vec::new();
    let mut placeholders = Vec::new();
    for (key, _) in &obj {
        validate_identifier(key).map_err(ApiResponse::bad_request)?;
        columns.push(format!("`{}`", key));
        placeholders.push("?");
    }

    let sql = format!(
        "INSERT INTO `{}` ({}) VALUES ({})",
        table_name,
        columns.join(", "),
        placeholders.join(", ")
    );
    let mut query = sqlx::query(&sql);
    for (_, val) in &obj {
        query = bind_json_value(query, val);
    }
    let result = query
        .execute(&state.pool)
        .await
        .map_err(|e| ApiResponse::internal_error(e.to_string()))?;

    let pk_sql = "SELECT COLUMN_NAME FROM information_schema.KEY_COLUMN_USAGE WHERE TABLE_SCHEMA = DATABASE() AND CONSTRAINT_NAME = 'PRIMARY' AND TABLE_NAME = ? LIMIT 1";
    let pk = sqlx::query_scalar::<_, String>(pk_sql)
        .bind(&table_name)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| ApiResponse::internal_error(e.to_string()))?
        .unwrap_or_else(|| "id".to_string());

    let select_sql = format!("SELECT * FROM `{}` WHERE `{}` = ?", table_name, pk);
    let row = sqlx::query(&select_sql)
        .bind(result.last_insert_id())
        .fetch_one(&state.pool)
        .await
        .map_err(|e| ApiResponse::internal_error(e.to_string()))?;

    Ok(ApiResponse::success(mysql_row_to_json(&row, ""), start))
}
