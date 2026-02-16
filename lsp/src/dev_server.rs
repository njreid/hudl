//! Dev server for hot-reload template rendering.
//!
//! Runs an HTTP server alongside the LSP that allows the Go runtime
//! to render templates via HTTP during development, avoiding WASM recompilation.

use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode, Method},
    response::{IntoResponse, Json, Sse, sse::Event as SseEvent},
    routing::{get, post},
};
use tower_http::cors::{Any, CorsLayer};
use futures_util::stream::Stream;
use notify::{Event, RecursiveMode, Watcher};
use serde::Serialize;
use std::collections::HashMap;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{StreamExt as _, StreamExt};

/// Parsed and cached template ready for rendering.
struct CachedTemplate {
    root: hudlc::ast::Root,
    schema: hudlc::proto::ProtoSchema,
}

/// Shared state for the dev server.
pub struct DevServerState {
    /// Cached templates: component name → (ast, schema, file_path)
    templates: Mutex<HashMap<String, CachedTemplate>>,
    /// Watch directory for .hudl files
    watch_dir: PathBuf,
    /// Broadcast channel for reload notifications
    reload_tx: broadcast::Sender<String>,
    /// Port this server is running on
    port: u16,
    /// Whether to log detailed render requests
    verbose: bool,
}

impl DevServerState {
    pub fn new(watch_dir: PathBuf, port: u16, verbose: bool) -> Self {
        let (reload_tx, _) = broadcast::channel(64);
        Self {
            templates: Mutex::new(HashMap::new()),
            watch_dir,
            reload_tx,
            port,
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
                // Notify via broadcast channel
                let msg = serde_json::json!({
                    "type": "reload",
                    "component": name,
                    "file": path.display().to_string()
                })
                .to_string();
                let _ = self.reload_tx.send(msg);
            }
            Err(err) => {
                // Notify error
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

/// GET /health
async fn health_handler(State(state): State<Arc<DevServerState>>) -> impl IntoResponse {
    let templates = state.templates.lock().unwrap();
    Json(HealthResponse {
        status: "ok".to_string(),
        templates_loaded: templates.len(),
    })
}

/// GET /__hudl/live_reload — SSE for live reload notifications.
async fn live_reload_handler(
    State(state): State<Arc<DevServerState>>,
) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let rx = state.reload_tx.subscribe();
    
    let reload_stream = BroadcastStream::new(rx)
        .filter_map(|msg| msg.ok())
        .map(|msg| {
            Ok(SseEvent::default().data(msg))
        });

    // Add a heartbeat every 15 seconds to keep the connection alive
    let heartbeat_stream = tokio_stream::StreamExt::map(
        tokio_stream::wrappers::IntervalStream::new(tokio::time::interval(std::time::Duration::from_secs(15))),
        |_| Ok(SseEvent::default().comment("heartbeat"))
    );

    let combined_stream = tokio_stream::StreamExt::merge(reload_stream, heartbeat_stream);

    Sse::new(combined_stream)
}

/// POST /render — Wire-format render (used by Go runtime)
async fn render_handler(
    State(state): State<Arc<DevServerState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    // ... (rest of the render_handler)
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
    
    // Build component map for the interpreter
    let mut components = HashMap::new();
    for (name, cached) in templates.iter() {
        components.insert(name.clone(), &cached.root);
    }

    match hudlc::interpreter::render(&cached.root, &cached.schema, &body, &components) {
        Ok(mut html) => {
            let elapsed = start.elapsed();
            if state.verbose {
                eprintln!(
                    "[dev-server] Rendered {} in {:.2}ms",
                    component_name,
                    elapsed.as_secs_f64() * 1000.0
                );
            }

            // Inject reload script using the LSP port
            let reload_script = format!(r#"
<script>
  (function() {{
    console.log('Hudl: Connecting to dev server for live reload on port {}...');
    const ev = new EventSource('http://localhost:{}/__hudl/live_reload');
    ev.onmessage = (e) => {{
      try {{
        const data = JSON.parse(e.data);
        if (data.type === 'reload') {{
          console.log('Hudl: File change detected, reloading...');
          location.reload();
        }}
      }} catch(err) {{}}
    }};
    ev.onerror = (e) => {{
      console.error('Hudl: Live reload connection error. Retrying...', e);
    }};
    ev.onopen = () => {{
      console.log('Hudl: Live reload connected.');
    }};
  }})();
</script>
"#, state.port, state.port);

            let lower_html = html.to_lowercase();
            if let Some(pos) = lower_html.find("</body>") {
                html.insert_str(pos, &reload_script);
            } else {
                html.push_str(&reload_script);
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

/// Create the dev server router for the given state.
pub fn create_router(state: Arc<DevServerState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    Router::new()
        .route("/health", get(health_handler))
        .route("/render", post(render_handler))
        .route("/__hudl/live_reload", get(live_reload_handler))
        .layer(cors)
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
    let state = Arc::new(DevServerState::new(watch_dir.clone(), port, verbose));

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

    let addr = format!("127.0.0.1:{}", port);
    eprintln!("[dev-server] Starting on http://{}", addr);
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

        let state = DevServerState::new(dir.path().to_path_buf(), 9999, false);
        let _ = state.load_file(&path);

        assert_eq!(state.templates_count(), 1);
        assert!(state.has_template("Card"));
    }

    #[test]
    fn test_load_file_parse_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_hudl_file(dir.path(), "bad.hudl", "// name: Bad\nthis is {{ not valid kdl");

        let state = DevServerState::new(dir.path().to_path_buf(), 9999, false);
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

        let state = DevServerState::new(dir.path().to_path_buf(), 9999, false);
        let _ = state.load_file(&path);

        assert_eq!(state.templates_count(), 0);
    }

    #[test]
    fn test_load_all_directory() {
        let dir = tempfile::tempdir().unwrap();
        write_hudl_file(dir.path(), "a.hudl", &valid_template("Alpha"));
        write_hudl_file(dir.path(), "b.hudl", &valid_template("Beta"));

        let state = DevServerState::new(dir.path().to_path_buf(), 9999, false);
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

        let state = DevServerState::new(dir.path().to_path_buf(), 9999, false);
        state.load_all();

        assert_eq!(state.templates_count(), 1);
        assert!(state.has_template("Alpha"));
    }

    #[test]
    fn test_reload_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_hudl_file(dir.path(), "card.hudl", &valid_template("Card"));

        let state = DevServerState::new(dir.path().to_path_buf(), 9999, false);
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

        let state = DevServerState::new(dir.path().to_path_buf(), 9999, false);
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

        let state = DevServerState::new(dir.path().to_path_buf(), 9999, false);
        let _ = state.load_file(&path_a);
        let _ = state.load_file(&path_b);

        assert_eq!(state.templates_count(), 2);
        assert!(state.has_template("Header"));
        assert!(state.has_template("Footer"));
    }

    #[test]
    fn test_cache_overwrite() {
        let dir = tempfile::tempdir().unwrap();

        let state = DevServerState::new(dir.path().to_path_buf(), 9999, false);

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