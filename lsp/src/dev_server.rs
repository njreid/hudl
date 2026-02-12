//! Dev server for hot-reload template rendering.
//!
//! Runs an HTTP server alongside the LSP that allows the Go runtime
//! to render templates via HTTP during development, avoiding WASM recompilation.
//! Also serves the component preview SPA for browser-based development.

use axum::{
    Router,
    extract::{Path as AxumPath, Query, State, WebSocketUpgrade},
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Json},
    routing::{get, post, put},
};
use futures_util::{SinkExt, StreamExt};
use notify::{Event, RecursiveMode, Watcher};
use rust_embed::Embed;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

/// Embedded preview frontend assets (built from lsp/preview/).
#[derive(Embed)]
#[folder = "preview/dist/"]
struct PreviewAssets;

/// Parsed and cached template ready for rendering.
struct CachedTemplate {
    root: hudlc::ast::Root,
    schema: hudlc::proto::ProtoSchema,
    file_path: PathBuf,
}

/// Shared state for the dev server.
pub struct DevServerState {
    /// Cached templates: component name → (ast, schema, file_path)
    templates: Mutex<HashMap<String, CachedTemplate>>,
    /// Watch directory for .hudl files
    watch_dir: PathBuf,
    /// Broadcast channel for WebSocket reload notifications
    reload_tx: broadcast::Sender<String>,
    /// Whether to log detailed render requests
    verbose: bool,
}

impl DevServerState {
    pub fn new(watch_dir: PathBuf, verbose: bool) -> Self {
        let (reload_tx, _) = broadcast::channel(64);
        Self {
            templates: Mutex::new(HashMap::new()),
            watch_dir,
            reload_tx,
            verbose,
        }
    }

    /// Return the number of cached templates.
    pub fn templates_count(&self) -> usize {
        self.templates.lock().unwrap().len()
    }

    /// Check if a template with the given name is cached.
    pub fn has_template(&self, name: &str) -> bool {
        self.templates.lock().unwrap().contains_key(name)
    }

    /// Load all .hudl files from the watch directory (recursive).
    pub fn load_all(&self) {
        self.load_dir(&self.watch_dir.clone());
    }

    fn load_dir(&self, dir: &Path) {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                self.load_dir(&path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("hudl") {
                let _ = self.load_file(&path);
            }
        }
    }

    /// Load a single .hudl file into the cache.
    pub fn load_file(&self, path: &Path) -> Result<String, String> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                let err = format!("Failed to read {}: {}", path.display(), e);
                eprintln!("[dev-server] {}", err);
                return Err(err);
            }
        };

        let schema = hudlc::proto::ProtoSchema::from_template(&content, path.parent())
            .unwrap_or_default();

        let doc = match hudlc::parser::parse(&content) {
            Ok(d) => d,
            Err(e) => {
                let err = format!("Parse error in {}: {}", path.display(), e);
                eprintln!("[dev-server] {}", err);
                return Err(err);
            }
        };

        let root = match hudlc::transformer::transform_with_metadata(&doc, &content) {
            Ok(r) => r,
            Err(e) => {
                let err = format!("Transform error in {}: {}", path.display(), e);
                eprintln!("[dev-server] {}", err);
                return Err(err);
            }
        };

        if let Some(name) = &root.name {
            let name = name.clone();
            let mut templates = self.templates.lock().unwrap();
            eprintln!("[dev-server] Loaded component: {}", name);
            templates.insert(
                name.clone(),
                CachedTemplate {
                    root,
                    schema,
                    file_path: path.to_path_buf(),
                },
            );
            Ok(name)
        } else {
            Err("No component name found in file".to_string())
        }
    }

    /// Reload a file (on change notification).
    pub fn reload_file(&self, path: &Path) {
        eprintln!("[dev-server] Reloading: {}", path.display());
        match self.load_file(path) {
            Ok(name) => {
                // Notify WebSocket clients of the reload success
                let msg = serde_json::json!({
                    "type": "reload",
                    "component": name
                })
                .to_string();
                let _ = self.reload_tx.send(msg);
            }
            Err(err) => {
                // Notify WebSocket clients of the reload error
                let msg = serde_json::json!({
                    "type": "error",
                    "error": err,
                    "file": path.display().to_string()
                })
                .to_string();
                let _ = self.reload_tx.send(msg);
            }
        }
    }
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    templates_loaded: usize,
}

