//! AI-friendly refactoring targets.

use super::common::is_skippable;
use mete::data::SingleFileResult;
use mete::lang::Language;
use mete::walk::Walker;
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

pub fn run_targets(
    path: &str,
    language: Option<&str>,
    pattern: &str,
    limit: usize,
    min_cc: u32,
    quiet: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(path);

    if !path.exists() {
        eprintln!("Error: Path does not exist: {}", path.display());
        std::process::exit(1);
    }

    let lang = language.and_then(Language::from_str);

    let results = if path.is_file() {
        analyze_file(path, lang, quiet)
    } else if path.is_dir() {
        analyze_directory(path, lang, pattern, quiet)
    } else {
        eprintln!("Error: Path must be a file or directory");
        std::process::exit(1);
    };

    let mut targets = collect_targets(&results, min_cc);
    targets.sort_by(|a, b| b.priority.partial_cmp(&a.priority).unwrap());
    targets.truncate(limit);

    print_targets_json(&targets);

    Ok(())
}

#[derive(Debug)]
struct RefactorTarget {
    file: String,
    name: String,
    start_line: u32,
    end_line: u32,
    loc: u32,
    cc: u32,
    cognitive: u32,
    depth: u32,
    priority: f64,
    reason: String,
}

fn collect_targets(results: &[SingleFileResult], min_cc: u32) -> Vec<RefactorTarget> {
    let mut targets = Vec::new();

    for result in results {
        for func in &result.functions {
            if func.cc < min_cc {
                continue;
            }

            // Priority: weighted combo of CC, Cognitive, and LOC
            // Higher = more urgent to refactor
            let priority = (func.cc as f64 * 2.0)
                + (func.cognitive as f64 * 1.5)
                + (func.loc as f64 * 0.1)
                + (func.depth as f64 * 1.0);

            let reason = determine_reason(func.cc, func.cognitive, func.loc, func.depth);

            targets.push(RefactorTarget {
                file: result.path.display().to_string(),
                name: func.name.clone().unwrap_or_else(|| "<anonymous>".to_string()),
                start_line: func.start_line,
                end_line: func.end_line,
                loc: func.loc,
                cc: func.cc,
                cognitive: func.cognitive,
                depth: func.depth,
                priority,
                reason,
            });
        }
    }

    targets
}

fn determine_reason(cc: u32, cognitive: u32, loc: u32, depth: u32) -> String {
    let mut reasons = Vec::new();

    if cc > 10 {
        reasons.push(format!("high cyclomatic complexity ({})", cc));
    } else if cc > 5 {
        reasons.push(format!("moderate complexity ({})", cc));
    }

    if cognitive > 15 {
        reasons.push(format!("high cognitive load ({})", cognitive));
    }

    if loc > 50 {
        reasons.push(format!("large function ({} lines)", loc));
    }

    if depth > 4 {
        reasons.push(format!("deeply nested (depth {})", depth));
    }

    if reasons.is_empty() {
        "minor complexity".to_string()
    } else {
        reasons.join(", ")
    }
}

fn print_targets_json(targets: &[RefactorTarget]) {
    let json_targets: Vec<_> = targets
        .iter()
        .map(|t| {
            serde_json::json!({
                "file": t.file,
                "name": t.name,
                "lines": { "start": t.start_line, "end": t.end_line },
                "metrics": {
                    "cc": t.cc,
                    "cognitive": t.cognitive,
                    "loc": t.loc,
                    "depth": t.depth
                },
                "priority": (t.priority * 10.0).round() / 10.0,
                "reason": t.reason
            })
        })
        .collect();

    let output = serde_json::json!({
        "targets": json_targets,
        "count": targets.len()
    });

    match serde_json::to_string_pretty(&output) {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("JSON error: {}", e),
    }
}

fn analyze_file(path: &Path, language: Option<Language>, quiet: bool) -> Vec<SingleFileResult> {
    let lang = language.or_else(|| Language::from_path(path));
    let lang = match lang {
        Some(l) => l,
        None => {
            if !quiet {
                eprintln!("Unknown language for {}", path.display());
            }
            return Vec::new();
        }
    };

    let source = match fs::read(path) {
        Ok(s) => s,
        Err(e) => {
            if !quiet {
                eprintln!("Error reading {}: {}", path.display(), e);
            }
            return Vec::new();
        }
    };

    let mut walker = match Walker::new(lang) {
        Ok(w) => w,
        Err(e) => {
            if !quiet {
                eprintln!("Walker failed for {}: {}", path.display(), e);
            }
            return Vec::new();
        }
    };

    match walker.analyze(path.to_path_buf(), &source) {
        Ok(result) => vec![result],
        Err(e) => {
            if !quiet {
                eprintln!("Analysis failed for {}: {}", path.display(), e);
            }
            Vec::new()
        }
    }
}

fn analyze_directory(
    dir: &Path,
    language: Option<Language>,
    pattern: &str,
    quiet: bool,
) -> Vec<SingleFileResult> {
    let glob_pattern = dir.join(pattern);
    let pattern_str = glob_pattern.to_string_lossy().to_string();

    let entries = match glob::glob(&pattern_str) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Invalid pattern: {}", e);
            return Vec::new();
        }
    };

    let file_paths: Vec<PathBuf> = entries
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
        .filter_map(|path| analyze_file(path, language, quiet).into_iter().next())
        .collect()
}
