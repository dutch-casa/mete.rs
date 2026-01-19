//! Functions command implementation.

use super::common::{analyze_directory, analyze_file};
use mete::data::{FunctionData, SingleFileResult};
use mete::lang::Language;
use mete::output::{colorize_cc, colorize_cognitive, colorize_depth};
use colored::*;
use std::path::{Path, PathBuf};

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
        analyze_file(path, lang, quiet)
    } else if path.is_dir() {
        analyze_directory(path, lang, pattern, quiet)
    } else {
        eprintln!("{}", "Error: Path must be a file or directory".red());
        std::process::exit(1);
    };

    let mut functions = flatten_functions(&results);
    apply_filters(&mut functions, complex, large, deep, min_complexity, min_loc);
    sort_functions(&mut functions, sort_by, sort_order);

    if functions.is_empty() && !quiet {
        println!("{}", "No functions found matching criteria".yellow());
        return Ok(());
    }

    if !quiet {
        display_functions(&functions, format);
    }

    Ok(())
}

#[derive(Clone)]
struct FunctionWithFile {
    file_path: PathBuf,
    function: FunctionData,
}

fn flatten_functions(results: &[SingleFileResult]) -> Vec<FunctionWithFile> {
    results
        .iter()
        .flat_map(|r| {
            r.functions.iter().map(|f| FunctionWithFile {
                file_path: r.path.clone(),
                function: f.clone(),
            })
        })
        .collect()
}

fn apply_filters(
    functions: &mut Vec<FunctionWithFile>,
    complex: bool,
    large: bool,
    deep: bool,
    min_complexity: Option<u32>,
    min_loc: Option<u32>,
) {
    if complex {
        functions.retain(|f| mete::metrics::is_complex(f.function.cc, f.function.loc));
    }
    if large {
        functions.retain(|f| mete::metrics::is_large(f.function.loc));
    }
    if deep {
        functions.retain(|f| mete::metrics::is_deeply_nested(f.function.depth));
    }
    if let Some(min_cc) = min_complexity {
        functions.retain(|f| f.function.cc >= min_cc);
    }
    if let Some(min) = min_loc {
        functions.retain(|f| f.function.loc >= min);
    }
}

fn sort_functions(functions: &mut [FunctionWithFile], sort_by: &str, sort_order: &str) {
    let cmp = |a: &FunctionWithFile, b: &FunctionWithFile| match sort_by {
        "cc" => a.function.cc.cmp(&b.function.cc),
        "cog" => a.function.cognitive.cmp(&b.function.cognitive),
        "loc" => a.function.loc.cmp(&b.function.loc),
        "depth" => a.function.depth.cmp(&b.function.depth),
        "name" => a.function.name.cmp(&b.function.name),
        "path" | _ => a.file_path.cmp(&b.file_path),
    };

    if sort_order == "desc" {
        functions.sort_by(|a, b| cmp(b, a));
    } else {
        functions.sort_by(cmp);
    }
}

fn display_functions(functions: &[FunctionWithFile], format: &str) {
    match format {
        "table" => display_table(functions),
        "json" => display_json(functions),
        "csv" => display_csv(functions),
        _ => {
            eprintln!("{}: {}", "Unknown format".red(), format);
            display_table(functions);
        }
    }
}

fn display_table(functions: &[FunctionWithFile]) {
    println!();
    println!("{}", "Function Metrics".cyan().bold());
    println!("{}", "─".repeat(110).dimmed());
    println!(
        "{:<40} {:>30} {:>6} {:>6} {:>6} {:>6} {:>10}",
        "Function".cyan(),
        "File".cyan(),
        "LOC".cyan(),
        "CC".cyan(),
        "COG".cyan(),
        "Depth".cyan(),
        "Lines".cyan()
    );
    println!("{}", "─".repeat(110).dimmed());

    for f in functions {
        let name = f.function.name.as_deref().unwrap_or("<anonymous>");
        let file_name = f.file_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        let cc_colored = colorize_cc(f.function.cc);
        let cog_colored = colorize_cognitive(f.function.cognitive);
        let depth_colored = colorize_depth(f.function.depth);

        println!(
            "{:<40} {:>30} {:>6} {:>6} {:>6} {:>6} {:>5}-{:<4}",
            name.chars().take(38).collect::<String>(),
            file_name.chars().take(28).collect::<String>(),
            f.function.loc,
            cc_colored,
            cog_colored,
            depth_colored,
            f.function.start_line,
            f.function.end_line
        );
    }
}

fn display_json(functions: &[FunctionWithFile]) {
    let json_functions: Vec<_> = functions
        .iter()
        .map(|f| {
            serde_json::json!({
                "name": f.function.name,
                "file": f.file_path.display().to_string(),
                "start_line": f.function.start_line,
                "end_line": f.function.end_line,
                "loc": f.function.loc,
                "cc": f.function.cc,
                "cognitive": f.function.cognitive,
                "depth": f.function.depth,
                "fingerprint": f.function.fingerprint,
            })
        })
        .collect();

    let json = serde_json::json!({ "functions": json_functions });

    match serde_json::to_string_pretty(&json) {
        Ok(output) => println!("{}", output),
        Err(e) => eprintln!("{}", format!("Error serializing JSON: {}", e).red()),
    }
}

fn display_csv(functions: &[FunctionWithFile]) {
    println!("name,file,start_line,end_line,loc,cc,cognitive,depth,fingerprint");

    for f in functions {
        println!(
            "{},{},{},{},{},{},{},{},{}",
            f.function.name.as_deref().unwrap_or(""),
            f.file_path.display(),
            f.function.start_line,
            f.function.end_line,
            f.function.loc,
            f.function.cc,
            f.function.cognitive,
            f.function.depth,
            f.function.fingerprint
        );
    }
}