#[derive(Serialize)]
struct RenderErrorResponse {
    error: String,
    file: Option<String>,
}

#[derive(Serialize)]
struct ComponentInfo {
    name: String,
    data_type: Option<String>,
    file: String,
}

#[derive(Deserialize)]
struct RenderPreviewRequest {
    component: String,
    textproto: String,
}

#[derive(Serialize)]
struct PreviewFileInfo {
    label: String,
    file: String,
}

#[derive(Deserialize)]
struct PreviewDataQuery {
    file: Option<String>,
}

#[derive(Deserialize)]
struct CreatePreviewFileRequest {
    label: String,
}

/// GET /health
async fn health_handler(State(state): State<Arc<DevServerState>>) -> impl IntoResponse {
    let templates = state.templates.lock().unwrap();
    Json(HealthResponse {
        status: "ok".to_string(),
        templates_loaded: templates.len(),
    })
}

/// POST /render — Wire-format render (used by Go runtime)
async fn render_handler(
    State(state): State<Arc<DevServerState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    // Get component name from header
    let component_name = match headers.get("X-Hudl-Component") {
        Some(v) => match v.to_str() {
            Ok(s) => s.to_string(),
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(RenderErrorResponse {
                        error: "Invalid X-Hudl-Component header".to_string(),
                        file: None,
                    }),
                )
                    .into_response();
            }
        },
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(RenderErrorResponse {
                    error: "Missing X-Hudl-Component header".to_string(),
                    file: None,
                }),
            )
                .into_response();
        }
    };

    if state.verbose {
        eprintln!("[dev-server] Rendering: {}", component_name);
    }

    // Look up the cached template
    let templates = state.templates.lock().unwrap();
    let cached = match templates.get(&component_name) {
        Some(c) => c,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(RenderErrorResponse {
                    error: format!("Component '{}' not found", component_name),
                    file: None,
                }),
            )
                .into_response();
        }
    };

    // Render the template
    let start = std::time::Instant::now();
    match hudlc::interpreter::render(&cached.root, &cached.schema, &body) {
        Ok(html) => {
            let elapsed = start.elapsed();
            if state.verbose {
                eprintln!(
                    "[dev-server] Rendered {} in {:.2}ms",
                    component_name,
                    elapsed.as_secs_f64() * 1000.0
                );
            }
            let mut response_headers = HeaderMap::new();
            response_headers.insert(
                "Content-Type",
                "text/html; charset=utf-8".parse().unwrap(),
            );
            response_headers.insert(
                "X-Hudl-Render-Time-Ms",
                format!("{:.1}", elapsed.as_secs_f64() * 1000.0)
                    .parse()
                    .unwrap(),
            );
            (StatusCode::OK, response_headers, html).into_response()
        }
        Err(e) => {
            if state.verbose {
                eprintln!("[dev-server] Render error in {}: {}", component_name, e.message);
            }
            (
                StatusCode::BAD_REQUEST,
                Json(RenderErrorResponse {
                    error: e.message,
                    file: None,
                }),
            )
                .into_response()
        }
    }
}

/// GET /api/components — List all loaded components with metadata.
async fn list_components_handler(State(state): State<Arc<DevServerState>>) -> impl IntoResponse {
    let templates = state.templates.lock().unwrap();
    let mut components: Vec<ComponentInfo> = templates
        .iter()
        .map(|(name, cached)| ComponentInfo {
            name: name.clone(),
            data_type: cached.root.data_type.clone(),
            file: cached.file_path.display().to_string(),
        })
        .collect();
    components.sort_by(|a, b| a.name.cmp(&b.name));
    Json(components)
}

/// GET /api/proto-schema/:name — Return the proto schema JSON for a component.
async fn proto_schema_handler(
    State(state): State<Arc<DevServerState>>,
    AxumPath(name): AxumPath<String>,
) -> impl IntoResponse {
    let templates = state.templates.lock().unwrap();
    match templates.get(&name) {
        Some(cached) => Json(serde_json::to_value(&cached.schema).unwrap()).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(RenderErrorResponse {
                error: format!("Component '{}' not found", name),
                file: None,
            }),
        )
            .into_response(),
    }
}

