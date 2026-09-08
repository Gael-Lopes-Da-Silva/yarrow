//! Yarrow language server.
//!
//! Speaks LSP over stdio and delegates analysis to `yarrow_core`. Stage 10 adds
//! process flags, init options, and the `yarrow lsp` CLI wrapper.

mod analysis;
mod completion;
mod config;
mod definition;
mod document;
mod format;
mod hover;
mod modules;
mod position;
mod references;
mod symbols;

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tower_lsp_server::jsonrpc::Result as LspResult;
use tower_lsp_server::ls_types::{
    CompletionOptions, CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DocumentFormattingParams,
    DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams, GotoDefinitionResponse,
    Hover, HoverParams, HoverProviderCapability, InitializeParams, InitializeResult,
    InitializedParams, Location, MessageType, OneOf, ReferenceParams, ServerCapabilities,
    ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextEdit, Uri,
};
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

pub use analysis::{check_document, uri_to_source_path};
pub use completion::completions;
pub use config::{InitializationOptions, LspConfig};
pub use definition::goto_definition;
pub use document::{Document, DocumentStore, LANGUAGE_ID};
pub use format::format_document;
pub use hover::hover as hover_at;
pub use position::{PositionEncoding, PositionMap};
pub use references::find_references;
pub use symbols::document_symbols;

/// Debounce window for rapid `didChange` before re-checking.
const CHANGE_DEBOUNCE: Duration = Duration::from_millis(200);

/// Errors from starting or running the language server.
#[derive(Debug, thiserror::Error)]
pub enum LspError {
    /// Placeholder for transport or setup failures in later stages.
    #[error("{0}")]
    Message(String),
}

impl From<&str> for LspError {
    fn from(value: &str) -> Self {
        Self::Message(value.to_string())
    }
}

impl From<String> for LspError {
    fn from(value: String) -> Self {
        Self::Message(value)
    }
}

/// Library entry used by the binary and by `yarrow lsp` (default config).
pub async fn run_stdio() -> Result<(), LspError> {
    run_stdio_with(LspConfig::default()).await
}

/// stdio server with process-level defaults (CLI `-L` / `--main` / `--no-format`).
pub async fn run_stdio_with(config: LspConfig) -> Result<(), LspError> {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    run_with_streams(stdin, stdout, config).await
}

/// Blocking stdio entry for sync CLI wrappers.
pub fn run_stdio_blocking(config: LspConfig) -> Result<(), LspError> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| LspError::Message(format!("tokio runtime: {e}")))?;
    rt.block_on(run_stdio_with(config))
}

/// Run the server over explicit async read/write streams (tests / embedding).
pub async fn run_with_streams<I, O>(stdin: I, stdout: O, config: LspConfig) -> Result<(), LspError>
where
    I: tokio::io::AsyncRead + Unpin,
    O: tokio::io::AsyncWrite,
{
    let (service, socket) = LspService::new(move |client| Backend::new(client, config));
    Server::new(stdin, stdout, socket).serve(service).await;
    Ok(())
}

struct ServerState {
    documents: Mutex<DocumentStore>,
    encoding: Mutex<PositionEncoding>,
    config: Mutex<LspConfig>,
    /// Per-URI generation counter; bumping cancels an in-flight debounce.
    analysis_gens: Mutex<HashMap<String, u64>>,
}

struct Backend {
    client: Client,
    state: Arc<ServerState>,
}

impl Backend {
    fn new(client: Client, config: LspConfig) -> Self {
        Self {
            client,
            state: Arc::new(ServerState {
                documents: Mutex::new(DocumentStore::default()),
                encoding: Mutex::new(PositionEncoding::Utf16),
                config: Mutex::new(config),
                analysis_gens: Mutex::new(HashMap::new()),
            }),
        }
    }

