//! Yarrow language server.
//!
//! Speaks LSP over stdio or TCP and delegates analysis to `yarrow_core`. Stage 21
//! adds optional multi-root `check_project` via `initializationOptions.projectRoots`.

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
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::net::TcpListener;
use tower_lsp_server::jsonrpc::{Error as LspErrorRpc, Result as LspResult};
use tower_lsp_server::ls_types::{
    CodeActionParams, CodeActionProviderCapability, CodeActionResponse, CompletionOptions,
    CompletionParams, CompletionResponse, Diagnostic, DiagnosticOptions,
    DiagnosticServerCapabilities, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DocumentDiagnosticParams, DocumentDiagnosticReport,
    DocumentDiagnosticReportResult, DocumentFormattingParams, DocumentOnTypeFormattingOptions,
    DocumentOnTypeFormattingParams, DocumentRangeFormattingParams, DocumentSymbolParams,
    DocumentSymbolResponse, ExecuteCommandOptions, ExecuteCommandParams,
    FullDocumentDiagnosticReport, GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams,
    HoverProviderCapability, InitializeParams, InitializeResult, InitializedParams, InlayHint,
    InlayHintParams, LSPAny, Location, MessageType, OneOf, PrepareRenameResponse, ReferenceParams,
    RelatedFullDocumentDiagnosticReport, RelatedUnchangedDocumentDiagnosticReport, RenameOptions,
    RenameParams, SemanticTokensFullOptions, SemanticTokensOptions, SemanticTokensParams,
    SemanticTokensResult, SemanticTokensServerCapabilities, ServerCapabilities, ServerInfo,
    SignatureHelp, SignatureHelpOptions, SignatureHelpParams, TextDocumentPositionParams,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions, TextEdit,
    UnchangedDocumentDiagnosticReport, Uri, WorkspaceDiagnosticParams, WorkspaceDiagnosticReport,
    WorkspaceDiagnosticReportResult, WorkspaceDocumentDiagnosticReport, WorkspaceEdit,
    WorkspaceFullDocumentDiagnosticReport, WorkspaceSymbolParams, WorkspaceSymbolResponse,
    WorkspaceUnchangedDocumentDiagnosticReport,
};
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

pub use analysis::{
    RootDiagnostics, check_document, check_project_roots, path_key, path_to_uri, uri_to_source_path,
};
pub use code_action::EXPLAIN_COMMAND;
pub use completion::completions;
pub use config::{InitializationOptions, LspConfig};
pub use definition::goto_definition;
pub use document::{Document, DocumentStore, LANGUAGE_ID};
pub use format::{format_document, format_document_range, format_on_type};
pub use hover::hover as hover_at;
pub use inlay_hints::inlay_hints as inlay_hints_at;
pub use position::{PositionEncoding, PositionMap};
pub use references::find_references;
pub use rename::{prepare_rename, rename};
pub use semantic_tokens::{legend as semantic_tokens_legend, semantic_tokens_full};
pub use signature_help::signature_help as signature_help_at;
pub use symbols::{document_symbols, workspace_symbols};

/// Debounce window for rapid `didChange` before re-checking.
///
/// Fixed at 200ms (not client-configurable). Newer `didChange` bumps the
/// per-URI / project generation so in-flight sleeps and checks are ignored.
const CHANGE_DEBOUNCE: Duration = Duration::from_millis(200);

/// Errors from starting or running the language server.
#[derive(Debug, thiserror::Error)]
pub enum LspError {
    /// Transport bind / accept / runtime failures.
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

/// Run the server over explicit async read/write streams (tests / embedding / TCP).
pub async fn run_with_streams<I, O>(stdin: I, stdout: O, config: LspConfig) -> Result<(), LspError>
where
    I: tokio::io::AsyncRead + Unpin,
    O: tokio::io::AsyncWrite,
{
    let (service, socket) = LspService::new(move |client| Backend::new(client, config));
    Server::new(stdin, stdout, socket).serve(service).await;
    Ok(())
}

/// Bind `addr` (`host:port`; port `0` = ephemeral), invoke `on_listen` with the
/// real socket address, accept one client, then serve over that connection.
pub async fn run_tcp_with<F>(addr: &str, config: LspConfig, on_listen: F) -> Result<(), LspError>
where
    F: FnOnce(SocketAddr),
{
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| LspError::Message(format!("tcp bind {addr}: {e}")))?;
    let local = listener
        .local_addr()
        .map_err(|e| LspError::Message(format!("tcp local_addr: {e}")))?;
    on_listen(local);
    let (stream, _) = listener
        .accept()
        .await
        .map_err(|e| LspError::Message(format!("tcp accept: {e}")))?;
    let (read, write) = tokio::io::split(stream);
    run_with_streams(read, write, config).await
}

