//! In-memory open document buffers for LSP sync.

use std::collections::HashMap;
use std::path::Path;

use tower_lsp_server::ls_types::Uri;

/// Language id editors should use for Yarrow buffers.
pub const LANGUAGE_ID: &str = "yarrow";

/// One open text document tracked by the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// LSP document version from the client.
    pub version: i32,
    /// Full buffer text.
    pub text: String,
}

/// URI → open document map.
#[derive(Debug, Default)]
pub struct DocumentStore {
    docs: HashMap<Uri, Document>,
}

impl DocumentStore {
    /// Insert or replace a document on `textDocument/didOpen`.
    pub fn open(&mut self, uri: Uri, version: i32, text: String) {
        self.docs.insert(uri, Document { version, text });
    }

    /// Replace full text on `textDocument/didChange` (full sync).
    ///
    /// Returns `false` if the URI was not open.
    pub fn set_text(&mut self, uri: &Uri, version: i32, text: String) -> bool {
        let Some(doc) = self.docs.get_mut(uri) else {
            return false;
        };
        doc.version = version;
        doc.text = text;
        true
    }

    /// Remove a document on `textDocument/didClose`.
    ///
    /// Returns `false` if the URI was not open.
    pub fn close(&mut self, uri: &Uri) -> bool {
        self.docs.remove(uri).is_some()
    }

    /// Look up an open document.
    pub fn get(&self, uri: &Uri) -> Option<&Document> {
        self.docs.get(uri)
    }

    /// Number of open documents.
    pub fn len(&self) -> usize {
        self.docs.len()
    }

    /// Whether no documents are open.
    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    /// Snapshot of open documents (URI + text) for workspace queries.
    pub fn snapshot_texts(&self) -> Vec<(Uri, String)> {
        self.docs
            .iter()
            .map(|(uri, doc)| (uri.clone(), doc.text.clone()))
            .collect()
    }
}

/// Whether this open should be tracked: language id `yarrow`, a `.yar` path,
/// or a virtual `yarrow-std:` buffer.
pub fn should_track(uri: &Uri, language_id: &str) -> bool {
    language_id == LANGUAGE_ID
        || uri_has_yar_extension(uri)
        || crate::modules::is_virtual_std_uri(uri)
}

fn uri_has_yar_extension(uri: &Uri) -> bool {
    let path = uri.path().as_str();
    Path::new(path)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("yar"))
}
