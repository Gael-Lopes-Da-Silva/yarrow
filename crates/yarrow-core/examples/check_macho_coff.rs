//! Stage 34 gate: Mach-O / COFF object emit (executable link stays E397).
//!
//! ```bash
//! cargo run -p yarrow_core --example check_macho_coff
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
    let mut opts = CompileOptions::new(format!("check_macho_coff_{triple}.yar"));
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
    artifact.bytes
}

fn assert_coff_x86_64(bytes: &[u8], triple: &str) {
    assert!(
        bytes.len() >= 2,
        "{triple}: COFF object too short ({})",
        bytes.len()
    );
    // IMAGE_FILE_MACHINE_AMD64 = 0x8664 (little-endian).
    let machine = u16::from_le_bytes([bytes[0], bytes[1]]);
    assert_eq!(
        machine, 0x8664,
        "{triple}: expected COFF machine AMD64 (0x8664), got {machine:#x}"
    );
}

fn assert_macho_64_le(bytes: &[u8], triple: &str) {
    assert!(
        bytes.len() >= 4,
        "{triple}: Mach-O object too short ({})",
        bytes.len()
    );
    // MH_MAGIC_64 little-endian: 0xFEEDFACF → CF FA ED FE.
    assert_eq!(
        &bytes[0..4],
        &[0xcf, 0xfa, 0xed, 0xfe],
        "{triple}: expected Mach-O 64-bit LE magic, got {:02x?}",
        &bytes[0..4]
    );
}

fn expect_link_e397(triple: &str) {
    let mut opts = CompileOptions::new(format!("check_macho_coff_link_{triple}.yar"));
    opts.target = Some(TargetTriple::parse(triple).expect("supported object triple"));
    match Session::new(opts).compile_executable_source(HELLO.to_string()) {
        Ok(_) => panic!("{triple}: executable link must stay out of scope (E397)"),
        Err(d) => {
            let text = render_batch(&d.batch, &d.file, ColorChoice::Never);
            assert!(
                text.contains("E397"),
                "{triple}: expected E397 for executable link, got:\n{text}"
            );
        }
    }
}

fn main() {
    let names = supported_triple_names();
    assert!(
        names.contains(&"x86_64-pc-windows-gnu"),
        "windows-gnu missing from supported list"
    );
    assert!(
        names.iter().any(|n| n.contains("apple-darwin")),
        "apple-darwin missing from supported list"
    );

    // MSVC stays unsupported (Stage 34 documents gnu COFF only).
    let err = TargetTriple::parse("x86_64-pc-windows-msvc").expect_err("msvc must be rejected");
    assert!(
        err.message.contains("unsupported") || err.message.contains("E397"),
        "unexpected reject message: {}",
        err.message
    );

    let coff = compile_object("x86_64-pc-windows-gnu");
    assert_coff_x86_64(&coff, "x86_64-pc-windows-gnu");
    expect_link_e397("x86_64-pc-windows-gnu");

    let macho_x64 = compile_object("x86_64-apple-darwin");
    assert_macho_64_le(&macho_x64, "x86_64-apple-darwin");
    expect_link_e397("x86_64-apple-darwin");

    let macho_arm = compile_object("aarch64-apple-darwin");
    assert_macho_64_le(&macho_arm, "aarch64-apple-darwin");
    expect_link_e397("aarch64-apple-darwin");

    // Host linux-gnu path unchanged.
    let host = TargetTriple::host().as_str();
    assert!(
        host.contains("linux"),
        "Stage 34 gate expects a linux host, got {host}"
    );
    let opts = CompileOptions::new("check_macho_coff_host.yar");
    let host_art = Session::new(opts)
        .compile_object_source(HELLO.to_string())
        .unwrap_or_else(|d| {
            eprintln!("{}", render_batch(&d.batch, &d.file, ColorChoice::Never));
            panic!("host object emit must succeed");
        });
    assert!(!host_art.bytes.is_empty());
    assert_eq!(&host_art.bytes[0..4], &[0x7f, b'E', b'L', b'F']);

    println!("ok: Stage 34 COFF + Mach-O object emit (link E397)");
}
