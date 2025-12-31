use colored::*;
use mete::{AnalysisService, AnalyzeRequest, DomainError, WantFlags};
use std::fs;
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

    let mut results: Vec<FileResult> = Vec::new();

    for entry in entries {
        match entry {
            Ok(path) if path.is_file() => {
                let lang = language.unwrap_or_else(|| detect_language(&path));
                let Some(request) = build_request_from_path(&path, lang) else {
                    continue;
                };

                match AnalysisService::analyze(request) {
                    Ok(response) => {
                        if let Some(file_metrics) = response.file {
                            let file_result = FileResult::from(
                                &path,
                                file_metrics,
                                response.duplicates.unwrap_or_default(),
                            );
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
            }
            Ok(_) => {}
            Err(e) => {
                if !quiet {
                    eprintln!("{} {}", "Error reading entry".yellow(), e);
                }
            }
        }
    }

    results
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
            "cs" => "c_sharp",
            "cpp" | "cc" | "cxx" => "cpp",
            "c" => "c",
            "ex" | "exs" => "elixir",
            _ => "rust",
        })
        .unwrap_or("rust")
}

fn build_request(text: &str, language: &str) -> Result<AnalyzeRequest, DomainError> {
    AnalyzeRequest::with_options(
        text.to_string(),
        language.to_string(),
        None,
        WantFlags::all(),
    )
}

