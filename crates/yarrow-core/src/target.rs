//! AOT / object target triples (Stage 26 / 33 / 34).
//!
//! Host linux-gnu remains the default. Supported classes:
//! - the other linux-gnu architecture among `x86_64` and `aarch64` (Stage 26)
//! - `*-linux-musl` for those same arches (Stage 33, static-friendly)
//! - `x86_64-pc-windows-gnu` (COFF object emit) and `*-apple-darwin` (Mach-O
//!   object emit) for Stage 34; executable link for those stays out of scope
//!   on linux hosts
//!
//! Unsupported triples fail with diagnostic `E397` (no panic).

use std::fmt;
use std::str::FromStr;

use cranelift_codegen::isa::{self, LookupError};
use target_lexicon::{Architecture, Environment, OperatingSystem, Triple};

/// Documented Stage 26 AOT triples (host plus the other linux-gnu arch).
const KNOWN_LINUX_GNU: &[&str] = &["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"];

/// Documented Stage 33 musl triples (same arches as gnu).
const KNOWN_LINUX_MUSL: &[&str] = &["x86_64-unknown-linux-musl", "aarch64-unknown-linux-musl"];

/// Documented Stage 34 Windows COFF object triple (gnu ABI; object emit only).
const KNOWN_WINDOWS_GNU: &[&str] = &["x86_64-pc-windows-gnu"];

/// Documented Stage 34 Mach-O object triples (object emit only on linux hosts).
const KNOWN_APPLE_DARWIN: &[&str] = &["x86_64-apple-darwin", "aarch64-apple-darwin"];

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
                    supported_triple_names().join(", ")
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

    /// Linux GNU userland targets we link with `ld` / `lld` (dynamic glibc).
    pub fn is_linux_gnu(&self) -> bool {
        matches_linux_env(&self.triple, Environment::Gnu)
    }

    /// Linux musl userland targets (Stage 33; static-friendly ELF).
    pub fn is_linux_musl(&self) -> bool {
        matches_linux_env(&self.triple, Environment::Musl)
    }

    /// Linux ELF AOT targets (gnu or musl).
    pub fn is_linux_elf(&self) -> bool {
        self.is_linux_gnu() || self.is_linux_musl()
    }

    /// Windows GNU (MinGW) COFF targets (Stage 34 object emit).
    pub fn is_windows_gnu(&self) -> bool {
        self.triple.operating_system == OperatingSystem::Windows
            && self.triple.environment == Environment::Gnu
            && matches!(self.triple.architecture, Architecture::X86_64)
    }

    /// Apple Darwin Mach-O targets (Stage 34 object emit).
    pub fn is_apple_darwin(&self) -> bool {
        matches!(
            self.triple.operating_system,
            OperatingSystem::Darwin(_) | OperatingSystem::MacOSX { .. }
        ) && matches!(
            self.triple.architecture,
            Architecture::X86_64 | Architecture::Aarch64(_)
        )
    }

    /// Non-ELF object formats accepted for emit (Stage 34).
    pub fn is_non_elf_object(&self) -> bool {
        self.is_windows_gnu() || self.is_apple_darwin()
    }

    /// Whether `link_executable` may succeed for this triple on a linux-gnu host.
    pub fn supports_executable_link(&self) -> bool {
        self.is_linux_elf()
    }

    /// Whether Yarrow AOT currently accepts this triple.
    pub fn is_supported(&self) -> bool {
        if self.is_host() {
            // Host path stays available wherever Stage 19 already linked.
            return cfg!(all(target_os = "linux", target_env = "gnu"))
                || self.is_linux_elf()
                || self.is_windows_gnu()
                || self.is_apple_darwin();
        }
        self.is_linux_elf() || self.is_windows_gnu() || self.is_apple_darwin()
    }

    /// Cranelift ISA builder for this triple.
    pub fn isa_builder(&self) -> Result<isa::Builder, LookupError> {
        isa::lookup(self.triple.clone())
    }

    /// ELF `ld -m` emulation for this triple.
    pub fn elf_emulation(&self) -> Option<&'static str> {
        if !self.is_linux_elf() {
            return None;
        }
        match self.triple.architecture {
            Architecture::X86_64 => Some("elf_x86_64"),
            Architecture::Aarch64(_) => Some("aarch64linux"),
            _ => None,
        }
    }

    /// Dynamic linker soname used when locating CRT for this triple.
    pub fn dynamic_linker_name(&self) -> Option<&'static str> {
        match (self.triple.environment, self.triple.architecture) {
            (Environment::Gnu, Architecture::X86_64)
                if self.triple.operating_system == OperatingSystem::Linux =>
            {
                Some("ld-linux-x86-64.so.2")
            }
            (Environment::Gnu, Architecture::Aarch64(_))
                if self.triple.operating_system == OperatingSystem::Linux =>
            {
                Some("ld-linux-aarch64.so.1")
            }
            (Environment::Musl, Architecture::X86_64) => Some("ld-musl-x86_64.so.1"),
            (Environment::Musl, Architecture::Aarch64(_)) => Some("ld-musl-aarch64.so.1"),
            _ => None,
        }
    }

    /// Env-var suffix for per-triple archive overrides (`YARROW_RUNTIME_AOT_ARCHIVE_*`).
    pub fn env_key_suffix(&self) -> String {
        self.as_str().replace(['-', '.'], "_")
    }
}

fn matches_linux_env(triple: &Triple, env: Environment) -> bool {
    triple.operating_system == OperatingSystem::Linux
        && triple.environment == env
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

/// Canonical names listed in `E397` / docs (linux then Windows / Darwin).
pub fn supported_triple_names() -> Vec<&'static str> {
    let mut names = Vec::with_capacity(
        KNOWN_LINUX_GNU.len()
            + KNOWN_LINUX_MUSL.len()
            + KNOWN_WINDOWS_GNU.len()
            + KNOWN_APPLE_DARWIN.len(),
    );
    names.extend_from_slice(KNOWN_LINUX_GNU);
    names.extend_from_slice(KNOWN_LINUX_MUSL);
    names.extend_from_slice(KNOWN_WINDOWS_GNU);
    names.extend_from_slice(KNOWN_APPLE_DARWIN);
    names
}

/// Triples documented as supported for object emit (host + Stage 26–34 matrix).
pub fn supported_triples() -> Vec<TargetTriple> {
    let mut out = Vec::new();
    let host = TargetTriple::host();
    if host.is_supported() {
        out.push(host);
    }
    for name in supported_triple_names() {
        if let Ok(t) = TargetTriple::parse(name)
            && !out.iter().any(|x| x == &t)
        {
            out.push(t);
        }
    }
    out
}
