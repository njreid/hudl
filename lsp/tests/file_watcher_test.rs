use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;
use tokio::net::TcpListener;

async fn body_string(body: Body) -> String {
    let bytes = body.collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn test_file_watcher_integration() {
    let dir = tempfile::tempdir().unwrap();
    let hudl_path = dir.path().join("card.hudl");

    // v1: initial template
    let v1_content = r#"// name: Card
el {
    div "v1"
}
"#;
    fs::write(&hudl_path, v1_content).unwrap();

    // Find an available port
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let watch_dir = dir.path().to_path_buf();
    
    // Start dev server in background
    tokio::spawn(async move {
        hudl_lsp::dev_server::start(port, watch_dir, true).await.unwrap();
    });

    // Wait for server to start and load initial file
    tokio::time::sleep(Duration::from_millis(500)).await;

    let client = reqwest::Client::new();
    let url = format!("http://localhost:{}/render", port);

    // 1. Render v1
    let resp = client.post(&url)
        .header("X-Hudl-Component", "Card")
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.text().await.unwrap();
    assert!(body.contains("v1"));

    // 2. Edit → v2 (Trigger file watcher)
    let v2_content = r#"// name: Card
el {
    div "v2 updated"
}
"#;
    fs::write(&hudl_path, v2_content).unwrap();

    // Wait for watcher to detect and reload (notify can take a bit)
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // Render again
    let resp = client.post(&url)
        .header("X-Hudl-Component", "Card")
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.text().await.unwrap();
    assert!(body.contains("v2 updated"), "Watcher should have detected change and reloaded. Body: {}", body);

    // 3. Break the file
    fs::write(&hudl_path, "// name: Card
broken {{ syntax").unwrap();
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // Render again — should return stale v2
    let resp = client.post(&url)
        .header("X-Hudl-Component", "Card")
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.text().await.unwrap();
    assert!(body.contains("v2 updated"), "Stale template should be preserved on error");

    // 4. Recover → v3
    let v3_content = r#"// name: Card
el {
    div "v3 recovered"
}
"#;
    fs::write(&hudl_path, v3_content).unwrap();
    tokio::time::sleep(Duration::from_millis(1000)).await;

    let resp = client.post(&url)
        .header("X-Hudl-Component", "Card")
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.text().await.unwrap();
    assert!(body.contains("v3 recovered"));
}
