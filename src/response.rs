use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::Value;
use std::time::Instant;

#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> Response {
        let status_code =
            StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status_code, Json(self)).into_response()
    }
}

impl ApiResponse<Value> {
    pub fn success(data: Value, start_time: Instant) -> Self {
        let duration = start_time.elapsed().as_millis() as u64;
        Self {
            success: true,
            status: 200,
            data: Some(data),
            error: None,
            duration_ms: Some(duration),
        }
    }

    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            status: 400,
            data: None,
            error: Some(msg.into()),
            duration_ms: None,
        }
    }

    pub fn internal_error(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            status: 500,
            data: None,
            error: Some(msg.into()),
            duration_ms: None,
        }
    }
}
