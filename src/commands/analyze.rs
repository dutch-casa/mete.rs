use colored::*;
use mete::{AnalysisService, AnalyzeRequest, DomainError, WantFlags};
use rayon::prelude::*;
use std::fs;
use std::path::{Component, Path};

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

    let results = if path.is_file() {
        analyze_file(path, language, quiet)
    } else if path.is_dir() {
        analyze_directory(path, language, pattern, quiet)
    } else {
        eprintln!("{}", "Error: Path must be a file or directory".red());
        std::process::exit(1);
    };

    if results.is_empty() && !quiet {
        println!("{}", "No files analyzed".yellow());
        return Ok(());
    }

    let filtered_results = apply_filters(&results, threshold, max_complexity, max_depth);

    let sorted_results = sort_results(&filtered_results, sort_by, sort_order);

    display_results(&sorted_results, format, threshold, quiet);

    Ok(())
}

fn analyze_file(path: &Path, language: Option<&str>, quiet: bool) -> Vec<FileResult> {
    let mut results: Vec<FileResult> = Vec::new();

    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => {
            if !quiet {
                eprintln!(
                    "{} {}",
                    "Error reading file".red(),
                    path.display().to_string().dimmed()
                );
            }
            return results;
        }
    };

    let lang = language.unwrap_or_else(|| detect_language(path));
    let request = match build_request(&text, lang) {
        Ok(req) => req,
        Err(e) => {
            if !quiet {
                eprintln!(
                    "{} {}: {}",
                    "Analysis failed".yellow(),
                    path.display().to_string().dimmed(),
                    e
                );
            }
            return results;
        }
    };

    match AnalysisService::analyze(request) {
        Ok(response) => {
            if let Some(file_metrics) = response.file {
                let file_result =
                    FileResult::from(path, file_metrics, response.duplicates.unwrap_or_default());
                results.push(file_result);
            }
        }
        Err(e) => {
            if !quiet {
                eprintln!(
                    "{} {}: {}",
                    "Analysis failed".yellow(),
                    path.display().to_string().dimmed(),
                    e
                );
            }
        }
    }

    results
}

/// Parallel file analysis function for use with rayon
fn analyze_file_parallel(path: &Path, language: Option<&str>, quiet: bool) -> Option<FileResult> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => {
            if !quiet {
                eprintln!(
                    "{} {}",
                    "Error reading file".red(),
                    path.display().to_string().dimmed()
                );
            }
            return None;
        }
    };

    if text.is_empty() {
        return None;
    }

    let lang = language.unwrap_or_else(|| detect_language(path));
    let request = match build_request(&text, lang) {
        Ok(req) => req,
        Err(e) => {
            if !quiet {
                eprintln!(
                    "{} {}: {}",
                    "Analysis failed".yellow(),
                    path.display().to_string().dimmed(),
                    e
                );
            }
            return None;
        }
    };

    match AnalysisService::analyze(request) {
        Ok(response) => {
            if let Some(file_metrics) = response.file {
                let file_result =
                    FileResult::from(path, file_metrics, response.duplicates.unwrap_or_default());
                Some(file_result)
            } else {
                None
            }
        }
        Err(e) => {
            if !quiet {
                eprintln!(
                    "{} {}: {}",
                    "Analysis failed".yellow(),
                    path.display().to_string().dimmed(),
                    e
                );
            }
            None
        }
    }
}

fn detect_language(path: &Path) -> &str {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| match ext {
            "rs" => "rust",
            "ts" | "tsx" => "typescript",
            "js" | "jsx" => "javascript",
            "py" => "python",
            "go" => "go",
            "java" => "java",
            "cs" => "csharp",
            "ex" | "exs" => "elixir",
            "cpp" | "cc" | "cxx" | "hpp" | "h" => "cpp",
            "c" => "c",
            _ => "rust",
        })
        .unwrap_or("rust")
}

fn analyze_directory(
    dir: &Path,
    language: Option<&str>,
    pattern: &str,
    quiet: bool,
) -> Vec<FileResult> {
    let pattern = dir.join(pattern);
    let pattern_str = pattern.to_string_lossy().to_string();

    let entries = match glob::glob(&pattern_str) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("{} {}", "Invalid pattern".red(), e);
            return Vec::new();
        }
    };

    let file_paths: Vec<_> = entries
        .filter_map(|entry| match entry {
            Ok(path) if path.is_file() && !is_skippable(&path) => Some(path),
            _ => None,
        })
        .collect();

    if file_paths.is_empty() {
        return Vec::new();
    }

    file_paths
        .par_iter()
        .filter_map(|path| analyze_file_parallel(path, language, quiet))
        .collect()
}

