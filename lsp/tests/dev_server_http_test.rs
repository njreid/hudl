use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use hudl_lsp::dev_server::{create_router, DevServerState};
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

fn valid_template(name: &str) -> String {
    format!(
        r#"// name: {name}
el {{
    div "hello from {name}"
}}
"#
    )
}

fn valid_template_with_proto(name: &str) -> String {
    format!(
        r#"/**
message {name}Data {{
    string title = 1;
}}
*/
// name: {name}
// data: {name}Data
el {{
    h1 `title`
}}
"#
    )
}

fn setup_with_content(files: &[(&str, &str)]) -> (TempDir, Arc<DevServerState>, axum::Router) {
    let dir = tempfile::tempdir().unwrap();
    for (filename, content) in files {
        fs::write(dir.path().join(filename), content).unwrap();
    }
    let state = Arc::new(DevServerState::new(dir.path().to_path_buf(), false));
    state.load_all();
    let router = create_router(Arc::clone(&state));
    (dir, state, router)
}

async fn body_string(body: Body) -> String {
    let bytes = body.collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn test_health_returns_ok() {
    let (_dir, _state, router) = setup_with_content(&[]);

    let response = router
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response.into_body()).await;
    assert!(body.contains(r#""status":"ok"#));
}

#[tokio::test]
async fn test_health_reflects_loaded() {
    let (_dir, _state, router) =
        setup_with_content(&[("a.hudl", &valid_template("Alpha")), ("b.hudl", &valid_template("Beta"))]);

    let response = router
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response.into_body()).await;
    assert!(body.contains(r#""templates_loaded":2"#));
}

#[tokio::test]
async fn test_render_simple_component() {
    let (_dir, _state, router) = setup_with_content(&[("card.hudl", &valid_template("Card"))]);

    let response = router
        .oneshot(
            Request::post("/render")
                .header("X-Hudl-Component", "Card")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("hello from Card"));
}

#[tokio::test]
async fn test_render_timing_header() {
    let (_dir, _state, router) = setup_with_content(&[("card.hudl", &valid_template("Card"))]);

    let response = router
        .oneshot(
            Request::post("/render")
                .header("X-Hudl-Component", "Card")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("x-hudl-render-time-ms"));
}

#[tokio::test]
async fn test_render_empty_data() {
    let (_dir, _state, router) =
        setup_with_content(&[("card.hudl", &valid_template_with_proto("Card"))]);

    let response = router
        .oneshot(
            Request::post("/render")
                .header("X-Hudl-Component", "Card")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    // With empty data, title defaults to "" (proto3 default)
    let body = body_string(response.into_body()).await;
    assert!(body.contains("<h1>"));
}

#[tokio::test]
async fn test_render_missing_header() {
    let (_dir, _state, router) = setup_with_content(&[("card.hudl", &valid_template("Card"))]);

    let response = router
        .oneshot(
            Request::post("/render")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("Missing X-Hudl-Component"));
}

#[tokio::test]
async fn test_render_unknown_component() {
    let (_dir, _state, router) = setup_with_content(&[("card.hudl", &valid_template("Card"))]);

    let response = router
        .oneshot(
            Request::post("/render")
                .header("X-Hudl-Component", "NonExistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("not found"));
}

#[tokio::test]
async fn test_render_bad_proto_data() {
    let (_dir, _state, router) =
        setup_with_content(&[("card.hudl", &valid_template_with_proto("Card"))]);

    // Send garbage proto bytes
    let garbage = vec![0xFF, 0xFF, 0xFF, 0xFF];

    let response = router
        .oneshot(
            Request::post("/render")
                .header("X-Hudl-Component", "Card")
                .body(Body::from(garbage))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("error"));
}

#[tokio::test]
async fn test_list_components_empty() {
    let (_dir, _state, router) = setup_with_content(&[]);

    let response = router
        .oneshot(Request::get("/api/components").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response.into_body()).await;
    assert_eq!(body, "[]");
}

#[tokio::test]
async fn test_list_components_populated() {
    let (_dir, _state, router) =
        setup_with_content(&[("a.hudl", &valid_template("Alpha")), ("b.hudl", &valid_template("Beta"))]);

    let response = router
        .oneshot(Request::get("/api/components").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("Alpha"));
    assert!(body.contains("Beta"));
}

#[tokio::test]
async fn test_edit_render_error_loop() {
    let dir = tempfile::tempdir().unwrap();
    let hudl_path = dir.path().join("card.hudl");

    // v1: initial template
    fs::write(&hudl_path, &valid_template("Card")).unwrap();

    let state = Arc::new(DevServerState::new(dir.path().to_path_buf(), false));
    state.load_all();

    // 1. Render v1
    let router = create_router(Arc::clone(&state));
    let response = router
        .oneshot(
            Request::post("/render")
                .header("X-Hudl-Component", "Card")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("hello from Card"));

    // 2. Edit → v2
    let v2_content = r#"// name: Card
el {
    div "updated content"
}
"#;
    fs::write(&hudl_path, v2_content).unwrap();
    state.reload_file(&hudl_path);

    let router = create_router(Arc::clone(&state));
    let response = router
        .oneshot(
            Request::post("/render")
                .header("X-Hudl-Component", "Card")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("updated content"));

    // 3. Break the file — stale v2 should be preserved
    fs::write(&hudl_path, "// name: Card\nbroken {{ syntax").unwrap();
    state.reload_file(&hudl_path);

    let router = create_router(Arc::clone(&state));
    let response = router
        .oneshot(
            Request::post("/render")
                .header("X-Hudl-Component", "Card")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("updated content"), "stale template should be preserved");

    // 4. Fix → v3
    let v3_content = r#"// name: Card
el {
    div "fixed v3"
}
"#;
    fs::write(&hudl_path, v3_content).unwrap();
    state.reload_file(&hudl_path);

    let router = create_router(Arc::clone(&state));
    let response = router
        .oneshot(
            Request::post("/render")
                .header("X-Hudl-Component", "Card")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("fixed v3"));
}

#[tokio::test]
async fn test_preview_data_auto_creates_file() {
    let (dir, _state, router) =
        setup_with_content(&[("card.hudl", &valid_template_with_proto("Card"))]);

    let preview_path = dir.path().join("card.preview.txtpb");
    assert!(!preview_path.exists(), "preview file should not exist yet");

    let response = router
        .oneshot(
            Request::get("/api/preview-data/Card")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response.into_body()).await;
    assert!(body.contains(r#""source":"created"#), "should return source=created, got: {}", body);

    // File should now exist on disk
    assert!(preview_path.exists(), "preview file should have been created on disk");
    let content = fs::read_to_string(&preview_path).unwrap();
    assert!(!content.is_empty(), "preview file should have skeleton content");
}

#[tokio::test]
async fn test_preview_data_returns_saved_on_second_get() {
    let (dir, state, _router) =
        setup_with_content(&[("card.hudl", &valid_template_with_proto("Card"))]);

    // First GET: creates the file
    let router = create_router(Arc::clone(&state));
    let response = router
        .oneshot(
            Request::get("/api/preview-data/Card")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response.into_body()).await;
    assert!(body.contains(r#""source":"created"#), "first GET should create, got: {}", body);

    let preview_path = dir.path().join("card.preview.txtpb");
    assert!(preview_path.exists());

    // Second GET: should read from disk
    let router = create_router(Arc::clone(&state));
    let response = router
        .oneshot(
            Request::get("/api/preview-data/Card")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response.into_body()).await;
    assert!(body.contains(r#""source":"saved"#), "second GET should return saved, got: {}", body);
}

#[tokio::test]
async fn test_preview_data_no_data_type_creates_file() {
    let (dir, _state, router) =
        setup_with_content(&[("card.hudl", &valid_template("Card"))]);

    let preview_path = dir.path().join("card.preview.txtpb");
    assert!(!preview_path.exists());

    let response = router
        .oneshot(
            Request::get("/api/preview-data/Card")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response.into_body()).await;
    assert!(body.contains(r#""source":"created"#), "should create file even without data type, got: {}", body);
    assert!(preview_path.exists(), "preview file should exist on disk");
}

#[tokio::test]
async fn test_preview_files_empty_initially() {
    let (_dir, _state, router) =
        setup_with_content(&[("card.hudl", &valid_template("Card"))]);

    let response = router
        .oneshot(
            Request::get("/api/preview-files/Card")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response.into_body()).await;
    assert_eq!(body, "[]");
}

#[tokio::test]
async fn test_preview_files_lists_default() {
    let (dir, state, _router) =
        setup_with_content(&[("card.hudl", &valid_template_with_proto("Card"))]);

    // Create the default preview file via GET
    let router = create_router(Arc::clone(&state));
    let _response = router
        .oneshot(
            Request::get("/api/preview-data/Card")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(dir.path().join("card.preview.txtpb").exists());

    // Now list files
    let router = create_router(Arc::clone(&state));
    let response = router
        .oneshot(
            Request::get("/api/preview-files/Card")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response.into_body()).await;
    assert!(body.contains(r#""label":"default"#), "should list default file, got: {}", body);
}

#[tokio::test]
async fn test_create_preview_file_variant() {
    let (dir, state, _router) =
        setup_with_content(&[("card.hudl", &valid_template_with_proto("Card"))]);

    // Create a variant
    let router = create_router(Arc::clone(&state));
    let response = router
        .oneshot(
            Request::post("/api/preview-files/Card")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"label":"empty"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response.into_body()).await;
    assert!(body.contains(r#""label":"empty"#), "should return created label, got: {}", body);

    // File should exist on disk
    let variant_path = dir.path().join("card.empty.preview.txtpb");
    assert!(variant_path.exists(), "variant file should be created on disk");
}

#[tokio::test]
async fn test_create_preview_file_conflict() {
    let (dir, state, _router) =
        setup_with_content(&[("card.hudl", &valid_template_with_proto("Card"))]);

    // Pre-create the variant file on disk
    fs::write(dir.path().join("card.empty.preview.txtpb"), "existing").unwrap();

    let router = create_router(Arc::clone(&state));
    let response = router
        .oneshot(
            Request::post("/api/preview-files/Card")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"label":"empty"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_preview_data_with_file_query_param() {
    let (_dir, state, _router) =
        setup_with_content(&[("card.hudl", &valid_template_with_proto("Card"))]);

    // Create a variant
    let router = create_router(Arc::clone(&state));
    let _response = router
        .oneshot(
            Request::post("/api/preview-files/Card")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"label":"empty"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    // Save custom data to the variant
    let router = create_router(Arc::clone(&state));
    let _response = router
        .oneshot(
            Request::put("/api/preview-data/Card?file=empty")
                .header("Content-Type", "text/plain")
                .body(Body::from("title: \"custom data\""))
                .unwrap(),
        )
        .await
        .unwrap();

    // Read it back
    let router = create_router(Arc::clone(&state));
    let response = router
        .oneshot(
            Request::get("/api/preview-data/Card?file=empty")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response.into_body()).await;
    assert!(body.contains("custom data"), "should return variant data, got: {}", body);
    assert!(body.contains(r#""source":"saved"#));
}

#[tokio::test]
async fn test_preview_files_lists_multiple() {
    let (dir, state, _router) =
        setup_with_content(&[("card.hudl", &valid_template_with_proto("Card"))]);

    // Create default + two variants on disk
    fs::write(dir.path().join("card.preview.txtpb"), "default data").unwrap();
    fs::write(dir.path().join("card.empty.preview.txtpb"), "empty data").unwrap();
    fs::write(dir.path().join("card.error-state.preview.txtpb"), "error data").unwrap();

    let router = create_router(Arc::clone(&state));
    let response = router
        .oneshot(
            Request::get("/api/preview-files/Card")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response.into_body()).await;
    let files: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    assert_eq!(files.len(), 3);
    // default should be first
    assert_eq!(files[0]["label"], "default");
    assert!(body.contains("empty"));
    assert!(body.contains("error-state"));
}
