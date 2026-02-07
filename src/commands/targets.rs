//! AI-friendly refactoring targets.

use super::common::{analyze_directory, analyze_file};
use mete::data::SingleFileResult;
use mete::lang::Language;
use std::path::Path;

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

    let lang = language.and_then(Language::from_name);

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
