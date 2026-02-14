use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use std::sync::Mutex;
use std::collections::HashMap;
use regex::Regex;
use cel_interpreter::{Value, Program};
use hudlc::cel::EvalContext;

mod analyzer_client;
mod component_registry;
mod datastar;
mod exhaustiveness;
mod param;
mod scope;

use hudl_lsp::dev_server;

use analyzer_client::AnalyzerClient;
use component_registry::ComponentRegistry;
use param::ViewMetadata;
use scope::Scope;

struct Backend {
    client: Client,
    document_map: Mutex<HashMap<Url, String>>,
    analyzer: Mutex<Option<AnalyzerClient>>,
    workspace_root: Mutex<Option<String>>,
    registry: Mutex<ComponentRegistry>,
}

const LEGEND_TYPE: &[SemanticTokenType] = &[
    SemanticTokenType::KEYWORD,
    SemanticTokenType::VARIABLE,
    SemanticTokenType::FUNCTION,
    SemanticTokenType::STRING,
    SemanticTokenType::DECORATOR,
];

impl Backend {
    async fn on_change(&self, params: TextDocumentItem) {
        self.document_map.lock().unwrap().insert(params.uri.clone(), params.text.clone());
        
        // Update registry if it's a file URI
        if let Ok(path) = params.uri.to_file_path() {
            let mut registry = self.registry.lock().unwrap();
            registry.process_file(&path);
        }

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
                message: e,
                ..Default::default()
            });
            self.client.publish_diagnostics(uri.clone(), diagnostics, None).await;
            return;
        }

        // 2. Type validation
        let metadata = param::extract_metadata(content);

        // Get base path for imports
        let base_path = uri.to_file_path().ok();
        let base_dir = base_path.as_ref().and_then(|p| p.parent());

        // Parse proto schema from template
        let (schema, proto_diagnostics) = match hudlc::proto::ProtoSchema::from_template(content, base_dir) {
            Ok(s) => (s, Vec::new()),
            Err(errors) => {
                let diags = errors.into_iter().map(|e| Diagnostic {
                    range: Range {
                        start: Position { line: e.line, character: 0 },
                        end: Position { line: e.line, character: 100 },
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    message: format!("Proto error: {}", e.message),
                    ..Default::default()
                }).collect();
                (hudlc::proto::ProtoSchema::default(), diags)
            }
        };
        diagnostics.extend(proto_diagnostics);

        // Build scopes with proper variable tracking
        let line_scopes = scope::build_scopes_from_content(
            content,
            &schema,
            metadata.data_type.as_deref(),
        );

        // Pre-load packages for all params
        self.preload_packages(&metadata).await;

        // Find all expressions in backticks and validate them
        let expr_diagnostics = self.validate_expressions(content, &line_scopes, &schema).await;
        diagnostics.extend(expr_diagnostics);

        // Check switch exhaustiveness (use root scope for now)
        let root_scope = scope::build_root_scope(&schema, metadata.data_type.as_deref());
        let switch_diagnostics = self.check_switch_exhaustiveness(content, &root_scope, &schema).await;
        diagnostics.extend(switch_diagnostics);

        // Validate component data type exists if specified
        if let Some(dt) = &metadata.data_type {
            if schema.get_message(dt).is_none() && schema.get_enum(dt).is_none() {
                // If it's not a primitive type either
                let primitives = ["string", "int32", "int64", "bool", "float", "double"];
                if !primitives.contains(&dt.as_str()) {
                    diagnostics.push(Diagnostic {
                        range: Range {
                            start: Position { line: 0, character: 0 }, // TODO: Better position
                            end: Position { line: 0, character: 10 },
                        },
                        severity: Some(DiagnosticSeverity::ERROR),
                        message: format!("Unknown data type '{}' specified in // data: comment.", dt),
                        ..Default::default()
                    });
                }
            }
        }

        // Check component invocations
        let component_diagnostics = self.validate_component_invocations(content, &line_scopes, &schema).await;
        diagnostics.extend(component_diagnostics);

        // Validate Datastar tilde attributes
        let datastar_diags = datastar::validate_datastar_attrs(content);
        diagnostics.extend(datastar_diags);

        self.client.publish_diagnostics(uri.clone(), diagnostics, None).await;
    }

    async fn validate_component_invocations(
        &self,
        content: &str,
        line_scopes: &HashMap<u32, Scope>,
        schema: &hudlc::proto::ProtoSchema,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Parse the document to get the AST
        let doc = match hudlc::parser::parse(content) {
            Ok(doc) => doc,
            Err(_) => return diagnostics,
        };

        let registry = self.registry.lock().unwrap();

        // Recursive helper to traverse AST
        fn check_nodes(
            nodes: &[kdl::KdlNode],
            line_scopes: &HashMap<u32, Scope>,
            schema: &hudlc::proto::ProtoSchema,
            registry: &ComponentRegistry,
            content: &str,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            for node in nodes {
                let name = node.name().value();
                
                // conventions: components start with uppercase
                if name.chars().next().map_or(false, |c| c.is_uppercase()) {
                    if let Some(info) = registry.get(name) {
                        // This is a known component!
                        
                        // Check if it expects data
                        if let Some(expected_type) = &info.data_type {
                            // Check if an argument was passed
                            let entries: Vec<_> = node.entries().iter().collect();
                            if entries.is_empty() {
                                // No data passed to component that expects it
                                let line = node.span().offset();
                                let line_num = content[..line].lines().count() as u32;
                                diagnostics.push(Diagnostic {
                                    range: Range {
                                        start: Position { line: line_num, character: 0 },
                                        end: Position { line: line_num, character: name.len() as u32 },
                                    },
                                    severity: Some(DiagnosticSeverity::ERROR),
                                    message: format!("Component '{}' expects data of type '{}' but none was provided.", name, expected_type),
                                    ..Default::default()
                                });
                            } else {
                                // Data was passed - check its type
                                let arg_val = entries[0].value().as_string()
                                    .map(|s| s.trim_matches('`').to_string());
                                
                                if let Some(expr_str) = arg_val {
                                    let line = node.span().offset();
                                    let line_num = content[..line].lines().count() as u32;
                                    let scope = scope::get_scope_for_line(line_scopes, line_num);
                                    
                                    // Resolve the type of the expression
                                    if let Some(actual_type) = infer_expr_type(&expr_str, &scope, schema) {
                                        // Compare CEL types for simple compatibility check
                                        let actual_cel = actual_type.cel_type();
                                        if actual_cel != *expected_type {
                                            diagnostics.push(Diagnostic {
                                                range: Range {
                                                    start: Position { line: line_num, character: 0 },
                                                    end: Position { line: line_num, character: (name.len() + expr_str.len() + 3) as u32 },
                                                },
                                                severity: Some(DiagnosticSeverity::ERROR),
                                                message: format!("Type mismatch: Component '{}' expects '{}', but got '{}'.", name, expected_type, actual_cel),
                                                ..Default::default()
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Recurse into children
                if let Some(children) = node.children() {
                    check_nodes(children.nodes(), line_scopes, schema, registry, content, diagnostics);
                }
            }
        }

        check_nodes(doc.nodes(), line_scopes, schema, &registry, content, &mut diagnostics);

        diagnostics
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
        line_scopes: &HashMap<u32, Scope>,
        schema: &hudlc::proto::ProtoSchema,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let backtick_re = Regex::new(r"`([^`]+)`").unwrap();

        for (line_num, line) in content.lines().enumerate() {
            // Get the scope for this line
            let line_scope = scope::get_scope_for_line(line_scopes, line_num as u32);

            for cap in backtick_re.captures_iter(line) {
                let expr_str = &cap[1];
                let match_start = cap.get(1).unwrap().start();

                // Compile expression using cel-interpreter
                match Program::compile(expr_str) {
                    Ok(program) => {
                        // Validate against scope
                        if let Some(diag) = self.validate_expr(&program, &line_scope, schema, line_num as u32, match_start as u32) {
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
                            message: format!("CEL parse error: {:?}", e),
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
        program: &Program,
        scope: &Scope,
        schema: &hudlc::proto::ProtoSchema,
        line: u32,
        col: u32,
    ) -> Option<Diagnostic> {
        // Check for unknown variables
        for var in program.references().variables() {
            let path = var.to_string();
            let parts: Vec<&str> = path.split('.').collect();
            let root = parts[0];

            if let Some(var_info) = scope.lookup(root) {
                // If it's a dotted path, validate field existence
                if parts.len() > 1 {
                    let field_path = parts[1..].join(".");
                    if let hudlc::proto::ProtoType::Message(msg_name) = &var_info.proto_type {
                        if let Err(e) = schema.resolve_field_path(msg_name, &field_path) {
                            return Some(Diagnostic {
                                range: Range {
                                    start: Position { line, character: col },
                                    end: Position { line, character: col + path.len() as u32 },
                                },
                                severity: Some(DiagnosticSeverity::ERROR),
                                message: e,
                                ..Default::default()
                            });
                        }
                    }
                }
            } else {
                // Variable not in scope
                return Some(Diagnostic {
                    range: Range {
                        start: Position { line, character: col },
                        end: Position { line, character: col + root.len() as u32 },
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    message: format!("Unknown variable '{}'. Available: {}", root, scope.all_vars().join(", ")),
                    ..Default::default()
                });
            }
        }
        None
    }

    async fn check_switch_exhaustiveness(
        &self,
        content: &str,
        scope: &Scope,
        schema: &hudlc::proto::ProtoSchema,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Extract switch statements from content
        let switches = exhaustiveness::extract_switches(content);

        let metadata = param::extract_metadata(content);

        for switch_info in &switches {
            // Use proto-based exhaustiveness check (uses inline enum definitions)
            if let Some(diag) = exhaustiveness::check_switch_with_proto(
                switch_info,
                schema,
                metadata.data_type.as_deref(),
            ) {
                diagnostics.push(diag);
                continue;
            }

            // Fall back to Go analyzer-based check (convert scope to old format)
            let old_scope: HashMap<String, String> = scope.all_vars()
                .into_iter()
                .map(|v| (v.clone(), "unknown".to_string()))
                .collect();
            let mut analyzer = self.analyzer.lock().unwrap();
            if let Some(ref mut client) = *analyzer {
                if let Some(diag) = exhaustiveness::check_switch(switch_info, &old_scope, client) {
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

fn infer_expr_type(
    expr_str: &str,
    scope: &Scope,
    schema: &hudlc::proto::ProtoSchema,
) -> Option<hudlc::proto::ProtoType> {
    // Simple path-based inference for now
    let parts: Vec<&str> = expr_str.split('.').collect();
    let root = parts[0];

    if let Some(var_info) = scope.lookup(root) {
        if parts.len() == 1 {
            return Some(var_info.proto_type.clone());
        } else {
            let field_path = parts[1..].join(".");
            if let hudlc::proto::ProtoType::Message(msg_name) = &var_info.proto_type {
                if let Ok(field_type) = schema.resolve_field_path(msg_name, &field_path) {
                    return Some(field_type.clone());
                }
            }
        }
    }
    None
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Extract workspace root
        if let Some(root_uri) = params.root_uri {
            if let Ok(path) = root_uri.to_file_path() {
                let root = path.to_string_lossy().to_string();
                *self.workspace_root.lock().unwrap() = Some(root.clone());
                
                // Scan workspace for components
                let mut registry = self.registry.lock().unwrap();
                registry.scan_workspace(&root);
            }
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                document_formatting_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        "~".into(), "@".into(), "$".into(), ":".into(),
                    ]),
                    ..Default::default()
                }),
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

        // Token types: 0=KEYWORD, 1=VARIABLE, 2=FUNCTION, 3=STRING, 4=DECORATOR
        let mut raw_tokens: Vec<(u32, u32, u32, u32)> = Vec::new(); // (line, char, len, type)

        // Pre-compute tilde block ranges for DECORATOR highlighting
        let tilde_ranges = datastar::find_tilde_block_ranges(content);

        let keywords = ["if", "else", "each", "switch", "case", "default", "el", "import"];

        for (line_num, line) in content.lines().enumerate() {
            let line_num = line_num as u32;
            let chars: Vec<char> = line.chars().collect();
            let mut i = 0;

            while i < chars.len() {
                // Skip whitespace
                if chars[i].is_whitespace() {
                    i += 1;
                    continue;
                }

                // Check for tilde markers and inline tilde attributes
                if chars[i] == '~' {
                    let start = i;
                    if i + 1 < chars.len() && chars[i + 1] == '>' {
                        // ~> binding shorthand: DECORATOR for ~>, then VARIABLE for signal name
                        raw_tokens.push((line_num, start as u32, 2, 4)); // DECORATOR for ~>
                        i += 2;
                        if i < chars.len() && (chars[i].is_ascii_alphabetic() || chars[i] == '_') {
                            let name_start = i;
                            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                                i += 1;
                            }
                            raw_tokens.push((line_num, name_start as u32, (i - name_start) as u32, 1)); // VARIABLE
                        }
                    } else if i + 1 < chars.len() && chars[i + 1] == ' ' && i + 2 < chars.len() && chars[i + 2] == '{' {
                        // ~ { tilde block opener: DECORATOR
                        raw_tokens.push((line_num, start as u32, 1, 4)); // DECORATOR
                        i += 1;
                    } else if i + 1 < chars.len() && chars[i + 1] == '{' {
                        // ~{ tilde block opener: DECORATOR
                        raw_tokens.push((line_num, start as u32, 1, 4)); // DECORATOR
                        i += 1;
                    } else if i + 1 < chars.len() && (chars[i + 1].is_ascii_alphabetic() || chars[i + 1] == '.') {
                        // ~attrName inline tilde: DECORATOR for the whole thing
                        i += 1;
                        while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == ':' || chars[i] == '.' || chars[i] == '~') {
                            i += 1;
                        }
                        raw_tokens.push((line_num, start as u32, (i - start) as u32, 4)); // DECORATOR
                    } else {
                        // Standalone ~
                        raw_tokens.push((line_num, start as u32, 1, 4)); // DECORATOR
                        i += 1;
                    }
                    continue;
                }

                // Check for comments
                if i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '/' {
                    break; // Rest of line is a comment
                }

                // Check for backtick expressions (variables)
                if chars[i] == '`' {
                    let start = i;
                    i += 1;
                    while i < chars.len() && chars[i] != '`' {
                        i += 1;
                    }
                    if i < chars.len() {
                        i += 1; // Include closing backtick
                        raw_tokens.push((line_num, start as u32, (i - start) as u32, 1)); // VARIABLE
                    }
                    continue;
                }

                // Check for strings
                if chars[i] == '"' {
                    let start = i;
                    i += 1;
                    while i < chars.len() && chars[i] != '"' {
                        if chars[i] == '\\' && i + 1 < chars.len() {
                            i += 2; // Skip escaped character
                        } else {
                            i += 1;
                        }
                    }
                    if i < chars.len() {
                        i += 1; // Include closing quote
                        raw_tokens.push((line_num, start as u32, (i - start) as u32, 3)); // STRING
                    }
                    continue;
                }

                // Check for keywords/identifiers
                if chars[i].is_ascii_alphabetic() || chars[i] == '_' || chars[i] == '.' {
                    let start = i;
                    // Inside tilde blocks, attribute names can contain : and .
                    let in_tilde = tilde_ranges.iter().any(|&(s, e)| line_num > s && line_num < e);
                    if in_tilde {
                        while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == ':' || chars[i] == '.') {
                            i += 1;
                        }
                        let word: String = chars[start..i].iter().collect();
                        // Treat attribute names inside tilde blocks as DECORATOR
                        raw_tokens.push((line_num, start as u32, word.len() as u32, 4)); // DECORATOR
                    } else {
                        while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                            i += 1;
                        }
                        let word: String = chars[start..i].iter().collect();
                        if keywords.contains(&word.as_str()) {
                            raw_tokens.push((line_num, start as u32, word.len() as u32, 0)); // KEYWORD
                        }
                    }
                    continue;
                }

                i += 1;
            }
        }

        // Sort tokens by position
        raw_tokens.sort_by(|a, b| {
            if a.0 != b.0 {
                a.0.cmp(&b.0)
            } else {
                a.1.cmp(&b.1)
            }
        });

        // Convert to delta format
        let mut tokens = Vec::new();
        let mut last_line = 0u32;
        let mut last_char = 0u32;

        for (line, char_pos, len, token_type) in raw_tokens {
            let delta_line = line - last_line;
            let delta_start = if delta_line == 0 {
                char_pos - last_char
            } else {
                char_pos
            };

            tokens.push(SemanticToken {
                delta_line,
                delta_start,
                length: len,
                token_type,
                token_modifiers_bitset: 0,
            });

            last_line = line;
            last_char = char_pos;
        }

        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: tokens,
        })))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let document_map = self.document_map.lock().unwrap();
        let content = match document_map.get(&uri) {
            Some(c) => c,
            None => return Ok(None),
        };

        let items = datastar::get_completions(content, position);
        if items.is_empty() {
            return Ok(None);
        }
        Ok(Some(CompletionResponse::Array(items)))
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
                // Use format options from editor
                let format_options = hudlc::formatter::FormatOptions::new(
                    params.options.tab_size,
                    params.options.insert_spaces,
                );
                let formatted = hudlc::formatter::format(&doc, &format_options);
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
    let args: Vec<String> = std::env::args().collect();

    // Check for dev-server mode
    if args.iter().any(|a| a == "--dev-server") {
        let port: u16 = args
            .windows(2)
            .find(|w| w[0] == "--port")
            .and_then(|w| w[1].parse().ok())
            .unwrap_or(9999);

        let watch_dir = args
            .windows(2)
            .find(|w| w[0] == "--watch")
            .map(|w| std::path::PathBuf::from(&w[1]))
            .unwrap_or_else(|| std::path::PathBuf::from("."));

        let verbose = args.iter().any(|a| a == "--verbose" || a == "-v");

        if let Err(e) = dev_server::start(port, watch_dir, verbose).await {
            eprintln!("Dev server error: {}", e);
            std::process::exit(1);
        }
        return;
    }

    // Default: run LSP mode
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        document_map: Mutex::new(HashMap::new()),
        analyzer: Mutex::new(None),
        workspace_root: Mutex::new(None),
        registry: Mutex::new(ComponentRegistry::new()),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
