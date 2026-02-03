//! Client for communicating with the hudl-analyzer Go process.
//!
//! Uses JSON-RPC over stdin/stdout for type analysis queries.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

/// Client for the hudl-analyzer process
pub struct AnalyzerClient {
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    request_id: AtomicU64,
    #[allow(dead_code)]
    child: Child,
}

#[derive(Debug, Serialize)]
struct Request<T: Serialize> {
    jsonrpc: &'static str,
    id: u64,
    method: &'static str,
    params: T,
}

#[derive(Debug, Deserialize)]
struct Response<T> {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: u64,
    result: Option<T>,
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    #[allow(dead_code)]
    code: i32,
    message: String,
}

// Request param types
#[derive(Debug, Serialize)]
struct InitializeParams {
    #[serde(rename = "workspaceRoot")]
    workspace_root: String,
}

#[derive(Debug, Serialize)]
struct ValidateExprParams {
    #[serde(rename = "rootType")]
    root_type: String,
    expression: String,
}

#[derive(Debug, Serialize)]
struct FindImplsParams {
    #[serde(rename = "packagePath")]
    package_path: String,
    #[serde(rename = "interfaceName")]
    interface_name: String,
}

#[derive(Debug, Serialize)]
struct LoadPackageParams {
    #[serde(rename = "packagePath")]
    package_path: String,
}

// Response result types
#[derive(Debug, Deserialize)]
struct InitializeResult {
    initialized: bool,
}

/// Result of expression validation
#[derive(Debug, Clone, Deserialize)]
pub struct ValidateExprResult {
    pub valid: bool,
    #[serde(rename = "resultType")]
    pub result_type: Option<String>,
    pub error: Option<String>,
}

/// Result of finding interface implementations
#[derive(Debug, Clone, Deserialize)]
pub struct FindImplsResult {
    pub implementations: Vec<String>,
}

impl AnalyzerClient {
    /// Spawn the hudl-analyzer process and initialize it with the workspace root.
    pub fn spawn(workspace_root: &str) -> Result<Self, String> {
        // Try to find hudl-analyzer in PATH or relative to the project
        let analyzer_path = std::env::var("HUDL_ANALYZER_PATH")
            .unwrap_or_else(|_| "hudl-analyzer".to_string());

        let mut child = Command::new(&analyzer_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("Failed to spawn hudl-analyzer ({}): {}", analyzer_path, e))?;

        let stdin = child.stdin.take().ok_or("Failed to get stdin")?;
        let stdout = child.stdout.take().ok_or("Failed to get stdout")?;

        let mut client = Self {
            stdin,
            stdout: BufReader::new(stdout),
            request_id: AtomicU64::new(0),
            child,
        };

        // Initialize
        let result: InitializeResult = client.call(
            "initialize",
            InitializeParams {
                workspace_root: workspace_root.to_string(),
            },
        )?;

        if !result.initialized {
            return Err("Analyzer failed to initialize".to_string());
        }

        Ok(client)
    }

    fn call<P: Serialize, R: for<'de> Deserialize<'de>>(
        &mut self,
        method: &'static str,
        params: P,
    ) -> Result<R, String> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);

        let request = Request {
            jsonrpc: "2.0",
            id,
            method,
            params,
        };

        let request_json =
            serde_json::to_string(&request).map_err(|e| format!("Serialize error: {}", e))?;

        writeln!(self.stdin, "{}", request_json).map_err(|e| format!("Write error: {}", e))?;
        self.stdin
            .flush()
            .map_err(|e| format!("Flush error: {}", e))?;

        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .map_err(|e| format!("Read error: {}", e))?;

        let response: Response<R> =
            serde_json::from_str(&line).map_err(|e| format!("Parse error: {} (line: {})", e, line))?;

        if let Some(err) = response.error {
            return Err(err.message);
        }

        response.result.ok_or_else(|| "No result in response".to_string())
    }

    /// Validate an expression path against a root type.
    ///
    /// # Arguments
    /// * `root_type` - Fully qualified type like "github.com/pkg/models.User"
    /// * `expression` - Field path like "profile.Address.City"
    pub fn validate_expression(
        &mut self,
        root_type: &str,
        expression: &str,
    ) -> Result<ValidateExprResult, String> {
        self.call(
            "validateExpression",
            ValidateExprParams {
                root_type: root_type.to_string(),
                expression: expression.to_string(),
            },
        )
    }

    /// Find all types implementing an interface.
    ///
    /// # Arguments
    /// * `package_path` - Package containing the interface
    /// * `interface_name` - Name of the interface
    pub fn find_implementations(
        &mut self,
        package_path: &str,
        interface_name: &str,
    ) -> Result<FindImplsResult, String> {
        self.call(
            "findImplementations",
            FindImplsParams {
                package_path: package_path.to_string(),
                interface_name: interface_name.to_string(),
            },
        )
    }

    /// Pre-load a package into the analyzer's cache.
    pub fn load_package(&mut self, package_path: &str) -> Result<(), String> {
        let _: serde_json::Value = self.call(
            "loadPackage",
            LoadPackageParams {
                package_path: package_path.to_string(),
            },
        )?;
        Ok(())
    }
}
