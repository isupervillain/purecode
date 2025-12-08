use crate::stats::{calculate_complexity, estimate_tokens, AnalysisResult, FileStats, LangStats};
use colored::*;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Plain,
    Json,
}

pub fn print_report(
    stats: &[FileStats],
    format: OutputFormat,
    per_file: bool,
    mode: &str,
    ci: bool,
) {
    let mut overall = LangStats::default();
    let mut lang_map: HashMap<String, LangStats> = HashMap::new();

    for file in stats {
        // Aggregate overall
        overall.total_added += file.lang_stats.total_added;
        overall.total_removed += file.lang_stats.total_removed;
        overall.pure_added += file.lang_stats.pure_added;
        overall.pure_removed += file.lang_stats.pure_removed;
        overall.comment_lines_added += file.lang_stats.comment_lines_added;
        overall.comment_lines_removed += file.lang_stats.comment_lines_removed;
        overall.docstring_lines_added += file.lang_stats.docstring_lines_added;
        overall.docstring_lines_removed += file.lang_stats.docstring_lines_removed;
        overall.blank_lines_added += file.lang_stats.blank_lines_added;
        overall.blank_lines_removed += file.lang_stats.blank_lines_removed;
        overall.code_words_added += file.lang_stats.code_words_added;
        overall.code_words_removed += file.lang_stats.code_words_removed;

        // Aggregate per language
        let entry = lang_map.entry(file.language.clone()).or_default();
        entry.total_added += file.lang_stats.total_added;
        entry.total_removed += file.lang_stats.total_removed;
        entry.pure_added += file.lang_stats.pure_added;
        entry.pure_removed += file.lang_stats.pure_removed;
        entry.comment_lines_added += file.lang_stats.comment_lines_added;
        entry.comment_lines_removed += file.lang_stats.comment_lines_removed;
        entry.docstring_lines_added += file.lang_stats.docstring_lines_added;
        entry.docstring_lines_removed += file.lang_stats.docstring_lines_removed;
        entry.blank_lines_added += file.lang_stats.blank_lines_added;
        entry.blank_lines_removed += file.lang_stats.blank_lines_removed;
        entry.code_words_added += file.lang_stats.code_words_added;
        entry.code_words_removed += file.lang_stats.code_words_removed;
    }

    let complexity = calculate_complexity(&overall);
    let token_estimate = estimate_tokens(overall.code_words_added);

    match format {
        OutputFormat::Json => {
            let result = AnalysisResult {
                summary: overall,
                language_stats: lang_map,
                file_stats: if per_file { Some(stats.to_vec()) } else { None },
                complexity_score: complexity,
                token_estimate,
                mode: mode.to_string(),
            };
            if let Ok(json) = serde_json::to_string_pretty(&result) {
                println!("{}", json);
            }
        }
        OutputFormat::Human | OutputFormat::Plain => {
            let use_color = !ci && format == OutputFormat::Human;

            if use_color {
                print_human_report(
                    stats,
                    &overall,
                    &lang_map,
                    per_file,
                    complexity,
                    token_estimate,
                );
            } else {
                print_plain_report(
                    stats,
                    &overall,
                    &lang_map,
                    per_file,
                    complexity,
                    token_estimate,
                );
            }
        }
    }

    if ci {
        // Print summary line
        let total_changes = overall.total_added + overall.total_removed;
        let pure_changes = overall.pure_added + overall.pure_removed;
        let noise_ratio = if total_changes > 0 {
            1.0 - (pure_changes as f64 / total_changes as f64)
        } else {
            0.0
        };

        println!("PURECODE_SUMMARY noise_ratio={:.2} pure_added={} pure_removed={} files_changed={} complexity={:.2}",
            noise_ratio,
            overall.pure_added,
            overall.pure_removed,
            stats.len(),
            complexity
        );
    }
}

fn print_human_report(
    files: &[FileStats],
    overall: &LangStats,
    lang_map: &HashMap<String, LangStats>,
    per_file: bool,
    complexity: f64,
    tokens: u64,
) {
    println!("{}", "PureCode Analysis Report".bold().underline());
    println!("Total Files: {}", files.len());
    println!("Net Pure Lines: {}", overall.net_pure().to_string().cyan());
    println!(
        "Review Complexity: {:.1} ({})",
        complexity,
        complexity_bucket(complexity)
    );
    println!("Estimated Tokens (Added): {}", tokens);

    println!("\n{}", "Language Breakdown:".bold());
    let mut sorted_langs: Vec<_> = lang_map.iter().collect();
    sorted_langs.sort_by_key(|(k, _)| *k);

    for (lang, stat) in sorted_langs {
        println!(
            "  {:<12} | Pure: {:>4} | Added: {:>4} | Removed: {:>4} | Noise: {:>4}",
            lang.blue(),
            stat.net_pure(),
            stat.pure_added.to_string().green(),
            stat.pure_removed.to_string().red(),
            (stat.noise_added() + stat.noise_removed())
        );
    }

    if per_file {
        println!("\n{}", "File Details:".bold());
        for file in files {
            println!(
                "  {:<30} [{}] | Pure: {:>3}",
                file.path,
                file.language.yellow(),
                file.lang_stats.net_pure()
            );
        }
    }
    println!();
}

fn print_plain_report(
    files: &[FileStats],
    overall: &LangStats,
    lang_map: &HashMap<String, LangStats>,
    per_file: bool,
    complexity: f64,
    tokens: u64,
) {
    println!("PureCode Analysis Report");
    println!("Total Files: {}", files.len());
    println!("Net Pure Lines: {}", overall.net_pure());
    println!(
        "Review Complexity: {:.1} ({})",
        complexity,
        complexity_bucket(complexity)
    );
    println!("Estimated Tokens (Added): {}", tokens);

    println!("\nLanguage Breakdown:");
    let mut sorted_langs: Vec<_> = lang_map.iter().collect();
    sorted_langs.sort_by_key(|(k, _)| *k);

    for (lang, stat) in sorted_langs {
        println!(
            "  {:<12} | Pure: {:>4} | Added: {:>4} | Removed: {:>4} | Noise: {:>4}",
            lang,
            stat.net_pure(),
            stat.pure_added,
            stat.pure_removed,
            (stat.noise_added() + stat.noise_removed())
        );
    }

    if per_file {
        println!("\nFile Details:");
        for file in files {
            println!(
                "  {:<30} [{}] | Pure: {:>3}",
                file.path,
                file.language,
                file.lang_stats.net_pure()
            );
        }
    }
    println!();
}

fn complexity_bucket(score: f64) -> &'static str {
    if score < 50.0 {
        "light"
    } else if score < 200.0 {
        "medium"
    } else {
        "heavy"
    }
}
