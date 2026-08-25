//! Host runtime re-exports and JIT symbol registration.
//!
//! Implementation lives in [`yarrow_runtime`] (linkable `staticlib` for AOT).
//! Symbol names and signatures come from [`HOST_FNS`] in that crate.

pub use yarrow_runtime::*;

use cranelift_jit::JITBuilder;

/// Register every host symbol with the JIT builder so `Linkage::Import`
/// declarations resolve at link time. Single source of truth: [`HOST_FNS`].
pub fn install_runtime(builder: &mut JITBuilder) {
    for h in HOST_FNS.iter() {
        builder.symbol(h.name, h.address as *const u8);
    }
}

// ---------------------------------------------------------------------------
// AOT link artifact (Stage 16)
// ---------------------------------------------------------------------------

/// Host runtime static library bytes for linking with object emit output.
pub struct RuntimeArchive {
    pub bytes: Vec<u8>,
}

/// Read the AOT `staticlib` archive (`libyarrow_runtime_aot.a` / `yarrow_runtime_aot.lib`).
/// Path is fixed when `yarrow-core` is built (`YARROW_RUNTIME_AOT_ARCHIVE`).
pub fn linkable_archive() -> Result<RuntimeArchive, String> {
    let path = option_env!("YARROW_RUNTIME_AOT_ARCHIVE").ok_or_else(|| {
        "YARROW_RUNTIME_AOT_ARCHIVE was not set at build time (rebuild yarrow-core)".to_string()
    })?;
    let bytes = std::fs::read(path).map_err(|e| format!("read runtime archive '{path}': {e}"))?;
    if bytes.is_empty() {
        return Err(format!("runtime archive '{path}' is empty"));
    }
    Ok(RuntimeArchive { bytes })
}

/// Linker-visible symbol names (same as [`HOST_FNS`] `name` fields).
pub fn link_symbol_names() -> impl Iterator<Item = &'static str> + Clone {
    HOST_FNS.iter().map(|h| h.name)
}