fn is_skippable(path: &Path) -> bool {
    // Skip node_modules and other non-source files
    path.components().any(|c| {
        if let Component::Normal(s) = c {
            s == "node_modules" || s == "dist" || s == "build" || s == ".next" || s == ".cache"
        } else {
            false
        }
    })
}

fn build_request(text: &str, language: &str) -> Result<AnalyzeRequest, DomainError> {
    AnalyzeRequest::with_options(
        text.to_string(),
        language.to_string(),
        None,
        WantFlags::all(),
    )
}

fn apply_filters(
    results: &[FileResult],
    threshold: Option<f64>,
    max_complexity: Option<u32>,
    max_depth: Option<u32>,
) -> Vec<FileResult> {
    results
        .iter()
        .filter(|r| threshold.map(|t| r.metrics.mi >= t).unwrap_or(true))
        .filter(|r| {
            max_complexity
                .map(|m| r.metrics.cc_max <= m)
                .unwrap_or(true)
        })
        .filter(|r| max_depth.map(|m| r.metrics.depth_max <= m).unwrap_or(true))
        .cloned()
        .collect()
}

fn sort_results(results: &[FileResult], sort_by: &str, sort_order: &str) -> Vec<FileResult> {
    let mut sorted: Vec<FileResult> = results.to_vec();

    let cmp = |a: &FileResult, b: &FileResult| match sort_by {
        "mi" => a.metrics.mi.partial_cmp(&b.metrics.mi).unwrap(),
        "cc" => a.metrics.cc_max.cmp(&b.metrics.cc_max),
        "loc" => a.metrics.loc.cmp(&b.metrics.loc),
        "depth" => a.metrics.depth_max.cmp(&b.metrics.depth_max),
        "functions" => a.metrics.functions_count.cmp(&b.metrics.functions_count),
        "dups" => a.metrics.dup_blocks.cmp(&b.metrics.dup_blocks),
        "path" | _ => a.filename.cmp(&b.filename),
    };

    if sort_order == "desc" {
        sorted.par_sort_unstable_by(|a, b| cmp(b, a));
    } else {
        sorted.par_sort_unstable_by(cmp);
    }

    sorted
}

fn display_results(results: &[FileResult], format: &str, threshold: Option<f64>, quiet: bool) {
    if quiet {
        return;
    }

    match format {
        "table" => display_table(results, threshold),
        "json" => display_json(results),
        "csv" => display_csv(results),
        "summary" => display_summary(results),
        _ => {
            eprintln!("{}: {}", "Unknown format".red(), format);
            display_table(results, threshold);
        }
    }
}

fn display_table(results: &[FileResult], threshold: Option<f64>) {
    let aggs = compute_aggregates(results);

    println!();
    println!("{}", "═".repeat(95).dimmed());
    println!(
        "{} {} files analyzed",
        "Code Quality Report".cyan().bold(),
        results.len()
    );
    if let Some(t) = threshold {
        println!(
            "{} MI threshold: {}",
            "Filter:".dimmed(),
            format!("{:.1}", t)
        );
    }
    println!("{}", "═".repeat(95).dimmed());
    println!();

    display_summary_row(&aggs);

    println!();
    println!("{}", "─".repeat(105).dimmed());
    println!("{}", "Details".cyan().bold());
    println!("{}", "─".repeat(105).dimmed());
    println!();
    println!(
        "{:<55} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>8} {:>6}",
        "File".cyan(),
        "LOC".cyan(),
        "CCmax".cyan(),
        "COG".cyan(),
        "MI".cyan(),
        "Depth".cyan(),
        "Fan-in".cyan(),
        "Fan-out".cyan(),
        "Dups".cyan(),
        "Funcs".cyan()
    );
    println!("{}", "─".repeat(105).dimmed());

    for result in results {
        let m = &result.metrics;

        let mi_colored = colorize(&format!("{:.1}", m.mi), mi_color(m.mi));
        let cc_colored = colorize(&m.cc_max.to_string(), cc_color(m.cc_max));
        let cognitive_colored = colorize(
            &m.cognitive_max.to_string(),
            cognitive_color(m.cognitive_max),
        );
        let depth_colored = colorize(&m.depth_max.to_string(), depth_color(m.depth_max));

        let name: String = result.filename.chars().take(52).collect();

        println!(
            "{:<55} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>8} {:>6}",
            name,
            m.loc.to_string(),
            cc_colored,
            cognitive_colored,
            mi_colored,
            depth_colored,
            m.fan_in.to_string(),
            m.fan_out.to_string(),
            m.dup_blocks.to_string(),
            m.functions_count.to_string()
        );
    }
}

