mod binder;
mod encoder;
mod handlers;
mod parser;
mod response;

use axum::{
    Router,
    routing::{get, post},
};
use handlers::{AppState, handle_create, handle_delete, handle_get, handle_list, handle_update};
use sqlx::mysql::MySqlPoolOptions;

use axum::http::{HeaderValue, Method};
use axum::response::IntoResponse;
use tower_http::cors::{AllowOrigin, CorsLayer};

use std::net::SocketAddr;
use std::sync::Arc;
use tower_governor::key_extractor::PeerIpKeyExtractor;
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder};

use crate::response::ApiResponse;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().expect("Failed to load .env file");

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env");
    let server_port = std::env::var("SERVER_PORT").unwrap_or_else(|_| "8080".to_string());

    let rate_limit_burst = std::env::var("RATE_LIMIT_BURST")
        .unwrap_or_else(|_| "10".to_string())
        .parse::<u32>()
        .unwrap_or(10);

    let rate_limit_per_sec = std::env::var("RATE_LIMIT_PER_SECOND")
        .unwrap_or_else(|_| "2".to_string())
        .parse::<u64>()
        .unwrap_or(2);

    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to MySQL");

    let state = AppState { pool };

    let allow_origin_rule = match std::env::var("CORS_ALLOWED_ORIGINS") {
        Ok(origins_str) if !origins_str.trim().is_empty() => {
            let origins: Vec<HeaderValue> = origins_str
                .split(',')
                .map(|s| s.trim().parse::<HeaderValue>())
                .filter_map(Result::ok)
                .collect();
            if origins.is_empty() {
                AllowOrigin::list(vec![])
            } else {
                AllowOrigin::list(origins)
            }
        }
        _ => AllowOrigin::list(vec![]),
    };
    let cors = CorsLayer::new()
        .allow_origin(allow_origin_rule)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(tower_http::cors::Any);

    let governor_config = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(rate_limit_per_sec)
            .burst_size(rate_limit_burst)
            .key_extractor(PeerIpKeyExtractor)
            .finish()
            .unwrap(),
    );

    let rate_limiter = GovernorLayer::new(governor_config).error_handler(|_error| {
        let custom_error = ApiResponse::too_many_requests(
            "Too Many Requests. Please slow down and try again later.",
        );
        custom_error.into_response()
    });

    let app = Router::new()
        .route("/api/v1/{table}", post(handle_create).get(handle_list))
        .route(
            "/api/v1/{table}/{id}",
            get(handle_get).put(handle_update).delete(handle_delete),
        )
        .with_state(state)
        .layer(cors)
        .layer(rate_limiter);

    let bind_address = format!("0.0.0.0:{}", server_port);
    let listener = tokio::net::TcpListener::bind(&bind_address).await.unwrap();
    println!(
        "🚀 Rust Zero-Code API Engine with Custom 429 JSON Response running on: http://{}",
        bind_address
    );

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
