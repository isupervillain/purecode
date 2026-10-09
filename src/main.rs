use clap::{Args, Parser, Subcommand, ValueEnum};
use purecode::{
    config::{self, Config, PathFilter},
    diff::{self, DiffTarget},
    files, parser, report,
    stats::{self, FileStats, ThresholdError},
};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(name = "purecode")]
#[command(version)]
#[command(about = "Analyzes code to count pure code vs noise", long_about = None)]
#[command(args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[command(flatten)]
    diff: DiffArgs,

    #[command(flatten)]
    report: ReportArgs,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Analyze git diffs
    Diff {
        #[command(flatten)]
        diff: DiffArgs,

        #[command(flatten)]
        report: ReportArgs,
    },
    /// Analyze files/directories (Snapshot mode)
    Files {
        /// Paths to include (defaults to all)
        #[arg(default_value = ".")]
        paths: Vec<String>,

        /// Read file list from stdin
        #[arg(long)]
        stdin: bool,

        #[command(flatten)]
        report: ReportArgs,
    },
}

#[derive(Args, Debug)]
struct DiffArgs {
    /// Base ref for git diff [default: config `base`, else origin/main]
    #[arg(long)]
    base: Option<String>,

    /// Head ref for git diff [default: HEAD]
    #[arg(long)]
    head: Option<String>,

    /// Read unified diff from stdin instead of running git
    #[arg(long, conflicts_with_all = ["base", "head"])]
    stdin: bool,

    /// Analyze changes staged for commit (for pre-commit hooks)
    #[arg(long, conflicts_with_all = ["base", "head", "stdin"])]
    staged: bool,
}

#[derive(Args, Debug)]
struct ReportArgs {
    /// Output format
    #[arg(long, value_enum)]
    format: Option<Format>,

    /// Show per-file statistics
    #[arg(long)]
    per_file: bool,

    /// Fail if noise ratio (comments/blanks) is greater than this value (0.0 - 1.0)
    #[arg(long, value_parser = parse_ratio)]
    max_noise_ratio: Option<f64>,

    /// Fail if the net pure lines is less than this value
    #[arg(long)]
    min_pure_lines: Option<i64>,

    /// Fail if the net pure code change is negative
    #[arg(long)]
    fail_on_decrease: bool,

    /// Only warn on threshold failures
    #[arg(long)]
    warn_only: bool,

    /// CI mode (no colors, summary output)
    #[arg(long)]
    ci: bool,

    /// Ignore .purecode.toml (for gates that must not be relaxed by the analyzed change)
    #[arg(long)]
    no_config: bool,
}