/// POST /api/render-preview — Render with textproto data.
async fn render_preview_handler(
    State(state): State<Arc<DevServerState>>,
    Json(req): Json<RenderPreviewRequest>,
) -> impl IntoResponse {
    if state.verbose {
        eprintln!("[dev-server] Preview rendering: {}", req.component);
    }
    let templates = state.templates.lock().unwrap();
    let cached = match templates.get(&req.component) {
        Some(c) => c,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": format!("Component '{}' not found", req.component)
                })),
            )
                .into_response();
        }
    };

    // Parse textproto into CelValue
    let data_type = match &cached.root.data_type {
        Some(dt) => dt.clone(),
        None => {
            // No data type — render with no data (empty wire bytes)
            let start = std::time::Instant::now();
            match hudlc::interpreter::render(&cached.root, &cached.schema, &[]) {
                Ok(html) => {
                    let elapsed = start.elapsed();
                    if state.verbose {
                        eprintln!(
                            "[dev-server] Preview rendered {} in {:.2}ms",
                            req.component,
                            elapsed.as_secs_f64() * 1000.0
                        );
                    }
                    return (
                        StatusCode::OK,
                        Json(serde_json::json!({
                            "html": html,
                            "render_time_ms": format!("{:.1}", elapsed.as_secs_f64() * 1000.0)
                        })),
                    )
                        .into_response();
                }
                Err(e) => {
                    if state.verbose {
                        eprintln!("[dev-server] Preview render error in {}: {}", req.component, e.message);
                    }
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({ "error": e.message })),
                    )
                        .into_response();
                }
            }
        }
    };

    let cel_value = match hudlc::textproto::parse(&req.textproto, &data_type, &cached.schema) {
        Ok(v) => v,
        Err(e) => {
            if state.verbose {
                eprintln!("[dev-server] Textproto parse error: {}", e);
            }
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    let start = std::time::Instant::now();
    match hudlc::interpreter::render_with_values(&cached.root, &cached.schema, cel_value) {
        Ok(html) => {
            let elapsed = start.elapsed();
            if state.verbose {
                eprintln!(
                    "[dev-server] Preview rendered {} in {:.2}ms",
                    req.component,
                    elapsed.as_secs_f64() * 1000.0
                );
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "html": html,
                    "render_time_ms": format!("{:.1}", elapsed.as_secs_f64() * 1000.0)
                })),
            )
                .into_response()
        }
        Err(e) => {
            if state.verbose {
                eprintln!("[dev-server] Preview render error in {}: {}", req.component, e.message);
            }
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e.message })),
            )
                .into_response()
        }
    }
}

/// Compute the preview file path for a given component's .hudl path and variant.
/// "default" or empty → `stem.preview.txtpb`, otherwise `stem.variant.preview.txtpb`.
fn preview_file_path(hudl_path: &Path, variant: &str) -> PathBuf {
    let stem = hudl_path.file_stem().unwrap().to_str().unwrap();
    let dir = hudl_path.parent().unwrap();
    if variant.is_empty() || variant == "default" {
        dir.join(format!("{}.preview.txtpb", stem))
    } else {
        dir.join(format!("{}.{}.preview.txtpb", stem, variant))
    }
}

/// Scan the directory for all preview files matching a component's hudl file stem.
fn list_preview_files(hudl_path: &Path) -> Vec<PreviewFileInfo> {
    let stem = hudl_path.file_stem().unwrap().to_str().unwrap();
    let dir = hudl_path.parent().unwrap();
    let suffix = ".preview.txtpb";

    let mut files = Vec::new();

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_str().unwrap_or("");
            if name.starts_with(stem) && name.ends_with(suffix) {
                // Extract variant: stem.preview.txtpb → "default", stem.foo.preview.txtpb → "foo"
                let middle = &name[stem.len()..name.len() - suffix.len()];
                if middle.is_empty() || middle == "." {
                    // This shouldn't match — default has no dot before "preview"
                    // Actually: "card.preview.txtpb" → stem="card", suffix=".preview.txtpb"
                    // middle = "" — that's the default
                    files.push(PreviewFileInfo {
                        label: "default".to_string(),
                        file: "default".to_string(),
                    });
                } else if let Some(variant) = middle.strip_prefix('.') {
                    files.push(PreviewFileInfo {
                        label: variant.to_string(),
                        file: variant.to_string(),
                    });
                }
            }
        }
    }

    files.sort_by(|a, b| {
        // "default" always first
        if a.file == "default" {
            std::cmp::Ordering::Less
        } else if b.file == "default" {
            std::cmp::Ordering::Greater
        } else {
            a.label.cmp(&b.label)
        }
    });

    files
}

