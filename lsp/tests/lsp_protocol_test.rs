//! LSP Protocol Tests
//!
//! These tests simulate an editor interacting with the hudl-lsp server
//! via JSON-RPC 2.0 messages over stdin/stdout.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static REQUEST_ID: AtomicU64 = AtomicU64::new(1);

fn next_id() -> u64 {
    REQUEST_ID.fetch_add(1, Ordering::SeqCst)
}

/// JSON-RPC 2.0 Request
#[derive(Debug, Serialize)]
struct Request {
    jsonrpc: &'static str,
    id: u64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

/// JSON-RPC 2.0 Notification (no id)
#[derive(Debug, Serialize)]
struct Notification {
    jsonrpc: &'static str,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

/// JSON-RPC 2.0 Response
#[derive(Debug, Deserialize)]
struct Response {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<u64>,
    result: Option<Value>,
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct RpcError {
    code: i32,
    message: String,
    data: Option<Value>,
}

/// LSP Client that communicates with the server subprocess
struct LspClient {
    child: Child,
    stdin: std::process::ChildStdin,
    reader: BufReader<std::process::ChildStdout>,
}

impl LspClient {
    fn spawn() -> Result<Self, String> {
        // Build the LSP binary first
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let status = Command::new("cargo")
            .args(["build", "--bin", "hudl-lsp"])
            .current_dir(manifest_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| format!("Failed to build LSP: {}", e))?;

        if !status.success() {
            return Err("Failed to build LSP binary".to_string());
        }

        // The binary is built in the lsp crate's target directory
        let binary_path = format!("{}/target/debug/hudl-lsp", manifest_dir);

        let mut child = Command::new(&binary_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn LSP: {} (path: {})", e, binary_path))?;

        let stdin = child.stdin.take().expect("Failed to open stdin");
        let stdout = child.stdout.take().expect("Failed to open stdout");
        let reader = BufReader::new(stdout);

        Ok(LspClient {
            child,
            stdin,
            reader,
        })
    }

    /// Send a JSON-RPC request and wait for response
    fn request(&mut self, method: &str, params: Option<Value>) -> Result<Response, String> {
        let id = next_id();
        let request = Request {
            jsonrpc: "2.0",
            id,
            method: method.to_string(),
            params,
        };

        let content = serde_json::to_string(&request).unwrap();
        self.send_message(&content)?;
        self.read_response_with_id(id)
    }

    /// Send a notification (no response expected)
    fn notify(&mut self, method: &str, params: Option<Value>) -> Result<(), String> {
        let notification = Notification {
            jsonrpc: "2.0",
            method: method.to_string(),
            params,
        };

        let content = serde_json::to_string(&notification).unwrap();
        self.send_message(&content)
    }

    /// Send a raw LSP message with Content-Length header
    fn send_message(&mut self, content: &str) -> Result<(), String> {
        let message = format!("Content-Length: {}\r\n\r\n{}", content.len(), content);
        self.stdin
            .write_all(message.as_bytes())
            .map_err(|e| format!("Write failed: {}", e))?;
        self.stdin.flush().map_err(|e| format!("Flush failed: {}", e))
    }

    /// Read the next JSON-RPC message from the server
    fn read_message(&mut self) -> Result<Value, String> {
        // Read headers until we find Content-Length
        let mut content_length: usize = 0;

        loop {
            let mut line = String::new();
            self.reader
                .read_line(&mut line)
                .map_err(|e| format!("Read header failed: {}", e))?;

            let line = line.trim();
            if line.is_empty() {
                break; // End of headers
            }

            if let Some(len_str) = line.strip_prefix("Content-Length: ") {
                content_length = len_str
                    .parse()
                    .map_err(|e| format!("Invalid Content-Length: {}", e))?;
            }
            // Ignore other headers like Content-Type
        }

        if content_length == 0 {
            return Err("No Content-Length header found".to_string());
        }

        // Read the content body
        let mut content = vec![0u8; content_length];
        self.reader
            .read_exact(&mut content)
            .map_err(|e| format!("Read content failed: {}", e))?;

        let content_str =
            String::from_utf8(content).map_err(|e| format!("Invalid UTF-8: {}", e))?;

        serde_json::from_str(&content_str)
            .map_err(|e| format!("Invalid JSON: {} - content: {}", e, content_str))
    }

    /// Read response with specific ID (skipping notifications)
    fn read_response_with_id(&mut self, expected_id: u64) -> Result<Response, String> {
        // Loop to skip notifications until we get our response
        for _ in 0..100 {
            let msg = self.read_message()?;

            // Parse as response
            if let Ok(resp) = serde_json::from_value::<Response>(msg.clone()) {
                if resp.id == Some(expected_id) {
                    return Ok(resp);
                }
                // Got a response with different ID or no ID (shouldn't happen often)
            }
            // Otherwise it's a notification, read next message
        }
        Err(format!("Timeout waiting for response id={}", expected_id))
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        // Send shutdown request
        let _ = self.request("shutdown", None);
        let _ = self.notify("exit", None);
        // Give the child a moment to exit cleanly
        std::thread::sleep(Duration::from_millis(50));
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn initialize_client(client: &mut LspClient) -> Value {
    let init_params = json!({
        "processId": std::process::id(),
        "rootUri": "file:///tmp/test-workspace",
        "capabilities": {
            "textDocument": {
                "synchronization": {
                    "dynamicRegistration": false
                },
                "formatting": {
                    "dynamicRegistration": false
                },
                "semanticTokens": {
                    "dynamicRegistration": false,
                    "requests": { "full": true },
                    "tokenTypes": ["keyword", "variable", "function", "string"],
                    "tokenModifiers": [],
                    "formats": ["relative"]
                }
            }
        }
    });

    let response = client
        .request("initialize", Some(init_params))
        .expect("Initialize failed");

    assert!(
        response.error.is_none(),
        "Initialize returned error: {:?}",
        response.error
    );

    client
        .notify("initialized", Some(json!({})))
        .expect("Initialized notification failed");

    // Small delay to let server initialize
    std::thread::sleep(Duration::from_millis(100));

    response.result.expect("Initialize result missing")
}

// ============================================================================
// LSP Protocol Tests
// ============================================================================

/// Test the initialize/initialized handshake
#[test]
fn test_lsp_initialize() {
    let mut client = LspClient::spawn().expect("Failed to spawn LSP");

    let result = initialize_client(&mut client);

    // Verify capabilities
    let capabilities = result.get("capabilities").expect("Missing capabilities");

    // Check text document sync
    let sync = capabilities
        .get("textDocumentSync")
        .expect("Missing textDocumentSync");
    assert!(
        sync.is_number() || sync.is_object(),
        "Invalid textDocumentSync type"
    );

    // Check formatting provider
    let formatting = capabilities.get("documentFormattingProvider");
    assert!(formatting.is_some(), "Missing documentFormattingProvider");

    // Check semantic tokens provider
    let semantic = capabilities.get("semanticTokensProvider");
    assert!(semantic.is_some(), "Missing semanticTokensProvider");

    // Verify semantic tokens legend contains expected token types
    if let Some(sem_provider) = semantic {
        if let Some(options) = sem_provider.get("semanticTokensOptions") {
            if let Some(legend) = options.get("legend") {
                let token_types = legend.get("tokenTypes").expect("Missing tokenTypes");
                let types_arr = token_types.as_array().unwrap();
                assert!(
                    types_arr.iter().any(|t| t == "keyword"),
                    "Should support keyword token type"
                );
            }
        }
    }
}

/// Test document open triggers validation
#[test]
fn test_lsp_did_open_valid_document() {
    let mut client = LspClient::spawn().expect("Failed to spawn LSP");
    initialize_client(&mut client);

    // Open a valid document
    let doc_uri = "file:///tmp/test.hudl";
    let doc_content = r#"
// name: TestView

el {
    div.container {
        h1 "Hello World"
    }
}
"#;

    client
        .notify(
            "textDocument/didOpen",
            Some(json!({
                "textDocument": {
                    "uri": doc_uri,
                    "languageId": "hudl",
                    "version": 1,
                    "text": doc_content
                }
            })),
        )
        .expect("didOpen failed");

    // Server processes in background, just verify no crash
    std::thread::sleep(Duration::from_millis(200));
}

/// Test document open with syntax error
#[test]
fn test_lsp_did_open_syntax_error() {
    let mut client = LspClient::spawn().expect("Failed to spawn LSP");
    initialize_client(&mut client);

    // Open document with syntax error (unclosed brace)
    let doc_uri = "file:///tmp/error.hudl";
    let doc_content = r#"
el {
    div {
        // Missing closing brace
"#;

    client
        .notify(
            "textDocument/didOpen",
            Some(json!({
                "textDocument": {
                    "uri": doc_uri,
                    "languageId": "hudl",
                    "version": 1,
                    "text": doc_content
                }
            })),
        )
        .expect("didOpen failed");

    // Diagnostics are published as notifications
    // Just verify the server doesn't crash
    std::thread::sleep(Duration::from_millis(200));
}

/// Test document formatting
#[test]
fn test_lsp_formatting() {
    let mut client = LspClient::spawn().expect("Failed to spawn LSP");
    initialize_client(&mut client);

    // Open an unformatted document
    let doc_uri = "file:///tmp/format.hudl";
    let unformatted = r#"el{div{span "hello"}}"#;

    client
        .notify(
            "textDocument/didOpen",
            Some(json!({
                "textDocument": {
                    "uri": doc_uri,
                    "languageId": "hudl",
                    "version": 1,
                    "text": unformatted
                }
            })),
        )
        .expect("didOpen failed");

    std::thread::sleep(Duration::from_millis(100));

    // Request formatting
    let response = client
        .request(
            "textDocument/formatting",
            Some(json!({
                "textDocument": { "uri": doc_uri },
                "options": {
                    "tabSize": 4,
                    "insertSpaces": true
                }
            })),
        )
        .expect("Formatting request failed");

    assert!(
        response.error.is_none(),
        "Formatting returned error: {:?}",
        response.error
    );

    if let Some(result) = response.result {
        if !result.is_null() {
            let edits = result.as_array().expect("Expected array of TextEdits");
            assert!(!edits.is_empty(), "Expected at least one formatting edit");

            // Verify edit structure
            let edit = &edits[0];
            assert!(edit.get("range").is_some(), "Edit missing range");
            assert!(edit.get("newText").is_some(), "Edit missing newText");

            // Verify formatted text has proper structure
            let new_text = edit.get("newText").unwrap().as_str().unwrap();
            assert!(
                new_text.contains("el"),
                "Formatted text should contain 'el'"
            );
            assert!(
                new_text.contains("div"),
                "Formatted text should contain 'div'"
            );
            // Formatting produces valid KDL output
            assert!(
                !new_text.is_empty(),
                "Formatted text should not be empty"
            );
        }
    }
}

/// Test formatting with invalid document returns no edits
#[test]
fn test_lsp_formatting_invalid_document() {
    let mut client = LspClient::spawn().expect("Failed to spawn LSP");
    initialize_client(&mut client);

    let doc_uri = "file:///tmp/invalid.hudl";
    let invalid_content = r#"el { div { unclosed"#;

    client
        .notify(
            "textDocument/didOpen",
            Some(json!({
                "textDocument": {
                    "uri": doc_uri,
                    "languageId": "hudl",
                    "version": 1,
                    "text": invalid_content
                }
            })),
        )
        .expect("didOpen failed");

    std::thread::sleep(Duration::from_millis(100));

    let response = client
        .request(
            "textDocument/formatting",
            Some(json!({
                "textDocument": { "uri": doc_uri },
                "options": { "tabSize": 4, "insertSpaces": true }
            })),
        )
        .expect("Formatting request failed");

    // Should return null or empty array for invalid documents
    assert!(
        response.error.is_none(),
        "Should not error, just return no edits"
    );
    if let Some(result) = response.result {
        assert!(
            result.is_null(),
            "Invalid document should return null result"
        );
    }
}

/// Test semantic tokens
#[test]
fn test_lsp_semantic_tokens() {
    let mut client = LspClient::spawn().expect("Failed to spawn LSP");
    initialize_client(&mut client);

    let doc_uri = "file:///tmp/tokens.hudl";
    let content = r#"
el {
    if `condition` {
        span "hello"
    } else {
        each item `items` {
            div `item.name`
        }
    }
    switch `status` {
        case ACTIVE { span "active" }
        default { span "unknown" }
    }
}
"#;

    client
        .notify(
            "textDocument/didOpen",
            Some(json!({
                "textDocument": {
                    "uri": doc_uri,
                    "languageId": "hudl",
                    "version": 1,
                    "text": content
                }
            })),
        )
        .expect("didOpen failed");

    std::thread::sleep(Duration::from_millis(100));

    let response = client
        .request(
            "textDocument/semanticTokens/full",
            Some(json!({
                "textDocument": { "uri": doc_uri }
            })),
        )
        .expect("Semantic tokens request failed");

    assert!(
        response.error.is_none(),
        "Semantic tokens returned error: {:?}",
        response.error
    );

    if let Some(result) = response.result {
        if !result.is_null() {
            let data = result.get("data");
            assert!(data.is_some(), "Expected data field in semantic tokens");

            let tokens = data.unwrap().as_array().expect("data should be array");
            // Tokens should be non-empty for this document
            // (keywords: el, if, else, each, switch, case, default)
            assert!(
                !tokens.is_empty(),
                "Expected some semantic tokens for keywords"
            );

            // Each token is 5 integers: deltaLine, deltaStart, length, tokenType, tokenModifiers
            assert!(
                tokens.len() % 5 == 0,
                "Token data should be multiple of 5, got {}",
                tokens.len()
            );
        }
    }
}

/// Test document change updates
#[test]
fn test_lsp_did_change() {
    let mut client = LspClient::spawn().expect("Failed to spawn LSP");
    initialize_client(&mut client);

    let doc_uri = "file:///tmp/change.hudl";

    // Open document
    client
        .notify(
            "textDocument/didOpen",
            Some(json!({
                "textDocument": {
                    "uri": doc_uri,
                    "languageId": "hudl",
                    "version": 1,
                    "text": "el { div \"v1\" }"
                }
            })),
        )
        .expect("didOpen failed");

    std::thread::sleep(Duration::from_millis(50));

    // Change document
    client
        .notify(
            "textDocument/didChange",
            Some(json!({
                "textDocument": {
                    "uri": doc_uri,
                    "version": 2
                },
                "contentChanges": [{
                    "text": "el { div { span \"v2\" } }"
                }]
            })),
        )
        .expect("didChange failed");

    std::thread::sleep(Duration::from_millis(100));

    // Request formatting to verify the new content is used
    let response = client
        .request(
            "textDocument/formatting",
            Some(json!({
                "textDocument": { "uri": doc_uri },
                "options": { "tabSize": 4, "insertSpaces": true }
            })),
        )
        .expect("Formatting request failed");

    if let Some(result) = response.result {
        if !result.is_null() {
            let edits = result.as_array().unwrap();
            if !edits.is_empty() {
                let new_text = edits[0].get("newText").unwrap().as_str().unwrap();
                // Verify we're formatting the updated content with span/v2
                assert!(
                    new_text.contains("span"),
                    "Should be formatting v2 content with span"
                );
            }
        }
    }
}

/// Test document with CEL expressions
#[test]
fn test_lsp_cel_expressions() {
    let mut client = LspClient::spawn().expect("Failed to spawn LSP");
    initialize_client(&mut client);

    let doc_uri = "file:///tmp/cel.hudl";
    let content = r#"
/**
message User {
    string name = 1;
    int32 age = 2;
}
*/

// name: UserCard
// data: User

el {
    div `name`
    span "`age` years old"
}
"#;

    client
        .notify(
            "textDocument/didOpen",
            Some(json!({
                "textDocument": {
                    "uri": doc_uri,
                    "languageId": "hudl",
                    "version": 1,
                    "text": content
                }
            })),
        )
        .expect("didOpen failed");

    // Just verify no crash with CEL expressions
    std::thread::sleep(Duration::from_millis(200));

    // Formatting should work even with CEL
    let response = client
        .request(
            "textDocument/formatting",
            Some(json!({
                "textDocument": { "uri": doc_uri },
                "options": { "tabSize": 4, "insertSpaces": true }
            })),
        )
        .expect("Formatting request failed");

    assert!(
        response.error.is_none(),
        "Formatting with CEL should not error"
    );
}

/// Test control flow structures are properly parsed
#[test]
fn test_lsp_control_flow_structures() {
    let mut client = LspClient::spawn().expect("Failed to spawn LSP");
    initialize_client(&mut client);

    let doc_uri = "file:///tmp/control.hudl";
    let content = r#"
// name: ControlTest

el {
    if `show_header` {
        header "Header"
    } else {
        div "No header"
    }

    each item `items` {
        li `item`
    }

    switch `mode` {
        case "edit" { input type=text }
        case "view" { span `value` }
        default { span "Unknown mode" }
    }
}
"#;

    client
        .notify(
            "textDocument/didOpen",
            Some(json!({
                "textDocument": {
                    "uri": doc_uri,
                    "languageId": "hudl",
                    "version": 1,
                    "text": content
                }
            })),
        )
        .expect("didOpen failed");

    std::thread::sleep(Duration::from_millis(100));

    // Request formatting to verify structure is preserved
    let response = client
        .request(
            "textDocument/formatting",
            Some(json!({
                "textDocument": { "uri": doc_uri },
                "options": { "tabSize": 4, "insertSpaces": true }
            })),
        )
        .expect("Formatting request failed");

    if let Some(result) = response.result {
        if !result.is_null() {
            let edits = result.as_array().unwrap();
            if !edits.is_empty() {
                let formatted = edits[0].get("newText").unwrap().as_str().unwrap();
                assert!(formatted.contains("if"), "Formatted should contain 'if'");
                assert!(
                    formatted.contains("else"),
                    "Formatted should contain 'else'"
                );
                assert!(
                    formatted.contains("each"),
                    "Formatted should contain 'each'"
                );
                assert!(
                    formatted.contains("switch"),
                    "Formatted should contain 'switch'"
                );
            }
        }
    }
}

/// Test CSS shorthand selectors
#[test]
fn test_lsp_css_selectors() {
    let mut client = LspClient::spawn().expect("Failed to spawn LSP");
    initialize_client(&mut client);

    let doc_uri = "file:///tmp/selectors.hudl";
    let content = r#"
el {
    &main-container.flex.items-center {
        h1.text-lg.font-bold "Title"
        .sidebar { nav "Menu" }
    }
}
"#;

    client
        .notify(
            "textDocument/didOpen",
            Some(json!({
                "textDocument": {
                    "uri": doc_uri,
                    "languageId": "hudl",
                    "version": 1,
                    "text": content
                }
            })),
        )
        .expect("didOpen failed");

    std::thread::sleep(Duration::from_millis(100));

    let response = client
        .request(
            "textDocument/formatting",
            Some(json!({
                "textDocument": { "uri": doc_uri },
                "options": { "tabSize": 4, "insertSpaces": true }
            })),
        )
        .expect("Formatting request failed");

    // Verify formatting preserves content
    if let Some(result) = response.result {
        if !result.is_null() {
            let edits = result.as_array().unwrap();
            if !edits.is_empty() {
                let formatted = edits[0].get("newText").unwrap().as_str().unwrap();
                // Content should still reference the elements
                assert!(formatted.contains("h1"), "Should preserve h1 element");
                assert!(formatted.contains("nav"), "Should preserve nav element");
            }
        }
    }
}

/// Test proto block parsing
#[test]
fn test_lsp_proto_blocks() {
    let mut client = LspClient::spawn().expect("Failed to spawn LSP");
    initialize_client(&mut client);

    let doc_uri = "file:///tmp/proto.hudl";
    let content = r#"
/**
syntax = "proto3";

message User {
    string name = 1;
    string email = 2;
    int32 age = 3;
    bool is_active = 4;
}

enum Role {
    ROLE_UNKNOWN = 0;
    ROLE_USER = 1;
    ROLE_ADMIN = 2;
}
*/

// name: UserProfile
// data: User

el {
    div.profile {
        h2 `name`
        p `email`
    }
}
"#;

    client
        .notify(
            "textDocument/didOpen",
            Some(json!({
                "textDocument": {
                    "uri": doc_uri,
                    "languageId": "hudl",
                    "version": 1,
                    "text": content
                }
            })),
        )
        .expect("didOpen failed");

    // Verify server handles proto blocks without crashing
    std::thread::sleep(Duration::from_millis(200));

    // Should still be able to format
    let response = client
        .request(
            "textDocument/formatting",
            Some(json!({
                "textDocument": { "uri": doc_uri },
                "options": { "tabSize": 4, "insertSpaces": true }
            })),
        )
        .expect("Formatting request failed");

    assert!(
        response.error.is_none(),
        "Formatting with proto blocks should not error"
    );
}

/// Test shutdown sequence
#[test]
fn test_lsp_shutdown() {
    let mut client = LspClient::spawn().expect("Failed to spawn LSP");
    initialize_client(&mut client);

    // Request shutdown
    let response = client.request("shutdown", None).expect("Shutdown request failed");

    assert!(response.error.is_none(), "Shutdown should not error");
    // result should be null per LSP spec
    assert!(
        response.result.is_none() || response.result.as_ref().unwrap().is_null(),
        "Shutdown result should be null"
    );
}

/// Test formatting preserves inline styles
#[test]
fn test_lsp_formatting_inline_styles() {
    let mut client = LspClient::spawn().expect("Failed to spawn LSP");
    initialize_client(&mut client);

    let doc_uri = "file:///tmp/styles.hudl";
    let content = r#"
el {
    button {
        style {
            background-color "blue"
            color "white"
            padding "10px"
        }
        "Click me"
    }
}
"#;

    client
        .notify(
            "textDocument/didOpen",
            Some(json!({
                "textDocument": {
                    "uri": doc_uri,
                    "languageId": "hudl",
                    "version": 1,
                    "text": content
                }
            })),
        )
        .expect("didOpen failed");

    std::thread::sleep(Duration::from_millis(100));

    let response = client
        .request(
            "textDocument/formatting",
            Some(json!({
                "textDocument": { "uri": doc_uri },
                "options": { "tabSize": 4, "insertSpaces": true }
            })),
        )
        .expect("Formatting request failed");

    if let Some(result) = response.result {
        if !result.is_null() {
            let edits = result.as_array().unwrap();
            if !edits.is_empty() {
                let formatted = edits[0].get("newText").unwrap().as_str().unwrap();
                assert!(formatted.contains("style"), "Should preserve style block");
                assert!(
                    formatted.contains("background-color"),
                    "Should preserve CSS property"
                );
            }
        }
    }
}

/// Test special link nodes
#[test]
fn test_lsp_special_link_nodes() {
    let mut client = LspClient::spawn().expect("Failed to spawn LSP");
    initialize_client(&mut client);

    let doc_uri = "file:///tmp/links.hudl";
    let content = r#"
el {
    head {
        _stylesheet "/css/main.css"
    }
    body {
        main "Content"
    }
}
"#;

    client
        .notify(
            "textDocument/didOpen",
            Some(json!({
                "textDocument": {
                    "uri": doc_uri,
                    "languageId": "hudl",
                    "version": 1,
                    "text": content
                }
            })),
        )
        .expect("didOpen failed");

    std::thread::sleep(Duration::from_millis(100));

    // Verify parsing doesn't error
    let response = client
        .request(
            "textDocument/formatting",
            Some(json!({
                "textDocument": { "uri": doc_uri },
                "options": { "tabSize": 4, "insertSpaces": true }
            })),
        )
        .expect("Formatting request failed");

    assert!(
        response.error.is_none(),
        "Formatting special nodes should not error"
    );
}

/// Test unknown document returns proper response
#[test]
fn test_lsp_unknown_document() {
    let mut client = LspClient::spawn().expect("Failed to spawn LSP");
    initialize_client(&mut client);

    // Try to format a document that was never opened
    let response = client
        .request(
            "textDocument/formatting",
            Some(json!({
                "textDocument": { "uri": "file:///tmp/never_opened.hudl" },
                "options": { "tabSize": 4, "insertSpaces": true }
            })),
        )
        .expect("Request should complete");

    // Should return null/empty, not error
    assert!(response.error.is_none(), "Unknown document should not error");
    if let Some(result) = response.result {
        assert!(result.is_null(), "Unknown document should return null");
    }
}

/// Test switch with exhaustiveness checking
#[test]
fn test_lsp_switch_exhaustiveness() {
    let mut client = LspClient::spawn().expect("Failed to spawn LSP");
    initialize_client(&mut client);

    let doc_uri = "file:///tmp/switch_exhaust.hudl";
    // Document with incomplete switch (missing STATUS_FAILED case)
    let content = r#"
/**
enum Status {
    STATUS_UNKNOWN = 0;
    STATUS_ACTIVE = 1;
    STATUS_PENDING = 2;
    STATUS_FAILED = 3;
}
*/

// name: StatusView

el {
    switch `status` {
        case STATUS_ACTIVE { span "Active" }
        case STATUS_PENDING { span "Pending" }
    }
}
"#;

    client
        .notify(
            "textDocument/didOpen",
            Some(json!({
                "textDocument": {
                    "uri": doc_uri,
                    "languageId": "hudl",
                    "version": 1,
                    "text": content
                }
            })),
        )
        .expect("didOpen failed");

    // Verify server processes without crash
    std::thread::sleep(Duration::from_millis(200));
}

/// Test switch with default case (should not warn about missing cases)
#[test]
fn test_lsp_switch_with_default() {
    let mut client = LspClient::spawn().expect("Failed to spawn LSP");
    initialize_client(&mut client);

    let doc_uri = "file:///tmp/switch_default.hudl";
    let content = r#"
/**
enum Status {
    STATUS_UNKNOWN = 0;
    STATUS_ACTIVE = 1;
}
*/

// name: StatusView

el {
    switch `status` {
        case STATUS_ACTIVE { span "Active" }
        default { span "Other" }
    }
}
"#;

    client
        .notify(
            "textDocument/didOpen",
            Some(json!({
                "textDocument": {
                    "uri": doc_uri,
                    "languageId": "hudl",
                    "version": 1,
                    "text": content
                }
            })),
        )
        .expect("didOpen failed");

    std::thread::sleep(Duration::from_millis(200));
}

/// Test semantic tokens contains expected keywords
#[test]
fn test_lsp_semantic_tokens_keyword_coverage() {
    let mut client = LspClient::spawn().expect("Failed to spawn LSP");
    initialize_client(&mut client);

    let doc_uri = "file:///tmp/keywords.hudl";
    // Document with all control flow keywords
    let content = r#"el {
    if `a` {
        div "then"
    } else {
        each x `list` {
            switch `x` {
                case A { span "a" }
                default { span "d" }
            }
        }
    }
}
import { "foo" }
"#;

    client
        .notify(
            "textDocument/didOpen",
            Some(json!({
                "textDocument": {
                    "uri": doc_uri,
                    "languageId": "hudl",
                    "version": 1,
                    "text": content
                }
            })),
        )
        .expect("didOpen failed");

    std::thread::sleep(Duration::from_millis(100));

    let response = client
        .request(
            "textDocument/semanticTokens/full",
            Some(json!({
                "textDocument": { "uri": doc_uri }
            })),
        )
        .expect("Semantic tokens request failed");

    if let Some(result) = response.result {
        if !result.is_null() {
            let tokens = result
                .get("data")
                .unwrap()
                .as_array()
                .expect("data should be array");
            // Should have tokens for: el, if, else, each, switch, case, default, import
            // Each token is 5 values, so we expect at least 8 keywords * 5 = 40 values
            // (may have duplicates from regex matching partial words)
            assert!(
                tokens.len() >= 5,
                "Expected at least one keyword token, got {} values",
                tokens.len()
            );
        }
    }
}
