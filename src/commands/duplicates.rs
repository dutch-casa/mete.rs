use colored::*;
use mete::{AnalysisService, AnalyzeRequest, WantFlags};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub fn run_duplicates(
    path: &str,
    language: Option<&str>,
    pattern: &str,
    min_instances: u32,
    show_code: bool,
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

    let filtered_dups = filter_by_instances(&results, min_instances);

    if filtered_dups.is_empty() && !quiet {
        println!("{}", "No duplicates found".green());
        return Ok(());
    }

    display_results(&filtered_dups, show_code, format, verbose, quiet);

    Ok(())
}

fn analyze_file(
    path: &Path,
    language: Option<&str>,
    verbose: bool,
    quiet: bool,
) -> Vec<DuplicateResult> {
    let mut results: Vec<DuplicateResult> = Vec::new();

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
            if let Some(duplicates) = response.duplicates {
                for dup in duplicates {
                    results.push(DuplicateResult::from(path, dup));
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
) -> Vec<DuplicateResult> {
    let pattern = dir.join(pattern);
    let pattern_str = pattern.to_string_lossy().to_string();

    let entries = match glob::glob(&pattern_str) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("{} {}", "Invalid pattern".red(), e);
            return Vec::new();
        }
    };

    let mut results: Vec<DuplicateResult> = Vec::new();

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
                        if let Some(duplicates) = response.duplicates {
                            for dup in duplicates {
                                results.push(DuplicateResult::from(&path, dup));
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

fn filter_by_instances(results: &[DuplicateResult], min_instances: u32) -> Vec<DuplicateResult> {
    results
        .iter()
        .filter(|r| r.instances.len() as u32 >= min_instances)
        .cloned()
        .collect()
}

fn display_results(
    results: &[DuplicateResult],
    show_code: bool,
    format: &str,
    _verbose: bool,
    quiet: bool,
) {
    if quiet {
        return;
    }

    match format {
        "table" => display_table(results, show_code),
        "json" => display_json(results),
        "csv" => display_csv(results),
        _ => {
            eprintln!("{}: {}", "Unknown format".red(), format);
            display_table(results, show_code);
        }
    }
}

fn display_table(results: &[DuplicateResult], show_code: bool) {
    let total_instances: usize = results.iter().map(|r| r.instances.len()).sum();
    let total_lines_saved: u32 = results
        .iter()
        .map(|r| {
            let first_loc = r
                .instances
                .first()
                .map(|i| i.end_line - i.start_line)
                .unwrap_or(0);
            (r.instances.len() - 1) as u32 * first_loc
        })
        .sum();

    println!();
    println!("{}", "═".repeat(95).dimmed());
    println!(
        "{} {} duplicate groups, {} total instances, {} lines could be saved",
        "Code Duplication Report".cyan().bold(),
        results.len(),
        total_instances,
        total_lines_saved
    );
    println!("{}", "═".repeat(95).dimmed());
    println!();

    for result in results {
        let severity = if result.instances.len() >= 5 {
            "red"
        } else if result.instances.len() >= 3 {
            "yellow"
        } else {
            "green"
        };

        let instances_str = format!("{} instances", result.instances.len());
        println!(
            "{} {} (fingerprint: {})",
            colorize(&instances_str, severity),
            result.filename.cyan(),
            format!("{:x}", result.fingerprint)
        );

        for instance in &result.instances {
            let name = instance.name.as_deref().unwrap_or("unnamed").dimmed();
            println!(
                "    {} {} lines {}-{} ({} lines)",
                "→".dimmed(),
                name,
                instance.start_line.to_string().white(),
                instance.end_line.to_string().white(),
                (instance.end_line - instance.start_line)
                    .to_string()
                    .white()
            );
        }

        if show_code {
            if let Some(_first_instance) = result.instances.first() {
                if let Some(content) = &result.code_snippet {
                    println!();
                    println!("    {}", "Code snippet:".dimmed());
                    for line in content.lines().take(10) {
                        println!("    {}", line.dimmed());
                    }
                    if content.lines().count() > 10 {
                        println!("    {}", "...".dimmed());
                    }
                }
            }
        }

        println!();
    }
}

fn display_json(results: &[DuplicateResult]) {
    let total_instances: usize = results.iter().map(|r| r.instances.len()).sum();

    let output = serde_json::to_string_pretty(&serde_json::json!({
        "summary": {
            "duplicate_groups": results.len(),
            "total_instances": total_instances,
        },
        "duplicates": results.iter().map(|r| {
            serde_json::json!({
                "fingerprint": r.fingerprint,
                "file": &r.filename,
                "instances": r.instances.iter().map(|i| {
                    serde_json::json!({
                        "name": i.name,
                        "start_line": i.start_line,
                        "end_line": i.end_line,
                        "span": {
                            "start": i.span.start,
                            "end": i.span.end,
                        }
                    })
                }).collect::<Vec<_>>()
            })
        }).collect::<Vec<_>>()
    }))
    .unwrap();

    println!("{}", output);
}

fn display_csv(results: &[DuplicateResult]) {
    println!("fingerprint,file,instance_name,start_line,end_line");

    for result in results {
        for instance in &result.instances {
            let name = instance.name.as_deref().unwrap_or("unnamed");
            println!(
                "{},{},{},{},{}",
                result.fingerprint, result.filename, name, instance.start_line, instance.end_line
            );
        }
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
pub struct DuplicateResult {
    pub fingerprint: u64,
    pub filename: String,
    pub instances: Vec<DuplicateInstance>,
    pub code_snippet: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DuplicateInstance {
    pub name: Option<String>,
    pub start_line: u32,
    pub end_line: u32,
    pub span: SpanInfo,
}

#[derive(Debug, Clone)]
pub struct SpanInfo {
    pub start: u32,
    pub end: u32,
}

impl DuplicateResult {
    fn from(path: &Path, dup: mete::DuplicateGroupDto) -> Self {
        Self {
            fingerprint: dup.fingerprint,
            filename: path.display().to_string(),
            instances: dup
                .instances
                .into_iter()
                .map(|i| DuplicateInstance {
                    name: i.name,
                    start_line: i.start_line,
                    end_line: i.end_line,
                    span: SpanInfo {
                        start: i.span.start,
                        end: i.span.end,
                    },
                })
                .collect(),
            code_snippet: None,
        }
    }
}
