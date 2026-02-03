use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use std::sync::Mutex;
use std::collections::HashMap;

#[derive(Debug)]
struct Backend {
    client: Client,
    document_map: Mutex<HashMap<Url, String>>,
}

impl Backend {
    async fn on_change(&self, params: TextDocumentItem) {
        self.document_map.lock().unwrap().insert(params.uri.clone(), params.text.clone());
        self.validate_document(&params.uri, &params.text).await;
    }

    async fn validate_document(&self, uri: &Url, content: &str) {
        let mut diagnostics = Vec::new();

        // 1. Parse check
        if let Err(e) = hudlc::parser::parse(content) {
            // Convert KdlError to Diagnostic
            // Simple mapping for now
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position { line: 0, character: 0 },
                    end: Position { line: 0, character: 0 },
                },
                severity: Some(DiagnosticSeverity::ERROR),
                message: format!("{}", e),
                ..Default::default()
            });
        }

        self.client.publish_diagnostics(uri.clone(), diagnostics, None).await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                document_formatting_provider: Some(OneOf::Left(true)),
                ..Default::capabilities()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Hudl LSP initialized")
            .await;
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
                            character: 1000, // naive end
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
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}