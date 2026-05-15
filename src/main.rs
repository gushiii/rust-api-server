mod encoder;
mod handlers;

use axum::{
    Router,
    routing::{get, post},
};
use handlers::{AppState, handle_create, handle_delete, handle_get, handle_list, handle_update};
use sqlx::mysql::MySqlPoolOptions;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().expect("Failed to load .env file");

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL environment variable must be set in .env");

    let server_port = std::env::var("SERVER_PORT").unwrap_or_else(|_| "8080".to_string());

    let pool = MySqlPoolOptions::new()
        .max_connections(
            std::env::var("DATABASE_POOL_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5),
        )
        .connect(&database_url)
        .await
        .expect("Failed to connect to MySQL using the provided DATABASE_URL");

    let state = AppState { pool };
    let app = Router::new()
        .route("/api/v1/{table}", post(handle_create).get(handle_list))
        .route(
            "/api/v1/{table}/{id}",
            get(handle_get).put(handle_update).delete(handle_delete),
        )
        .with_state(state);

    let bind_address = format!("0.0.0.0:{}", server_port);
    let listener = tokio::net::TcpListener::bind(&bind_address).await.unwrap();
    println!(
        "🚀 Zero-Code API Engine running on: http://{}",
        bind_address
    );

    axum::serve(listener, app).await.unwrap();
}
