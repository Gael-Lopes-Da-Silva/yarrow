//! Stage 39 gate: Windows-gnu host→host executable link.
//!
//! On a Windows-gnu host (CI: `.github/workflows/stage-39-windows-gnu-exe.yml`):
//! compile `docs/examples/valid/01_hello.yar` to a PE exe, run it, expect
//! `Hello, Yarrow!`.
//!
//! On linux-gnu hosts: assert Windows-gnu executable link still returns `E397`
//! (object emit only; no fake PE link on linux).
//!
//! ```bash
//! cargo run -p yarrow_core --example check_windows_exe
//! ```

use yarrow_core::{ColorChoice, CompileOptions, Session, render_batch};

#[cfg(all(target_os = "linux", target_env = "gnu"))]
use yarrow_core::TargetTriple;

#[cfg(all(target_os = "windows", target_env = "gnu"))]
fn repo_root() -> std::path::PathBuf {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/yarrow-core → repo root")
        .to_path_buf()
}

#[cfg(all(target_os = "windows", target_env = "gnu"))]
fn gate_windows_gnu_host() {
    let repo = repo_root();
    let rel = "docs/examples/valid/01_hello.yar";
    let path = repo.join(rel);
    let source = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {rel}: {e}"));

    let opts = CompileOptions::new(path.display().to_string());
    let session = Session::new(opts);
    let artifact = session
        .compile_executable_source(source)
        .unwrap_or_else(|d| {
            eprintln!("{}", render_batch(&d.batch, &d.file, ColorChoice::Never));
            panic!("{rel} must compile_executable_source on Windows-gnu");
        });

    assert!(
        artifact.target.is_windows_gnu() && artifact.target.is_host(),
        "expected host windows-gnu target, got {}",
        artifact.target.as_str()
    );
    assert!(
        !artifact.bytes.is_empty(),
        "linked executable must be non-empty"
    );

    let out_dir = std::env::temp_dir().join(format!(
        "yarrow-stage39-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&out_dir).expect("temp dir");
    let exe_path = out_dir.join("hello.exe");
    std::fs::write(&exe_path, &artifact.bytes).expect("write exe");

    let output = std::process::Command::new(&exe_path)
        .output()
        .unwrap_or_else(|e| panic!("run {}: {e}", exe_path.display()));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "hello.exe failed ({:?}): stdout={stdout:?} stderr={stderr:?}",
        output.status
    );
    assert!(
        stdout.contains("Hello, Yarrow!"),
        "expected Hello, Yarrow! in stdout, got {stdout:?}"
    );

    let _ = std::fs::remove_dir_all(&out_dir);
    println!("ok: Windows-gnu host→host exe for {rel}");
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn gate_linux_windows_still_object_only() {
    let source = r#"
"std.io" io require
main function do
	"Hello, Yarrow!" io.write_line call
end
"#;
    let mut opts = CompileOptions::new("windows-exe-gate.yar");
    opts.target = Some(TargetTriple::parse("x86_64-pc-windows-gnu").expect("supported"));
    let err = match Session::new(opts).compile_executable_source(source.into()) {
        Ok(_) => panic!("Windows-gnu exe on linux must stay E397"),
        Err(e) => e,
    };
    assert!(
        err.batch.iter().any(|d| d.code == "E397"),
        "expected E397, got {}",
        render_batch(&err.batch, &err.file, ColorChoice::Never)
    );

    // Object emit for the same triple must still succeed.
    let mut opts = CompileOptions::new("windows-obj-gate.yar");
    opts.target = Some(TargetTriple::parse("x86_64-pc-windows-gnu").expect("supported"));
    let obj = Session::new(opts)
        .compile_object_source(source.into())
        .unwrap_or_else(|d| {
            eprintln!("{}", render_batch(&d.batch, &d.file, ColorChoice::Never));
            panic!("Windows-gnu object emit must still work on linux");
        });
    assert!(obj.target.is_windows_gnu());
    assert!(!obj.bytes.is_empty());

    println!("ok: linux keeps Windows-gnu object-only (E397 for exe)");
}

fn main() {
    #[cfg(all(target_os = "windows", target_env = "gnu"))]
    gate_windows_gnu_host();

    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    gate_linux_windows_still_object_only();

    #[cfg(not(any(
        all(target_os = "windows", target_env = "gnu"),
        all(target_os = "linux", target_env = "gnu")
    )))]
    {
        println!("skip: Stage 39 gate (need windows-gnu host or linux-gnu regression check)");
    }
}
