//! Yarrow language server.
//!
//! Speaks LSP over stdio and delegates analysis to `yarrow_core`. Stage 0 only
//! implements the initialize / shutdown handshake.

use std::fmt;

use tower_lsp_server::jsonrpc::Result as LspResult;
use tower_lsp_server::ls_types::{
    InitializeParams, InitializeResult, InitializedParams, MessageType, ServerCapabilities,
    ServerInfo,
};
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

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

/// Library entry used by the binary and later by `yarrow lsp`.
pub async fn run_stdio() -> Result<(), LspError> {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    run_with_streams(stdin, stdout).await
}

/// Run the server over explicit async read/write streams (tests / embedding).
pub async fn run_with_streams<I, O>(stdin: I, stdout: O) -> Result<(), LspError>
where
    I: tokio::io::AsyncRead + Unpin,
    O: tokio::io::AsyncWrite,
{
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
    Ok(())
}

struct Backend {
    client: Client,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self { client }
    }
}

impl fmt::Debug for Backend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Backend").finish_non_exhaustive()
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> LspResult<InitializeResult> {
        // Keep capabilities empty until Stage 1 (document sync).
        Ok(InitializeResult {
            capabilities: ServerCapabilities::default(),
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
}
