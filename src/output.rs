//! Output formatting for analysis results.
//!
//! Supports table, JSON, CSV, and summary formats.

use crate::data::{FileResults, FunctionResults, SingleFileResult, StringInterner};
use crate::metrics::{CcLevel, CognitiveLevel, DepthLevel, MiLevel};
use colored::*;

/// Format file results as a table.
pub fn print_table(results: &[SingleFileResult], threshold: Option<f64>, show_mi: bool) {
    if results.is_empty() {
        println!("{}", "No files analyzed".yellow());
        return;
    }

    let aggs = compute_aggregates(results);

    println!();
    println!("{}", "═".repeat(95).dimmed());
    println!(
        "{} {} files analyzed",
        "Code Quality Report".cyan().bold(),
        results.len()
    );
    if let Some(t) = threshold {
        println!("{} MI threshold: {:.1}", "Filter:".dimmed(), t);
    }
    println!("{}", "═".repeat(95).dimmed());
    println!();

    print_summary_row(&aggs, show_mi);

    println!();
    println!("{}", "─".repeat(100).dimmed());
    println!("{}", "Details".cyan().bold());
    println!("{}", "─".repeat(100).dimmed());
    println!();

    if show_mi {
        println!(
            "{:<55} {:>6} {:>6} {:>6} {:>6} {:>6} {:>8} {:>6}",
            "File".cyan(),
            "LOC".cyan(),
            "CCmax".cyan(),
            "COG".cyan(),
            "MI".cyan(),
            "Depth".cyan(),
            "Dups".cyan(),
            "Funcs".cyan()
        );
    } else {
        println!(
            "{:<55} {:>6} {:>6} {:>6} {:>6} {:>8} {:>6}",
            "File".cyan(),
            "LOC".cyan(),
            "CCmax".cyan(),
            "COG".cyan(),
            "Depth".cyan(),
            "Dups".cyan(),
            "Funcs".cyan()
        );
    }
    println!("{}", "─".repeat(100).dimmed());

    for result in results {
        let cc_colored = colorize_cc(result.cc_max);
        let cog_colored = colorize_cognitive(result.cognitive_max);
        let depth_colored = colorize_depth(result.depth_max);
        let name: String = result.path.display().to_string().chars().take(52).collect();

        if show_mi {
            let mi_colored = colorize_mi(result.mi);
            println!(
                "{:<55} {:>6} {:>6} {:>6} {:>6} {:>6} {:>8} {:>6}",
                name,
                result.loc,
                cc_colored,
                cog_colored,
                mi_colored,
                depth_colored,
                result.dup_count,
                result.function_count
            );
        } else {
            println!(
                "{:<55} {:>6} {:>6} {:>6} {:>6} {:>8} {:>6}",
                name,
                result.loc,
                cc_colored,
                cog_colored,
                depth_colored,
                result.dup_count,
                result.function_count
            );
        }
    }
}

/// Format file results as JSON.
pub fn print_json(results: &[SingleFileResult]) {
    let aggs = compute_aggregates(results);

    let json = serde_json::json!({
        "summary": {
            "files": results.len(),
            "total_loc": aggs.total_loc,
            "avg_mi": aggs.avg_mi,
            "avg_cc_max": aggs.avg_cc_max,
            "avg_cognitive_max": aggs.avg_cognitive_max,
            "avg_depth": aggs.avg_depth,
            "total_dups": aggs.total_dups,
            "avg_functions": aggs.avg_functions,
        },
        "files": results.iter().map(|r| {
            serde_json::json!({
                "path": r.path.display().to_string(),
                "metrics": {
                    "loc": r.loc,
                    "cc_max": r.cc_max,
                    "cc_sum": r.cc_sum,
                    "cognitive_max": r.cognitive_max,
                    "cognitive_sum": r.cognitive_sum,
                    "depth_max": r.depth_max,
                    "imports": r.imports,
                    "exports": r.exports,
                    "mi": r.mi,
                    "dup_count": r.dup_count,
                    "function_count": r.function_count,
                }
            })
        }).collect::<Vec<_>>()
    });

    match serde_json::to_string_pretty(&json) {
        Ok(output) => println!("{}", output),
        Err(e) => eprintln!("{}", format!("Error serializing JSON: {}", e).red()),
    }
}

