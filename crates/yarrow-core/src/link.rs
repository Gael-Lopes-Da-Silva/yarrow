//! Link a program object + host runtime archive into a host executable.
//!
//! Invokes a system linker (`ld` / `lld`). Does **not** use `cc` / `gcc` /
//! `clang` as a compile or link driver. CRT object paths may be discovered via
//! `cc -print-file-name` when present (path lookup only).

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::diagnostics::{Diagnostic, DiagnosticBatch, Span};

/// Failure while locating the linker, CRT, or running the link.
#[derive(Debug)]
pub struct LinkError {
    pub code: &'static str,
    pub message: String,
    pub help: Option<String>,
    pub note: Option<String>,
}

impl LinkError {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            help: None,
            note: None,
        }
    }

    pub(crate) fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    pub(crate) fn into_batch(self, error_limit: usize) -> DiagnosticBatch {
        let mut batch = DiagnosticBatch::with_limit(error_limit);
        let mut diag = Diagnostic::error(self.code, self.message).with_primary(Span::default(), "");
        if let Some(note) = self.note {
            diag = diag.with_note(note);
        }
        if let Some(help) = self.help {
            diag = diag.with_help(help);
        }
        batch.push(diag);
        batch
    }
}

/// Link `object_bytes` with `archive_bytes` into a host executable image.
pub fn link_executable(object_bytes: &[u8], archive_bytes: &[u8]) -> Result<Vec<u8>, LinkError> {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        link_linux_gnu(object_bytes, archive_bytes)
    }
    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    {
        let _ = (object_bytes, archive_bytes);
        Err(LinkError::new(
            "E394",
            "AOT executable link is only supported on linux-gnu hosts",
        )
        .with_help("use JIT (`ExecutionMode::Jit`) on this host, or link on a linux-gnu machine"))
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn link_linux_gnu(object_bytes: &[u8], archive_bytes: &[u8]) -> Result<Vec<u8>, LinkError> {
    let linker = find_linker()?;
    let crt = CrtFiles::discover()?;
    let work = WorkDir::create()?;

    let obj_path = work.path.join("program.o");
    let arch_path = work.path.join("libyarrow_runtime_aot.a");
    let out_path = work.path.join("a.out");
    fs::write(&obj_path, object_bytes)
        .map_err(|e| LinkError::new("E395", format!("failed to write temporary object: {e}")))?;
    fs::write(&arch_path, archive_bytes).map_err(|e| {
        LinkError::new(
            "E395",
            format!("failed to write temporary runtime archive: {e}"),
        )
    })?;

    let mut cmd = Command::new(&linker);
    cmd.arg("-o").arg(&out_path);
    cmd.arg("-pie");
    cmd.arg("--eh-frame-hdr");
    cmd.arg("-m").arg(elf_emulation());
    cmd.arg("-dynamic-linker").arg(&crt.dynamic_linker);
    cmd.arg(&crt.scrt1);
    cmd.arg(&crt.crti);
    cmd.arg(&crt.crtbegin);
    cmd.arg(&obj_path);
    // Pull all AOT exports / Rust std objects out of the archive.
    cmd.arg("--whole-archive");
    cmd.arg(&arch_path);
    cmd.arg("--no-whole-archive");
    for dir in &crt.lib_dirs {
        cmd.arg("-L").arg(dir);
    }
    cmd.arg("-lpthread");
    cmd.arg("-ldl");
    cmd.arg("-lm");
    cmd.arg("--push-state");
    cmd.arg("--as-needed");
    cmd.arg("-lgcc_s");
    cmd.arg("--pop-state");
    cmd.arg("-lc");
    if let Some(libgcc) = &crt.libgcc_a {
        cmd.arg(libgcc);
    }
    cmd.arg(&crt.crtend);
    cmd.arg(&crt.crtn);

    let output = cmd.output().map_err(|e| {
        LinkError::new(
            "E395",
            format!("failed to invoke linker '{}': {e}", linker.display()),
        )
        .with_help("install a system linker such as `ld` or `lld` (not a C compiler)")
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else if !stdout.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            format!("linker exited with status {}", output.status)
        };
        return Err(LinkError::new("E395", format!("link failed: {detail}"))
            .with_help("need a system linker (`ld` / `lld`) and host libc/CRT; not a C toolchain compile step")
            .with_note(format!("linker: {}", linker.display())));
    }

    let bytes = fs::read(&out_path)
        .map_err(|e| LinkError::new("E395", format!("failed to read linked executable: {e}")))?;
    if bytes.is_empty() {
        return Err(LinkError::new(
            "E395",
            "linker produced an empty executable",
        ));
    }
    Ok(bytes)
}

fn find_linker() -> Result<PathBuf, LinkError> {
    // Prefer a real linker binary. Never use cc/gcc/clang as the driver.
    const CANDIDATES: &[&str] = &["ld", "ld.lld", "lld", "ld.bfd", "ld.gold"];
    for name in CANDIDATES {
        if let Some(path) = which(name) {
            let base = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if matches!(base, "cc" | "gcc" | "clang" | "c++" | "g++" | "clang++") {
                continue;
            }
            return Ok(path);
        }
    }
    Err(LinkError::new("E394", "system linker not found (`ld` / `lld`)")
        .with_help("install a system linker such as binutils `ld` or LLVM `lld` (a C compiler is not required)"))
}

fn which(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
struct CrtFiles {
    scrt1: PathBuf,
    crti: PathBuf,
    crtn: PathBuf,
    crtbegin: PathBuf,
    crtend: PathBuf,
    dynamic_linker: PathBuf,
    libgcc_a: Option<PathBuf>,
    lib_dirs: Vec<PathBuf>,
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
impl CrtFiles {
    fn discover() -> Result<Self, LinkError> {
        let scrt1 = crt_file("Scrt1.o", &["crt1.o"])?;
        let crti = crt_file("crti.o", &[])?;
        let crtn = crt_file("crtn.o", &[])?;
        let crtbegin = crt_file("crtbeginS.o", &["crtbegin.o"])?;
        let crtend = crt_file("crtendS.o", &["crtend.o"])?;
        let dynamic_linker = crt_file(dynamic_linker_name(), &[])?;
        let libgcc_a = print_file_name("libgcc.a")
            .filter(|p| p.is_file())
            .or_else(|| {
                // Next to crtbegin when print-file-name is unavailable.
                crtbegin
                    .parent()
                    .map(|d| d.join("libgcc.a"))
                    .filter(|p| p.is_file())
            });

        let mut lib_dirs = Vec::new();
        for p in [
            &scrt1,
            &crti,
            &crtbegin,
            &dynamic_linker,
            libgcc_a.as_ref().unwrap_or(&scrt1),
        ] {
            if let Some(dir) = p.parent() {
                let dir = dir.to_path_buf();
                if !lib_dirs.iter().any(|d| d == &dir) {
                    lib_dirs.push(dir);
                }
            }
        }
        // libgcc_s often lives in a sibling gcc-lib store path on NixOS.
        if let Some(p) = print_file_name("libgcc_s.so").filter(|p| p.is_file())
            && let Some(dir) = p.parent()
        {
            let dir = dir.to_path_buf();
            if !lib_dirs.iter().any(|d| d == &dir) {
                lib_dirs.push(dir);
            }
        }
        if let Some(p) = print_file_name("libc.so").filter(|p| p.is_file())
            && let Some(dir) = p.parent()
        {
            let dir = dir.to_path_buf();
            if !lib_dirs.iter().any(|d| d == &dir) {
                lib_dirs.push(dir);
            }
        }

        Ok(Self {
            scrt1,
            crti,
            crtn,
            crtbegin,
            crtend,
            dynamic_linker,
            libgcc_a,
            lib_dirs,
        })
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn crt_file(name: &str, alts: &[&str]) -> Result<PathBuf, LinkError> {
    let mut tried = vec![name.to_string()];
    if let Some(p) = print_file_name(name).filter(|p| p.is_file()) {
        return Ok(p);
    }
    for alt in alts {
        tried.push((*alt).to_string());
        if let Some(p) = print_file_name(alt).filter(|p| p.is_file()) {
            return Ok(p);
        }
    }
    Err(LinkError::new(
        "E394",
        format!("missing host CRT object ({})", tried.join(" / ")),
    )
    .with_help(
        "need host libc/CRT objects for `ld` (on NixOS: a stdenv with glibc). A C compiler is only used to locate paths, not to compile",
    ))
}

/// Locate a linker/CRT file. Prefer `cc -print-file-name` when `cc` exists
/// (NixOS gcc-wrapper); never compile with it.
fn print_file_name(name: &str) -> Option<PathBuf> {
    for driver in ["cc", "gcc"] {
        let output = Command::new(driver)
            .arg(format!("-print-file-name={name}"))
            .output()
            .ok()?;
        if !output.status.success() {
            continue;
        }
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() || path == name {
            continue;
        }
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

struct WorkDir {
    path: PathBuf,
}

impl WorkDir {
    fn create() -> Result<Self, LinkError> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("yarrow-link-{nanos}-{}", std::process::id()));
        fs::create_dir_all(&path)
            .map_err(|e| LinkError::new("E395", format!("failed to create link temp dir: {e}")))?;
        Ok(Self { path })
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn elf_emulation() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "elf_x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64linux"
    } else {
        "elf_x86_64"
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn dynamic_linker_name() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "ld-linux-x86-64.so.2"
    } else if cfg!(target_arch = "aarch64") {
        "ld-linux-aarch64.so.1"
    } else {
        "ld-linux-x86-64.so.2"
    }
}
