//! Link a program object + runtime archive into an executable for a target triple.
//!
//! Invokes a system linker (`ld` / `lld`). Does **not** use `cc` / `gcc` /
//! `clang` as a compile or link driver. CRT object paths may be discovered via
//! `cc -print-file-name` when present (path lookup only), or via
//! `YARROW_AOT_SYSROOT` / `YARROW_AOT_CRT_DIR` for cross targets (Stage 26 / 33).
//! Stage 34 Mach-O / Windows targets accept object emit only; executable link
//! returns `E397` on linux hosts (no `ld64` / `link.exe` path yet).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::diagnostics::{Diagnostic, DiagnosticBatch, Span};
use crate::target::TargetTriple;

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

/// Link `object_bytes` with `archive_bytes` into an executable for `target`.
pub fn link_executable(
    object_bytes: &[u8],
    archive_bytes: &[u8],
    target: &TargetTriple,
) -> Result<Vec<u8>, LinkError> {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        if target.is_linux_gnu() {
            return link_linux_gnu(object_bytes, archive_bytes, target);
        }
        if target.is_linux_musl() {
            return link_linux_musl(object_bytes, archive_bytes, target);
        }
        if target.is_non_elf_object() {
            return Err(LinkError::new(
                "E397",
                format!(
                    "AOT executable link is not available for '{}' on this host (Stage 34 is object emit only)",
                    target.as_str()
                ),
            )
            .with_help(
                "use Session::compile_object_source for Mach-O / COFF; link executables on the target OS or stick to linux-gnu / linux-musl",
            ));
        }
        Err(LinkError::new(
            "E397",
            format!(
                "AOT executable link does not support target '{}'",
                target.as_str()
            ),
        )
        .with_help(
            "use a linux-gnu or linux-musl triple (x86_64 or aarch64) or emit an object only",
        ))
    }
    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    {
        let _ = (object_bytes, archive_bytes, target);
        Err(LinkError::new(
            "E394",
            "AOT executable link is only supported on linux-gnu hosts",
        )
        .with_help("use JIT (`ExecutionMode::Jit`) on this host, or link on a linux-gnu machine"))
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn link_linux_gnu(
    object_bytes: &[u8],
    archive_bytes: &[u8],
    target: &TargetTriple,
) -> Result<Vec<u8>, LinkError> {
    let emulation = target.elf_emulation().ok_or_else(|| {
        LinkError::new(
            "E397",
            format!("no ELF linker emulation for '{}'", target.as_str()),
        )
    })?;
    let linker = find_linker()?;
    let crt = GnuCrtFiles::discover(target)?;
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
    cmd.arg("-m").arg(emulation);
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

    run_linker(cmd, &linker, &out_path, target, emulation)
}

/// Static-friendly musl link (Stage 33). Prefer `-static` with musl CRT + `libc.a`.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn link_linux_musl(
    object_bytes: &[u8],
    archive_bytes: &[u8],
    target: &TargetTriple,
) -> Result<Vec<u8>, LinkError> {
    let emulation = target.elf_emulation().ok_or_else(|| {
        LinkError::new(
            "E397",
            format!("no ELF linker emulation for '{}'", target.as_str()),
        )
    })?;
    let linker = find_linker()?;
    let crt = MuslCrtFiles::discover(target)?;
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
    cmd.arg("-static");
    cmd.arg("--eh-frame-hdr");
    cmd.arg("-m").arg(emulation);
    cmd.arg(&crt.crt1);
    cmd.arg(&crt.crti);
    cmd.arg(&obj_path);
    cmd.arg("--whole-archive");
    cmd.arg(&arch_path);
    cmd.arg("--no-whole-archive");
    for dir in &crt.lib_dirs {
        cmd.arg("-L").arg(dir);
    }
    cmd.arg("-lpthread");
    cmd.arg("-ldl");
    cmd.arg("-lm");
    cmd.arg("-lc");
    if let Some(libgcc) = &crt.libgcc_a {
        cmd.arg(libgcc);
    }
    cmd.arg(&crt.crtn);

    run_linker(cmd, &linker, &out_path, target, emulation)
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn run_linker(
    mut cmd: Command,
    linker: &Path,
    out_path: &Path,
    target: &TargetTriple,
    emulation: &str,
) -> Result<Vec<u8>, LinkError> {
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
        let help = if target.is_host() {
            "need a system linker (`ld` / `lld`) and host libc/CRT; not a C toolchain compile step"
                .to_string()
        } else if target.is_linux_musl() {
            format!(
                "musl link for '{}' needs matching CRT / libc.a (set YARROW_AOT_SYSROOT or \
                 YARROW_AOT_CRT_DIR to a musl prefix; see docs/RUNTIME.md)",
                target.as_str()
            )
        } else {
            format!(
                "cross-link for '{}' needs a linker that supports `-m {emulation}` and matching CRT \
                 (set YARROW_AOT_SYSROOT or YARROW_AOT_CRT_DIR; see docs/RUNTIME.md)",
                target.as_str()
            )
        };
        return Err(LinkError::new("E395", format!("link failed: {detail}"))
            .with_help(help)
            .with_note(format!(
                "linker: {}; target: {}",
                linker.display(),
                target.as_str()
            )));
    }

    let bytes = fs::read(out_path)
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
struct GnuCrtFiles {
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
impl GnuCrtFiles {
    fn discover(target: &TargetTriple) -> Result<Self, LinkError> {
        let dyn_name = target.dynamic_linker_name().ok_or_else(|| {
            LinkError::new(
                "E397",
                format!("no dynamic linker name for '{}'", target.as_str()),
            )
        })?;
        let scrt1 = crt_file(target, "Scrt1.o", &["crt1.o"])?;
        let crti = crt_file(target, "crti.o", &[])?;
        let crtn = crt_file(target, "crtn.o", &[])?;
        let crtbegin = crt_file(target, "crtbeginS.o", &["crtbegin.o"])?;
        let crtend = crt_file(target, "crtendS.o", &["crtend.o"])?;
        let dynamic_linker = crt_file(target, dyn_name, &[])?;
        let libgcc_a = print_file_name(target, "libgcc.a")
            .filter(|p| p.is_file())
            .or_else(|| {
                crtbegin
                    .parent()
                    .map(|d| d.join("libgcc.a"))
                    .filter(|p| p.is_file())
            });

        let mut lib_dirs = Vec::new();
        collect_lib_dirs(
            &mut lib_dirs,
            [
                &scrt1,
                &crti,
                &crtbegin,
                &dynamic_linker,
                libgcc_a.as_ref().unwrap_or(&scrt1),
            ],
        );
        if let Some(p) = print_file_name(target, "libgcc_s.so").filter(|p| p.is_file()) {
            push_parent(&mut lib_dirs, &p);
        }
        if let Some(p) = print_file_name(target, "libc.so").filter(|p| p.is_file()) {
            push_parent(&mut lib_dirs, &p);
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

/// Musl CRT: no gcc `crtbegin` / `crtend`; static link uses `crt1` + `libc.a`.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
struct MuslCrtFiles {
    crt1: PathBuf,
    crti: PathBuf,
    crtn: PathBuf,
    libgcc_a: Option<PathBuf>,
    lib_dirs: Vec<PathBuf>,
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
impl MuslCrtFiles {
    fn discover(target: &TargetTriple) -> Result<Self, LinkError> {
        let crt1 = crt_file(target, "crt1.o", &["Scrt1.o"])?;
        let crti = crt_file(target, "crti.o", &[])?;
        let crtn = crt_file(target, "crtn.o", &[])?;
        // libc.a must exist for `-static -lc`.
        let libc_a = crt_file(target, "libc.a", &[])?;
        let libgcc_a = print_file_name(target, "libgcc.a").filter(|p| p.is_file());

        let mut lib_dirs = Vec::new();
        collect_lib_dirs(
            &mut lib_dirs,
            [&crt1, &crti, &libc_a, libgcc_a.as_ref().unwrap_or(&crt1)],
        );
        if let Some(p) = print_file_name(target, "libc.a").filter(|p| p.is_file()) {
            push_parent(&mut lib_dirs, &p);
        }

        Ok(Self {
            crt1,
            crti,
            crtn,
            libgcc_a,
            lib_dirs,
        })
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn collect_lib_dirs<'a>(lib_dirs: &mut Vec<PathBuf>, paths: impl IntoIterator<Item = &'a PathBuf>) {
    for p in paths {
        push_parent(lib_dirs, p);
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn push_parent(lib_dirs: &mut Vec<PathBuf>, path: &Path) {
    if let Some(dir) = path.parent() {
        let dir = dir.to_path_buf();
        if !lib_dirs.iter().any(|d| d == &dir) {
            lib_dirs.push(dir);
        }
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn crt_file(target: &TargetTriple, name: &str, alts: &[&str]) -> Result<PathBuf, LinkError> {
    let mut tried = vec![name.to_string()];
    if let Some(p) = print_file_name(target, name).filter(|p| p.is_file()) {
        return Ok(p);
    }
    for alt in alts {
        tried.push((*alt).to_string());
        if let Some(p) = print_file_name(target, alt).filter(|p| p.is_file()) {
            return Ok(p);
        }
    }
    let cross_hint = if target.is_host() {
        "need host libc/CRT objects for `ld` (on NixOS: a stdenv with glibc). A C compiler is only used to locate paths, not to compile"
            .to_string()
    } else if target.is_linux_musl() {
        format!(
            "missing musl CRT for '{}'. Set YARROW_AOT_SYSROOT to a musl sysroot/prefix containing \
             lib/{}, or YARROW_AOT_CRT_DIR to that lib directory (see docs/RUNTIME.md)",
            target.as_str(),
            tried.join(" / ")
        )
    } else {
        format!(
            "missing CRT for cross target '{}'. Set YARROW_AOT_SYSROOT to a sysroot containing usr/lib, \
             or YARROW_AOT_CRT_DIR to a directory with {}, or install a matching cross toolchain",
            target.as_str(),
            tried.join(" / ")
        )
    };
    Err(LinkError::new(
        "E394",
        format!(
            "missing CRT object for '{}' ({})",
            target.as_str(),
            tried.join(" / ")
        ),
    )
    .with_help(cross_hint))
}

/// Locate a linker/CRT file. Prefer env overrides, then `cc -print-file-name`
/// (optionally with `--target=`), then bare `cc` / `gcc`. Never compile with it.
fn print_file_name(target: &TargetTriple, name: &str) -> Option<PathBuf> {
    if let Some(p) = lookup_in_crt_env(name) {
        return Some(p);
    }

    let mut drivers: Vec<(String, Vec<String>)> = Vec::new();
    if !target.is_host() {
        let t = target.as_str();
        // Prefixed cross compilers when present.
        for prefix in [
            format!("{t}-gcc"),
            format!("{t}-cc"),
            "aarch64-linux-gnu-gcc".into(),
            "aarch64-unknown-linux-gnu-gcc".into(),
            "x86_64-linux-gnu-gcc".into(),
            "musl-gcc".into(),
            "x86_64-linux-musl-gcc".into(),
            "x86_64-unknown-linux-musl-gcc".into(),
            "aarch64-linux-musl-gcc".into(),
            "aarch64-unknown-linux-musl-gcc".into(),
        ] {
            drivers.push((prefix, Vec::new()));
        }
        drivers.push(("clang".into(), vec![format!("--target={t}")]));
        drivers.push(("cc".into(), vec![format!("--target={t}")]));
    }
    drivers.push(("cc".into(), Vec::new()));
    drivers.push(("gcc".into(), Vec::new()));

    for (driver, extra) in drivers {
        let mut cmd = Command::new(&driver);
        for arg in &extra {
            cmd.arg(arg);
        }
        cmd.arg(format!("-print-file-name={name}"));
        let Ok(output) = cmd.output() else {
            continue;
        };
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

fn lookup_in_crt_env(name: &str) -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("YARROW_AOT_CRT_DIR") {
        let p = Path::new(&dir).join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Ok(sysroot) = std::env::var("YARROW_AOT_SYSROOT") {
        let candidates = [
            Path::new(&sysroot).join("usr/lib").join(name),
            Path::new(&sysroot).join("lib").join(name),
            Path::new(&sysroot).join("usr/lib64").join(name),
            Path::new(&sysroot).join("lib64").join(name),
        ];
        for p in candidates {
            if p.is_file() {
                return Some(p);
            }
        }
        // Nested arch dirs (Debian multiarch / Nix).
        if let Ok(entries) = fs::read_dir(Path::new(&sysroot).join("usr/lib")) {
            for entry in entries.flatten() {
                let p = entry.path().join(name);
                if p.is_file() {
                    return Some(p);
                }
            }
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
