mod classifier;
mod diff;
mod parser;
mod report;
mod stats;

use clap::Parser;
use std::collections::HashMap;
use std::process::exit;

#[derive(Parser, Debug)]
#[command(name = "purecode")]
#[command(author = "PureCode Author")]
#[command(version = "1.0")]
#[command(about = "Analyzes git diffs to count pure code vs noise", long_about = None)]
struct Cli {
    /// Base ref for git diff
    #[arg(long, default_value = "origin/main")]
    base: String,

    /// Head ref for git diff
    #[arg(long, default_value = "HEAD")]
    head: String,

    /// Read unified diff from stdin instead of running git
    #[arg(long)]
    stdin: bool,
}

fn main() {
    let args = Cli::parse();

    let reader: Box<dyn std::io::BufRead> = if args.stdin {
        diff::get_stdin_diff()
    } else {
        match diff::get_git_diff(&args.base, &args.head) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Error running git diff: {}", e);
                exit(1);
            }
        }
    };

    let mut stats = HashMap::new();
    if let Err(e) = parser::parse_diff(reader, &mut stats) {
        eprintln!("Error parsing diff: {}", e);
        exit(1);
    }

    report::print_report(&stats);
}
