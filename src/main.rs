use clap::{Parser, ValueEnum};
use purecode::{diff, parser, report, stats::LangStats};
use std::collections::HashMap;
use std::process::exit;

#[derive(Parser, Debug)]
#[command(name = "purecode")]
#[command(author = "PureCode Author")]
#[command(version = "0.1.0")]
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

    /// Fail if noise ratio (comments/blanks) is greater than this value (0.0 - 1.0)
    #[arg(long)]
    max_noise_ratio: Option<f64>,

    /// Fail if the number of net pure lines is less than this value
    #[arg(long)]
    min_pure_lines: Option<i64>,

    /// Fail if the net pure code change is negative
    #[arg(long)]
    fail_on_decrease: bool,
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

    if let Err(e) = check_thresholds(&stats, &args) {
        eprintln!("{}", e);
        exit(1);
    }
}

fn check_thresholds(stats: &HashMap<String, LangStats>, args: &Cli) -> Result<(), String> {
    // Aggregate overall stats
    let mut overall = LangStats::default();
    for s in stats.values() {
        overall.total_added += s.total_added;
        overall.total_removed += s.total_removed;
        overall.pure_added += s.pure_added;
        overall.pure_removed += s.pure_removed;
    }

    // Check Max Noise Ratio
    if let Some(max_ratio) = args.max_noise_ratio {
        let total_changes = overall.total_added + overall.total_removed;
        if total_changes > 0 {
            let pure_changes = overall.pure_added + overall.pure_removed;
            let pure_ratio = pure_changes as f64 / total_changes as f64;
            let noise_ratio = 1.0 - pure_ratio;

            if noise_ratio > max_ratio {
                return Err(format!(
                    "Failure: Noise ratio {:.2} exceeds limit {:.2}",
                    noise_ratio, max_ratio
                ));
            }
        }
    }

    // Check Min Pure Lines (Total touched)
    // Treat "pure lines" as the amount of pure code touched (added or removed),
    // so cleanup PRs that delete code still count toward the threshold.
    if let Some(min_lines) = args.min_pure_lines {
        let pure_touched = overall.pure_added + overall.pure_removed;
        if pure_touched < min_lines {
            return Err(format!(
                "Failure: Pure lines touched {} is less than minimum {}",
                pure_touched, min_lines
            ));
        }
    }

    // Check Fail on Decrease
    if args.fail_on_decrease {
        if overall.net_pure() < 0 {
            return Err(format!(
                "Failure: Net pure code decreased ({})",
                overall.net_pure()
            ));
        }
    }

    Ok(())
}
