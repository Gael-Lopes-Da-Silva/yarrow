//! Yarrow language server.
//!
//! Speaks LSP over stdio and delegates analysis to `yarrow_core`. Stage 17 adds
//! `textDocument/rangeFormatting` via `yarrow_fmt::format_range`.

mod analysis;
mod code_action;
mod completion;
mod config;
mod definition;
mod document;
mod format;
mod hover;
mod inlay_hints;
mod modules;
mod position;
mod references;
mod rename;
mod semantic_tokens;
mod signature_help;
mod symbols;

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tower_lsp_server::jsonrpc::{Error as LspErrorRpc, Result as LspResult};
use tower_lsp_server::ls_types::{
    CodeActionParams, CodeActionProviderCapability, CodeActionResponse, CompletionOptions,
    CompletionParams, CompletionResponse, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DocumentFormattingParams, DocumentRangeFormattingParams,
    DocumentSymbolParams, DocumentSymbolResponse, ExecuteCommandOptions, ExecuteCommandParams,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams, HoverProviderCapability,
    InitializeParams, InitializeResult, InitializedParams, InlayHint, InlayHintParams, LSPAny,
    Location, MessageType, OneOf, PrepareRenameResponse, ReferenceParams, RenameOptions,
    RenameParams, SemanticTokensFullOptions, SemanticTokensOptions, SemanticTokensParams,
    SemanticTokensResult, SemanticTokensServerCapabilities, ServerCapabilities, ServerInfo,
    SignatureHelp, SignatureHelpOptions, SignatureHelpParams, TextDocumentPositionParams,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions, TextEdit, Uri,
    WorkspaceEdit, WorkspaceSymbolParams, WorkspaceSymbolResponse,
};
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

pub use analysis::{check_document, uri_to_source_path};
pub use code_action::EXPLAIN_COMMAND;
pub use completion::completions;
pub use config::{InitializationOptions, LspConfig};
pub use definition::goto_definition;
pub use document::{Document, DocumentStore, LANGUAGE_ID};
pub use format::{format_document, format_document_range};
pub use hover::hover as hover_at;
pub use inlay_hints::inlay_hints as inlay_hints_at;
pub use position::{PositionEncoding, PositionMap};
pub use references::find_references;
pub use rename::{prepare_rename, rename};
pub use semantic_tokens::{legend as semantic_tokens_legend, semantic_tokens_full};
pub use signature_help::signature_help as signature_help_at;
pub use symbols::{document_symbols, workspace_symbols};

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

/// stdio server with process-level defaults (CLI `-L` / `--main` / `--no-format` / `--no-inlay`).
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

        let (format_enable, inlay_hints_enable) = {
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
            (cfg.format_enable, cfg.inlay_hints_enable)
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
                workspace_symbol_provider: Some(OneOf::Left(true)),
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
                document_range_formatting_provider: if format_enable {
                    Some(OneOf::Left(true))
                } else {
                    None
                },
                // On-type formatting deferred: mid-edit buffers often fail to parse,
                // and format_range expands to whole top-level items (too aggressive
                // for a keystroke). Prefer explicit range / full-document format.
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec![code_action::EXPLAIN_COMMAND.into()],
                    ..Default::default()
                }),
                // Postfix `name call`: space (and `call` as retrigger) is the useful trigger.
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec![" ".into()]),
                    retrigger_characters: Some(vec![" ".into()]),
                    ..Default::default()
                }),
                inlay_hint_provider: if inlay_hints_enable {
                    Some(OneOf::Left(true))
                } else {
                    None
                },
                semantic_tokens_provider: Some(SemanticTokensServerCapabilities::from(
                    SemanticTokensOptions {
                        legend: semantic_tokens::legend(),
                        full: Some(SemanticTokensFullOptions::Bool(true)),
                        range: Some(false),
                        ..Default::default()
                    },
                )),
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: Default::default(),
                })),
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

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> LspResult<Option<WorkspaceSymbolResponse>> {
        let open = {
            let Ok(store) = self.state.documents.lock() else {
                return Ok(Some(WorkspaceSymbolResponse::Flat(Vec::new())));
            };
            store.snapshot_texts()
        };
        let encoding = self
            .state
            .encoding
            .lock()
            .map(|g| *g)
            .unwrap_or(PositionEncoding::Utf16);
        let config = self.config_snapshot();
        let symbols = workspace_symbols(&open, &params.query, encoding, &config);
        Ok(Some(WorkspaceSymbolResponse::Flat(symbols)))
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

    async fn signature_help(
        &self,
        params: SignatureHelpParams,
    ) -> LspResult<Option<SignatureHelp>> {
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
        Ok(signature_help::signature_help(
            &path, &text, encoding, position, &config,
        ))
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> LspResult<Option<Vec<InlayHint>>> {
        let uri = params.text_document.uri.clone();
        let text = {
            let Ok(store) = self.state.documents.lock() else {
                return Ok(Some(Vec::new()));
            };
            let Some(doc) = store.get(&uri) else {
                return Ok(Some(Vec::new()));
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
        if !config.inlay_hints_enable {
            return Ok(Some(Vec::new()));
        }
        Ok(Some(inlay_hints::inlay_hints(
            &path,
            &text,
            encoding,
            params.range,
            &config,
        )))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> LspResult<Option<SemanticTokensResult>> {
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
        let tokens = semantic_tokens::semantic_tokens_full(&path, &text, encoding, &config);
        Ok(Some(SemanticTokensResult::Tokens(tokens)))
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

    async fn range_formatting(
        &self,
        params: DocumentRangeFormattingParams,
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
        let _ = params.options;
        Ok(format::format_document_range(&text, params.range, encoding))
    }

    async fn code_action(&self, params: CodeActionParams) -> LspResult<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        let range = params.range;
        let only = params.context.only;
        let mut diagnostics = params.context.diagnostics;
        if diagnostics.is_empty() {
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
            let config = self.config_snapshot();
            diagnostics = check_document(&uri, &text, encoding, &config);
        }
        Ok(code_action::code_actions(
            &diagnostics,
            range,
            only.as_deref(),
        ))
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> LspResult<Option<LSPAny>> {
        if params.command != code_action::EXPLAIN_COMMAND {
            return Ok(None);
        }
        let Some(text) = code_action::explain_command_text(&params.arguments) else {
            return Ok(None);
        };
        self.client
            .show_message(MessageType::INFO, text.clone())
            .await;
        Ok(Some(LSPAny::String(text)))
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> LspResult<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri;
        let position = params.position;
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
        Ok(rename::prepare_rename(
            &path, &text, encoding, position, &config,
        ))
    }

    async fn rename(&self, params: RenameParams) -> LspResult<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let new_name = params.new_name;
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
        match rename::rename(&uri, &path, &text, encoding, position, &new_name, &config) {
            Ok(edit) => Ok(Some(edit)),
            Err(msg) => Err(LspErrorRpc::invalid_params(msg)),
        }
    }
}
