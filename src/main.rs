use clap::{Parser, ValueEnum};
use purecode::{diff, parser, report};
use std::collections::HashMap;
use std::process::exit;

#[derive(Parser, Debug)]
#[command(name = "purecode")]
#[command(author = "PureCode Author")]
#[command(version = "0.3.0")]
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

    /// Output format
    #[arg(long, value_enum, default_value_t = Format::Human)]
    format: Format,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum Format {
    Human,
    Plain,
    Json,
}

impl From<Format> for report::OutputFormat {
    fn from(f: Format) -> Self {
        match f {
            Format::Human => report::OutputFormat::Human,
            Format::Plain => report::OutputFormat::Plain,
            Format::Json => report::OutputFormat::Json,
        }
    }
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

    report::print_report(&stats, args.format.into());
}
