use std::io::{self, BufReader};
use std::process::{Command, Stdio};

pub fn get_git_diff(base: &str, head: &str) -> io::Result<Box<dyn std::io::BufRead>> {
    let output = Command::new("git")
        .args([
            "diff",
            &format!("{}...{}", base, head),
            "--unified=0",
            "--no-color",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let output = output.wait_with_output()?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(io::Error::other(format!("git diff failed: {}", err_msg)));
    }

    let cursor = std::io::Cursor::new(output.stdout);
    Ok(Box::new(BufReader::new(cursor)))
}

pub fn get_stdin_diff() -> Box<dyn std::io::BufRead> {
    Box::new(BufReader::new(io::stdin()))
}
