use crate::stats::LangStats;
use std::collections::HashMap;

pub fn print_report(stats: &HashMap<String, LangStats>) {
    let mut grand_total_added = 0;
    let mut grand_total_removed = 0;
    let mut grand_pure_added = 0;
    let mut grand_pure_removed = 0;

    let mut languages: Vec<_> = stats.keys().collect();
    languages.sort();

    for lang in &languages {
        if let Some(s) = stats.get(*lang) {
            grand_total_added += s.total_added;
            grand_total_removed += s.total_removed;
            grand_pure_added += s.pure_added;
            grand_pure_removed += s.pure_removed;
        }
    }

    let net_total = grand_total_added - grand_total_removed;
    println!("=== OVERALL TOTAL (all languages, all lines) ===");
    println!(
        "TOTAL lines changed : +{:<13} -{:<13} (net {})",
        grand_total_added, grand_total_removed, net_total
    );
    println!();

    println!("=== PER LANGUAGE ===");
    for lang in languages {
        if let Some(s) = stats.get(lang) {
            if s.total_added == 0 && s.total_removed == 0 {
                continue;
            }
            let net_lang = s.net_total();
            let net_pure = s.net_pure();

            println!("{}:", lang);
            println!(
                "  TOTAL : +{:<3} -{:<3} (net {})",
                s.total_added, s.total_removed, net_lang
            );
            println!(
                "  PURE  : +{:<3} -{:<3} (net {})",
                s.pure_added, s.pure_removed, net_pure
            );
            println!();
        }
    }

    let net_pure_all = grand_pure_added - grand_pure_removed;
    println!("=== PURE CODE ONLY (all languages) ===");
    println!(
        "PURE code lines     : +{:<13} -{:<13} (net {})",
        grand_pure_added, grand_pure_removed, net_pure_all
    );
}
