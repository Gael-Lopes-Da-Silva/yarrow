//! stdio entry for `cargo run -p yarrow_lsp`.

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    match yarrow_lsp::run_stdio().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("yarrow-lsp: {err}");
            ExitCode::FAILURE
        }
    }
}