/// GET /api/preview-files/:name — List all preview files for a component.
async fn list_preview_files_handler(
    State(state): State<Arc<DevServerState>>,
    AxumPath(name): AxumPath<String>,
) -> impl IntoResponse {
    let templates = state.templates.lock().unwrap();
    let cached = match templates.get(&name) {
        Some(c) => c,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": format!("Component '{}' not found", name)
                })),
            )
                .into_response();
        }
    };

    let files = list_preview_files(&cached.file_path);
    Json(files).into_response()
}

/// POST /api/preview-files/:name — Create a new named preview file variant.
async fn create_preview_file_handler(
    State(state): State<Arc<DevServerState>>,
    AxumPath(name): AxumPath<String>,
    Json(req): Json<CreatePreviewFileRequest>,
) -> impl IntoResponse {
    let templates = state.templates.lock().unwrap();
    let cached = match templates.get(&name) {
        Some(c) => c,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": format!("Component '{}' not found", name)
                })),
            )
                .into_response();
        }
    };

    let path = preview_file_path(&cached.file_path, &req.label);
    let data_type = cached.root.data_type.clone();
    let schema = cached.schema.clone();
    drop(templates);

    if path.exists() {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": format!("Preview file '{}' already exists", req.label)
            })),
        )
            .into_response();
    }

    let content = if let Some(dt) = &data_type {
        match hudlc::textproto::generate_skeleton(dt, &schema) {
            Ok(s) => s,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
                    .into_response();
            }
        }
    } else {
        "# No data type defined for this component\n".to_string()
    };

    match std::fs::write(&path, &content) {
        Ok(()) => Json(serde_json::json!({
            "label": req.label,
            "textproto": content,
            "path": path.display().to_string()
        }))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to create file: {}", e)
            })),
        )
            .into_response(),
    }
}

/// GET /api/preview-data/:name — Load saved textproto or generate skeleton.
async fn get_preview_data_handler(
    State(state): State<Arc<DevServerState>>,
    AxumPath(name): AxumPath<String>,
    Query(query): Query<PreviewDataQuery>,
) -> impl IntoResponse {
    let variant = query.file.unwrap_or_else(|| "default".to_string());

    let templates = state.templates.lock().unwrap();
    let cached = match templates.get(&name) {
        Some(c) => c,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": format!("Component '{}' not found", name)
                })),
            )
                .into_response();
        }
    };

    let preview_path = preview_file_path(&cached.file_path, &variant);
    let data_type = cached.root.data_type.clone();
    let schema = cached.schema.clone();

    // Drop the Mutex before doing file I/O
    drop(templates);

    if preview_path.exists() {
        match std::fs::read_to_string(&preview_path) {
            Ok(content) => {
                return Json(serde_json::json!({
                    "textproto": content,
                    "source": "saved"
                }))
                .into_response();
            }
            Err(_) => {} // Fall through to skeleton
        }
    }

    // Generate skeleton and write to disk
    if let Some(dt) = &data_type {
        match hudlc::textproto::generate_skeleton(dt, &schema) {
            Ok(skeleton) => {
                // Auto-create the preview file on disk
                match std::fs::write(&preview_path, &skeleton) {
                    Ok(()) => {
                        eprintln!(
                            "[dev-server] Created preview file: {}",
                            preview_path.display()
                        );
                        Json(serde_json::json!({
                            "textproto": skeleton,
                            "source": "created"
                        }))
                        .into_response()
                    }
                    Err(_) => {
                        // Couldn't write but still return the skeleton
                        Json(serde_json::json!({
                            "textproto": skeleton,
                            "source": "skeleton"
                        }))
                        .into_response()
                    }
                }
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response(),
        }
    } else {
        let content = "# No data type defined for this component\n";
        let _ = std::fs::write(&preview_path, content);
        Json(serde_json::json!({
            "textproto": content,
            "source": "created"
        }))
        .into_response()
    }
}