/// Format file results as CSV.
pub fn print_csv(results: &[SingleFileResult]) {
    println!(
        "path,loc,cc_max,cc_sum,cognitive_max,cognitive_sum,depth_max,imports,exports,mi,dup_count,function_count"
    );

    for r in results {
        println!(
            "{},{},{},{},{},{},{},{},{},{},{},{}",
            r.path.display(),
            r.loc,
            r.cc_max,
            r.cc_sum,
            r.cognitive_max,
            r.cognitive_sum,
            r.depth_max,
            r.imports,
            r.exports,
            r.mi,
            r.dup_count,
            r.function_count
        );
    }
}

/// Print summary only.
pub fn print_summary(results: &[SingleFileResult], show_mi: bool) {
    let aggs = compute_aggregates(results);

    println!();
    println!("{}", "Code Quality Summary".cyan().bold());
    println!();
    println!("  {} {}", "Files:".cyan(), results.len());
    println!("  {} {}", "Total LOC:".cyan(), aggs.total_loc);
    if show_mi {
        println!("  {} {:.1}", "Avg MI:".cyan(), aggs.avg_mi);
    }
    println!("  {} {:.1}", "Avg CCmax:".cyan(), aggs.avg_cc_max);
    println!("  {} {:.1}", "Avg Cognitive:".cyan(), aggs.avg_cognitive_max);
    println!("  {} {:.1}", "Avg Depth:".cyan(), aggs.avg_depth);
    println!("  {} {}", "Total Duplicates:".cyan(), aggs.total_dups);
    println!("  {} {:.1}", "Avg Functions:".cyan(), aggs.avg_functions);
}

/// Print function-level results as table.
pub fn print_functions_table(
    file_results: &FileResults,
    fn_results: &FunctionResults,
    interner: &StringInterner,
) {
    println!();
    println!("{}", "Function Metrics".cyan().bold());
    println!("{}", "─".repeat(100).dimmed());
    println!(
        "{:<50} {:>8} {:>6} {:>6} {:>6} {:>6} {:>10}",
        "Function".cyan(),
        "File".cyan(),
        "LOC".cyan(),
        "CC".cyan(),
        "COG".cyan(),
        "Depth".cyan(),
        "Lines".cyan()
    );
    println!("{}", "─".repeat(100).dimmed());

    for i in fn_results.indices() {
        let file_idx = fn_results.file_idx[i] as usize;
        let name_offset = fn_results.name_offset[i];
        let name_len = fn_results.name_len[i];

        let name = if name_len > 0 {
            interner.get(name_offset, name_len)
        } else {
            "<anonymous>"
        };

        let file_name: String = file_results.paths[file_idx]
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default()
            .chars()
            .take(8)
            .collect();

        let cc_colored = colorize_cc(fn_results.cc[i]);
        let cog_colored = colorize_cognitive(fn_results.cognitive[i]);
        let depth_colored = colorize_depth(fn_results.depth[i]);

        println!(
            "{:<50} {:>8} {:>6} {:>6} {:>6} {:>6} {:>5}-{:<4}",
            name.chars().take(48).collect::<String>(),
            file_name,
            fn_results.loc[i],
            cc_colored,
            cog_colored,
            depth_colored,
            fn_results.start_line[i],
            fn_results.end_line[i]
        );
    }
}

/// Print function-level results as JSON.
pub fn print_functions_json(
    file_results: &FileResults,
    fn_results: &FunctionResults,
    interner: &StringInterner,
) {
    let functions: Vec<_> = fn_results
        .indices()
        .map(|i| {
            let file_idx = fn_results.file_idx[i] as usize;
            let name_offset = fn_results.name_offset[i];
            let name_len = fn_results.name_len[i];

            let name = if name_len > 0 {
                Some(interner.get(name_offset, name_len).to_string())
            } else {
                None
            };

            serde_json::json!({
                "name": name,
                "file": file_results.paths[file_idx].display().to_string(),
                "start_line": fn_results.start_line[i],
                "end_line": fn_results.end_line[i],
                "loc": fn_results.loc[i],
                "cc": fn_results.cc[i],
                "cognitive": fn_results.cognitive[i],
                "depth": fn_results.depth[i],
                "fingerprint": fn_results.fingerprint[i],
            })
        })
        .collect();

    let json = serde_json::json!({ "functions": functions });

    match serde_json::to_string_pretty(&json) {
        Ok(output) => println!("{}", output),
        Err(e) => eprintln!("{}", format!("Error serializing JSON: {}", e).red()),
    }
}