/// Blocking TCP entry for sync CLI wrappers (one client, then exit).
pub fn run_tcp_blocking<F>(addr: &str, config: LspConfig, on_listen: F) -> Result<(), LspError>
where
    F: FnOnce(SocketAddr),
{
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| LspError::Message(format!("tokio runtime: {e}")))?;
    rt.block_on(run_tcp_with(addr, config, on_listen))
}

/// Last published / pulled diagnostics for one URI (keyed by document version).
#[derive(Clone)]
struct CachedDiagnostics {
    version: i32,
    result_id: String,
    items: Vec<Diagnostic>,
}

/// Generation key for multi-root project rechecks (shared across open buffers).
const PROJECT_ANALYSIS_KEY: &str = "__project__";

struct ServerState {
    documents: Mutex<DocumentStore>,
    encoding: Mutex<PositionEncoding>,
    config: Mutex<LspConfig>,
    /// Per-URI generation counter; bumping cancels an in-flight debounce.
    analysis_gens: Mutex<HashMap<String, u64>>,
    /// Push and pull share this cache so both paths stay in sync (uri → version).
    diag_cache: Mutex<HashMap<String, CachedDiagnostics>>,
    /// Root URIs last published in project mode (cleared when a root drops).
    published_project_uris: Mutex<Vec<Uri>>,
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
                diag_cache: Mutex::new(HashMap::new()),
                published_project_uris: Mutex::new(Vec::new()),
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
        self.bump_analysis_key(uri.as_str())
    }

    fn bump_analysis_key(&self, key: &str) -> u64 {
        let Ok(mut gens) = self.state.analysis_gens.lock() else {
            return 0;
        };
        let entry = gens.entry(key.to_string()).or_insert(0);
        *entry = entry.saturating_add(1);
        *entry
    }

    fn analysis_gen(state: &ServerState, key: &str) -> u64 {
        state
            .analysis_gens
            .lock()
            .ok()
            .and_then(|g| g.get(key).copied())
            .unwrap_or(0)
    }

    fn collect_overlays(state: &ServerState) -> HashMap<String, String> {
        let Ok(store) = state.documents.lock() else {
            return HashMap::new();
        };
        let mut overlays = HashMap::new();
        for (uri, text) in store.snapshot_texts() {
            if let Some(path) = uri.to_file_path() {
                overlays.insert(path_key(path.as_ref()), text);
            }
        }
        overlays
    }

    fn open_version(state: &ServerState, uri: &Uri) -> Option<i32> {
        let Ok(store) = state.documents.lock() else {
            return None;
        };
        store.get(uri).map(|d| d.version)
    }

    fn is_project_root(config: &LspConfig, uri: &Uri) -> bool {
        let Some(path) = uri.to_file_path() else {
            return false;
        };
        let key = path_key(path.as_ref());
        config
            .project_roots
            .iter()
            .any(|root| path_key(root) == key)
    }

    /// Cache push/pull diagnostics. Refuses to overwrite a newer version.
    fn store_diagnostics(state: &ServerState, uri: &Uri, version: i32, items: Vec<Diagnostic>) {
        let result_id = format!("v{version}");
        if let Ok(mut cache) = state.diag_cache.lock() {
            if let Some(existing) = cache.get(uri.as_str())
                && existing.version > version
            {
                return;
            }
            cache.insert(
                uri.as_str().to_string(),
                CachedDiagnostics {
                    version,
                    result_id,
                    items,
                },
            );
        }
    }

    /// True when this analysis ticket is still the latest for `key` and (when
    /// `uri`/`version` are set) the open buffer still matches that version.
    fn analysis_still_current(
        state: &ServerState,
        key: &str,
        ticket: u64,
        uri: Option<&Uri>,
        version: Option<i32>,
    ) -> bool {
        if Self::analysis_gen(state, key) != ticket {
            return false;
        }
        match (uri, version) {
            (Some(uri), Some(version)) => Self::open_version(state, uri) == Some(version),
            _ => true,
        }
    }

    fn clear_diagnostics_cache(state: &ServerState, uri: &Uri) {
        if let Ok(mut cache) = state.diag_cache.lock() {
            cache.remove(uri.as_str());
        }
    }

    /// Diagnostics for the open buffer at `uri`, preferring the uri+version cache.
    fn diagnostics_for(
        state: &ServerState,
        uri: &Uri,
        previous_result_id: Option<&str>,
    ) -> DocumentDiagnosticReport {
        let (version, text) = {
            let Ok(store) = state.documents.lock() else {
                return empty_full_report(None);
            };
            let Some(doc) = store.get(uri) else {
                return empty_full_report(None);
            };
            (doc.version, doc.text.clone())
        };

        if let Ok(cache) = state.diag_cache.lock()
            && let Some(cached) = cache.get(uri.as_str())
            && cached.version == version
        {
            if previous_result_id.is_some_and(|id| id == cached.result_id) {
                return DocumentDiagnosticReport::Unchanged(
                    RelatedUnchangedDocumentDiagnosticReport {
                        related_documents: None,
                        unchanged_document_diagnostic_report: UnchangedDocumentDiagnosticReport {
                            result_id: cached.result_id.clone(),
                        },
                    },
                );
            }
            return DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
                related_documents: None,
                full_document_diagnostic_report: FullDocumentDiagnosticReport {
                    result_id: Some(cached.result_id.clone()),
                    items: cached.items.clone(),
                },
            });
        }

        let encoding = state
            .encoding
            .lock()
            .map(|g| *g)
            .unwrap_or(PositionEncoding::Utf16);
        let config = state.config.lock().map(|g| g.clone()).unwrap_or_default();

        let items = if config.project_mode() && Self::is_project_root(&config, uri) {
            let overlays = Self::collect_overlays(state);
            let roots = check_project_roots(encoding, &config, &overlays);
            let want = uri
                .to_file_path()
                .map(|p| path_key(p.as_ref()))
                .unwrap_or_default();
            roots
                .into_iter()
                .find(|r| path_key(&r.path) == want)
                .map(|r| r.items)
                .unwrap_or_default()
        } else {
            check_document(uri, &text, encoding, &config)
        };

        Self::store_diagnostics(state, uri, version, items.clone());
        DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
            related_documents: None,
            full_document_diagnostic_report: FullDocumentDiagnosticReport {
                result_id: Some(format!("v{version}")),
                items,
            },
        })
    }

    /// Workspace pull: open buffers plus configured project roots (no disk walk).
    fn workspace_diagnostics_for(
        state: &ServerState,
        previous: &HashMap<String, String>,
    ) -> Vec<WorkspaceDocumentDiagnosticReport> {
        let encoding = state
            .encoding
            .lock()
            .map(|g| *g)
            .unwrap_or(PositionEncoding::Utf16);
        let config = state.config.lock().map(|g| g.clone()).unwrap_or_default();

        let open_uris: Vec<Uri> = {
            let Ok(store) = state.documents.lock() else {
                return Vec::new();
            };
            store
                .snapshot_texts()
                .into_iter()
                .map(|(uri, _)| uri)
                .collect()
        };

        let mut items = Vec::new();
        let mut reported_keys = std::collections::HashSet::new();

        if config.project_mode() {
            let overlays = Self::collect_overlays(state);
            let roots = check_project_roots(encoding, &config, &overlays);
            for root in roots {
                if let Some(path) = root.uri.to_file_path() {
                    reported_keys.insert(path_key(path.as_ref()));
                }
                let version = Self::open_version(state, &root.uri);
                if let Some(v) = version {
                    Self::store_diagnostics(state, &root.uri, v, root.items.clone());
                }
                let result_id = version.map(|v| format!("v{v}"));
                let prev = previous.get(root.uri.as_str()).map(String::as_str);
                if let (Some(rid), Some(prev_id)) = (result_id.as_deref(), prev)
                    && rid == prev_id
                {
                    items.push(WorkspaceDocumentDiagnosticReport::Unchanged(
                        WorkspaceUnchangedDocumentDiagnosticReport {
                            uri: root.uri,
                            version: version.map(i64::from),
                            unchanged_document_diagnostic_report:
                                UnchangedDocumentDiagnosticReport {
                                    result_id: rid.to_string(),
                                },
                        },
                    ));
                    continue;
                }
                items.push(WorkspaceDocumentDiagnosticReport::Full(
                    WorkspaceFullDocumentDiagnosticReport {
                        uri: root.uri,
                        version: version.map(i64::from),
                        full_document_diagnostic_report: FullDocumentDiagnosticReport {
                            result_id,
                            items: root.items,
                        },
                    },
                ));
            }
        }

        for uri in open_uris {
            if let Some(path) = uri.to_file_path()
                && reported_keys.contains(&path_key(path.as_ref()))
            {
                continue;
            }
            let prev = previous.get(uri.as_str()).map(String::as_str);
            let report = Self::diagnostics_for(state, &uri, prev);
            let version = Self::open_version(state, &uri).map(i64::from);
            items.push(doc_report_to_workspace(uri, version, report));
        }

        items
    }

    fn schedule_analysis(&self, uri: Uri, version: i32, debounce: bool) {
        let config = self.config_snapshot();
        if config.project_mode() {
            self.schedule_project_analysis(Some((uri, version)), debounce);
        } else {
            self.schedule_single_analysis(uri, version, debounce);
        }
    }

    fn schedule_single_analysis(&self, uri: Uri, version: i32, debounce: bool) {
        let key = uri.as_str().to_string();
        let ticket = self.bump_analysis_gen(&uri);
        let state = Arc::clone(&self.state);
        let client = self.client.clone();

        tokio::spawn(async move {
            if debounce {
                tokio::time::sleep(CHANGE_DEBOUNCE).await;
                if !Backend::analysis_still_current(&state, &key, ticket, Some(&uri), Some(version))
                {
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
            let check_uri = uri.clone();
            let diagnostics = match tokio::task::spawn_blocking(move || {
                check_document(&check_uri, &text, encoding, &config)
            })
            .await
            {
                Ok(items) => items,
                Err(_) => return,
            };

            if !Backend::analysis_still_current(&state, &key, ticket, Some(&uri), Some(version)) {
                return;
            }

            Backend::store_diagnostics(&state, &uri, version, diagnostics.clone());
            if !Backend::analysis_still_current(&state, &key, ticket, Some(&uri), Some(version)) {
                return;
            }
            client
                .publish_diagnostics(uri, diagnostics, Some(version))
                .await;
        });
    }

    /// Recheck all configured project roots (overlays for open buffers).
    ///
    /// When `trigger` is an open non-root buffer, also publish single-file
    /// diagnostics for that URI after the project pass.
    fn schedule_project_analysis(&self, trigger: Option<(Uri, i32)>, debounce: bool) {
        let ticket = self.bump_analysis_key(PROJECT_ANALYSIS_KEY);
        let state = Arc::clone(&self.state);
        let client = self.client.clone();

        tokio::spawn(async move {
            if debounce {
                tokio::time::sleep(CHANGE_DEBOUNCE).await;
                if Backend::analysis_gen(&state, PROJECT_ANALYSIS_KEY) != ticket {
                    return;
                }
            }

            if let Some((ref uri, version)) = trigger
                && !Backend::analysis_still_current(
                    &state,
                    PROJECT_ANALYSIS_KEY,
                    ticket,
                    Some(uri),
                    Some(version),
                )
            {
                return;
            }

            let encoding = state
                .encoding
                .lock()
                .map(|g| *g)
                .unwrap_or(PositionEncoding::Utf16);
            let config = state.config.lock().map(|g| g.clone()).unwrap_or_default();
            let overlays = Backend::collect_overlays(&state);
            let roots = match tokio::task::spawn_blocking(move || {
                check_project_roots(encoding, &config, &overlays)
            })
            .await
            {
                Ok(roots) => roots,
                Err(_) => return,
            };

            if Backend::analysis_gen(&state, PROJECT_ANALYSIS_KEY) != ticket {
                return;
            }

            let mut published: Vec<Uri> = Vec::new();
            for root in roots {
                if Backend::analysis_gen(&state, PROJECT_ANALYSIS_KEY) != ticket {
                    return;
                }
                let version = Backend::open_version(&state, &root.uri);
                if let Some(v) = version {
                    Backend::store_diagnostics(&state, &root.uri, v, root.items.clone());
                }
                published.push(root.uri.clone());
                client
                    .publish_diagnostics(root.uri, root.items, version)
                    .await;
            }

            if Backend::analysis_gen(&state, PROJECT_ANALYSIS_KEY) != ticket {
                return;
            }

            // Clear diagnostics for roots that dropped from the configured set.
            let stale = if let Ok(mut slot) = state.published_project_uris.lock() {
                let prev = std::mem::replace(&mut *slot, published.clone());
                prev.into_iter()
                    .filter(|u| !published.iter().any(|k| k.as_str() == u.as_str()))
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            for uri in stale {
                if Backend::analysis_gen(&state, PROJECT_ANALYSIS_KEY) != ticket {
                    return;
                }
                Backend::clear_diagnostics_cache(&state, &uri);
                client.publish_diagnostics(uri, Vec::new(), None).await;
            }

            // Non-root open buffers still get a single-file check.
            if let Some((uri, version)) = trigger {
                let config = state.config.lock().map(|g| g.clone()).unwrap_or_default();
                if !Backend::is_project_root(&config, &uri) {
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
                    let check_uri = uri.clone();
                    let diagnostics = match tokio::task::spawn_blocking(move || {
                        check_document(&check_uri, &text, encoding, &config)
                    })
                    .await
                    {
                        Ok(items) => items,
                        Err(_) => return,
                    };
                    if !Backend::analysis_still_current(
                        &state,
                        PROJECT_ANALYSIS_KEY,
                        ticket,
                        Some(&uri),
                        Some(version),
                    ) {
                        return;
                    }
                    Backend::store_diagnostics(&state, &uri, version, diagnostics.clone());
                    if !Backend::analysis_still_current(
                        &state,
                        PROJECT_ANALYSIS_KEY,
                        ticket,
                        Some(&uri),
                        Some(version),
                    ) {
                        return;
                    }
                    client
                        .publish_diagnostics(uri, diagnostics, Some(version))
                        .await;
                }
            }
        });
    }
}

fn empty_full_report(result_id: Option<String>) -> DocumentDiagnosticReport {
    DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
        related_documents: None,
        full_document_diagnostic_report: FullDocumentDiagnosticReport {
            result_id,
            items: Vec::new(),
        },
    })
}

