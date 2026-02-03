use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use std::sync::Mutex;
use std::collections::HashMap;
use regex::Regex;

mod analyzer_client;
mod exhaustiveness;
mod param;

use analyzer_client::AnalyzerClient;
use param::ViewMetadata;

struct Backend {
    client: Client,
    document_map: Mutex<HashMap<Url, String>>,
    analyzer: Mutex<Option<AnalyzerClient>>,
    workspace_root: Mutex<Option<String>>,
}

const LEGEND_TYPE: &[SemanticTokenType] = &[
    SemanticTokenType::KEYWORD,
    SemanticTokenType::VARIABLE,
    SemanticTokenType::FUNCTION,
    SemanticTokenType::STRING,
];

impl Backend {
    async fn on_change(&self, params: TextDocumentItem) {
        self.document_map.lock().unwrap().insert(params.uri.clone(), params.text.clone());
        self.validate_document(&params.uri, &params.text).await;
    }

    async fn validate_document(&self, uri: &Url, content: &str) {
        let mut diagnostics = Vec::new();

        // 1. Syntax validation
        if let Err(e) = hudlc::parser::parse(content) {
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position { line: 0, character: 0 },
                    end: Position { line: 0, character: 0 },
                },
                severity: Some(DiagnosticSeverity::ERROR),
                message: format!("{}", e),
                ..Default::default()
            });
            self.client.publish_diagnostics(uri.clone(), diagnostics, None).await;
            return;
        }

        // 2. Type validation (if analyzer is available)
        let metadata = param::extract_metadata(content);

        // Build scope from params
        let scope = self.build_scope(&metadata);

        // Pre-load packages for all params
        self.preload_packages(&metadata).await;

        // Find all expressions in backticks and validate them
        let expr_diagnostics = self.validate_expressions(content, &scope).await;
        diagnostics.extend(expr_diagnostics);

        // Check switch exhaustiveness
        let switch_diagnostics = self.check_switch_exhaustiveness(content, &scope).await;
        diagnostics.extend(switch_diagnostics);

        self.client.publish_diagnostics(uri.clone(), diagnostics, None).await;
    }

    fn build_scope(&self, metadata: &ViewMetadata) -> HashMap<String, String> {
        let mut scope = HashMap::new();
        for p in &metadata.params {
            let qualified = param::qualified_type(p);
            scope.insert(p.name.clone(), qualified);
        }
        scope
    }

    async fn preload_packages(&self, metadata: &ViewMetadata) {
        let mut analyzer = self.analyzer.lock().unwrap();
        if let Some(ref mut client) = *analyzer {
            for import in &metadata.imports {
                let _ = client.load_package(&import.path);
            }
        }
    }

    async fn validate_expressions(
        &self,
        content: &str,
        scope: &HashMap<String, String>,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let backtick_re = Regex::new(r"`([^`]+)`").unwrap();

        for (line_num, line) in content.lines().enumerate() {
            for cap in backtick_re.captures_iter(line) {
                let expr_str = &cap[1];
                let match_start = cap.get(1).unwrap().start();

                // Parse expression
                match hudlc::expr::parse(expr_str) {
                    Ok(expr) => {
                        // Validate against scope
                        if let Some(diag) = self.validate_expr(&expr, scope, line_num as u32, match_start as u32) {
                            diagnostics.push(diag);
                        }
                    }
                    Err(e) => {
                        diagnostics.push(Diagnostic {
                            range: Range {
                                start: Position { line: line_num as u32, character: match_start as u32 },
                                end: Position { line: line_num as u32, character: (match_start + expr_str.len()) as u32 },
                            },
                            severity: Some(DiagnosticSeverity::ERROR),
                            message: format!("Expression parse error: {}", e),
                            ..Default::default()
                        });
                    }
                }
            }
        }

        diagnostics
    }

    fn validate_expr(
        &self,
        expr: &hudlc::expr::Expr,
        scope: &HashMap<String, String>,
        line: u32,
        col: u32,
    ) -> Option<Diagnostic> {
        match expr {
            hudlc::expr::Expr::Variable(path) => {
                let parts: Vec<&str> = path.split('.').collect();
                let root = parts[0];

                // Check if root variable is in scope
                if let Some(root_type) = scope.get(root) {
                    // Validate field path via Go analyzer
                    if parts.len() > 1 {
                        let field_path = parts[1..].join(".");
                        let mut analyzer = self.analyzer.lock().unwrap();
                        if let Some(ref mut client) = *analyzer {
                            match client.validate_expression(root_type, &field_path) {
                                Ok(result) if !result.valid => {
                                    return Some(Diagnostic {
                                        range: Range {
                                            start: Position { line, character: col },
                                            end: Position { line, character: col + path.len() as u32 },
                                        },
                                        severity: Some(DiagnosticSeverity::ERROR),
                                        message: result.error.unwrap_or_else(|| "Invalid field access".to_string()),
                                        ..Default::default()
                                    });
                                }
                                Err(e) => {
                                    // Log but don't show to user (analyzer issue)
                                    eprintln!("Analyzer error: {}", e);
                                }
                                _ => {}
                            }
                        }
                    }
                } else {
                    return Some(Diagnostic {
                        range: Range {
                            start: Position { line, character: col },
                            end: Position { line, character: col + root.len() as u32 },
                        },
                        severity: Some(DiagnosticSeverity::ERROR),
                        message: format!("Unknown variable: {}", root),
                        ..Default::default()
                    });
                }
                None
            }
            hudlc::expr::Expr::Binary(left, _, right) => {
                self.validate_expr(left, scope, line, col)
                    .or_else(|| self.validate_expr(right, scope, line, col))
            }
            hudlc::expr::Expr::Unary(_, inner) => {
                self.validate_expr(inner, scope, line, col)
            }
            hudlc::expr::Expr::Call(name, args) => {
                // Validate built-in functions
                let known_funcs = ["len"];
                if !known_funcs.contains(&name.as_str()) {
                    return Some(Diagnostic {
                        range: Range {
                            start: Position { line, character: col },
                            end: Position { line, character: col + name.len() as u32 },
                        },
                        severity: Some(DiagnosticSeverity::WARNING),
                        message: format!("Unknown function: {}", name),
                        ..Default::default()
                    });
                }
                // Validate arguments
                for arg in args {
                    if let Some(diag) = self.validate_expr(arg, scope, line, col) {
                        return Some(diag);
                    }
                }
                None
            }
            hudlc::expr::Expr::MethodCall(receiver, _method, args) => {
                // Validate receiver
                if let Some(diag) = self.validate_expr(receiver, scope, line, col) {
                    return Some(diag);
                }
                // Validate arguments
                for arg in args {
                    if let Some(diag) = self.validate_expr(arg, scope, line, col) {
                        return Some(diag);
                    }
                }
                // Note: We can't validate method existence without more type info
                None
            }
            hudlc::expr::Expr::Literal(_) => None,
        }
    }

    async fn check_switch_exhaustiveness(
        &self,
        content: &str,
        scope: &HashMap<String, String>,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Extract switch statements from content
        let switches = exhaustiveness::extract_switches(content);

        // Check each switch for exhaustiveness
        let mut analyzer = self.analyzer.lock().unwrap();
        if let Some(ref mut client) = *analyzer {
            for switch_info in switches {
                if let Some(diag) = exhaustiveness::check_switch(&switch_info, scope, client) {
                    diagnostics.push(diag);
                }
            }
        }

        diagnostics
    }

    async fn try_init_analyzer(&self) {
        let workspace_root = self.workspace_root.lock().unwrap().clone();
        if let Some(root) = workspace_root {
            match AnalyzerClient::spawn(&root) {
                Ok(client) => {
                    *self.analyzer.lock().unwrap() = Some(client);
                    self.client.log_message(MessageType::INFO, "Go analyzer initialized").await;
                }
                Err(e) => {
                    self.client.log_message(
                        MessageType::WARNING,
                        format!("Go analyzer not available: {}. Type checking disabled.", e)
                    ).await;
                }
            }
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Extract workspace root
        if let Some(root_uri) = params.root_uri {
            if let Ok(path) = root_uri.to_file_path() {
                let root = path.to_string_lossy().to_string();
                *self.workspace_root.lock().unwrap() = Some(root);
            }
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                document_formatting_provider: Some(OneOf::Left(true)),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
                        SemanticTokensRegistrationOptions {
                            text_document_registration_options: {
                                TextDocumentRegistrationOptions {
                                    document_selector: Some(vec![DocumentFilter {
                                        language: Some("hudl".to_string()),
                                        scheme: Some("file".to_string()),
                                        pattern: None,
                                    }]),
                                }
                            },
                            semantic_tokens_options: SemanticTokensOptions {
                                work_done_progress_options: WorkDoneProgressOptions::default(),
                                legend: SemanticTokensLegend {
                                    token_types: LEGEND_TYPE.to_vec(),
                                    token_modifiers: vec![],
                                },
                                range: Some(false),
                                full: Some(SemanticTokensFullOptions::Bool(true)),
                            },
                            static_registration_options: StaticRegistrationOptions::default(),
                        },
                    ),
                ),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client.log_message(MessageType::INFO, "Hudl LSP initialized").await;
        // Try to initialize the Go analyzer
        self.try_init_analyzer().await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.on_change(TextDocumentItem {
            uri: params.text_document.uri,
            text: params.text_document.text,
            version: params.text_document.version,
            language_id: params.text_document.language_id,
        }).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(event) = params.content_changes.into_iter().next() {
            self.on_change(TextDocumentItem {
                uri: params.text_document.uri,
                text: event.text,
                version: params.text_document.version,
                language_id: "hudl".to_string(),
            }).await;
        }
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let document_map = self.document_map.lock().unwrap();
        let content = match document_map.get(&uri) {
            Some(c) => c,
            None => return Ok(None),
        };

        // Simplified implementation: regex based highlighting for keywords
        let mut tokens = Vec::new();
        let mut last_line = 0;
        let mut last_char = 0;

        let keywords = ["if", "else", "each", "switch", "case", "default", "el", "import"];

        for (i, line) in content.lines().enumerate() {
            for kw in keywords {
                if let Some(pos) = line.find(kw) {
                    let delta_line = i as u32 - last_line;
                    let delta_start = if delta_line == 0 {
                        pos as u32 - last_char
                    } else {
                        pos as u32
                    };

                    tokens.push(SemanticToken {
                        delta_line,
                        delta_start,
                        length: kw.len() as u32,
                        token_type: 0, // KEYWORD
                        token_modifiers_bitset: 0,
                    });

                    last_line = i as u32;
                    last_char = pos as u32;
                }
            }
        }

        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: tokens,
        })))
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let document_map = self.document_map.lock().unwrap();
        let content = match document_map.get(&uri) {
            Some(c) => c,
            None => return Ok(None),
        };

        match hudlc::parser::parse(content) {
            Ok(doc) => {
                let formatted = format!("{}", doc);
                Ok(Some(vec![TextEdit {
                    range: Range {
                        start: Position { line: 0, character: 0 },
                        end: Position {
                            line: content.lines().count() as u32,
                            character: 1000,
                        },
                    },
                    new_text: formatted,
                }]))
            }
            Err(_) => Ok(None),
        }
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        document_map: Mutex::new(HashMap::new()),
        analyzer: Mutex::new(None),
        workspace_root: Mutex::new(None),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
