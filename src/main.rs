mod encoder;
mod handlers;

use axum::{
    Router,
    routing::{get, post},
};
use handlers::{AppState, handle_create, handle_delete, handle_list, handle_update};
use sqlx::mysql::MySqlPoolOptions;

use crate::handlers::handle_get;

#[tokio::main]
async fn main() {
    let database_url = "mysql://root:password@127.0.0.1:3306/test_db";

    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
        .expect("Failed to connect to MySQL");

    let state = AppState { pool };
    let app = Router::new()
        .route("/api/v1/{table}", post(handle_create).get(handle_list))
        .route(
            "/api/v1/{table}/{id}",
            get(handle_get).put(handle_update).delete(handle_delete),
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("🚀 Zero-Code API Engine running on: http://127.0.0.1:8080");

    axum::serve(listener, app).await.unwrap();
}
