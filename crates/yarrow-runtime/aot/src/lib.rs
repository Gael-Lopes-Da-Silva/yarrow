//! Linkable static archive for AOT: re-exports the host runtime without pulling
//! a second copy into JIT binaries (see `yarrow_runtime` rlib-only crate).

pub use yarrow_runtime::*;
