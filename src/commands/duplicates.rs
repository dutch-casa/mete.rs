//! Duplicates command implementation.

use super::common::{analyze_directory, analyze_file};
use mete::data::{FunctionData, SingleFileResult};
use mete::dup::DuplicateIndex;
use mete::lang::Language;
use colored::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub fn run_duplicates(
    path: &str,
    language: Option<&str>,
    pattern: &str,
    min_instances: u32,
    show_code: bool,
    format: &str,
    threshold: Option<f32>,
    cross_file: bool,
    min_loc: u32,
    include_anonymous: bool,
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
    let similarity_threshold = threshold.unwrap_or(0.8);

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

    let groups = if cross_file {
        find_cross_file_duplicates(&results, similarity_threshold, min_instances, min_loc, include_anonymous)
    } else {
        find_within_file_duplicates(&results, min_instances, min_loc, include_anonymous)
    };

    if groups.is_empty() && !quiet {
        println!("{}", "No duplicates found".green());
        return Ok(());
    }

    if !quiet {
        display_results(&groups, &results, show_code, format);
    }

    Ok(())
}

fn find_cross_file_duplicates(
    results: &[SingleFileResult],
    threshold: f32,
    min_instances: u32,
    min_loc: u32,
    include_anonymous: bool,
) -> Vec<DuplicateGroupResult> {
    let mut index = DuplicateIndex::new(threshold);

    for (file_idx, result) in results.iter().enumerate() {
        for (fn_idx, func) in result.functions.iter().enumerate() {
            if func.loc < min_loc {
                continue;
            }
            if !include_anonymous && func.name.is_none() {
                continue;
            }

            let location = mete::dup::FunctionLocation {
                file_idx: file_idx as u32,
                fn_idx: fn_idx as u32,
            };

            let tokens = vec![func.fingerprint];
            index.add(location, func.fingerprint, &tokens);
        }
    }

    let groups = index.find_similar_duplicates();

    groups
        .into_iter()
        .filter(|g| g.instances.len() as u32 >= min_instances)
        .map(|g| {
            let instances: Vec<DuplicateInstanceResult> = g
                .instances
                .iter()
                .map(|(loc, similarity)| {
                    let result = &results[loc.file_idx as usize];
                    let func = &result.functions[loc.fn_idx as usize];
                    DuplicateInstanceResult {
                        file_path: result.path.clone(),
                        name: func.name.clone(),
                        start_line: func.start_line,
                        end_line: func.end_line,
                        similarity: *similarity,
                    }
                })
                .collect();

            DuplicateGroupResult {
                fingerprint: results[g.canonical.file_idx as usize].functions[g.canonical.fn_idx as usize]
                    .fingerprint,
                similarity: g.similarity,
                instances,
            }
        })
        .collect()
}

fn find_within_file_duplicates(
    results: &[SingleFileResult],
    min_instances: u32,
    min_loc: u32,
    include_anonymous: bool,
) -> Vec<DuplicateGroupResult> {
    let mut all_groups: Vec<DuplicateGroupResult> = Vec::new();

    for result in results {
        let mut by_fingerprint: HashMap<u64, Vec<&FunctionData>> = HashMap::new();
        for func in &result.functions {
            if func.loc < min_loc {
                continue;
            }
            if !include_anonymous && func.name.is_none() {
                continue;
            }
            by_fingerprint.entry(func.fingerprint).or_default().push(func);
        }

        for (fingerprint, funcs) in by_fingerprint {
            if funcs.len() as u32 >= min_instances {
                let instances: Vec<DuplicateInstanceResult> = funcs
                    .iter()
                    .map(|f| DuplicateInstanceResult {
                        file_path: result.path.clone(),
                        name: f.name.clone(),
                        start_line: f.start_line,
                        end_line: f.end_line,
                        similarity: 1.0,
                    })
                    .collect();

                all_groups.push(DuplicateGroupResult {
                    fingerprint,
                    similarity: 1.0,
                    instances,
                });
            }
        }
    }

    all_groups.sort_by(|a, b| b.instances.len().cmp(&a.instances.len()));
    all_groups
}

