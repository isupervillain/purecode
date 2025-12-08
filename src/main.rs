use clap::{Parser, Subcommand, ValueEnum};
use purecode::{
    config, diff, files, parser, report,
    stats::{FileStats, LangStats, ThresholdError},
};
use std::io::BufReader;
use std::process::exit;

#[derive(Parser, Debug)]
#[command(name = "purecode")]
#[command(author = "PureCode Author")]
#[command(version = "0.2.0")]
#[command(about = "Analyzes code to count pure code vs noise", long_about = None)]
#[command(args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    // Fallback flags for root command (diff mode)
    /// Base ref for git diff
    #[arg(long)]
    base: Option<String>,

    /// Head ref for git diff
    #[arg(long)]
    head: Option<String>,

    /// Read unified diff from stdin instead of running git
    #[arg(long)]
    stdin: bool,

    /// Output format
    #[arg(long, value_enum)]
    format: Option<Format>,

    /// Show per-file statistics
    #[arg(long)]
    per_file: bool,

    /// Fail if noise ratio (comments/blanks) is greater than this value (0.0 - 1.0)
    #[arg(long)]
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
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Analyze git diffs
    Diff {
        /// Base ref for git diff
        #[arg(long, default_value = "origin/main")]
        base: String,

        /// Head ref for git diff
        #[arg(long, default_value = "HEAD")]
        head: String,

        /// Read unified diff from stdin
        #[arg(long)]
        stdin: bool,

        /// Output format
        #[arg(long, value_enum)]
        format: Option<Format>,

        /// Show per-file statistics
        #[arg(long)]
        per_file: bool,

        /// Fail if noise ratio (comments/blanks) is greater than this value (0.0 - 1.0)
        #[arg(long)]
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

        /// CI mode
        #[arg(long)]
        ci: bool,
    },
    /// Analyze files/directories (Snapshot mode)
    Files {
        /// Paths to include (defaults to all)
        #[arg(default_value = ".")]
        paths: Vec<String>,

        /// Read file list from stdin
        #[arg(long)]
        stdin: bool,

        /// Output format
        #[arg(long, value_enum)]
        format: Option<Format>,

        /// Show per-file statistics
        #[arg(long)]
        per_file: bool,

        /// Fail if noise ratio (comments/blanks) is greater than this value (0.0 - 1.0)
        #[arg(long)]
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

        /// CI mode
        #[arg(long)]
        ci: bool,
    },
    /// History analysis (Scaffolding)
    History,
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

