//! Module loader for `require`: resolves dotted module paths (like
//! `"std.math.sqrt"`) to Yarrow source.
//!
//! The standard library is embedded in the binary as Yarrow snippets; user
//! modules are loaded from a configurable search path as `path/to/module.yar`.
//! Loaded modules are parsed by the same parser and compiled into the same
//! JIT module, so `require` really does import code, not just symbols.

use std::path::PathBuf;

use crate::parser::ast::Program;
use crate::tokenizer::token::Location;

use super::errors::CompileError;
use super::types::CResult;

/// A module imported with `require`, paired with its optional alias.
#[derive(Debug, Clone)]
pub struct RequiredModule {
    pub path: String,
    pub alias: Option<String>,
    pub program: Program,
}

/// Resolves module paths to Yarrow source.
pub struct ModuleLoader {
    search_paths: Vec<PathBuf>,
}

impl ModuleLoader {
    pub fn new() -> Self {
        Self {
            search_paths: Vec::new(),
        }
    }

    /// Add a directory searched for user modules. A require of `"a.b"` looks
    /// for `a/b.yar` under every registered path.
    pub fn add_search_path(&mut self, path: impl Into<PathBuf>) {
        self.search_paths.push(path.into());
    }

    /// Load the source of a module by its dotted path.
    pub fn load(&self, path: &str) -> CResult<String> {
        for (name, source) in STD_MODULES {
            if *name == path {
                return Ok((*source).to_string());
            }
        }
        let relative = path.replace('.', "/");
        for root in &self.search_paths {
            let file = root.join(&relative).with_extension("yar");
            if file.is_file() {
                return std::fs::read_to_string(&file).map_err(|e| {
                    CompileError::new(
                        format!("cannot read module '{path}': {e}"),
                        Location::default(),
                        "E380",
                    )
                });
            }
        }
        Err(CompileError::new(
            format!("unknown module '{path}'"),
            Location::default(),
            "E380",
        ))
    }
}

// ---------------------------------------------------------------------------
// Standard library
// ---------------------------------------------------------------------------

/// Embedded std-library modules: dotted path -> Yarrow source.
pub const STD_MODULES: &[(&str, &str)] = &[
    ("std.io", STD_IO),
    ("std.math.sqrt", STD_MATH_SQRT),
    ("std.string", STD_STRING),
    ("std.list", STD_LIST),
    ("std.map", STD_MAP),
];

/// Text I/O. `write_line` prints a string followed by a newline.
const STD_IO: &str = r#"
write_line function
    string
do
    @print
    @print_newline
end
"#;

/// Square root over f64.
const STD_MATH_SQRT: &str = r#"
sqrt function
    f64
do
    @sqrt
end with f64
"#;

/// String utilities.
const STD_STRING: &str = r#"
len function
    string
do
    @string_len
end with i64

join function
    string
    string
    string
do
    @string_join
end with string
"#;

/// List operations for `list<i32>`.
const STD_LIST: &str = r#"
push function
    list<i32>
    i32
do
    @list_push
end

len function
    list<i32>
do
    @list_len
end with i64

get function
    list<i32>
    i64
do
    @list_get
end with i32

put function
    list<i32>
    i64
    i32
do
    @list_set
end"#;

/// Map operations for `hashmap<i64 i32>`. `get` pushes the value and a found
/// flag (like the `@map_get` builtin).
const STD_MAP: &str = r#"
len function
    hashmap<i64 i32>
do
    @map_len
end with i64

get function
    hashmap<i64 i32>
    i64
do
    @map_get
end with i32 bool

put function
    hashmap<i64 i32>
    i64
    i32
do
    @map_set
end
"#;
