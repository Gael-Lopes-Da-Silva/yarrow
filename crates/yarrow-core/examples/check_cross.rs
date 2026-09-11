//! Stage 33 gate: broader linux cross-compile matrix (musl + Stage 26 gnu).
//!
//! ```bash
//! cargo run -p yarrow_core --example check_cross
//! ```

use yarrow_core::{
    ColorChoice, CompileOptions, Session, TargetTriple, render_batch, supported_triple_names,
};

const HELLO: &str = r#"
"std.io" io require

main function do
	"hello" io.write_line call
end
"#;

fn compile_object(triple: &str) -> Vec<u8> {
    let target = TargetTriple::parse(triple).unwrap_or_else(|e| {
        panic!("parse {triple}: {e}");
    });
    let mut opts = CompileOptions::new(format!("check_cross_{triple}.yar"));
    opts.target = Some(target.clone());
    let artifact = Session::new(opts)
        .compile_object_source(HELLO.to_string())
        .unwrap_or_else(|d| {
            eprintln!("{}", render_batch(&d.batch, &d.file, ColorChoice::Never));
            panic!("{triple} must emit a non-empty object");
        });
    assert_eq!(artifact.target.as_str(), target.as_str());
    assert!(
        !artifact.bytes.is_empty(),
        "{triple}: empty object artifact"
    );
    assert_elf(&artifact.bytes, triple);
    artifact.bytes
}

fn assert_elf(bytes: &[u8], triple: &str) {
    assert!(
        bytes.len() >= 20 && bytes[0..4] == [0x7f, b'E', b'L', b'F'],
        "{triple}: expected ELF magic"
    );
    // e_machine at offset 18 (ELF64 little-endian).
    let machine = u16::from_le_bytes([bytes[18], bytes[19]]);
    let expect = if triple.starts_with("aarch64-") {
        183u16 // EM_AARCH64
    } else if triple.starts_with("x86_64-") {
        62u16 // EM_X86_64
    } else {
        panic!("unexpected triple {triple}");
    };
    assert_eq!(
        machine, expect,
        "{triple}: ELF e_machine {machine}, expected {expect}"
    );
}

fn main() {
    // Documented matrix includes both gnu and musl.
    let names = supported_triple_names();
    assert!(names.iter().any(|n| n.contains("linux-gnu")));
    assert!(names.iter().any(|n| n.contains("linux-musl")));

    // Unsupported class stays E397 (no panic). MSVC is not in the Stage 34 matrix.
    let err = TargetTriple::parse("x86_64-pc-windows-msvc").expect_err("msvc must be rejected");
    assert!(
        err.message.contains("unsupported") || err.message.contains("E397"),
        "unexpected reject message: {}",
        err.message
    );

    // Stage 26: other linux-gnu arch object.
    let host = TargetTriple::host().as_str();
    let other_gnu = if host.starts_with("x86_64-") {
        "aarch64-unknown-linux-gnu"
    } else if host.starts_with("aarch64-") {
        "x86_64-unknown-linux-gnu"
    } else {
        panic!("Stage 33 gate expects a linux host, got {host}");
    };
    compile_object(other_gnu);

    // Stage 33: host-arch musl object (primary new triple class).
    let host_musl = if host.starts_with("x86_64-") {
        "x86_64-unknown-linux-musl"
    } else {
        "aarch64-unknown-linux-musl"
    };
    compile_object(host_musl);

    // Cross-arch musl object as well.
    let other_musl = if host.starts_with("x86_64-") {
        "aarch64-unknown-linux-musl"
    } else {
        "x86_64-unknown-linux-musl"
    };
    compile_object(other_musl);

    // Host object still works.
    let mut opts = CompileOptions::new("check_cross_host.yar");
    let host_art = Session::new(opts)
        .compile_object_source(HELLO.to_string())
        .unwrap_or_else(|d| {
            eprintln!("{}", render_batch(&d.batch, &d.file, ColorChoice::Never));
            panic!("host object emit must succeed");
        });
    assert!(!host_art.bytes.is_empty());
    assert_elf(&host_art.bytes, &host_art.target.as_str());

    // Missing musl CRT / archive must fail with a stable diagnostic (not panic).
    opts = CompileOptions::new("check_cross_musl_link.yar");
    opts.target = Some(TargetTriple::parse(host_musl).expect("musl"));
    // Clear CRT env so discovery cannot accidentally succeed from a polluted shell.
    // SAFETY: single-threaded example process; we restore nothing because the gate exits.
    unsafe {
        std::env::remove_var("YARROW_AOT_CRT_DIR");
        std::env::remove_var("YARROW_AOT_SYSROOT");
        std::env::remove_var(format!(
            "YARROW_RUNTIME_AOT_ARCHIVE_{}",
            TargetTriple::parse(host_musl)
                .expect("musl")
                .env_key_suffix()
        ));
    }
    match Session::new(opts).compile_executable_source(HELLO.to_string()) {
        Ok(_) => {
            // Archive + musl CRT happened to be available; still a success for the stage.
            println!("ok: musl executable link also succeeded (CRT/archive present)");
        }
        Err(d) => {
            let text = render_batch(&d.batch, &d.file, ColorChoice::Never);
            let ok = text.contains("E394") || text.contains("E395") || text.contains("E396");
            assert!(
                ok,
                "musl executable without setup must yield E394/E395/E396, got:\n{text}"
            );
        }
    }

    println!("ok: cross object emit Stage 26 gnu + Stage 33 musl");
}
