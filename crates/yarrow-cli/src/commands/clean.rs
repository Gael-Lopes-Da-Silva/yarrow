//! Implementation of the `clean` subcommand.

use std::process::ExitCode;

use crate::args::GlobalArgs;
use crate::artifacts;

/// Delete artifacts listed in `.yarrow-build/artifacts` only.
pub fn clean_artifacts(global: &GlobalArgs) -> ExitCode {
    artifacts::clean(global)
}
