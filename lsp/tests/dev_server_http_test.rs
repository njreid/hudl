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
    // Verify script injection
    assert!(body.contains("<script>"));
    assert!(body.contains("EventSource('/events')"));
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