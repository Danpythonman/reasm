//! End-to-end tests of the `reasm` binary: modes, exit codes, config
//! discovery and `--set` (docs/CLI.md).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

/// A fresh, empty directory under the system temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static N: AtomicU32 = AtomicU32::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("reasm-cli-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn write(&self, rel: &str, contents: &str) -> PathBuf {
        let p = self.0.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, contents).unwrap();
        p
    }

    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.0.join(rel)).unwrap()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn reasm(dir: &Path, args: &[&str], stdin: Option<&str>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_reasm"));
    cmd.current_dir(dir)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.stdin(if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    let mut child = cmd.spawn().unwrap();
    if let Some(input) = stdin {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    child.wait_with_output().unwrap()
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8(o.stdout.clone()).unwrap()
}

fn stderr(o: &Output) -> String {
    String::from_utf8(o.stderr.clone()).unwrap()
}

const MESSY: &str = "mov eax, 1   \n\n\n";
const CLEAN: &str = "        mov eax, 1\n";

#[test]
fn formats_file_in_place() {
    let t = TempDir::new();
    t.write("a.asm", MESSY);
    let o = reasm(&t.0, &["a.asm"], None);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(t.read("a.asm"), CLEAN);
    assert!(stdout(&o).is_empty());
}

#[test]
fn unchanged_file_is_not_rewritten() {
    let t = TempDir::new();
    let p = t.write("a.asm", CLEAN);
    let before = std::fs::metadata(&p).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    assert_eq!(code(&reasm(&t.0, &["a.asm"], None)), 0);
    assert_eq!(std::fs::metadata(&p).unwrap().modified().unwrap(), before);
}

#[test]
fn check_mode_reports_and_does_not_write() {
    let t = TempDir::new();
    t.write("dirty.asm", MESSY);
    t.write("clean.asm", CLEAN);
    let o = reasm(&t.0, &["--check", "dirty.asm", "clean.asm"], None);
    assert_eq!(code(&o), 1);
    assert_eq!(stdout(&o), "dirty.asm\n");
    assert_eq!(t.read("dirty.asm"), MESSY);
    assert_eq!(code(&reasm(&t.0, &["--check", "clean.asm"], None)), 0);
}

#[test]
fn stdout_mode() {
    let t = TempDir::new();
    t.write("a.asm", MESSY);
    t.write("b.asm", MESSY);
    let o = reasm(&t.0, &["--stdout", "a.asm"], None);
    assert_eq!(stdout(&o), CLEAN);
    assert_eq!(t.read("a.asm"), MESSY);
    let o = reasm(&t.0, &["--stdout", "a.asm", "b.asm"], None);
    assert_eq!(
        stdout(&o),
        format!("==> a.asm <==\n{CLEAN}==> b.asm <==\n{CLEAN}")
    );
}

#[test]
fn stdin_to_stdout() {
    let t = TempDir::new();
    for args in [&[][..], &["-"][..]] {
        let o = reasm(&t.0, args, Some(MESSY));
        assert_eq!(code(&o), 0, "{}", stderr(&o));
        assert_eq!(stdout(&o), CLEAN);
    }
    let o = reasm(&t.0, &["--check"], Some(MESSY));
    assert_eq!((code(&o), stdout(&o)), (1, "<stdin>\n".to_string()));
}

#[test]
fn walks_directories_by_extension() {
    let t = TempDir::new();
    t.write("src/a.asm", MESSY);
    t.write("src/inc/b.INC", MESSY);
    t.write("src/c.txt", MESSY);
    t.write("src/.hidden/d.asm", MESSY);
    t.write("target/e.asm", MESSY);
    let o = reasm(&t.0, &["--check", "."], None);
    assert_eq!(code(&o), 1);
    let mut listed: Vec<String> = stdout(&o)
        .lines()
        .map(|l| l.replace('\\', "/").trim_start_matches("./").to_string())
        .collect();
    listed.sort();
    assert_eq!(listed, ["src/a.asm", "src/inc/b.INC"]);
}

#[test]
fn config_discovery_and_overrides() {
    let t = TempDir::new();
    t.write("reasm.toml", "line_ending = \"crlf\"\n");
    t.write("sub/a.asm", "mov eax, 1\n");
    let o = reasm(&t.0, &["--stdout", "sub/a.asm"], None);
    assert_eq!(stdout(&o), "        mov eax, 1\r\n");
    let o = reasm(
        &t.0,
        &["--stdout", "--set", "line_ending=lf", "sub/a.asm"],
        None,
    );
    assert_eq!(stdout(&o), "        mov eax, 1\n");
    let o = reasm(&t.0, &["--stdout", "--no-config", "sub/a.asm"], None);
    assert_eq!(stdout(&o), "        mov eax, 1\n");
    // --stdin-filepath drives discovery for stdin.
    let o = reasm(&t.0, &["--stdin-filepath", "sub/x.asm"], Some("nop\n"));
    assert_eq!(stdout(&o), "        nop\r\n");
}

#[test]
fn bad_config_is_a_user_error_with_location() {
    let t = TempDir::new();
    t.write("reasm.toml", "[labels]\nindnet = 4\n");
    t.write("a.asm", CLEAN);
    let o = reasm(&t.0, &["a.asm"], None);
    assert_eq!(code(&o), 2);
    let err = stderr(&o);
    assert!(
        err.contains("reasm.toml") && err.contains("indnet") && err.contains("line 2"),
        "{err}"
    );
}

#[test]
fn ambiguous_config_files() {
    let t = TempDir::new();
    t.write("reasm.toml", "");
    t.write(".reasm.toml", "");
    t.write("a.asm", CLEAN);
    let o = reasm(&t.0, &["a.asm"], None);
    assert_eq!(code(&o), 2);
    assert!(stderr(&o).contains("both reasm.toml and .reasm.toml"));
}

#[test]
fn print_config() {
    let t = TempDir::new();
    let o = reasm(&t.0, &["--print-config", "default"], None);
    assert_eq!(code(&o), 0);
    assert_eq!(stdout(&o), reasm::Config::default().to_toml());

    t.write("reasm.toml", "[comments]\ncolumn = 48\n");
    let o = reasm(
        &t.0,
        &["--print-config", "--set", "indent.instruction=4"],
        None,
    );
    let printed: reasm::Config = toml::from_str(&stdout(&o)).unwrap();
    assert_eq!(printed.comments.column, 48);
    assert_eq!(printed.indent.instruction, 4);
}

#[test]
fn user_errors_exit_2_and_continue() {
    let t = TempDir::new();
    t.write("a.asm", MESSY);
    std::fs::write(t.0.join("bad.asm"), b"mov eax, \xff\n").unwrap();
    let o = reasm(&t.0, &["missing.asm", "bad.asm", "a.asm"], None);
    assert_eq!(code(&o), 2);
    let err = stderr(&o);
    assert!(err.contains("missing.asm: no such file"), "{err}");
    assert!(err.contains("bad.asm: not valid UTF-8"), "{err}");
    assert_eq!(t.read("a.asm"), CLEAN, "other files are still formatted");

    for args in [
        &["--set", "nope"][..],
        &["--dialect", "gas", "a.asm"][..],
        &["--check", "--stdout"][..],
    ] {
        assert_eq!(code(&reasm(&t.0, args, None)), 2, "args {args:?}");
    }
}

#[test]
fn help_and_version() {
    let t = TempDir::new();
    let o = reasm(&t.0, &["--help"], None);
    assert_eq!(code(&o), 0);
    for flag in [
        "--check",
        "--stdout",
        "--stdin-filepath",
        "--config",
        "--no-config",
        "--set",
        "--dialect",
        "--print-config",
        "--allow",
        "--deny-warnings",
        "--quiet",
        "--verbose",
    ] {
        assert!(stdout(&o).contains(flag), "--help is missing {flag}");
    }
    let o = reasm(&t.0, &["--version"], None);
    assert_eq!(
        stdout(&o).trim(),
        format!("reasm {}", env!("CARGO_PKG_VERSION"))
    );
}
