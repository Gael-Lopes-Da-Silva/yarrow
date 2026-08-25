//! Long-form explanations for diagnostic codes (`yarrow explain E308`).

/// One catalog entry: code, short title, and a paragraph for `--explain`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExplainEntry {
    pub code: &'static str,
    pub title: &'static str,
    pub body: &'static str,
}

/// Teachable notes from Phase B diagnostics, keyed by code.
const CATALOG: &[ExplainEntry] = &[
    ExplainEntry {
        code: "E308",
        title: "unwrap and fallible returns",
        body: "\
A function declared `with |T Err|` is fallible: success carries `T` (or nothing \
if `T` is void), and failure carries an error member in an envelope.

`unwrap` applies to that envelope. On success it pushes the payload. On failure \
it propagates the error to the caller, so the caller must also be fallible \
(`with |T Err|`). Using `unwrap` in a function that cannot error is rejected.

On a value that is not an error envelope, `unwrap` is an identity (a no-op).

If the caller cannot propagate, recover with `handle ... fallback ... end` \
instead of `unwrap`. Anonymous `|T U|` unions are only supported as this \
fallible-return form, not as general types.",
    },
    ExplainEntry {
        code: "E370",
        title: "unsafe operation outside an unsafe context",
        body: "\
Raw pointers, `mem.allocate` / `free` / `load` / `store`, and calls to \
`unsafe function` need an unsafe context. Wrap the operation in \
`unsafe ... end`, or mark the enclosing function `unsafe function`.

`unsafe` does not turn off type, stack, ownership, or borrow checking.",
    },
    ExplainEntry {
        code: "E373",
        title: "use after move",
        body: "\
`move` transfers ownership: the source name is empty afterwards. Read from the \
destination variable, or avoid moving if you still need the source.",
    },
    ExplainEntry {
        code: "E374",
        title: "mutation or drop while a borrow is live",
        body: "\
A live `borrow` (or region put) pins the owner until the reference is released. \
Pop or otherwise consume the reference before mutating or dropping the owner.",
    },
    ExplainEntry {
        code: "E375",
        title: "second overlapping borrow",
        body: "\
Yarrow allows only one live borrow of a value at a time. Release the first \
reference (for example with `pop`) before borrowing again.",
    },
    ExplainEntry {
        code: "E376",
        title: "use after region free",
        body: "\
Values put into a region become invalid after `region.free`. Finish using \
borrows of region-attached values before freeing the region.",
    },
    ExplainEntry {
        code: "E334",
        title: "integer-only remainder and power",
        body: "\
`%` is integer remainder and `^` is integer exponentiation. They apply to \
integer operands only, not to floats.

For floats, use `/` for division. There is no float power operator yet; use \
integer `^` or an explicit conversion if you need exponentiation on integers \
first.",
    },
    ExplainEntry {
        code: "E394",
        title: "system linker or CRT missing",
        body: "\
Native executable emit links the program object with the host runtime archive \
using a system linker (`ld` / `lld`). Yarrow does not drive `cc` / `gcc` / \
`clang` to compile CRT or user code.

Install binutils `ld` or LLVM `lld`, and ensure host libc CRT objects are \
visible (on NixOS, a stdenv with glibc). Path discovery may use \
`cc -print-file-name` when present; that is lookup only.",
    },
    ExplainEntry {
        code: "E395",
        title: "native link failed",
        body: "\
The system linker ran but failed to produce an executable. Check the linker \
message for missing libraries or CRT objects. Fix the host link environment; \
do not fall back to JIT for `--target object`.",
    },
    ExplainEntry {
        code: "E396",
        title: "runtime archive unavailable",
        body: "\
AOT link needs `libyarrow_runtime_aot` (the Stage 16 static archive). Rebuild \
`yarrow-core` so `YARROW_RUNTIME_AOT_ARCHIVE` points at a non-empty archive.",
    },
];

/// Normalize a user-supplied code (`e308`, `308`, `E308`) to `E308` form.
pub fn normalize_code(code: &str) -> String {
    let trimmed = code.trim().trim_start_matches('#');
    let upper = trimmed.to_ascii_uppercase();
    if upper.starts_with('E') {
        upper
    } else if !upper.is_empty() && upper.chars().all(|c| c.is_ascii_digit()) {
        format!("E{upper}")
    } else {
        upper
    }
}

/// Look up the long-form explanation for a diagnostic code.
pub fn explain_code(code: &str) -> Option<&'static ExplainEntry> {
    let key = normalize_code(code);
    CATALOG.iter().find(|e| e.code == key)
}

/// Render a catalog entry the way `rustc --explain` prints a code.
pub fn format_explain(entry: &ExplainEntry) -> String {
    format!("error[{}]: {}\n\n{}\n", entry.code, entry.title, entry.body)
}