fn parse_ratio(s: &str) -> Result<f64, String> {
    let value: f64 = s.parse().map_err(|e| format!("{e}"))?;
    config::check_ratio(value)
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

/// Report and threshold settings after merging CLI flags over the config file.
struct Settings {
    format: Format,
    per_file: bool,
    max_noise_ratio: Option<f64>,
    min_pure_lines: Option<i64>,
    fail_on_decrease: bool,
    warn_only: bool,
    ci: bool,
}

impl Settings {
    fn merge(args: ReportArgs, config: &Config) -> Self {
        Self {
            format: args.format.unwrap_or(match config.format.as_str() {
                "json" => Format::Json,
                "plain" => Format::Plain,
                _ => Format::Human,
            }),
            per_file: args.per_file,
            max_noise_ratio: args.max_noise_ratio.or(config.max_noise_ratio),
            min_pure_lines: args.min_pure_lines.or(config.min_pure_lines),
            fail_on_decrease: args.fail_on_decrease || config.fail_on_decrease,
            warn_only: args.warn_only || config.warn_only,
            ci: args.ci || config.ci,
        }
    }
}

/// Parses the selected diff and keeps the files the config's include/exclude select
/// (matched against repository-relative paths).
fn diff_stats(
    args: &DiffArgs,
    config: &Config,
    filter: &PathFilter,
) -> Result<Vec<FileStats>, String> {
    let reader = if args.stdin {
        diff::get_stdin_diff()
    } else {
        let target = if args.staged {
            DiffTarget::Staged
        } else {
            DiffTarget::Refs {
                base: args.base.as_deref().unwrap_or(&config.base),
                head: args.head.as_deref().unwrap_or("HEAD"),
            }
        };
        diff::get_git_diff(target).map_err(|e| format!("Error running git diff: {e}"))?
    };

    let mut stats = Vec::new();
    parser::parse_diff(reader, &mut stats, &mut diff::GitBlobs::new())
        .map_err(|e| format!("Error parsing diff: {e}"))?;
    stats.retain(|f| filter.selects(Path::new(&f.path)));
    Ok(stats)
}

fn run() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let no_config = match &cli.command {
        Some(Commands::Diff { report, .. } | Commands::Files { report, .. }) => report.no_config,
        None => cli.report.no_config,
    };
    let (config, project_root) = if no_config {
        (Config::default(), std::env::current_dir()?)
    } else {
        config::load_config()?
    };
    let filter = PathFilter::new(&config.include, &config.exclude)?;

    let (stats, mode, report_args) = match cli.command {
        Some(Commands::Files {
            paths,
            stdin,
            report,
        }) => {
            let reader =
                stdin.then(|| Box::new(BufReader::new(std::io::stdin())) as Box<dyn BufRead>);
            let stats = files::analyze_files(&paths, &filter, &project_root, reader)
                .map_err(|e| format!("Error analyzing files: {e}"))?;
            (stats, "snapshot", report)
        }
        Some(Commands::Diff { diff, report }) => {
            (diff_stats(&diff, &config, &filter)?, "diff", report)
        }
        None => (diff_stats(&cli.diff, &config, &filter)?, "diff", cli.report),
    };
    let settings = Settings::merge(report_args, &config);

    report::print_report(
        &stats,
        settings.format.into(),
        settings.per_file,
        mode,
        settings.ci,
    );

    if let Err(e) = check_thresholds(&stats, &settings) {
        // JSON output stays a single valid document; the reason goes to stderr below.
        if settings.ci && settings.format != Format::Json {
            println!(
                "PURECODE_FAIL reason={} {}",
                error_reason(&e),
                error_details(&e)
            );
        }

        eprintln!("{e}");
        if !settings.warn_only {
            return Ok(ExitCode::from(2));
        }
    }

    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            // Errors can quote config values and git output from an untrusted repository.
            eprintln!("{}", report::printable(&e.to_string()));
            ExitCode::from(1)
        }
    }
}

fn check_thresholds(file_stats: &[FileStats], args: &Settings) -> Result<(), ThresholdError> {
    let overall = stats::aggregate_stats(file_stats);

    if let Some(max_ratio) = args.max_noise_ratio {
        let total_changes = overall.total_added + overall.total_removed;
        if total_changes > 0 {
            let pure_changes = overall.pure_added + overall.pure_removed;
            let pure_ratio = pure_changes as f64 / total_changes as f64;
            let noise_ratio = 1.0 - pure_ratio;

            if noise_ratio > max_ratio {
                return Err(ThresholdError::NoiseRatioExceeded {
                    actual: noise_ratio,
                    max: max_ratio,
                });
            }
        }
    }

    if let Some(min_lines) = args.min_pure_lines {
        if overall.net_pure() < min_lines {
            return Err(ThresholdError::MinPureLines {
                actual: overall.net_pure(),
                min: min_lines,
            });
        }
    }

    if args.fail_on_decrease && overall.net_pure() < 0 {
        return Err(ThresholdError::PureLinesDecreased {
            actual: overall.net_pure(),
        });
    }

    Ok(())
}

fn error_reason(e: &ThresholdError) -> &'static str {
    match e {
        ThresholdError::NoiseRatioExceeded { .. } => "noise_ratio_exceeded",
        ThresholdError::MinPureLines { .. } => "min_pure_lines_not_met",
        ThresholdError::PureLinesDecreased { .. } => "pure_lines_decreased",
    }
}

fn error_details(e: &ThresholdError) -> String {
    match e {
        ThresholdError::NoiseRatioExceeded { actual, max } => {
            format!("noise_ratio={actual:.2} max_noise_ratio={max:.2}")
        }
        ThresholdError::MinPureLines { actual, min } => {
            format!("net_pure_lines={actual} min_pure_lines={min}")
        }
        ThresholdError::PureLinesDecreased { actual } => {
            format!("net_pure_lines={actual}")
        }
    }
}
