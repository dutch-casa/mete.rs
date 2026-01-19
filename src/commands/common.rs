//! Shared utilities for command modules.

use mete::data::SingleFileResult;
use mete::lang::Language;
use mete::walk::Walker;
use colored::*;
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

pub use mete::is_skippable;

/// Analyze a single file.
pub fn analyze_file(path: &Path, language: Option<Language>, quiet: bool) -> Vec<SingleFileResult> {
    let lang = language.or_else(|| Language::from_path(path));
    let lang = match lang {
        Some(l) => l,
        None => {
            if !quiet {
                eprintln!(
                    "{} {}",
                    "Unknown language for".yellow(),
                    path.display().to_string().dimmed()
                );
            }
            return Vec::new();
        }
    };

    let source = match fs::read(path) {
        Ok(s) => s,
        Err(e) => {
            if !quiet {
                eprintln!(
                    "{} {}: {}",
                    "Error reading file".red(),
                    path.display().to_string().dimmed(),
                    e
                );
            }
            return Vec::new();
        }
    };

    let mut walker = match Walker::new(lang) {
        Ok(w) => w,
        Err(e) => {
            if !quiet {
                eprintln!(
                    "{} {}: {}",
                    "Walker creation failed".red(),
                    path.display().to_string().dimmed(),
                    e
                );
            }
            return Vec::new();
        }
    };

    match walker.analyze(path.to_path_buf(), &source) {
        Ok(result) => vec![result],
        Err(e) => {
            if !quiet {
                eprintln!(
                    "{} {}: {}",
                    "Analysis failed".yellow(),
                    path.display().to_string().dimmed(),
                    e
                );
            }
            Vec::new()
        }
    }
}

/// Analyze all files in a directory matching a glob pattern.
pub fn analyze_directory(
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
        .filter_map(|path| analyze_file(path, language, quiet).into_iter().next())
        .collect()
}
