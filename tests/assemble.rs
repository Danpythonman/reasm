//! Assembly equivalence of the golden tests (TESTING.md §3, COR-1): each
//! case's `input.asm` and `expected.asm` must assemble to identical bytes
//! (`nasm -f elf64`, then `-f bin`). A case whose input doesn't assemble on
//! its own has a `no-assemble` file giving the reason. Skipped, with a
//! message, when nasm isn't on PATH.

use std::fs;
use std::path::Path;

use reasm::assemble::{Assembler, Verdict};

#[test]
fn golden_cases_assemble_identically() {
    let nasm = Assembler::new(None);
    if let Err(e) = nasm.probe() {
        eprintln!("skipping: cannot run nasm: {e}");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases");
    let mut dirs: Vec<_> = fs::read_dir(&root)
        .expect("tests/cases")
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();

    let (mut failures, mut compared) = (Vec::new(), 0);
    for dir in &dirs {
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let input = fs::read_to_string(dir.join("input.asm")).unwrap();
        let expected = fs::read_to_string(dir.join("expected.asm")).unwrap();
        let marked = dir.join("no-assemble").exists();
        // Assembled as if they were the case's input file, so includes
        // resolve from the case directory.
        let verdict = nasm
            .compare(&input, &expected, Some(&dir.join("input.asm")))
            .unwrap_or_else(|e| panic!("{name}: cannot run nasm: {e}"));
        match verdict {
            Verdict::Same { .. } if marked => failures.push(format!(
                "{name}: assembles, so remove its `no-assemble` file"
            )),
            Verdict::Same { .. } => compared += 1,
            Verdict::OriginalFails { .. } if marked => {}
            Verdict::OriginalFails { message } => failures.push(format!(
                "{name}: input.asm doesn't assemble ({message}); fix it or add a \
                 `no-assemble` file with the reason"
            )),
            v => failures.push(format!("{name}: {v:?}")),
        }
    }
    eprintln!(
        "{compared} of {} golden cases assemble identically",
        dirs.len()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
