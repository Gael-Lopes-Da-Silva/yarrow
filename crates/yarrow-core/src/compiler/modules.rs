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
///
/// `item` is `Some(name)` for an item import (`"a.b.c" require` where `c` is a
/// function of module `a.b`): only that single function is exposed. `None`
/// means the whole module is imported.
#[derive(Debug, Clone)]
pub struct RequiredModule {
    pub path: String,
    pub alias: Option<String>,
    pub item: Option<String>,
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
        self.try_load(path).ok_or_else(|| {
            CompileError::new(
                format!("unknown module '{path}'"),
                Location::default(),
                "E380",
            )
        })
    }

    /// Resolve `path` to source if the module exists, else `None`. Unlike
    /// `load`, non-existence is not an error, so callers can probe for a
    /// module (used by parent-first item resolution).
    pub fn try_load(&self, path: &str) -> Option<String> {
        for (name, source) in STD_MODULES {
            if *name == path {
                return Some((*source).to_string());
            }
        }
        let relative = path.replace('.', "/");
        for root in &self.search_paths {
            let file = root.join(&relative).with_extension("yar");
            if file.is_file() {
                return std::fs::read_to_string(&file).ok();
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Standard library
// ---------------------------------------------------------------------------

// Embedded std-library modules: dotted path -> Yarrow source. Generated at
// build time by `build.rs` from `lib/std/**/*.yar`: each file
// `lib/std/<name>.yar` becomes module `std.<name>`.
include!(concat!(env!("OUT_DIR"), "/std_modules.rs"));