/// PUT /api/preview-data/:name — Save textproto alongside the .hudl file.
async fn put_preview_data_handler(
    State(state): State<Arc<DevServerState>>,
    AxumPath(name): AxumPath<String>,
    Query(query): Query<PreviewDataQuery>,
    body: String,
) -> impl IntoResponse {
    let variant = query.file.unwrap_or_else(|| "default".to_string());

    let templates = state.templates.lock().unwrap();
    let cached = match templates.get(&name) {
        Some(c) => c,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": format!("Component '{}' not found", name)
                })),
            )
                .into_response();
        }
    };

    let preview_path = preview_file_path(&cached.file_path, &variant);
    match std::fs::write(&preview_path, &body) {
        Ok(()) => Json(serde_json::json!({
            "saved": true,
            "path": preview_path.display().to_string()
        }))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to save: {}", e)
            })),
        )
            .into_response(),
    }
}

/// GET /ws — WebSocket for live reload notifications.
async fn ws_handler(
    State(state): State<Arc<DevServerState>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        let mut rx = state.reload_tx.subscribe();
        let (mut sender, mut receiver) = socket.split();

        // Spawn task to forward broadcast messages to the WebSocket
        let send_task = tokio::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                if sender
                    .send(axum::extract::ws::Message::Text(msg.into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });

        // Read (and discard) incoming messages to detect disconnect
        let recv_task = tokio::spawn(async move {
            while let Some(Ok(_)) = receiver.next().await {}
        });

        // When either task finishes, abort the other
        tokio::select! {
            _ = send_task => {},
            _ = recv_task => {},
        }
    })
}

/// Serve embedded preview frontend assets.
async fn preview_static(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match PreviewAssets::get(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                StatusCode::OK,
                [(
                    axum::http::header::CONTENT_TYPE,
                    mime.as_ref().to_string(),
                )],
                file.data.to_vec(),
            )
                .into_response()
        }
        None => {
            // SPA fallback: serve index.html for unmatched routes
            match PreviewAssets::get("index.html") {
                Some(file) => (
                    StatusCode::OK,
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "text/html; charset=utf-8".to_string(),
                    )],
                    file.data.to_vec(),
                )
                    .into_response(),
                None => StatusCode::NOT_FOUND.into_response(),
            }
        }
    }
}

/// Create the dev server router for the given state.
pub fn create_router(state: Arc<DevServerState>) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/render", post(render_handler))
        .route("/api/components", get(list_components_handler))
        .route("/api/proto-schema/{name}", get(proto_schema_handler))
        .route("/api/render-preview", post(render_preview_handler))
        .route("/api/preview-data/{name}", get(get_preview_data_handler))
        .route("/api/preview-data/{name}", put(put_preview_data_handler))
        .route("/api/preview-files/{name}", get(list_preview_files_handler))
        .route(
            "/api/preview-files/{name}",
            post(create_preview_file_handler),
        )
        .route("/ws", get(ws_handler))
        .fallback(preview_static)
        .with_state(state)
}

