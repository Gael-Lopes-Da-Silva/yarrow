//! Host runtime re-exports and JIT symbol registration.
//!
//! Implementation lives in [`yarrow_runtime`] (linkable `staticlib` for AOT).
//! Symbol names and signatures come from [`HOST_FNS`] in that crate.

pub use yarrow_runtime::*;

use cranelift_jit::JITBuilder;

use crate::target::TargetTriple;

/// Register every host symbol with the JIT builder so `Linkage::Import`
/// declarations resolve at link time. Single source of truth: [`HOST_FNS`].
pub fn install_runtime(builder: &mut JITBuilder) {
    for h in HOST_FNS.iter() {
        builder.symbol(h.name, h.address as *const u8);
    }
}

// ---------------------------------------------------------------------------
// AOT link artifact (Stage 16 / 26)
// ---------------------------------------------------------------------------

/// Host runtime static library bytes for linking with object emit output.
pub struct RuntimeArchive {
    pub bytes: Vec<u8>,
    /// Triple this archive was built for (when known).
    pub target: TargetTriple,
}

/// Read the host AOT `staticlib` archive (`libyarrow_runtime_aot.a`).
///
/// Equivalent to [`linkable_archive_for`] with [`TargetTriple::host`].
pub fn linkable_archive() -> Result<RuntimeArchive, String> {
    linkable_archive_for(&TargetTriple::host())
}

/// Read the AOT `staticlib` archive for `target`.
///
/// Lookup order:
/// 1. `YARROW_RUNTIME_AOT_ARCHIVE_<triple_with_underscores>` (runtime override)
/// 2. Build-time path for that triple (`YARROW_RUNTIME_AOT_ARCHIVE_*` rustc-env)
/// 3. Host-only fallback: `YARROW_RUNTIME_AOT_ARCHIVE` when `target` is host
pub fn linkable_archive_for(target: &TargetTriple) -> Result<RuntimeArchive, String> {
    let suffix = target.env_key_suffix();
    let override_key = format!("YARROW_RUNTIME_AOT_ARCHIVE_{suffix}");
    if let Ok(path) = std::env::var(&override_key) {
        return read_archive_at(&path, target);
    }

    if let Some(path) = baked_archive_path(target) {
        return read_archive_at(path, target);
    }

    if target.is_host() {
        let path = option_env!("YARROW_RUNTIME_AOT_ARCHIVE").ok_or_else(|| {
            "YARROW_RUNTIME_AOT_ARCHIVE was not set at build time (rebuild yarrow-core)".to_string()
        })?;
        return read_archive_at(path, target);
    }

    Err(format!(
        "no runtime archive for '{}': build with `cargo build -p yarrow_runtime_aot --target {}` \
         and set {override_key}, or rebuild yarrow-core after installing that Rust target",
        target.as_str(),
        target.as_str()
    ))
}

fn baked_archive_path(target: &TargetTriple) -> Option<&'static str> {
    let want = target.as_str();
    let table = option_env!("YARROW_RUNTIME_AOT_ARCHIVE_TABLE").unwrap_or("");
    for entry in table.split(';').filter(|s| !s.is_empty()) {
        let Some((triple, path)) = entry.split_once('=') else {
            continue;
        };
        if triple == want {
            return Some(path);
        }
    }
    None
}

fn read_archive_at(path: &str, target: &TargetTriple) -> Result<RuntimeArchive, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read runtime archive '{path}': {e}"))?;
    if bytes.is_empty() {
        return Err(format!("runtime archive '{path}' is empty"));
    }
    Ok(RuntimeArchive {
        bytes,
        target: target.clone(),
    })
}

/// Linker-visible symbol names (same as [`HOST_FNS`] `name` fields).
pub fn link_symbol_names() -> impl Iterator<Item = &'static str> + Clone {
    HOST_FNS.iter().map(|h| h.name)
}