fn display_json(results: &[FileResult]) {
    let aggs = compute_aggregates(results);

    let output = serde_json::to_string_pretty(&serde_json::json!({
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
                "path": &r.filename,
                "metrics": {
                    "loc": r.metrics.loc,
                    "cc_max": r.metrics.cc_max,
                    "cc_sum": r.metrics.cc_sum,
                    "cognitive_max": r.metrics.cognitive_max,
                    "cognitive_sum": r.metrics.cognitive_sum,
                    "depth_max": r.metrics.depth_max,
                    "fan_in": r.metrics.fan_in,
                    "fan_out": r.metrics.fan_out,
                    "exports": r.metrics.exports,
                    "mi": r.metrics.mi,
                    "dup_blocks": r.metrics.dup_blocks,
                    "functions_count": r.metrics.functions_count,
                    "stability": r.metrics.stability,
                }
            })
        }).collect::<Vec<_>>()
    }))
    .unwrap();

    println!("{}", output);
}

fn display_csv(results: &[FileResult]) {
    println!(
        "path,loc,cc_max,cc_sum,cognitive_max,cognitive_sum,depth_max,fan_in,fan_out,exports,mi,dup_blocks,functions_count,stability"
    );

    for result in results {
        let m = &result.metrics;
        println!(
            "{},{},{},{},{},{},{},{},{},{},{:.2},{},{},{:.2}",
            result.filename,
            m.loc,
            m.cc_max,
            m.cc_sum,
            m.cognitive_max,
            m.cognitive_sum,
            m.depth_max,
            m.fan_in,
            m.fan_out,
            m.exports,
            m.mi,
            m.dup_blocks,
            m.functions_count,
            m.stability
        );
    }
}

fn display_summary(results: &[FileResult]) {
    let aggs = compute_aggregates(results);

    println!();
    println!("{}", "Code Quality Summary".cyan().bold());
    println!();
    println!("  {} {}", "Files:".cyan(), results.len());
    println!("  {} {}", "Total LOC:".cyan(), aggs.total_loc);
    println!("  {} {}", "Avg MI:".cyan(), format!("{:.1}", aggs.avg_mi));
    println!(
        "  {} {}",
        "Avg CCmax:".cyan(),
        format!("{:.1}", aggs.avg_cc_max)
    );
    println!(
        "  {} {}",
        "Avg Cognitive:".cyan(),
        format!("{:.1}", aggs.avg_cognitive_max)
    );
    println!(
        "  {} {}",
        "Avg Depth:".cyan(),
        format!("{:.1}", aggs.avg_depth)
    );
    println!("  {} {}", "Total Duplicates:".cyan(), aggs.total_dups);
    println!(
        "  {} {}",
        "Avg Functions:".cyan(),
        format!("{:.1}", aggs.avg_functions)
    );
}

fn display_summary_row(aggs: &Aggregates) {
    let mi = aggs.avg_mi;
    let cc = aggs.avg_cc_max;
    let cognitive = aggs.avg_cognitive_max;

    let mi_formatted = colorize(&format!("{:.1}", mi), mi_color(mi));
    let cc_formatted = colorize(&format!("{:.1}", cc), cc_color(cc as u32));
    let cognitive_formatted = colorize(
        &format!("{:.1}", cognitive),
        cognitive_color(cognitive as u32),
    );

    println!(
        "{}  {}  {}  {}  {}  {}  {}  {}  {}  {}  {}",
        "LOC:".cyan(),
        aggs.total_loc.to_string().white().bold(),
        "CCavg:".cyan(),
        cc_formatted,
        "COG:".cyan(),
        cognitive_formatted,
        "MIavg:".cyan(),
        mi_formatted,
        "Dups:".cyan(),
        aggs.total_dups.to_string().white(),
        format!("Funcs: {:.1}", aggs.avg_functions).white(),
    );
}

