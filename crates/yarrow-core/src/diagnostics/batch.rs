//! Multi-error collection with a rustc-like output cap.

use super::Diagnostic;

/// Default maximum number of errors reported in one run (rustc uses 128;
/// keep this smaller so cascades stay readable).
pub const DEFAULT_ERROR_LIMIT: usize = 20;

/// Accumulates diagnostics until a configurable limit is reached.
#[derive(Debug, Clone)]
pub struct DiagnosticBatch {
    items: Vec<Diagnostic>,
    limit: usize,
}

impl Default for DiagnosticBatch {
    fn default() -> Self {
        Self::new()
    }
}

impl DiagnosticBatch {
    pub fn new() -> Self {
        Self::with_limit(DEFAULT_ERROR_LIMIT)
    }

    pub fn with_limit(limit: usize) -> Self {
        Self {
            items: Vec::new(),
            limit: limit.max(1),
        }
    }

    /// Record `diag`. Returns `false` when the batch is already at the limit
    /// (the diagnostic is discarded).
    pub fn push(&mut self, diag: Diagnostic) -> bool {
        if self.items.len() >= self.limit {
            return false;
        }
        self.items.push(diag);
        true
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

    pub fn is_at_limit(&self) -> bool {
        self.items.len() >= self.limit
    }

    pub fn iter(&self) -> impl Iterator<Item = &Diagnostic> {
        self.items.iter()
    }

    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.items
    }

    /// Take ownership of the collected items, leaving an empty batch with the
    /// same limit.
    pub fn take(&mut self) -> DiagnosticBatch {
        DiagnosticBatch {
            items: std::mem::take(&mut self.items),
            limit: self.limit,
        }
    }
}

impl From<Diagnostic> for DiagnosticBatch {
    fn from(diag: Diagnostic) -> Self {
        let mut batch = Self::new();
        batch.push(diag);
        batch
    }
}

impl FromIterator<Diagnostic> for DiagnosticBatch {
    fn from_iter<I: IntoIterator<Item = Diagnostic>>(iter: I) -> Self {
        let mut batch = Self::new();
        for d in iter {
            if !batch.push(d) {
                break;
            }
        }
        batch
    }
}