fn build_request_from_path(path: &Path, language: &str) -> Option<AnalyzeRequest> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return None,
    };
    if text.is_empty() {
        return None;
    }
    build_request(&text, language).ok()
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

    match sort_by {
        "mi" => sorted.sort_by(|a, b| {
            if sort_order == "desc" {
                b.metrics.mi.partial_cmp(&a.metrics.mi).unwrap()
            } else {
                a.metrics.mi.partial_cmp(&b.metrics.mi).unwrap()
            }
        }),
        "cc" => sorted.sort_by(|a, b| {
            if sort_order == "desc" {
                b.metrics.cc_max.cmp(&a.metrics.cc_max)
            } else {
                a.metrics.cc_max.cmp(&b.metrics.cc_max)
            }
        }),
        "loc" => sorted.sort_by(|a, b| {
            if sort_order == "desc" {
                b.metrics.loc.cmp(&a.metrics.loc)
            } else {
                a.metrics.loc.cmp(&b.metrics.loc)
            }
        }),
        "depth" => sorted.sort_by(|a, b| {
            if sort_order == "desc" {
                b.metrics.depth_max.cmp(&a.metrics.depth_max)
            } else {
                a.metrics.depth_max.cmp(&b.metrics.depth_max)
            }
        }),
        "functions" => sorted.sort_by(|a, b| {
            if sort_order == "desc" {
                b.metrics.functions_count.cmp(&a.metrics.functions_count)
            } else {
                a.metrics.functions_count.cmp(&b.metrics.functions_count)
            }
        }),
        "dups" => sorted.sort_by(|a, b| {
            if sort_order == "desc" {
                b.metrics.dup_blocks.cmp(&a.metrics.dup_blocks)
            } else {
                a.metrics.dup_blocks.cmp(&b.metrics.dup_blocks)
            }
        }),
        "path" | _ => sorted.sort_by(|a, b| {
            if sort_order == "desc" {
                b.filename.cmp(&a.filename)
            } else {
                a.filename.cmp(&b.filename)
            }
        }),
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
    println!("{}", "─".repeat(95).dimmed());
    println!("{}", "Details".cyan().bold());
    println!("{}", "─".repeat(95).dimmed());
    println!();
    println!(
        "{:<60} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8}",
        "File".cyan(),
        "LOC".cyan(),
        "CCmax".cyan(),
        "CCsum".cyan(),
        "MI".cyan(),
        "Depth".cyan(),
        "Fan-in".cyan(),
        "Fan-out".cyan(),
        "Dups".cyan(),
        "Funcs".cyan()
    );
    println!("{}", "─".repeat(95).dimmed());

    for result in results {
        let m = &result.metrics;

        let mi_color = if m.mi >= 85.0 {
            "green"
        } else if m.mi >= 65.0 {
            "yellow"
        } else {
            "red"
        };

        let cc_color = if m.cc_max <= 5 {
            "green"
        } else if m.cc_max <= 10 {
            "yellow"
        } else {
            "red"
        };

        let depth_color = if m.depth_max <= 3 {
            "green"
        } else if m.depth_max <= 5 {
            "yellow"
        } else {
            "red"
        };

        let cc_str = format!("{}", m.cc_max);
        let mi_str = format!("{:.1}", m.mi);
        let depth_str = format!("{}", m.depth_max);

        let mi_colored = colorize(&mi_str, mi_color);
        let cc_colored = colorize(&cc_str, cc_color);
        let depth_colored = colorize(&depth_str, depth_color);

        let name: String = result.filename.chars().take(57).collect();

        println!(
            "{:<60} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8}",
            name,
            m.loc.to_string(),
            cc_colored,
            m.cc_sum.to_string(),
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
        "path,loc,cc_max,cc_sum,depth_max,fan_in,fan_out,exports,mi,dup_blocks,functions_count,stability"
    );

    for result in results {
        let m = &result.metrics;
        println!(
            "{},{},{},{},{},{},{},{},{},{},{},{}",
            result.filename,
            m.loc,
            m.cc_max,
            m.cc_sum,
            m.depth_max,
            m.fan_in,
            m.fan_out,
            m.exports,
            format!("{:.2}", m.mi),
            m.dup_blocks,
            m.functions_count,
            format!("{:.2}", m.stability)
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
    let mi_color = if mi >= 85.0 {
        "green"
    } else if mi >= 65.0 {
        "yellow"
    } else {
        "red"
    };

    let cc = aggs.avg_cc_max;
    let cc_color = if cc <= 5.0 {
        "green"
    } else if cc <= 10.0 {
        "yellow"
    } else {
        "red"
    };

    let depth = aggs.avg_depth;
    let depth_color = if depth <= 3.0 {
        "green"
    } else if depth <= 5.0 {
        "yellow"
    } else {
        "red"
    };

    let mi_colored = format!("{:.1}", mi);
    let cc_colored = format!("{:.1}", cc);
    let depth_colored = format!("{:.1}", depth);

    let mi_formatted = colorize(&mi_colored, mi_color);
    let cc_formatted = colorize(&cc_colored, cc_color);
    let depth_formatted = colorize(&depth_colored, depth_color);

    println!(
        "{}  {}  {}  {}  {}  {}  {}  {}  {}  {}  {}",
        "LOC:".cyan(),
        aggs.total_loc.to_string().white().bold(),
        "CCavg:".cyan(),
        cc_formatted,
        "MIavg:".cyan(),
        mi_formatted,
        "Depth:".cyan(),
        "Dups:".cyan(),
        aggs.total_dups.to_string().white(),
        "Funcs:".cyan(),
        format!("{:.1}", aggs.avg_functions).white(),
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

fn compute_aggregates(results: &[FileResult]) -> Aggregates {
    let count = results.len() as f64;

    if count == 0.0 {
        return Aggregates {
            total_loc: 0,
            avg_cc_max: 0.0,
            avg_mi: 0.0,
            avg_depth: 0.0,
            total_dups: 0,
            avg_functions: 0.0,
        };
    }

    let total_loc: u32 = results.iter().map(|r| r.metrics.loc).sum();
    let avg_cc_max: f64 = results.iter().map(|r| r.metrics.cc_max as f64).sum::<f64>() / count;
    let total_mi: f64 = results.iter().map(|r| r.metrics.mi).sum::<f64>();
    let avg_depth: f64 = results
        .iter()
        .map(|r| r.metrics.depth_max as f64)
        .sum::<f64>()
        / count;
    let total_dups: u32 = results.iter().map(|r| r.metrics.dup_blocks).sum();
    let avg_functions: f64 = results
        .iter()
        .map(|r| r.metrics.functions_count as f64)
        .sum::<f64>()
        / count;

    Aggregates {
        total_loc,
        avg_cc_max,
        avg_mi: total_mi / count,
        avg_depth,
        total_dups,
        avg_functions,
    }
}

struct Aggregates {
    total_loc: u32,
    avg_cc_max: f64,
    avg_mi: f64,
    avg_depth: f64,
    total_dups: u32,
    avg_functions: f64,
}

#[derive(Debug, Clone)]
pub struct FileResult {
    pub filename: String,
    pub metrics: FileMetrics,
    pub duplicates: Vec<DuplicateInfo>,
}

#[derive(Debug, Clone)]
pub struct FileMetrics {
    pub loc: u32,
    pub cc_max: u32,
    pub cc_sum: u32,
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
    pub fingerprint: u64,
    pub instances: Vec<DuplicateInstance>,
}

#[derive(Debug, Clone)]
pub struct DuplicateInstance {
    pub name: Option<String>,
    pub start_line: u32,
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