/// Print function-level results as CSV.
pub fn print_functions_csv(
    file_results: &FileResults,
    fn_results: &FunctionResults,
    interner: &StringInterner,
) {
    println!("name,file,start_line,end_line,loc,cc,cognitive,depth,fingerprint");

    for i in fn_results.indices() {
        let file_idx = fn_results.file_idx[i] as usize;
        let name_offset = fn_results.name_offset[i];
        let name_len = fn_results.name_len[i];

        let name = if name_len > 0 {
            interner.get(name_offset, name_len)
        } else {
            ""
        };

        println!(
            "{},{},{},{},{},{},{},{},{}",
            name,
            file_results.paths[file_idx].display(),
            fn_results.start_line[i],
            fn_results.end_line[i],
            fn_results.loc[i],
            fn_results.cc[i],
            fn_results.cognitive[i],
            fn_results.depth[i],
            fn_results.fingerprint[i]
        );
    }
}

struct Aggregates {
    total_loc: u32,
    avg_cc_max: f64,
    avg_cognitive_max: f64,
    avg_mi: f64,
    avg_depth: f64,
    total_dups: u32,
    avg_functions: f64,
}

fn compute_aggregates(results: &[SingleFileResult]) -> Aggregates {
    if results.is_empty() {
        return Aggregates {
            total_loc: 0,
            avg_cc_max: 0.0,
            avg_cognitive_max: 0.0,
            avg_mi: 0.0,
            avg_depth: 0.0,
            total_dups: 0,
            avg_functions: 0.0,
        };
    }

    let total_loc: u32 = results.iter().map(|r| r.loc).sum();
    let total_cc_max: u32 = results.iter().map(|r| r.cc_max).sum();
    let total_cognitive_max: u32 = results.iter().map(|r| r.cognitive_max).sum();
    let total_mi: u32 = results.iter().map(|r| r.mi as u32).sum();
    let total_depth: u32 = results.iter().map(|r| r.depth_max).sum();
    let total_dups: u32 = results.iter().map(|r| r.dup_count).sum();
    let total_functions: u32 = results.iter().map(|r| r.function_count).sum();

    let count = results.len() as f64;

    Aggregates {
        total_loc,
        avg_cc_max: total_cc_max as f64 / count,
        avg_cognitive_max: total_cognitive_max as f64 / count,
        avg_mi: total_mi as f64 / count,
        avg_depth: total_depth as f64 / count,
        total_dups,
        avg_functions: total_functions as f64 / count,
    }
}

fn print_summary_row(aggs: &Aggregates, show_mi: bool) {
    let cc_colored = colorize_cc(aggs.avg_cc_max as u32);
    let cog_colored = colorize_cognitive(aggs.avg_cognitive_max as u32);

    if show_mi {
        let mi_colored = colorize_mi(aggs.avg_mi as u8);
        println!(
            "{}  {}  {}  {}  {}  {}  {}  {}  {}  {}  {}",
            "LOC:".cyan(),
            aggs.total_loc.to_string().white().bold(),
            "CCavg:".cyan(),
            cc_colored,
            "COG:".cyan(),
            cog_colored,
            "MIavg:".cyan(),
            mi_colored,
            "Dups:".cyan(),
            aggs.total_dups.to_string().white(),
            format!("Funcs: {:.1}", aggs.avg_functions).white(),
        );
    } else {
        println!(
            "{}  {}  {}  {}  {}  {}  {}  {}  {}",
            "LOC:".cyan(),
            aggs.total_loc.to_string().white().bold(),
            "CCavg:".cyan(),
            cc_colored,
            "COG:".cyan(),
            cog_colored,
            "Dups:".cyan(),
            aggs.total_dups.to_string().white(),
            format!("Funcs: {:.1}", aggs.avg_functions).white(),
        );
    }
}

fn colorize(text: String, color: &str) -> ColoredString {
    match color {
        "green" => text.green(),
        "yellow" => text.yellow(),
        "red" => text.red(),
        _ => text.white(),
    }
}

pub fn colorize_mi(mi: u8) -> ColoredString {
    colorize(mi.to_string(), MiLevel::from_value(mi).color())
}

pub fn colorize_cc(cc: u32) -> ColoredString {
    colorize(cc.to_string(), CcLevel::from_value(cc).color())
}

pub fn colorize_cognitive(cog: u32) -> ColoredString {
    colorize(cog.to_string(), CognitiveLevel::from_value(cog).color())
}

pub fn colorize_depth(depth: u32) -> ColoredString {
    colorize(depth.to_string(), DepthLevel::from_value(depth).color())
}
