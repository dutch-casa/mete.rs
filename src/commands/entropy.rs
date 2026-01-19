//! Entropy command implementation.

use super::common::is_skippable;
use mete::lang::Language;
use colored::*;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Run entropy analysis on files/directories
pub fn run_entropy(
    path: &str,
    language: Option<&str>,
    pattern: &str,
    top_n: Option<usize>,
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

    let lang = language.and_then(Language::from_str);

    let results = if path.is_file() {
        analyze_file_entropy(path, lang, quiet)
    } else if path.is_dir() {
        analyze_directory_entropy(path, lang, pattern, quiet)
    } else {
        eprintln!("{}", "Error: Path must be a file or directory".red());
        std::process::exit(1);
    };

    if results.is_empty() {
        if !quiet {
            println!("{}", "No files analyzed".yellow());
        }
        return Ok(());
    }

    let mut sorted_results = results;
    sorted_results.sort_by(|a, b| b.metric_mass.total_cmp(&a.metric_mass));

    let display_results = if let Some(n) = top_n {
        sorted_results.into_iter().take(n).collect()
    } else {
        sorted_results
    };

    if !quiet {
        display_entropy_results(&display_results, format);
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub struct EntropyFileResult {
    pub path: PathBuf,
    pub entropy: f64,
    pub metric_mass: f64,
    pub node_count: u32,
    pub unique_types: usize,
}

fn analyze_file_entropy(
    path: &Path,
    language: Option<Language>,
    quiet: bool,
) -> Vec<EntropyFileResult> {
    let lang = match language.or_else(|| Language::from_path(path)) {
        Some(l) => l,
        None => {
            if !quiet {
                eprintln!("{} {}", "Unknown language for".yellow(), path.display().to_string().dimmed());
            }
            return Vec::new();
        }
    };

    let source = match fs::read(path) {
        Ok(s) => s,
        Err(e) => {
            if !quiet {
                eprintln!("{} {}: {}", "Error reading file".red(), path.display().to_string().dimmed(), e);
            }
            return Vec::new();
        }
    };

    let tree = match parse_source(&lang, &source) {
        Some(t) => t,
        None => {
            if !quiet {
                eprintln!("{} {}", "Parse failed".red(), path.display().to_string().dimmed());
            }
            return Vec::new();
        }
    };

    let (type_counts, node_count) = count_node_types(&tree);
    if node_count == 0 {
        return Vec::new();
    }

    let (entropy, metric_mass) = compute_entropy(&type_counts, node_count);

    vec![EntropyFileResult {
        path: path.to_path_buf(),
        entropy,
        metric_mass,
        node_count,
        unique_types: type_counts.len(),
    }]
}

fn parse_source(lang: &Language, source: &[u8]) -> Option<tree_sitter::Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang.tree_sitter_language()).ok()?;
    parser.parse(source, None)
}

fn count_node_types(tree: &tree_sitter::Tree) -> (HashMap<&str, u32>, u32) {
    let mut type_counts: HashMap<&str, u32> = HashMap::new();
    let mut node_count: u32 = 0;
    let mut cursor = tree.walk();

    loop {
        let kind = cursor.node().kind();
        *type_counts.entry(kind).or_insert(0) += 1;
        node_count += 1;

        if cursor.goto_first_child() {
            continue;
        }

        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                return (type_counts, node_count);
            }
        }
    }
}

fn compute_entropy(type_counts: &HashMap<&str, u32>, node_count: u32) -> (f64, f64) {
    let total = node_count as f64;
    let entropy: f64 = type_counts
        .values()
        .map(|&count| {
            let p = count as f64 / total;
            -p * p.log2()
        })
        .sum();

    let metric_mass = entropy * total.ln();
    (entropy, metric_mass)
}

fn analyze_directory_entropy(
    dir: &Path,
    language: Option<Language>,
    pattern: &str,
    quiet: bool,
) -> Vec<EntropyFileResult> {
    let glob_pattern = dir.join(pattern);
    let pattern_str = glob_pattern.to_string_lossy().to_string();

    let entries = match glob::glob(&pattern_str) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("{} {}", "Invalid pattern".red(), e);
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
        .filter_map(|path| analyze_file_entropy(path, language, quiet).into_iter().next())
        .collect()
}

fn display_entropy_results(results: &[EntropyFileResult], format: &str) {
    match format {
        "json" => display_json(results),
        "csv" => display_csv(results),
        _ => display_table(results),
    }
}

fn display_json(results: &[EntropyFileResult]) {
    let output: Vec<_> = results
        .iter()
        .map(|r| {
            serde_json::json!({
                "path": r.path.display().to_string(),
                "entropy": r.entropy,
                "metric_mass": r.metric_mass,
                "node_count": r.node_count,
                "unique_types": r.unique_types
            })
        })
        .collect();
    match serde_json::to_string_pretty(&output) {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("JSON serialization failed: {}", e),
    }
}

fn display_csv(results: &[EntropyFileResult]) {
    println!("path,entropy,metric_mass,node_count,unique_types");
    for r in results {
        println!(
            "{},{:.4},{:.4},{},{}",
            r.path.display(),
            r.entropy,
            r.metric_mass,
            r.node_count,
            r.unique_types
        );
    }
}

fn display_table(results: &[EntropyFileResult]) {
    println!();
    println!("{}", "═".repeat(95).dimmed());
    println!(
        "{} {} files analyzed",
        "Structural Entropy Report".cyan().bold(),
        results.len()
    );
    println!(
        "{}",
        "Sorted by Metric Mass (M) - most complex/confusing files first".dimmed()
    );
    println!("{}", "═".repeat(95).dimmed());
    println!();

    let avg_entropy: f64 = results.iter().map(|r| r.entropy).sum::<f64>() / results.len() as f64;
    let max_mass = results
        .iter()
        .map(|r| r.metric_mass)
        .max_by(|a, b| a.total_cmp(b))
        .unwrap_or(0.0);

    println!(
        "{}  {:.2}  {}  {:.2}",
        "Avg Entropy:".cyan(),
        avg_entropy,
        "Max Mass:".cyan(),
        max_mass
    );
    println!();

    // Table header
    println!("{}", "─".repeat(95).dimmed());
    println!(
        "{:<55} {:>10} {:>10} {:>10} {:>10}",
        "File".cyan(),
        "Entropy".cyan(),
        "Mass (M)".cyan(),
        "Nodes".cyan(),
        "Types".cyan(),
    );
    println!("{}", "─".repeat(95).dimmed());

    for result in results {
        let level = classify_entropy(result.entropy);
        let entropy_colored = match level {
            "simple" => format!("{:.2}", result.entropy).green(),
            "moderate" => format!("{:.2}", result.entropy).yellow(),
            _ => format!("{:.2}", result.entropy).red(),
        };

        let name = result
            .path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let name: String = name.chars().take(52).collect();

        println!(
            "{:<55} {:>10} {:>10.2} {:>10} {:>10}",
            name,
            entropy_colored,
            result.metric_mass,
            result.node_count,
            result.unique_types,
        );
    }
}

fn classify_entropy(entropy: f64) -> &'static str {
    if entropy <= 2.0 {
        "simple"
    } else if entropy > 6.0 {
        "complex"
    } else {
        "moderate"
    }
}
