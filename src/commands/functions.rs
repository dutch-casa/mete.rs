use colored::*;
use mete::{AnalysisService, AnalyzeRequest, WantFlags};
use std::fs;
use std::path::Path;

pub fn run_functions(
    path: &str,
    language: Option<&str>,
    pattern: &str,
    complex: bool,
    large: bool,
    deep: bool,
    min_complexity: Option<u32>,
    min_loc: Option<u32>,
    sort_by: &str,
    sort_order: &str,
    format: &str,
    verbose: bool,
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
        analyze_file(path, language, verbose, quiet)
    } else if path.is_dir() {
        analyze_directory(path, language, pattern, verbose, quiet)
    } else {
        eprintln!("{}", "Error: Path must be a file or directory".red());
        std::process::exit(1);
    };

    let filtered_functions = apply_filters(&results, complex, large, deep, min_complexity, min_loc);

    let sorted_functions = sort_functions(&filtered_functions, sort_by, sort_order);

    if sorted_functions.is_empty() && !quiet {
        println!("{}", "No functions found matching criteria".yellow());
        return Ok(());
    }

    display_results(&sorted_functions, format, verbose, quiet);

    Ok(())
}

fn analyze_file(
    path: &Path,
    language: Option<&str>,
    verbose: bool,
    quiet: bool,
) -> Vec<FunctionResult> {
    let mut results: Vec<FunctionResult> = Vec::new();

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
    let request = build_request(&text, lang);

    match AnalysisService::analyze(request) {
        Ok(response) => {
            if let Some(functions) = response.functions {
                for func in functions {
                    results.push(FunctionResult::from(path, func));
                }
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
    verbose: bool,
    quiet: bool,
) -> Vec<FunctionResult> {
    let pattern = dir.join(pattern);
    let pattern_str = pattern.to_string_lossy().to_string();

    let entries = match glob::glob(&pattern_str) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("{} {}", "Invalid pattern".red(), e);
            return Vec::new();
        }
    };

    let mut results: Vec<FunctionResult> = Vec::new();

    if verbose && !quiet {
        println!("{}", "Scanning files...".dimmed());
    }

    for entry in entries {
        match entry {
            Ok(path) if path.is_file() => {
                let lang = language.unwrap_or_else(|| detect_language(&path));
                let request = build_request_from_path(&path, lang);

                match AnalysisService::analyze(request) {
                    Ok(response) => {
                        if let Some(functions) = response.functions {
                            for func in functions {
                                results.push(FunctionResult::from(&path, func));
                            }

                            if verbose && !quiet {
                                println!("  {}", path.display().to_string().dimmed());
                            }
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

fn build_request(text: &str, language: &str) -> AnalyzeRequest {
    AnalyzeRequest::with_options(
        text.to_string(),
        language.to_string(),
        None,
        WantFlags::all(),
    )
    .unwrap_or_else(|_| AnalyzeRequest::new(text.to_string(), "rust".to_string()).unwrap())
}

fn build_request_from_path(path: &Path, language: &str) -> AnalyzeRequest {
    let text = fs::read_to_string(path).unwrap_or_default();
    build_request(&text, language)
}

fn apply_filters(
    results: &[FunctionResult],
    complex: bool,
    large: bool,
    deep: bool,
    min_complexity: Option<u32>,
    min_loc: Option<u32>,
) -> Vec<FunctionResult> {
    results
        .iter()
        .filter(|f| if complex { f.is_complex() } else { true })
        .filter(|f| if large { f.is_large() } else { true })
        .filter(|f| if deep { f.is_deep() } else { true })
        .filter(|f| min_complexity.map(|m| f.function.cc >= m).unwrap_or(true))
        .filter(|f| min_loc.map(|m| f.function.loc >= m).unwrap_or(true))
        .cloned()
        .collect()
}

fn sort_functions(
    results: &[FunctionResult],
    sort_by: &str,
    sort_order: &str,
) -> Vec<FunctionResult> {
    let mut sorted: Vec<FunctionResult> = results.to_vec();

    match sort_by {
        "cc" => sorted.sort_by(|a, b| {
            if sort_order == "desc" {
                b.function.cc.cmp(&a.function.cc)
            } else {
                a.function.cc.cmp(&b.function.cc)
            }
        }),
        "loc" => sorted.sort_by(|a, b| {
            if sort_order == "desc" {
                b.function.loc.cmp(&a.function.loc)
            } else {
                a.function.loc.cmp(&b.function.loc)
            }
        }),
        "depth" => sorted.sort_by(|a, b| {
            if sort_order == "desc" {
                b.function.depth.cmp(&a.function.depth)
            } else {
                a.function.depth.cmp(&b.function.depth)
            }
        }),
        "name" => sorted.sort_by(|a, b| {
            let name_a = a.function.name.as_deref().unwrap_or("");
            let name_b = b.function.name.as_deref().unwrap_or("");
            if sort_order == "desc" {
                name_b.cmp(name_a)
            } else {
                name_a.cmp(name_b)
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

fn display_results(results: &[FunctionResult], format: &str, verbose: bool, quiet: bool) {
    if quiet {
        return;
    }

    match format {
        "table" => display_table(results),
        "json" => display_json(results),
        "csv" => display_csv(results),
        _ => {
            eprintln!("{}: {}", "Unknown format".red(), format);
            display_table(results);
        }
    }
}

fn display_table(results: &[FunctionResult]) {
    println!();
    println!("{}", "═".repeat(100).dimmed());
    println!(
        "{} {} functions",
        "Function Analysis".cyan().bold(),
        results.len()
    );
    println!("{}", "═".repeat(100).dimmed());
    println!();
    println!(
        "{:<60} {:>8} {:>8} {:>8} {:>8} {:>16}",
        "Function".cyan(),
        "LOC".cyan(),
        "CC".cyan(),
        "Depth".cyan(),
        "Fingerprint".cyan(),
        "File".cyan()
    );
    println!("{}", "─".repeat(100).dimmed());

    for result in results {
        let f = &result.function;

        let cc_color = if f.cc <= 5 {
            "green"
        } else if f.cc <= 10 {
            "yellow"
        } else {
            "red"
        };

        let depth_color = if f.depth <= 3 {
            "green"
        } else if f.depth <= 5 {
            "yellow"
        } else {
            "red"
        };

        let loc_color = if f.loc <= 25 {
            "green"
        } else if f.loc <= 50 {
            "yellow"
        } else {
            "red"
        };

        let name = f.name.as_deref().unwrap_or("<anonymous>");

        let cc_colored = colorize(&format!("{}", f.cc), cc_color);
        let depth_colored = colorize(&format!("{}", f.depth), depth_color);
        let loc_colored = colorize(&format!("{}", f.loc), loc_color);

        let name_display: String = format!("{}:{}", result.filename, name)
            .chars()
            .take(57)
            .collect();

        println!(
            "{:<60} {:>8} {:>8} {:>8} {:>16} {:>30}",
            name_display,
            loc_colored,
            cc_colored,
            depth_colored,
            format!("{:x}", f.fingerprint),
            result.filename.chars().take(28).collect::<String>()
        );
    }
}

fn display_json(results: &[FunctionResult]) {
    let output = serde_json::to_string_pretty(&serde_json::json!({
        "functions": results.len(),
        "results": results.iter().map(|r| {
            serde_json::json!({
                "file": &r.filename,
                "name": r.function.name,
                "loc": r.function.loc,
                "cc": r.function.cc,
                "depth": r.function.depth,
                "fingerprint": r.function.fingerprint,
                "span": {
                    "start": r.function.span.start,
                    "end": r.function.span.end,
                },
            })
        }).collect::<Vec<_>>()
    }))
    .unwrap();

    println!("{}", output);
}

fn display_csv(results: &[FunctionResult]) {
    println!("file,name,loc,cc,depth,fingerprint,start,end");

    for result in results {
        let f = &result.function;
        let name = f.name.as_deref().unwrap_or("<anonymous>");
        println!(
            "{},{},{},{},{},{},{},{}",
            result.filename, name, f.loc, f.cc, f.depth, f.fingerprint, f.span.start, f.span.end
        );
    }
}

fn colorize(text: &str, color: &str) -> ColoredString {
    match color {
        "green" => text.green(),
        "yellow" => text.yellow(),
        "red" => text.red(),
        _ => text.white(),
    }
}

#[derive(Debug, Clone)]
pub struct FunctionResult {
    pub filename: String,
    pub function: FunctionMetrics,
}

#[derive(Debug, Clone)]
pub struct FunctionMetrics {
    pub name: Option<String>,
    pub span: SpanInfo,
    pub loc: u32,
    pub cc: u32,
    pub depth: u32,
    pub fingerprint: u64,
}

#[derive(Debug, Clone)]
pub struct SpanInfo {
    pub start: u32,
    pub end: u32,
}

impl FunctionResult {
    fn from(path: &Path, func: mete::NodeMetricsDto) -> Self {
        Self {
            filename: path.display().to_string(),
            function: FunctionMetrics {
                name: func.name,
                span: SpanInfo {
                    start: func.span.start,
                    end: func.span.end,
                },
                loc: func.loc,
                cc: func.cc,
                depth: func.depth,
                fingerprint: func.fingerprint,
            },
        }
    }

    fn is_complex(&self) -> bool {
        self.function.cc > 10 || (self.function.cc as f64 / self.function.loc as f64) > 0.3
    }

    fn is_large(&self) -> bool {
        self.function.loc > 50
    }

    fn is_deep(&self) -> bool {
        self.function.depth > 3
    }
}
