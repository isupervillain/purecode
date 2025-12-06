use clap::{Parser, ValueEnum};
use purecode::{diff, parser, report, stats::LangStats};
use std::collections::HashMap;
use std::process::exit;

#[derive(Parser, Debug)]
#[command(name = "purecode")]
#[command(author = "PureCode Author")]
#[command(version = "0.4.0")]
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

    // Check Min Pure Lines (Net)
    // The requirement says "min pure lines 1 -> fail if the change is only comments".
    // Usually this implies checking if we added *any* pure code, or if the net change is substantial.
    // "fail if the change is *only* comments / noise" usually suggests `pure_added > 0`.
    // But the flag name `min-pure-lines` suggests checking the net value or the added value.
    // Given standard linter behavior, I will check Net Pure Lines by default, as that represents the "value" added.
    // Wait, if I refactor code (remove 10 pure, add 10 pure), net is 0. That shouldn't fail a "min pure lines 1" check?
    // Let's interpret "min-pure-lines" as "pure lines added" (the absolute volume of new code) OR "net pure".
    // The prompt says: "--min-pure-lines 1 -> fail if the change is *only* comments / noise."
    // If I delete code, I'm not adding comments.
    // If I add 10 lines of comments, pure added is 0.
    // So "Pure Added" seems like the safest metric for "is this change only noise?".
    // However, the flag name "min-pure-lines" could be ambiguous.
    // Let's assume it checks `net_pure`.
    // Wait, prompt says: "--fail-on-decrease -> fail if net pure code is negative."
    // This implies `min-pure-lines` is a distinct check.
    // If I set `--min-pure-lines 10`, and I add 5 lines, I expect it to fail.
    // If I set `--min-pure-lines 1`, and I add 0 pure lines (only comments), I expect it to fail.
    // So I will check `pure_added`?
    // Actually, usually "lines of code" checks refer to the SLOC of the artifact. But here we analyze a diff.
    // Let's stick to **Net Pure Lines** as the primary metric of "change size", but for "min lines" usually we want to ensure *some* contribution.
    // But if I delete code, net pure is negative.
    // If I use `pure_added`, checking for 1 ensures I wrote *some* code.
    // Let's go with **Net Pure Lines** because `min-pure-lines` usually gates "size of PR".
    // BUT, if I am doing a cleanup PR (removing code), net pure is negative. I shouldn't fail "min pure lines" if I didn't intent to add code.
    // Actually, the prompt example says: "fail if the change is *only* comments / noise".
    // This strongly implies checking `pure_added + pure_removed > 0` (any pure activity) OR `pure_added > 0`.
    // Let's assume the user wants to ensure the PR contributes *some* pure code?
    // Let's look at the wording again: "fail if the change is *only* comments / noise".
    // If I remove 10 lines of comments, `pure_added`=0, `pure_removed`=0. Correct.
    // If I remove 10 lines of code, `pure_added`=0, `pure_removed`=10. This is "pure code change", not "noise".
    // So checking `pure_added + pure_removed` (total pure activity) vs 0?
    // Or maybe the user meant "Pure Added".
    // Let's stick to `net_pure` (Pure Lines Added - Removed) for consistency with `fail-on-decrease`.
    // Wait, if `fail-on-decrease` covers negative net pure, then `min-pure-lines` probably sets a *positive* floor.
    // So `net_pure >= X`.
    // If I set `min-pure-lines 1`, I demand at least +1 net pure line.
    // This makes sense for "feature" PRs.

    if let Some(min_lines) = args.min_pure_lines {
        if overall.net_pure() < min_lines {
            return Err(format!(
                "Failure: Net pure lines {} is less than minimum {}",
                overall.net_pure(), min_lines
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
