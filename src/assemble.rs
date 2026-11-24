//! Assembly equivalence for `--verify` and the golden tests (CLI-7, COR-1,
//! ADR-0023): assemble the original and the formatted text with `nasm` and
//! compare the output bytes.
//!
//! Each text is written under the same file name into its own temporary
//! directory, and nasm runs there, so both object files embed the same
//! source name. Includes are found through `-I` for the file's own
//! directory, the working directory, and any extra directories, in that
//! order, which is where nasm would have looked from the file's directory.

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

/// Output formats tried, in order, when no format is given. `bin` covers
/// boot sectors and other flat binaries that `elf64` rejects.
pub const DEFAULT_FORMATS: &[&str] = &["elf64", "bin"];

/// The result of one comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Both assembled with `format` to identical bytes.
    Same { format: String },
    /// Both assembled, but the bytes differ starting at `offset`.
    Differs { format: String, offset: usize },
    /// The original assembled but the formatted text didn't.
    FormattedFails { format: String, message: String },
    /// The original doesn't assemble on its own with any format tried, so
    /// nothing can be compared. `message` is nasm's first error line.
    OriginalFails { message: String },
}

/// How to run nasm.
#[derive(Debug, Clone)]
pub struct Assembler {
    pub program: OsString,
    /// Formats to try in order; the first one the original assembles with
    /// is used for both texts.
    pub formats: Vec<String>,
    /// Extra include directories, searched after the file's directory and
    /// the working directory.
    pub include_dirs: Vec<PathBuf>,
}

impl Assembler {
    /// nasm from PATH with the given formats (or [`DEFAULT_FORMATS`]).
    pub fn new(format: Option<&str>) -> Self {
        Self {
            program: "nasm".into(),
            formats: match format {
                Some(f) => vec![f.to_string()],
                None => DEFAULT_FORMATS.iter().map(|f| f.to_string()).collect(),
            },
            include_dirs: Vec::new(),
        }
    }

    /// Checks that nasm can be run; returns its version line.
    pub fn probe(&self) -> io::Result<String> {
        let out = Command::new(&self.program)
            .arg("-v")
            .stdin(Stdio::null())
            .output()?;
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    /// Assembles `original` and `formatted` as if they were the file `path`
    /// (or `input.asm` for stdin) and compares the results.
    pub fn compare(
        &self,
        original: &str,
        formatted: &str,
        path: Option<&Path>,
    ) -> io::Result<Verdict> {
        let name = path
            .and_then(Path::file_name)
            .unwrap_or(OsStr::new("input.asm"));
        let mut includes = Vec::new();
        let file_dir = match path.and_then(Path::parent) {
            Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
            _ => PathBuf::from("."),
        };
        let cwd = std::env::current_dir()?;
        for dir in [file_dir, cwd.clone()].iter().chain(&self.include_dirs) {
            includes.push(dir_arg(&cwd.join(dir)));
        }

        let tmp = TempDir::new()?;
        let (a, b) = (tmp.0.join("a"), tmp.0.join("b"));
        for (dir, text) in [(&a, original), (&b, formatted)] {
            std::fs::create_dir(dir)?;
            std::fs::write(dir.join(name), text)?;
        }
        let mut first_error = String::new();
        for format in &self.formats {
            let ra = self.run(&a, name, format, &includes)?;
            let bytes_a = match ra {
                Ok(bytes) => bytes,
                Err(msg) => {
                    if first_error.is_empty() {
                        first_error = msg;
                    }
                    continue;
                }
            };
            return Ok(match self.run(&b, name, format, &includes)? {
                Err(message) => Verdict::FormattedFails {
                    format: format.clone(),
                    message,
                },
                Ok(bytes_b) if bytes_a == bytes_b => Verdict::Same {
                    format: format.clone(),
                },
                Ok(bytes_b) => Verdict::Differs {
                    format: format.clone(),
                    offset: bytes_a
                        .iter()
                        .zip(&bytes_b)
                        .position(|(x, y)| x != y)
                        .unwrap_or(bytes_a.len().min(bytes_b.len())),
                },
            });
        }
        Ok(Verdict::OriginalFails {
            message: first_error,
        })
    }

    /// Runs nasm in `dir`; returns the output bytes or nasm's first error.
    fn run(
        &self,
        dir: &Path,
        name: &OsStr,
        format: &str,
        includes: &[OsString],
    ) -> io::Result<Result<Vec<u8>, String>> {
        let out_file = dir.join("reasm-verify.out");
        let out = Command::new(&self.program)
            .current_dir(dir)
            .arg("-f")
            .arg(format)
            .args(includes)
            .arg("-o")
            .arg(&out_file)
            .arg(name)
            .stdin(Stdio::null())
            .output()?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let line = stderr
                .lines()
                .find(|l| l.contains("error"))
                .or_else(|| stderr.lines().next())
                .unwrap_or("nasm failed")
                .to_string();
            return Ok(Err(line));
        }
        let bytes = std::fs::read(&out_file)?;
        std::fs::remove_file(&out_file)?;
        Ok(Ok(bytes))
    }
}