fn colorize(text: &str, color: &str) -> ColoredString {
    match color {
        "green" => text.green(),
        "yellow" => text.yellow(),
        "red" => text.red(),
        _ => text.white(),
    }
}

fn mi_color(mi: f64) -> &'static str {
    match mi {
        m if m >= 85.0 => "green",
        m if m >= 65.0 => "yellow",
        _ => "red",
    }
}

fn cc_color(cc: u32) -> &'static str {
    match cc {
        c if c <= 5 => "green",
        c if c <= 10 => "yellow",
        _ => "red",
    }
}

fn cognitive_color(cog: u32) -> &'static str {
    match cog {
        c if c <= 8 => "green",
        c if c <= 15 => "yellow",
        _ => "red",
    }
}

fn depth_color(depth: u32) -> &'static str {
    match depth {
        d if d <= 3 => "green",
        d if d <= 5 => "yellow",
        _ => "red",
    }
}

fn compute_aggregates(results: &[FileResult]) -> Aggregates {
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

    let mut total_loc = 0;
    let mut total_cc_max = 0;
    let mut total_cognitive_max = 0;
    let mut total_mi = 0.0;
    let mut total_depth = 0;
    let mut total_dups = 0;
    let mut total_functions = 0;

    for r in results {
        total_loc += r.metrics.loc;
        total_cc_max += r.metrics.cc_max;
        total_cognitive_max += r.metrics.cognitive_max;
        total_mi += r.metrics.mi;
        total_depth += r.metrics.depth_max;
        total_dups += r.metrics.dup_blocks;
        total_functions += r.metrics.functions_count;
    }

    let count = results.len() as f64;
    Aggregates {
        total_loc,
        avg_cc_max: total_cc_max as f64 / count,
        avg_cognitive_max: total_cognitive_max as f64 / count,
        avg_mi: total_mi / count,
        avg_depth: total_depth as f64 / count,
        total_dups,
        avg_functions: total_functions as f64 / count,
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

#[derive(Debug, Clone)]
pub struct FileResult {
    pub filename: String,
    pub metrics: FileMetrics,
    #[allow(dead_code)]
    pub duplicates: Vec<DuplicateInfo>,
}

#[derive(Debug, Clone)]
pub struct FileMetrics {
    pub loc: u32,
    pub cc_max: u32,
    pub cc_sum: u32,
    pub cognitive_max: u32,
    pub cognitive_sum: u32,
    pub depth_max: u32,
    pub fan_in: u32,
    pub fan_out: u32,
    pub exports: u32,
    pub mi: f64,
    pub dup_blocks: u32,
    pub functions_count: u32,
    pub stability: f64,
}

#[derive(Debug, Clone)]
pub struct DuplicateInfo {
    #[allow(dead_code)]
    pub fingerprint: u64,
    #[allow(dead_code)]
    pub instances: Vec<DuplicateInstance>,
}

#[derive(Debug, Clone)]
pub struct DuplicateInstance {
    #[allow(dead_code)]
    pub name: Option<String>,
    #[allow(dead_code)]
    pub start_line: u32,
    #[allow(dead_code)]
    pub end_line: u32,
}

impl FileResult {
    fn from(
        path: &Path,
        metrics: mete::FileMetricsDto,
        duplicates: Vec<mete::DuplicateGroupDto>,
    ) -> Self {
        Self {
            filename: path.display().to_string(),
            metrics: FileMetrics {
                loc: metrics.loc,
                cc_max: metrics.cc_max,
                cc_sum: metrics.cc_sum,
                cognitive_max: metrics.cognitive_max,
                cognitive_sum: metrics.cognitive_sum,
                depth_max: metrics.depth_max,
                fan_in: metrics.fan_in,
                fan_out: metrics.fan_out,
                exports: metrics.exports,
                mi: metrics.mi,
                dup_blocks: metrics.dup_blocks,
                functions_count: metrics.functions_count,
                stability: metrics.stability,
            },
            duplicates: duplicates
                .into_iter()
                .map(|d| DuplicateInfo {
                    fingerprint: d.fingerprint,
                    instances: d
                        .instances
                        .into_iter()
                        .map(|i| DuplicateInstance {
                            name: i.name,
                            start_line: i.start_line,
                            end_line: i.end_line,
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}