fn display_results(
    groups: &[DuplicateGroupResult],
    _results: &[SingleFileResult],
    show_code: bool,
    format: &str,
) {
    match format {
        "table" => display_table(groups, show_code),
        "json" => display_json(groups),
        "csv" => display_csv(groups),
        _ => {
            eprintln!("{}: {}", "Unknown format".red(), format);
            display_table(groups, show_code);
        }
    }
}

fn display_table(groups: &[DuplicateGroupResult], _show_code: bool) {
    let total_instances: usize = groups.iter().map(|g| g.instances.len()).sum();
    let total_lines_saved: u32 = groups
        .iter()
        .map(|g| {
            let first_loc = g
                .instances
                .first()
                .map(|i| i.end_line.saturating_sub(i.start_line))
                .unwrap_or(0);
            (g.instances.len().saturating_sub(1)) as u32 * first_loc
        })
        .sum();

    println!();
    println!("{}", "═".repeat(95).dimmed());
    println!(
        "{} {} duplicate groups, {} total instances, ~{} lines could be saved",
        "Code Duplication Report".cyan().bold(),
        groups.len(),
        total_instances,
        total_lines_saved
    );
    println!("{}", "═".repeat(95).dimmed());
    println!();

    for group in groups {
        let severity = if group.instances.len() >= 5 {
            "red"
        } else if group.instances.len() >= 3 {
            "yellow"
        } else {
            "green"
        };

        let instances_str = format!("{} instances", group.instances.len());
        let colored_str = match severity {
            "red" => instances_str.red(),
            "yellow" => instances_str.yellow(),
            _ => instances_str.green(),
        };

        println!(
            "{} (fingerprint: {:x}, similarity: {:.0}%)",
            colored_str,
            group.fingerprint,
            group.similarity * 100.0
        );

        for instance in &group.instances {
            let name = instance.name.as_deref().unwrap_or("<anonymous>");
            let file_name = instance
                .file_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            println!(
                "    {} {} in {} lines {}-{} ({} lines)",
                "→".dimmed(),
                name.dimmed(),
                file_name.cyan(),
                instance.start_line.to_string().white(),
                instance.end_line.to_string().white(),
                instance.end_line.saturating_sub(instance.start_line)
            );
        }

        println!();
    }
}

fn display_json(groups: &[DuplicateGroupResult]) {
    let total_instances: usize = groups.iter().map(|g| g.instances.len()).sum();

    let output = serde_json::json!({
        "summary": {
            "duplicate_groups": groups.len(),
            "total_instances": total_instances,
        },
        "duplicates": groups.iter().map(|g| {
            serde_json::json!({
                "fingerprint": g.fingerprint,
                "similarity": g.similarity,
                "instances": g.instances.iter().map(|i| {
                    serde_json::json!({
                        "file": i.file_path.display().to_string(),
                        "name": i.name,
                        "start_line": i.start_line,
                        "end_line": i.end_line,
                        "similarity": i.similarity,
                    })
                }).collect::<Vec<_>>()
            })
        }).collect::<Vec<_>>()
    });

    match serde_json::to_string_pretty(&output) {
        Ok(json_string) => println!("{}", json_string),
        Err(e) => eprintln!("{}: {}", "Failed to serialize JSON".red(), e),
    }
}

fn display_csv(groups: &[DuplicateGroupResult]) {
    println!("fingerprint,similarity,file,name,start_line,end_line");

    for group in groups {
        for instance in &group.instances {
            let name = instance.name.as_deref().unwrap_or("");
            println!(
                "{:x},{:.2},{},{},{},{}",
                group.fingerprint,
                group.similarity,
                instance.file_path.display(),
                name,
                instance.start_line,
                instance.end_line
            );
        }
    }
}

#[derive(Debug, Clone)]
struct DuplicateGroupResult {
    fingerprint: u64,
    similarity: f32,
    instances: Vec<DuplicateInstanceResult>,
}

#[derive(Debug, Clone)]
struct DuplicateInstanceResult {
    file_path: PathBuf,
    name: Option<String>,
    start_line: u32,
    end_line: u32,
    similarity: f32,
}