    fn config_snapshot(&self) -> LspConfig {
        self.state
            .config
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    fn bump_analysis_gen(&self, uri: &Uri) -> u64 {
        let Ok(mut gens) = self.state.analysis_gens.lock() else {
            return 0;
        };
        let entry = gens.entry(uri.as_str().to_string()).or_insert(0);
        *entry = entry.saturating_add(1);
        *entry
    }

    fn schedule_analysis(&self, uri: Uri, version: i32, debounce: bool) {
        let ticket = self.bump_analysis_gen(&uri);
        let state = Arc::clone(&self.state);
        let client = self.client.clone();

        tokio::spawn(async move {
            if debounce {
                tokio::time::sleep(CHANGE_DEBOUNCE).await;
                let current = state
                    .analysis_gens
                    .lock()
                    .ok()
                    .and_then(|g| g.get(uri.as_str()).copied())
                    .unwrap_or(0);
                if current != ticket {
                    return;
                }
            }

            let text = {
                let Ok(store) = state.documents.lock() else {
                    return;
                };
                let Some(doc) = store.get(&uri) else {
                    return;
                };
                if doc.version != version {
                    return;
                }
                doc.text.clone()
            };

            let encoding = state
                .encoding
                .lock()
                .map(|g| *g)
                .unwrap_or(PositionEncoding::Utf16);
            let config = state.config.lock().map(|g| g.clone()).unwrap_or_default();
            let diagnostics = check_document(&uri, &text, encoding, &config);

            {
                let Ok(store) = state.documents.lock() else {
                    return;
                };
                match store.get(&uri) {
                    Some(doc) if doc.version == version => {}
                    _ => return,
                }
                let current = state
                    .analysis_gens
                    .lock()
                    .ok()
                    .and_then(|g| g.get(uri.as_str()).copied())
                    .unwrap_or(0);
                if current != ticket {
                    return;
                }
            }

            client
                .publish_diagnostics(uri, diagnostics, Some(version))
                .await;
        });
    }
}

impl fmt::Debug for Backend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Backend").finish_non_exhaustive()
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> LspResult<InitializeResult> {
        let encodings = params
            .capabilities
            .general
            .as_ref()
            .and_then(|g| g.position_encodings.as_deref());
        let encoding = PositionEncoding::negotiate(encodings);
        if let Ok(mut slot) = self.state.encoding.lock() {
            *slot = encoding;
        }

        let init_opts = InitializationOptions::from_value(params.initialization_options.as_ref());
        let mut folders = Vec::new();
        if let Some(ws) = params.workspace_folders {
            for folder in ws {
                if let Some(path) = folder.uri.to_file_path() {
                    folders.push(path.into_owned());
                }
            }
        }

        let format_enable = {
            let Ok(mut cfg) = self.state.config.lock() else {
                return Ok(InitializeResult {
                    capabilities: ServerCapabilities::default(),
                    server_info: Some(ServerInfo {
                        name: "yarrow-lsp".into(),
                        version: Some(env!("CARGO_PKG_VERSION").into()),
                    }),
                    ..Default::default()
                });
            };
            cfg.apply_initialize(init_opts.as_ref(), &folders);
            cfg.format_enable
        };

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                position_encoding: Some(encoding.as_lsp()),
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        ..Default::default()
                    },
                )),
                document_symbol_provider: Some(OneOf::Left(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec!["\"".into()]),
                    ..Default::default()
                }),
                document_formatting_provider: if format_enable {
                    Some(OneOf::Left(true))
                } else {
                    None
                },
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "yarrow-lsp".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "yarrow-lsp initialized")
            .await;
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let doc = params.text_document;
        if !document::should_track(&doc.uri, &doc.language_id) {
            self.client
                .log_message(
                    MessageType::INFO,
                    format!("didOpen ignored (not yarrow): {}", doc.uri.as_str()),
                )
                .await;
            return;
        }

        let uri_str = doc.uri.as_str().to_string();
        let version = doc.version;
        let len = doc.text.len();
        {
            let Ok(mut store) = self.state.documents.lock() else {
                return;
            };
            store.open(doc.uri.clone(), version, doc.text);
        }

        self.client
            .log_message(
                MessageType::INFO,
                format!("didOpen {uri_str} v{version} ({len} bytes)"),
            )
            .await;

        self.schedule_analysis(doc.uri, version, false);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        let uri_str = uri.as_str().to_string();

        // Full sync: take the last change that replaces the whole document.
        let Some(text) = params
            .content_changes
            .into_iter()
            .rev()
            .find(|change| change.range.is_none())
            .map(|change| change.text)
        else {
            self.client
                .log_message(
                    MessageType::WARNING,
                    format!("didChange {uri_str}: expected full-text change"),
                )
                .await;
            return;
        };

        let len = text.len();
        let updated = {
            let Ok(mut store) = self.state.documents.lock() else {
                return;
            };
            store.set_text(&uri, version, text)
        };

        if updated {
            self.client
                .log_message(
                    MessageType::INFO,
                    format!("didChange {uri_str} v{version} ({len} bytes)"),
                )
                .await;
            self.schedule_analysis(uri, version, true);
        } else {
            self.client
                .log_message(
                    MessageType::INFO,
                    format!("didChange ignored (not open): {uri_str}"),
                )
                .await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        let uri_str = uri.as_str().to_string();
        let _ = self.bump_analysis_gen(&uri);
        let (closed, remaining) = {
            let Ok(mut store) = self.state.documents.lock() else {
                return;
            };
            let closed = store.close(&uri);
            (closed, store.len())
        };

        if closed {
            self.client.publish_diagnostics(uri, Vec::new(), None).await;
            self.client
                .log_message(
                    MessageType::INFO,
                    format!("didClose {uri_str} (store len {remaining})"),
                )
                .await;
        } else {
            self.client
                .log_message(
                    MessageType::INFO,
                    format!("didClose ignored (not open): {uri_str}"),
                )
                .await;
        }
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> LspResult<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let text = {
            let Ok(store) = self.state.documents.lock() else {
                return Ok(None);
            };
            let Some(doc) = store.get(&uri) else {
                return Ok(None);
            };
            doc.text.clone()
        };
        let encoding = self
            .state
            .encoding
            .lock()
            .map(|g| *g)
            .unwrap_or(PositionEncoding::Utf16);
        let path = uri_to_source_path(&uri);
        let config = self.config_snapshot();
        Ok(document_symbols(&path, &text, encoding, &config).map(DocumentSymbolResponse::Nested))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let text = {
            let Ok(store) = self.state.documents.lock() else {
                return Ok(None);
            };
            let Some(doc) = store.get(&uri) else {
                return Ok(None);
            };
            doc.text.clone()
        };
        let encoding = self
            .state
            .encoding
            .lock()
            .map(|g| *g)
            .unwrap_or(PositionEncoding::Utf16);
        let path = uri_to_source_path(&uri);
        let config = self.config_snapshot();
        Ok(definition::goto_definition(
            &uri, &path, &text, encoding, position, &config,
        ))
    }

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let text = {
            let Ok(store) = self.state.documents.lock() else {
                return Ok(None);
            };
            let Some(doc) = store.get(&uri) else {
                return Ok(None);
            };
            doc.text.clone()
        };
        let encoding = self
            .state
            .encoding
            .lock()
            .map(|g| *g)
            .unwrap_or(PositionEncoding::Utf16);
        let path = uri_to_source_path(&uri);
        let config = self.config_snapshot();
        Ok(hover::hover(&path, &text, encoding, position, &config))
    }

    async fn completion(&self, params: CompletionParams) -> LspResult<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let text = {
            let Ok(store) = self.state.documents.lock() else {
                return Ok(None);
            };
            let Some(doc) = store.get(&uri) else {
                return Ok(None);
            };
            doc.text.clone()
        };
        let encoding = self
            .state
            .encoding
            .lock()
            .map(|g| *g)
            .unwrap_or(PositionEncoding::Utf16);
        let path = uri_to_source_path(&uri);
        let config = self.config_snapshot();
        Ok(completion::completions(
            &path, &text, encoding, position, &config,
        ))
    }

    async fn references(&self, params: ReferenceParams) -> LspResult<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let include_declaration = params.context.include_declaration;
        let text = {
            let Ok(store) = self.state.documents.lock() else {
                return Ok(None);
            };
            let Some(doc) = store.get(&uri) else {
                return Ok(None);
            };
            doc.text.clone()
        };
        let encoding = self
            .state
            .encoding
            .lock()
            .map(|g| *g)
            .unwrap_or(PositionEncoding::Utf16);
        let path = uri_to_source_path(&uri);
        let config = self.config_snapshot();
        Ok(references::find_references(
            &uri,
            &path,
            &text,
            encoding,
            position,
            include_declaration,
            &config,
        ))
    }

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> LspResult<Option<Vec<TextEdit>>> {
        if !self.config_snapshot().format_enable {
            return Ok(None);
        }
        let uri = params.text_document.uri;
        let text = {
            let Ok(store) = self.state.documents.lock() else {
                return Ok(None);
            };
            let Some(doc) = store.get(&uri) else {
                return Ok(None);
            };
            doc.text.clone()
        };
        let encoding = self
            .state
            .encoding
            .lock()
            .map(|g| *g)
            .unwrap_or(PositionEncoding::Utf16);
        // Client FormattingOptions are ignored; style comes from yarrow-fmt defaults.
        let _ = params.options;
        Ok(format::format_document(&text, encoding))
    }
}
