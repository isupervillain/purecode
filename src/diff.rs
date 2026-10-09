use crate::parser::BlobSource;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// What to diff.
pub enum DiffTarget<'a> {
    /// Changes on `head` since it diverged from `base` (`git diff base...head`).
    Refs { base: &'a str, head: &'a str },
    /// Changes staged for the next commit (`git diff --cached`).
    Staged,
}

/// Runs `git diff` in the form the parser expects, regardless of the user's git config.
pub fn get_git_diff(target: DiffTarget) -> io::Result<Box<dyn BufRead>> {
    // Outside a repository `git diff` would fall back to `--no-index` and print its usage;
    // report git's own reason instead (not a repository, `safe.directory` ownership, ...).
    let check = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()?;
    if !check.status.success() {
        return Err(io::Error::other(format!(
            "{} (use --stdin to analyze a diff from elsewhere)",
            String::from_utf8_lossy(&check.stderr).trim()
        )));
    }

    let mut cmd = Command::new("git");
    // `diff.relative` would limit and re-root paths; `--no-relative` needs git 2.28+.
    cmd.args(["-c", "diff.relative=false"]);
    cmd.args([
        "diff",
        "--unified=0",
        "--no-color",
        // An external diff tool, textconv filter or missing/custom path prefixes would change
        // or empty the output and silently pass a CI gate.
        "--no-ext-diff",
        "--no-textconv",
        "--src-prefix=a/",
        "--dst-prefix=b/",
        // Full blob ids let the parser classify changed lines within their whole file.
        "--full-index",
    ]);
    match target {
        DiffTarget::Refs { base, head } => {
            for r in [base, head] {
                // A ref like `--output=/path` from a repo's config would be a git option.
                if r.starts_with('-') || r.is_empty() {
                    return Err(io::Error::other(format!("invalid git ref '{r}'")));
                }
            }
            cmd.arg(format!("{base}...{head}"));
        }
        DiffTarget::Staged => {
            cmd.arg("--cached");
        }
    }
    let output = cmd.arg("--").stderr(Stdio::piped()).output()?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(io::Error::other(err_msg.trim().to_string()));
    }

    Ok(Box::new(io::Cursor::new(output.stdout)))
}

pub fn get_stdin_diff() -> Box<dyn BufRead> {
    Box::new(BufReader::new(io::stdin()))
}

/// Reads blobs from the current repository through one long-lived `git cat-file --batch`.
/// Outside a repository (or for unknown ids) lookups return `None`.
pub struct GitBlobs {
    process: Option<(Child, ChildStdin, BufReader<ChildStdout>)>,
}

impl GitBlobs {
    pub fn new() -> Self {
        let process = Command::new("git")
            .args(["cat-file", "--batch"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()
            .and_then(|mut child| {
                let stdin = child.stdin.take()?;
                let stdout = BufReader::new(child.stdout.take()?);
                Some((child, stdin, stdout))
            });
        Self { process }
    }
}

impl Default for GitBlobs {
    fn default() -> Self {
        Self::new()
    }
}

impl BlobSource for GitBlobs {
    fn blob(&mut self, id: &str) -> Option<Vec<u8>> {
        let (_, stdin, stdout) = self.process.as_mut()?;
        writeln!(stdin, "{id}").ok()?;
        stdin.flush().ok()?;
        // "<oid> <type> <size>\n<content>\n", or "<id> missing\n".
        let mut header = String::new();
        stdout.read_line(&mut header).ok()?;
        let mut parts = header.split_whitespace().skip(1);
        let (kind, size) = (parts.next()?, parts.next()?.parse::<usize>().ok()?);
        let mut content = vec![0; size + 1];
        stdout.read_exact(&mut content).ok()?;
        content.pop();
        (kind == "blob").then_some(content)
    }
}

impl Drop for GitBlobs {
    fn drop(&mut self) {
        if let Some((mut child, stdin, _)) = self.process.take() {
            drop(stdin); // EOF ends cat-file
            let _ = child.wait();
        }
    }
}
