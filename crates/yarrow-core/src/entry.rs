//! Program entry / CRT glue for AOT (Stage 17).
//!
//! Object emit exports a fixed trampoline [`ENTRY_LINK_SYMBOL`] (`yarrow_entry`)
//! with ABI `int64_t yarrow_entry(void)`. The host CRT defines C `main`, calls
//! that trampoline, and uses the `i64` as the process exit status.
//!
//! The Yarrow function that becomes the trampoline target is chosen by
//! [`crate::CompileOptions::entry_name`] (default `"main"`). That name is not
//! the C process entry: exporting a Yarrow `main` as linker `main` would clash
//! with this CRT.

/// Linker symbol object emit exports for the CRT to call.
pub const ENTRY_LINK_SYMBOL: &str = "yarrow_entry";

/// Internal object-local symbol for the real Yarrow entry body (not CRT-visible).
pub(crate) const USER_ENTRY_LINK_SYMBOL: &str = "yarrow_user_entry";

/// Default language entry name (`CompileOptions::entry_name`).
pub const DEFAULT_ENTRY_NAME: &str = "main";

/// CRT artifact: C source that defines process `main` and calls [`ENTRY_LINK_SYMBOL`].
#[derive(Debug, Clone)]
pub struct EntryCrt {
    /// Yarrow entry name this CRT was generated for (documentation / Stage 18).
    pub entry_name: String,
    /// Symbol the CRT imports (`yarrow_entry`).
    pub link_symbol: &'static str,
    /// Complete C translation unit (compile with host `cc` in Stage 18).
    pub source: String,
}

/// Build CRT C source for the given Yarrow entry name.
///
/// The generated `main` always calls [`ENTRY_LINK_SYMBOL`]; `entry_name` selects
/// which Yarrow function object emit binds to that trampoline (see compiler).
pub fn entry_crt_source(entry_name: &str) -> EntryCrt {
    let name = if entry_name.is_empty() {
        DEFAULT_ENTRY_NAME
    } else {
        entry_name
    };
    let source = format!(
        r#"/* Yarrow CRT: process entry for AOT binaries.
 * Yarrow entry function: `{name}` (CompileOptions::entry_name).
 * Object emit exports `{link}` with ABI:
 *   int64_t {link}(void);
 * Mapping: void / non-integer -> 0; integer -> value; fallible error -> 1.
 */
#include <stdint.h>

extern int64_t {link}(void);

int main(void) {{
    return (int){link}();
}}
"#,
        name = name,
        link = ENTRY_LINK_SYMBOL,
    );
    EntryCrt {
        entry_name: name.to_string(),
        link_symbol: ENTRY_LINK_SYMBOL,
        source,
    }
}