fn doc_report_to_workspace(
    uri: Uri,
    version: Option<i64>,
    report: DocumentDiagnosticReport,
) -> WorkspaceDocumentDiagnosticReport {
    match report {
        DocumentDiagnosticReport::Full(full) => {
            WorkspaceDocumentDiagnosticReport::Full(WorkspaceFullDocumentDiagnosticReport {
                uri,
                version,
                full_document_diagnostic_report: full.full_document_diagnostic_report,
            })
        }
        DocumentDiagnosticReport::Unchanged(unchanged) => {
            WorkspaceDocumentDiagnosticReport::Unchanged(
                WorkspaceUnchangedDocumentDiagnosticReport {
                    uri,
                    version,
                    unchanged_document_diagnostic_report: unchanged
                        .unchanged_document_diagnostic_report,
                },
            )
        }
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

        let (format_enable, inlay_hints_enable, project_mode) = {
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
            (
                cfg.format_enable,
                cfg.inlay_hints_enable,
                cfg.project_mode(),
            )
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
                // Stage 23: only `\n` after an `end` line; local indent fix, no
                // format_range expansion (still too aggressive mid-edit).
                document_on_type_formatting_provider: if format_enable {
                    Some(DocumentOnTypeFormattingOptions {
                        first_trigger_character: "\n".into(),
                        more_trigger_character: None,
                    })
                } else {
                    None
                },
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
                diagnostic_provider: Some(DiagnosticServerCapabilities::Options(
                    DiagnosticOptions {
                        identifier: Some("yarrow".into()),
                        // Project mode rechecks all configured roots together.
                        inter_file_dependencies: project_mode,
                        // Open buffers + configured project roots only (Stage 24).
                        workspace_diagnostics: true,
                        ..Default::default()
                    },
                )),
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

    async fn diagnostic(
        &self,
        params: DocumentDiagnosticParams,
    ) -> LspResult<DocumentDiagnosticReportResult> {
        let uri = params.text_document.uri;
        if let Some(id) = params.identifier.as_deref()
            && id != "yarrow"
        {
            return Ok(DocumentDiagnosticReportResult::Report(empty_full_report(
                None,
            )));
        }
        let report =
            Backend::diagnostics_for(&self.state, &uri, params.previous_result_id.as_deref());
        Ok(DocumentDiagnosticReportResult::Report(report))
    }

    async fn workspace_diagnostic(
        &self,
        params: WorkspaceDiagnosticParams,
    ) -> LspResult<WorkspaceDiagnosticReportResult> {
        if let Some(id) = params.identifier.as_deref()
            && id != "yarrow"
        {
            return Ok(WorkspaceDiagnosticReportResult::Report(
                WorkspaceDiagnosticReport { items: Vec::new() },
            ));
        }
        let previous: HashMap<String, String> = params
            .previous_result_ids
            .into_iter()
            .map(|p| (p.uri.as_str().to_string(), p.value))
            .collect();
        let items = Backend::workspace_diagnostics_for(&self.state, &previous);
        Ok(WorkspaceDiagnosticReportResult::Report(
            WorkspaceDiagnosticReport { items },
        ))
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
            Backend::clear_diagnostics_cache(&self.state, &uri);
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

    async fn on_type_formatting(
        &self,
        params: DocumentOnTypeFormattingParams,
    ) -> LspResult<Option<Vec<TextEdit>>> {
        if !self.config_snapshot().format_enable {
            return Ok(None);
        }
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let ch = params.ch;
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
        Ok(format::format_on_type(&text, position, &ch, encoding))
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
