use crate::stats::LangStats;
use colored::Colorize;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Plain,
    Json,
}

pub fn print_report(stats: &HashMap<String, LangStats>, format: OutputFormat) {
    match format {
        OutputFormat::Human => print_human(stats, true, true),
        OutputFormat::Plain => print_human(stats, false, false),
        OutputFormat::Json => print_json(stats),
    }
}

fn print_human(stats: &HashMap<String, LangStats>, use_emojis: bool, use_color: bool) {
    // Aggregate overall stats
    let mut overall = LangStats::default();

    let mut languages: Vec<_> = stats.keys().collect();
    languages.sort();

    for lang in &languages {
        if let Some(s) = stats.get(*lang) {
            overall.total_added += s.total_added;
            overall.total_removed += s.total_removed;
            overall.pure_added += s.pure_added;
            overall.pure_removed += s.pure_removed;
            overall.comment_lines_added += s.comment_lines_added;
            overall.comment_lines_removed += s.comment_lines_removed;
            overall.docstring_lines_added += s.docstring_lines_added;
            overall.docstring_lines_removed += s.docstring_lines_removed;
            overall.blank_lines_added += s.blank_lines_added;
            overall.blank_lines_removed += s.blank_lines_removed;
            overall.code_words_added += s.code_words_added;
            overall.code_words_removed += s.code_words_removed;
        }
    }

    let net_total = overall.net_total();
    let net_pure = overall.net_pure();

    // Ratios
    let total_changes = overall.total_added + overall.total_removed;
    let pure_changes = overall.pure_added + overall.pure_removed;

    let pure_ratio = if total_changes > 0 {
        (pure_changes as f64 / total_changes as f64) * 100.0
    } else {
        0.0
    };
    let noise_ratio = 100.0 - pure_ratio;

    let e_pure = if use_emojis { "✅ " } else { "" };
    let e_noise = if use_emojis { "🧹 " } else { "" };
    let e_summary = if use_emojis { "📊 " } else { "" };
    let e_sparkles = if use_emojis { "✨ " } else { "" };
    let e_comments = if use_emojis { "📝 " } else { "" };
    let e_docs = if use_emojis { "📚 " } else { "" };

    if use_color {
        println!("{}", format!("{}=== PureCode Summary ===", e_summary).bold());
    } else {
        println!("{}=== PureCode Summary ===", e_summary);
    }

    println!(
        "TOTAL lines  : +{:<4} -{:<4} (net {})",
        overall.total_added, overall.total_removed, net_total
    );
    println!(
        "PURE  lines  : +{:<4} -{:<4} (net {})",
        overall.pure_added, overall.pure_removed, net_pure
    );
    println!(
        "NOISE lines  : +{:<4} -{:<4} (comments/docstrings/blanks)",
        overall.total_added - overall.pure_added,
        overall.total_removed - overall.pure_removed
    );
    println!();

    if use_color {
        println!(
            "{}{}",
            e_sparkles,
            format!("Pure ratio : {:.0}% of changes are pure code", pure_ratio).green()
        );
        let noise_msg = format!("Noise      : {:.0}% comments & formatting", noise_ratio);
        if noise_ratio > 50.0 {
            println!("{}{}", e_noise, noise_msg.yellow());
        } else {
            println!("{}{}", e_noise, noise_msg.dimmed());
        }
    } else {
        println!(
            "{}Pure ratio : {:.0}% of changes are pure code",
            e_sparkles, pure_ratio
        );
        println!(
            "{}Noise      : {:.0}% comments & formatting",
            e_noise, noise_ratio
        );
    }
    println!();

    if use_color {
        println!("{}", "=== Per language ===".bold());
    } else {
        println!("=== Per language ===");
    }

    for lang in languages {
        if let Some(s) = stats.get(lang) {
            if s.total_added == 0 && s.total_removed == 0 {
                continue;
            }

            if use_color {
                println!("{}", lang.cyan().bold());
            } else {
                println!("{}", lang);
            }

            println!(
                "  TOTAL : +{:<4} -{:<4} (net {})",
                s.total_added,
                s.total_removed,
                s.net_total()
            );
            println!(
                "  PURE  : +{:<4} -{:<4} (net {})  {}",
                s.pure_added,
                s.pure_removed,
                s.net_pure(),
                e_pure
            );

            if s.comment_lines_added > 0 || s.comment_lines_removed > 0 {
                println!(
                    "  Comments   : +{:<4} -{:<4}  {}",
                    s.comment_lines_added, s.comment_lines_removed, e_comments
                );
            }
            if s.docstring_lines_added > 0 || s.docstring_lines_removed > 0 {
                println!(
                    "  Docstrings : +{:<4} -{:<4}  {}",
                    s.docstring_lines_added, s.docstring_lines_removed, e_docs
                );
            }
            if s.blank_lines_added > 0 || s.blank_lines_removed > 0 {
                println!(
                    "  Blanks     : +{:<4} -{:<4}",
                    s.blank_lines_added, s.blank_lines_removed
                );
            }

            let net_words = s.code_words_added - s.code_words_removed;
            let est_tokens_net = s.estimated_tokens_added() - s.estimated_tokens_removed();

            println!(
                "  Words      : +{:<4} -{:<4} (net {} words, est. ~{} tokens)",
                s.code_words_added, s.code_words_removed, net_words, est_tokens_net
            );
            println!();
        }
    }
}

