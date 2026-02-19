use crate::db::{Database, Item};
use axum::{
    extract::{Path, State as AxumState},
    http::StatusCode,
    response::IntoResponse,
    routing::{patch, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
struct CreateSessionRequest {
    command: String,
    title: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CreateSessionResponse {
    id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct UpdateSessionRequest {
    status: String,
}

pub struct LocalServerState {
    pub db: Arc<Database>,
}

pub async fn start_local_server(db: Arc<Database>) -> anyhow::Result<()> {
    let state = LocalServerState { db };

    let app = Router::new()
        .route("/api/sessions", post(create_session))
        .route("/api/sessions/:id", patch(update_session))
        .with_state(Arc::new(state));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:19532").await?;
    println!("Local server listening on http://127.0.0.1:19532");

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            eprintln!("Local server error: {}", e);
        }
    });

    Ok(())
}

async fn create_session(
    AxumState(state): AxumState<Arc<LocalServerState>>,
    Json(payload): Json<CreateSessionRequest>,
) -> impl IntoResponse {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let mut metadata = std::collections::HashMap::new();
    metadata.insert("command".to_string(), payload.command);

    let item = Item {
        id: id.clone(),
        item_type: "cli_session".to_string(),
        title: payload.title,
        url: None,
        status: "in_progress".to_string(),
        previous_status: None,
        metadata: serde_json::to_string(&metadata).unwrap_or_else(|_| "{}".to_string()),
        last_checked_at: Some(now.clone()),
        last_updated_at: Some(now.clone()),
        created_at: now,
        archived: false,
        polling_interval_override: None,
        checked: false,
    };

    match state.db.add_item(&item) {
        Ok(_) => (
            StatusCode::CREATED,
            Json(CreateSessionResponse { id }),
        ),
        Err(e) => {
            eprintln!("Failed to create session: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreateSessionResponse {
                    id: "error".to_string(),
                }),
            )
        }
    }
}

async fn update_session(
    AxumState(state): AxumState<Arc<LocalServerState>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateSessionRequest>,
) -> impl IntoResponse {
    match state.db.update_item_status(&id, &payload.status, None) {
        Ok(_) => StatusCode::OK,
        Err(e) => {
            eprintln!("Failed to update session: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
