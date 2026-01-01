use colored::*;
use mete::EntropyRules;
use std::fs;
use std::path::Path;

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

    let results = if path.is_file() {
        analyze_file_entropy(path, language, quiet)
    } else if path.is_dir() {
        analyze_directory_entropy(path, language, pattern, quiet)
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

    // Sort by metric mass (descending - most confusing first)
    let mut sorted_results = results;
    sorted_results.sort_by(|a, b| b.metric_mass.partial_cmp(&a.metric_mass).unwrap());

    // Apply top_n limit if specified
    let display_results = if let Some(n) = top_n {
        sorted_results.into_iter().take(n).collect()
    } else {
        sorted_results
    };

    display_entropy_results(&display_results, format, quiet);

    Ok(())
}

#[derive(Debug, Clone)]
pub struct EntropyFileResult {
    pub path: String,
    pub entropy: f64,
    pub metric_mass: f64,
    pub node_count: u32,
    pub unique_symbols: usize,
}

fn analyze_file_entropy(
    path: &Path,
    language: Option<&str>,
    quiet: bool,
) -> Vec<EntropyFileResult> {
    let mut results: Vec<EntropyFileResult> = Vec::new();

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

    if text.trim().is_empty() {
        return results;
    }

    let lang = language.unwrap_or_else(|| detect_language(path));
    let lang_id = match mete::LanguageId::from_str(lang) {
        Ok(id) => id,
        Err(_) => {
            if !quiet {
                eprintln!(
                    "{} {}: {}",
                    "Unsupported language".yellow(),
                    path.display().to_string().dimmed(),
                    lang
                );
            }
            return results;
        }
    };

    let mut adapter = match mete::TreeSitterAdapter::new(lang_id) {
        Ok(a) => a,
        Err(_) => {
            if !quiet {
                eprintln!(
                    "{} {}",
                    "Failed to create parser".red(),
                    path.display().to_string().dimmed()
                );
            }
            return results;
        }
    };

    let source_text = match mete::SourceText::new(text) {
        Ok(t) => t,
        Err(_) => {
            if !quiet {
                eprintln!(
                    "{} {}",
                    "Invalid source text".red(),
                    path.display().to_string().dimmed()
                );
            }
            return results;
        }
    };

    let distribution = match adapter.extract_symbol_distribution(&source_text) {
        Ok(d) => d,
        Err(_) => {
            if !quiet {
                eprintln!(
                    "{} {}",
                    "Parse failed".red(),
                    path.display().to_string().dimmed()
                );
            }
            return results;
        }
    };

    let (entropy, metric_mass, node_count) = match EntropyRules::analyze(distribution) {
        Ok(r) => r,
        Err(_) => {
            return results;
        }
    };

    results.push(EntropyFileResult {
        path: path.display().to_string(),
        entropy: entropy.as_f64(),
        metric_mass: metric_mass.as_f64(),
        node_count: node_count.as_u32(),
        unique_symbols: 0,
    });

    results
}

fn analyze_directory_entropy(
    dir: &Path,
    language: Option<&str>,
    pattern: &str,
    quiet: bool,
) -> Vec<EntropyFileResult> {
    let pattern = dir.join(pattern);
    let pattern_str = pattern.to_string_lossy().to_string();

    let entries = match glob::glob(&pattern_str) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("{} {}", "Invalid pattern".red(), e);
            return Vec::new();
        }
    };

    // Collect all file paths first for parallel processing
    let file_paths: Vec<_> = entries
        .filter_map(|entry| match entry {
            Ok(path) if path.is_file() => Some(path),
            _ => None,
        })
        .collect();

    if file_paths.is_empty() {
        return Vec::new();
    }

    // Parallel file analysis using rayon
    use rayon::prelude::*;

    let results: Vec<EntropyFileResult> = file_paths
        .par_iter()
        .filter_map(|path| {
            let result = analyze_file_entropy(path, language, quiet);
            if result.is_empty() {
                None
            } else {
                Some(result.into_iter().next().unwrap())
            }
        })
        .collect();

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

fn display_entropy_results(results: &[EntropyFileResult], format: &str, quiet: bool) {
    if quiet {
        return;
    }

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
                "path": r.path,
                "entropy": r.entropy,
                "metric_mass": r.metric_mass,
                "node_count": r.node_count,
                "unique_symbols": r.unique_symbols
            })
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

fn display_csv(results: &[EntropyFileResult]) {
    println!("path,entropy,metric_mass,node_count,unique_symbols");
    for r in results {
        println!(
            "{},{:.4},{:.4},{},{}",
            r.path, r.entropy, r.metric_mass, r.node_count, r.unique_symbols
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

    // Summary
    let avg_entropy: f64 = results.iter().map(|r| r.entropy).sum::<f64>() / results.len() as f64;
    let max_mass = results
        .iter()
        .map(|r| r.metric_mass)
        .max_by(|a, b| a.partial_cmp(b).unwrap())
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
    println!("{}", "─".repeat(105).dimmed());
    println!(
        "{:<55} {:>10} {:>10} {:>10} {:>12} {:>10}",
        "File".cyan(),
        "Entropy".cyan(),
        "Mass (M)".cyan(),
        "Nodes".cyan(),
        "Unique Types".cyan(),
        "Level".cyan()
    );
    println!("{}", "─".repeat(105).dimmed());

    for result in results {
        let level = classify_entropy(result.entropy);
        let level_color = match level {
            "simple" => "green",
            "moderate" => "yellow",
            "complex" => "red",
            _ => "white",
        };

        let name: String = result.path.chars().take(52).collect();

        println!(
            "{:<55} {:>10.2} {:>10.2} {:>10} {:>12} {:>10}",
            name,
            result.entropy,
            result.metric_mass,
            result.node_count,
            result.unique_symbols,
            colorize(level, level_color)
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

fn colorize(text: &str, color: &str) -> ColoredString {
    match color {
        "green" => text.green(),
        "yellow" => text.yellow(),
        "red" => text.red(),
        _ => text.white(),
    }
}