/// `-I<dir>/`: nasm before 2.14 needs the trailing separator.
fn dir_arg(dir: &Path) -> OsString {
    let mut arg = OsString::from("-I");
    arg.push(dir.as_os_str());
    if !dir.as_os_str().to_string_lossy().ends_with(['/', '\\']) {
        arg.push(std::path::MAIN_SEPARATOR_STR);
    }
    arg
}

/// A fresh directory under the system temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> io::Result<Self> {
        static N: AtomicU32 = AtomicU32::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("reasm-verify-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        Ok(Self(dir))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nasm() -> Option<Assembler> {
        let a = Assembler::new(None);
        a.probe().ok().map(|_| a)
    }

    #[test]
    fn dir_arg_adds_separator() {
        let sep = std::path::MAIN_SEPARATOR_STR;
        assert_eq!(
            dir_arg(Path::new("inc")),
            OsString::from(format!("-Iinc{sep}"))
        );
        assert_eq!(dir_arg(Path::new("inc/")), OsString::from("-Iinc/"));
    }

    #[test]
    fn verdicts() {
        let Some(nasm) = nasm() else {
            eprintln!("nasm not found; skipping");
            return;
        };
        let v = nasm
            .compare("mov eax,1\n", "        mov     eax, 1\n", None)
            .unwrap();
        assert_eq!(
            v,
            Verdict::Same {
                format: "elf64".into()
            }
        );
        let v = nasm.compare("mov eax,1\n", "mov eax,2\n", None).unwrap();
        assert!(matches!(v, Verdict::Differs { .. }), "{v:?}");
        let v = nasm.compare("mov eax,1\n", "mov eax,\n", None).unwrap();
        assert!(matches!(v, Verdict::FormattedFails { .. }), "{v:?}");
        let v = nasm.compare("mov eax,\n", "mov eax,\n", None).unwrap();
        assert!(matches!(v, Verdict::OriginalFails { .. }), "{v:?}");
        // elf64 rejects `org`; bin is tried next.
        let v = nasm
            .compare("org 0x7c00\njmp $\n", "org 0x7c00\njmp $\n", None)
            .unwrap();
        assert_eq!(
            v,
            Verdict::Same {
                format: "bin".into()
            }
        );
        // A forced format is the only one tried.
        let elf = Assembler {
            formats: vec!["elf64".into()],
            ..nasm.clone()
        };
        let v = elf.compare("org 0x7c00\n", "org 0x7c00\n", None).unwrap();
        assert!(matches!(v, Verdict::OriginalFails { .. }), "{v:?}");
    }

    #[test]
    fn includes_resolve_from_the_files_directory() {
        let Some(nasm) = nasm() else {
            return;
        };
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.0.join("src")).unwrap();
        std::fs::write(tmp.0.join("src/defs.inc"), "%define VAL 7\n").unwrap();
        let file = tmp.0.join("src/main.asm");
        let src = "%include \"defs.inc\"\nmov eax, VAL\n";
        let v = nasm.compare(src, src, Some(&file)).unwrap();
        assert_eq!(
            v,
            Verdict::Same {
                format: "elf64".into()
            }
        );
        let v = nasm
            .compare(src, "%include \"defs.inc\"\nmov eax, 8\n", Some(&file))
            .unwrap();
        assert!(matches!(v, Verdict::Differs { .. }), "{v:?}");
    }
}
