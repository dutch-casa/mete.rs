//! Analyze command implementation.

use super::common::{analyze_directory, analyze_file};
use mete::data::SingleFileResult;
use mete::lang::Language;
use mete::output;
use colored::*;
use rayon::prelude::*;
use std::path::Path;

pub fn run_analyze(
    path: &str,
    language: Option<&str>,
    pattern: &str,
    threshold: Option<f64>,
    sort_by: &str,
    sort_order: &str,
    max_complexity: Option<u32>,
    max_depth: Option<u32>,
    format: &str,
    show_mi: bool,
    _verbose: bool,
    quiet: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(path);

    if !path.exists() {
        eprintln!(
            "{}",
            format!("Error: Path does not exist: {}", path.display()).red()
        );
        std::process::exit(1);
    }

    let lang = language.and_then(Language::from_name);

    let results = if path.is_file() {
        analyze_file(path, lang, quiet)
    } else if path.is_dir() {
        analyze_directory(path, lang, pattern, quiet)
    } else {
        eprintln!("{}", "Error: Path must be a file or directory".red());
        std::process::exit(1);
    };

    if results.is_empty() && !quiet {
        println!("{}", "No files analyzed".yellow());
        return Ok(());
    }

    let filtered = apply_filters(&results, threshold, max_complexity, max_depth);
    let sorted = sort_results(&filtered, sort_by, sort_order);

    if !quiet {
        display_results(&sorted, format, threshold, show_mi);
    }

    Ok(())
}

fn apply_filters(
    results: &[SingleFileResult],
    threshold: Option<f64>,
    max_complexity: Option<u32>,
    max_depth: Option<u32>,
) -> Vec<SingleFileResult> {
    results
        .iter()
        .filter(|r| threshold.map(|t| r.mi as f64 >= t).unwrap_or(true))
        .filter(|r| max_complexity.map(|m| r.cc_max <= m).unwrap_or(true))
        .filter(|r| max_depth.map(|m| r.depth_max <= m).unwrap_or(true))
        .cloned()
        .collect()
}

fn sort_results(results: &[SingleFileResult], sort_by: &str, sort_order: &str) -> Vec<SingleFileResult> {
    let mut sorted = results.to_vec();

    let cmp = |a: &SingleFileResult, b: &SingleFileResult| match sort_by {
        "mi" => a.mi.cmp(&b.mi),
        "cc" => a.cc_max.cmp(&b.cc_max),
        "cog" => a.cognitive_max.cmp(&b.cognitive_max),
        "loc" => a.loc.cmp(&b.loc),
        "depth" => a.depth_max.cmp(&b.depth_max),
        "functions" => a.function_count.cmp(&b.function_count),
        "dups" => a.dup_count.cmp(&b.dup_count),
        "path" | _ => a.path.cmp(&b.path),
    };

    if sort_order == "desc" {
        sorted.par_sort_unstable_by(|a, b| cmp(b, a));
    } else {
        sorted.par_sort_unstable_by(cmp);
    }

    sorted
}

fn display_results(results: &[SingleFileResult], format: &str, threshold: Option<f64>, show_mi: bool) {
    match format {
        "table" => output::print_table(results, threshold, show_mi),
        "json" => output::print_json(results),
        "csv" => output::print_csv(results),
        "summary" => output::print_summary(results, show_mi),
        _ => {
            eprintln!("{}: {}", "Unknown format".red(), format);
            output::print_table(results, threshold, show_mi);
        }
    }
}