fn print_json(stats: &HashMap<String, LangStats>) {
    // Simple manual JSON construction to avoid adding serde/serde_json dependencies for now.
    // If complex, we should add serde.

    // Overall aggregation
    let mut overall = LangStats::default();
    for s in stats.values() {
        overall.total_added += s.total_added;
        overall.total_removed += s.total_removed;
        overall.pure_added += s.pure_added;
        overall.pure_removed += s.pure_removed;
        overall.comment_lines_added += s.comment_lines_added;
        overall.comment_lines_removed += s.comment_lines_removed;
        overall.docstring_lines_added += s.docstring_lines_added;
        overall.docstring_lines_removed += s.docstring_lines_removed;
        overall.blank_lines_added += s.blank_lines_added;
        overall.blank_lines_removed += s.blank_lines_removed;
        overall.code_words_added += s.code_words_added;
        overall.code_words_removed += s.code_words_removed;
    }

    println!("{{");
    println!("  \"overall\": {{");
    print_json_stats_fields(&overall, 4);
    println!("  }},");
    println!("  \"languages\": {{");

    let mut languages: Vec<_> = stats.keys().collect();
    languages.sort();

    for (i, lang) in languages.iter().enumerate() {
        let s = stats.get(*lang).unwrap();
        println!("    \"{}\": {{", lang.to_lowercase());
        print_json_stats_fields(s, 6);
        if i < languages.len() - 1 {
            println!("    }},");
        } else {
            println!("    }}");
        }
    }

    println!("  }}");
    println!("}}");
}

fn print_json_stats_fields(s: &LangStats, indent: usize) {
    let pad = " ".repeat(indent);
    println!("{}\"total_added\": {},", pad, s.total_added);
    println!("{}\"total_removed\": {},", pad, s.total_removed);
    println!("{}\"pure_added\": {},", pad, s.pure_added);
    println!("{}\"pure_removed\": {},", pad, s.pure_removed);
    println!("{}\"comment_added\": {},", pad, s.comment_lines_added);
    println!("{}\"comment_removed\": {},", pad, s.comment_lines_removed);
    println!("{}\"docstring_added\": {},", pad, s.docstring_lines_added);
    println!("{}\"docstring_removed\": {},", pad, s.docstring_lines_removed);
    println!("{}\"blank_added\": {},", pad, s.blank_lines_added);
    println!("{}\"blank_removed\": {},", pad, s.blank_lines_removed);
    println!("{}\"words_added\": {},", pad, s.code_words_added);
    println!("{}\"words_removed\": {},", pad, s.code_words_removed);
    println!("{}\"tokens_added_est\": {},", pad, s.estimated_tokens_added());
    println!("{}\"tokens_removed_est\": {}", pad, s.estimated_tokens_removed());
}
