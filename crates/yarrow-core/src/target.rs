//! AOT / object target triples (Stage 26).
//!
//! Host linux-gnu remains the default. The first additional triple is the other
//! linux-gnu architecture among `x86_64` and `aarch64`. Unsupported triples
//! fail with diagnostic `E397` (no panic).

use std::fmt;
use std::str::FromStr;

use cranelift_codegen::isa::{self, LookupError};
use target_lexicon::{Architecture, Environment, OperatingSystem, Triple};

/// Documented Stage 26 AOT triples (host plus the other linux-gnu arch).
const KNOWN_LINUX_GNU: &[&str] = &["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"];

/// Canonical target for object emit and executable link.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TargetTriple {
    triple: Triple,
}

impl TargetTriple {
    /// Host triple (`target_lexicon::Triple::host` / compile-time host).
    pub fn host() -> Self {
        Self {
            triple: Triple::host(),
        }
    }

    /// Parse and accept a supported AOT triple, or reject with a clear message.
    pub fn parse(input: &str) -> Result<Self, TargetError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(TargetError::unsupported(
                trimmed,
                "target triple must not be empty",
            ));
        }
        let triple = Triple::from_str(trimmed).map_err(|e| {
            TargetError::unsupported(trimmed, format!("invalid target triple: {e}"))
        })?;
        let parsed = Self { triple };
        if !parsed.is_supported() {
            return Err(TargetError::unsupported(
                trimmed,
                format!(
                    "unsupported AOT target '{trimmed}' (supported: {})",
                    KNOWN_LINUX_GNU.join(", ")
                ),
            ));
        }
        if let Err(e) = parsed.isa_builder() {
            return Err(TargetError::unsupported(
                trimmed,
                format!("Cranelift has no ISA for '{trimmed}': {e}"),
            ));
        }
        Ok(parsed)
    }

    /// Canonical string form (`x86_64-unknown-linux-gnu`, …).
    pub fn as_str(&self) -> String {
        self.triple.to_string()
    }

    /// Underlying target-lexicon triple.
    pub fn triple(&self) -> &Triple {
        &self.triple
    }

    /// True when this is the compile host.
    pub fn is_host(&self) -> bool {
        self.triple == Triple::host()
    }

    /// Linux GNU userland targets we link with `ld` / `lld`.
    pub fn is_linux_gnu(&self) -> bool {
        matches_linux_gnu(&self.triple)
    }

    /// Whether Yarrow AOT currently accepts this triple.
    pub fn is_supported(&self) -> bool {
        if self.is_host() {
            // Host path stays available wherever Stage 19 already linked.
            return cfg!(all(target_os = "linux", target_env = "gnu"))
                || matches_linux_gnu(&self.triple);
        }
        matches_linux_gnu(&self.triple)
    }

    /// Cranelift ISA builder for this triple.
    pub fn isa_builder(&self) -> Result<isa::Builder, LookupError> {
        isa::lookup(self.triple.clone())
    }

    /// ELF `ld -m` emulation for this triple.
    pub fn elf_emulation(&self) -> Option<&'static str> {
        match self.triple.architecture {
            Architecture::X86_64 => Some("elf_x86_64"),
            Architecture::Aarch64(_) => Some("aarch64linux"),
            _ => None,
        }
    }

    /// Dynamic linker soname used when locating CRT for this triple.
    pub fn dynamic_linker_name(&self) -> Option<&'static str> {
        match self.triple.architecture {
            Architecture::X86_64 => Some("ld-linux-x86-64.so.2"),
            Architecture::Aarch64(_) => Some("ld-linux-aarch64.so.1"),
            _ => None,
        }
    }

    /// Env-var suffix for per-triple archive overrides (`YARROW_RUNTIME_AOT_ARCHIVE_*`).
    pub fn env_key_suffix(&self) -> String {
        self.as_str().replace(['-', '.'], "_")
    }
}

fn matches_linux_gnu(triple: &Triple) -> bool {
    triple.operating_system == OperatingSystem::Linux
        && triple.environment == Environment::Gnu
        && matches!(
            triple.architecture,
            Architecture::X86_64 | Architecture::Aarch64(_)
        )
}

impl fmt::Display for TargetTriple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.triple)
    }
}

impl Default for TargetTriple {
    fn default() -> Self {
        Self::host()
    }
}

/// Failure to select an AOT target (maps to diagnostic `E397`).
#[derive(Debug, Clone)]
pub struct TargetError {
    pub triple: String,
    pub message: String,
}

impl TargetError {
    fn unsupported(triple: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            triple: triple.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for TargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for TargetError {}

/// Triples documented as supported for object emit (host + first cross).
pub fn supported_triples() -> Vec<TargetTriple> {
    let mut out = Vec::new();
    let host = TargetTriple::host();
    if host.is_supported() {
        out.push(host);
    }
    for name in KNOWN_LINUX_GNU {
        if let Ok(t) = TargetTriple::parse(name)
            && !out.iter().any(|x| x == &t)
        {
            out.push(t);
        }
    }
    out
}