/// Start the dev server.
///
/// # Arguments
/// * `port` - Port to listen on
/// * `watch_dir` - Directory containing .hudl files to serve
/// * `verbose` - Whether to enable detailed logging
pub async fn start(
    port: u16,
    watch_dir: PathBuf,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(DevServerState::new(watch_dir.clone(), verbose));

    // Load all templates initially
    state.load_all();

    // Set up file watcher
    let watcher_state = Arc::clone(&state);
    let watch_path = watch_dir.clone();
    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            if event.kind.is_modify() || event.kind.is_create() {
                for path in &event.paths {
                    if path.extension().and_then(|e| e.to_str()) == Some("hudl") {
                        watcher_state.reload_file(path);
                    }
                }
            }
        }
    })?;
    watcher.watch(&watch_path, RecursiveMode::Recursive)?;

    let app = create_router(Arc::clone(&state));

    let addr = format!("0.0.0.0:{}", port);
    eprintln!("[dev-server] Starting on http://localhost:{}", port);
    eprintln!("[dev-server] Watching: {}", watch_dir.display());

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    // Keep the watcher alive
    drop(watcher);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_hudl_file(dir: &Path, filename: &str, content: &str) -> PathBuf {
        let path = dir.join(filename);
        fs::write(&path, content).unwrap();
        path
    }

    fn valid_template(name: &str) -> String {
        format!(
            r#"// name: {}
el {{
    div "hello"
}}
"#,
            name
        )
    }

    #[test]
    fn test_load_file_valid() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_hudl_file(dir.path(), "card.hudl", &valid_template("Card"));

        let state = DevServerState::new(dir.path().to_path_buf(), false);
        let _ = state.load_file(&path);

        assert_eq!(state.templates_count(), 1);
        assert!(state.has_template("Card"));
    }

    #[test]
    fn test_load_file_parse_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_hudl_file(dir.path(), "bad.hudl", "// name: Bad\nthis is {{ not valid kdl");

        let state = DevServerState::new(dir.path().to_path_buf(), false);
        let _ = state.load_file(&path);

        assert_eq!(state.templates_count(), 0);
    }

    #[test]
    fn test_load_file_no_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_hudl_file(
            dir.path(),
            "noname.hudl",
            r#"el {
    div "no name comment"
}
"#,
        );

        let state = DevServerState::new(dir.path().to_path_buf(), false);
        let _ = state.load_file(&path);

        assert_eq!(state.templates_count(), 0);
    }

    #[test]
    fn test_load_all_directory() {
        let dir = tempfile::tempdir().unwrap();
        write_hudl_file(dir.path(), "a.hudl", &valid_template("Alpha"));
        write_hudl_file(dir.path(), "b.hudl", &valid_template("Beta"));

        let state = DevServerState::new(dir.path().to_path_buf(), false);
        state.load_all();

        assert_eq!(state.templates_count(), 2);
        assert!(state.has_template("Alpha"));
        assert!(state.has_template("Beta"));
    }

    #[test]
    fn test_load_all_skips_non_hudl() {
        let dir = tempfile::tempdir().unwrap();
        write_hudl_file(dir.path(), "a.hudl", &valid_template("Alpha"));
        fs::write(dir.path().join("readme.txt"), "not a template").unwrap();
        fs::write(dir.path().join("data.json"), "{}").unwrap();

        let state = DevServerState::new(dir.path().to_path_buf(), false);
        state.load_all();

        assert_eq!(state.templates_count(), 1);
        assert!(state.has_template("Alpha"));
    }

    #[test]
    fn test_reload_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_hudl_file(dir.path(), "card.hudl", &valid_template("Card"));

        let state = DevServerState::new(dir.path().to_path_buf(), false);
        let _ = state.load_file(&path);
        assert!(state.has_template("Card"));

        // Overwrite with a new component name
        fs::write(&path, valid_template("CardV2")).unwrap();
        state.reload_file(&path);

        assert!(state.has_template("CardV2"));
    }

    #[test]
    fn test_reload_file_introduces_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_hudl_file(dir.path(), "card.hudl", &valid_template("Card"));

        let state = DevServerState::new(dir.path().to_path_buf(), false);
        let _ = state.load_file(&path);
        assert!(state.has_template("Card"));

        // Overwrite with broken content — original "Card" should remain
        fs::write(&path, "// name: Card\nbroken {{ syntax").unwrap();
        state.reload_file(&path);

        // The stale "Card" is preserved because parse failed
        assert!(state.has_template("Card"));
    }

    #[test]
    fn test_cache_multiple_components() {
        let dir = tempfile::tempdir().unwrap();
        let path_a = write_hudl_file(dir.path(), "a.hudl", &valid_template("Header"));
        let path_b = write_hudl_file(dir.path(), "b.hudl", &valid_template("Footer"));

        let state = DevServerState::new(dir.path().to_path_buf(), false);
        let _ = state.load_file(&path_a);
        let _ = state.load_file(&path_b);

        assert_eq!(state.templates_count(), 2);
        assert!(state.has_template("Header"));
        assert!(state.has_template("Footer"));
    }

    #[test]
    fn test_cache_overwrite() {
        let dir = tempfile::tempdir().unwrap();

        let state = DevServerState::new(dir.path().to_path_buf(), false);

        // Two files with the same component name
        let path_a = write_hudl_file(dir.path(), "a.hudl", &valid_template("Widget"));
        let path_b = write_hudl_file(dir.path(), "b.hudl", &valid_template("Widget"));

        let _ = state.load_file(&path_a);
        assert_eq!(state.templates_count(), 1);

        let _ = state.load_file(&path_b);
        // Same name → still one entry (overwritten)
        assert_eq!(state.templates_count(), 1);
        assert!(state.has_template("Widget"));
    }
}
