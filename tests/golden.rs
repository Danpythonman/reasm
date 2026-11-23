//! Golden tests (TESTING.md §2): every directory in `tests/cases/` holds
//! `input.asm`, `expected.asm` and an optional `reasm.toml`. Each case also
//! checks idempotency (COR-2) and that the safety check passes (ADR-0011).
//! `REASM_BLESS=1 cargo test --test golden` rewrites `expected.asm`.

use std::fs;
use std::path::Path;

use reasm::config::load;
use reasm::dialect::nasm::Nasm;
use reasm::format::format;

#[test]
fn golden_cases() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases");
    let bless = std::env::var_os("REASM_BLESS").is_some();
    let mut dirs: Vec<_> = fs::read_dir(&root)
        .expect("tests/cases")
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    assert!(!dirs.is_empty());

    let mut failures = Vec::new();
    for dir in &dirs {
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let toml = dir.join("reasm.toml");
        let config = load(toml.exists().then_some(toml.as_path()), &[])
            .unwrap_or_else(|e| panic!("{name}: bad reasm.toml: {e}"));
        let input = fs::read_to_string(dir.join("input.asm"))
            .unwrap_or_else(|e| panic!("{name}: input.asm: {e}"));
        let out = format(&input, None, &config, &Nasm);
        if let Some(e) = &out.check_failure {
            failures.push(format!("{name}: {e}"));
            continue;
        }
        let expected_path = dir.join("expected.asm");
        if bless {
            fs::write(&expected_path, &out.text).expect("write expected.asm");
        }
        let expected = fs::read_to_string(&expected_path).unwrap_or_default();
        if out.text != expected {
            failures.push(format!(
                "{name}: output differs from expected.asm\n--- expected\n{expected}--- actual\n{}",
                out.text
            ));
            continue;
        }
        let again = format(&out.text, None, &config, &Nasm);
        if again.text != out.text || again.check_failure.is_some() {
            failures.push(format!(
                "{name}: not idempotent\n--- first\n{}--- second\n{}",
                out.text, again.text
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} golden cases failed:\n\n{}",
        failures.len(),
        dirs.len(),
        failures.join("\n\n")
    );
}