fn resolve_format(cli_format: Option<Format>, config_format: &str) -> Format {
    if let Some(f) = cli_format {
        f
    } else {
        match config_format {
            "json" => Format::Json,
            "plain" => Format::Plain,
            _ => Format::Human,
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let config = config::load_config();

    // Determine mode and arguments
    let (stats, mode, active_config) = match cli.command {
        Some(Commands::History) => {
            println!("History analysis not implemented yet.");
            exit(0);
        }
        Some(Commands::Files {
            paths,
            stdin,
            format,
            per_file,
            max_noise_ratio,
            min_pure_lines,
            fail_on_decrease,
            warn_only,
            ci,
        }) => {
            let final_format = resolve_format(format, &config.format);

            let _files_to_scan = if paths == vec!["."] {
                vec![".".to_string()]
            } else {
                paths
            };

            let include = if config.include.is_empty() {
                vec!["**/*".to_string()]
            } else {
                config.include.clone()
            };
            let exclude = config.exclude.clone();

            let reader: Option<Box<dyn std::io::BufRead>> = if stdin {
                Some(Box::new(BufReader::new(std::io::stdin())))
            } else {
                None
            };

            let stats = match files::analyze_files(&include, &exclude, reader) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error analyzing files: {}", e);
                    exit(1);
                }
            };

            (
                stats,
                "snapshot",
                FilesConfig {
                    format: final_format,
                    per_file,
                    max_noise_ratio: max_noise_ratio.or(config.max_noise_ratio),
                    min_pure_lines: min_pure_lines.or(config.min_pure_lines),
                    fail_on_decrease: fail_on_decrease || config.fail_on_decrease,
                    warn_only: warn_only || config.warn_only,
                    ci: ci || config.ci,
                },
            )
        }
        Some(Commands::Diff {
            base,
            head,
            stdin,
            format,
            per_file,
            max_noise_ratio,
            min_pure_lines,
            fail_on_decrease,
            warn_only,
            ci,
        }) => {
            let final_format = resolve_format(format, &config.format);

            let reader: Box<dyn std::io::BufRead> = if stdin {
                diff::get_stdin_diff()
            } else {
                match diff::get_git_diff(&base, &head) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Error running git diff: {}", e);
                        exit(1);
                    }
                }
            };

            let mut file_stats = Vec::new();
            if let Err(e) = parser::parse_diff(reader, &mut file_stats) {
                eprintln!("Error parsing diff: {}", e);
                exit(1);
            }

            (
                file_stats,
                "diff",
                FilesConfig {
                    format: final_format,
                    per_file,
                    max_noise_ratio: max_noise_ratio.or(config.max_noise_ratio),
                    min_pure_lines: min_pure_lines.or(config.min_pure_lines),
                    fail_on_decrease: fail_on_decrease || config.fail_on_decrease,
                    warn_only: warn_only || config.warn_only,
                    ci: ci || config.ci,
                },
            )
        }
        None => {
            // Root command fallback
            let base = cli.base.unwrap_or(config.base);
            let head = cli.head.unwrap_or("HEAD".to_string());
            let format = resolve_format(cli.format, &config.format);

            let reader: Box<dyn std::io::BufRead> = if cli.stdin {
                diff::get_stdin_diff()
            } else {
                match diff::get_git_diff(&base, &head) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Error running git diff: {}", e);
                        exit(1);
                    }
                }
            };

            let mut file_stats = Vec::new();
            if let Err(e) = parser::parse_diff(reader, &mut file_stats) {
                eprintln!("Error parsing diff: {}", e);
                exit(1);
            }

            (
                file_stats,
                "diff",
                FilesConfig {
                    format,
                    per_file: cli.per_file,
                    max_noise_ratio: cli.max_noise_ratio.or(config.max_noise_ratio),
                    min_pure_lines: cli.min_pure_lines.or(config.min_pure_lines),
                    fail_on_decrease: cli.fail_on_decrease || config.fail_on_decrease,
                    warn_only: cli.warn_only || config.warn_only,
                    ci: cli.ci || config.ci,
                },
            )
        }
    };

    report::print_report(
        &stats,
        active_config.format.into(),
        active_config.per_file,
        mode,
        active_config.ci,
    );

    if let Err(e) = check_thresholds(&stats, &active_config) {
        // Print fail summary for CI
        if active_config.ci {
            println!(
                "PURECODE_FAIL reason={} {}",
                error_reason(&e),
                error_details(&e)
            );
        }

        eprintln!("{}", e);
        if !active_config.warn_only {
            exit(2);
        }
    } else {
        // Success summary is printed in print_report if CI mode
    }
}

struct FilesConfig {
    format: Format,
    per_file: bool,
    max_noise_ratio: Option<f64>,
    min_pure_lines: Option<i64>,
    fail_on_decrease: bool,
    warn_only: bool,
    ci: bool,
}

fn check_thresholds(file_stats: &[FileStats], args: &FilesConfig) -> Result<(), ThresholdError> {
    let mut overall = LangStats::default();
    for s in file_stats {
        overall.total_added += s.lang_stats.total_added;
        overall.total_removed += s.lang_stats.total_removed;
        overall.pure_added += s.lang_stats.pure_added;
        overall.pure_removed += s.lang_stats.pure_removed;
        overall.comment_lines_added += s.lang_stats.comment_lines_added;
        overall.docstring_lines_added += s.lang_stats.docstring_lines_added;
        overall.blank_lines_added += s.lang_stats.blank_lines_added;
    }

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
            format!("noise_ratio={:.2} max_noise_ratio={:.2}", actual, max)
        }
        ThresholdError::MinPureLines { actual, min } => {
            format!("net_pure_lines={} min_pure_lines={}", actual, min)
        }
        ThresholdError::PureLinesDecreased { actual } => format!("net_pure_lines={}", actual),
    }
}
