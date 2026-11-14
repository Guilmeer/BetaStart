//! Servidor MCP local: HTTP/SSE (legacy) + POST JSON no mesmo processo.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use axum::extract::{Query, State as AxumState};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;

use crate::mcp::McpContext;

pub static HTTP_RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Clone)]
struct HttpState {
    app_data: PathBuf,
    sessions: Arc<Mutex<HashMap<String, mpsc::Sender<String>>>>,
}

pub fn start(app_data: PathBuf, host: String, port: u16) {
    tauri::async_runtime::spawn(async move {
        let state = HttpState {
            app_data,
            sessions: Arc::new(Mutex::new(HashMap::new())),
        };

        let router = Router::new()
            .route("/health", get(health))
            .route("/sse", get(sse))
            .route("/messages", post(messages))
            .route("/mcp", post(mcp_post))
            .with_state(state);

        let address = format!("{host}:{port}");
        match tokio::net::TcpListener::bind(&address).await {
            Ok(listener) => {
                HTTP_RUNNING.store(true, Ordering::SeqCst);
                eprintln!("[betastart mcp] HTTP/SSE em http://{address}");
                if let Err(error) = axum::serve(listener, router).await {
                    eprintln!("[betastart mcp] servidor HTTP parou: {error}");
                }
            }
            Err(error) => {
                eprintln!("[betastart mcp] falha ao abrir {address}: {error}");
            }
        }
    });
}

async fn health() -> Json<Value> {
    Json(json!({
        "ok": true,
        "name": "betastart",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

fn token_from(headers: &HeaderMap, query: &HashMap<String, String>) -> Option<String> {
    if let Some(value) = query.get("token") {
        return Some(value.clone());
    }
    let header = headers.get(axum::http::header::AUTHORIZATION)?;
    let text = header.to_str().ok()?;
    text.strip_prefix("Bearer ")
        .map(|token| token.trim().to_string())
        .or_else(|| Some(text.trim().to_string()))
}

fn authenticate(app_data: &std::path::Path, token: &str) -> bool {
    McpContext::load_from(app_data)
        .ok()
        .and_then(|ctx| crate::db::mcp::verify_token(&ctx.db, token).ok().flatten())
        .is_some()
}

fn handle_message(app_data: &std::path::Path, token: &str, message: Value) -> Option<Value> {
    let mut ctx = McpContext::load_from(app_data).ok()?;
    let client = crate::db::mcp::verify_token(&ctx.db, token).ok()??;
    crate::mcp::handle(&mut ctx, &client, message)
}

async fn sse(
    AxumState(state): AxumState<HttpState>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let Some(token) = token_from(&headers, &query) else {
        return (StatusCode::UNAUTHORIZED, "missing token").into_response();
    };
    if !authenticate(&state.app_data, &token) {
        return (StatusCode::UNAUTHORIZED, "invalid token").into_response();
    }

    let session_id = uuid::Uuid::new_v4().simple().to_string();
    let (sender, receiver) = mpsc::channel::<String>(32);

    {
        let mut sessions = state.sessions.lock().unwrap();
        sessions.retain(|_, sender| !sender.is_closed());
        sessions.insert(session_id.clone(), sender);
    }

    let endpoint = format!("/messages?sessionId={session_id}&token={token}");
    let first = tokio_stream::once(Ok::<_, std::convert::Infallible>(
        Event::default().event("endpoint").data(endpoint),
    ));
    let stream = first.chain(ReceiverStream::new(receiver).map(|message| {
        Ok::<_, std::convert::Infallible>(Event::default().event("message").data(message))
    }));

    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

async fn messages(
    AxumState(state): AxumState<HttpState>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    Json(message): Json<Value>,
) -> Response {
    let Some(token) = token_from(&headers, &query) else {
        return (StatusCode::UNAUTHORIZED, "missing token").into_response();
    };
    if !authenticate(&state.app_data, &token) {
        return (StatusCode::UNAUTHORIZED, "invalid token").into_response();
    }

    let session_id = query.get("sessionId").cloned().unwrap_or_default();
    let sender = state.sessions.lock().unwrap().get(&session_id).cloned();

    let Some(sender) = sender else {
        return (StatusCode::NOT_FOUND, "unknown session").into_response();
    };

    let app_data = state.app_data.clone();
    tauri::async_runtime::spawn(async move {
        let response =
            tokio::task::spawn_blocking(move || handle_message(&app_data, &token, message))
                .await
                .ok()
                .flatten();

        if let Some(response) = response {
            let _ = sender.send(response.to_string()).await;
        }
    });

    StatusCode::ACCEPTED.into_response()
}

async fn mcp_post(
    AxumState(state): AxumState<HttpState>,
    headers: HeaderMap,
    Json(message): Json<Value>,
) -> Response {
    let Some(token) = token_from(&headers, &HashMap::new()) else {
        return (StatusCode::UNAUTHORIZED, "missing token").into_response();
    };
    if !authenticate(&state.app_data, &token) {
        return (StatusCode::UNAUTHORIZED, "invalid token").into_response();
    }

    let app_data = state.app_data.clone();
    let response = tokio::task::spawn_blocking(move || handle_message(&app_data, &token, message))
        .await
        .ok()
        .flatten();

    match response {
        Some(response) => Json(response).into_response(),
        None => StatusCode::ACCEPTED.into_response(),
    }
}
