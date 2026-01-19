//! Mete: Structural metrics engine for code quality analysis.
//!
//! Data-oriented design: flat arrays, single-pass traversal, no intermediate events.
//!
//! # Architecture
//!
//! - `data`: SoA data structures for results
//! - `metrics`: Pure metric computation functions
//! - `lang`: Language specifications and tree-sitter integration
//! - `walk`: Single-pass AST walker
//! - `dup`: Cross-file duplicate detection
//! - `output`: Result formatting
//!
//! # Example
//!
//! ```rust,ignore
//! use mete::{analyze_file, Language};
//! use std::path::PathBuf;
//!
//! let result = analyze_file(PathBuf::from("src/main.rs"), Language::Rust)?;
//! println!("MI: {}, CC: {}", result.mi, result.cc_max);
//! ```

pub mod data;
pub mod dup;
pub mod lang;
pub mod metrics;
pub mod output;
pub mod walk;

// Re-exports for convenience
pub use data::{FileResults, FunctionData, FunctionResults, SingleFileResult, StringInterner};
pub use dup::{DuplicateGroup, DuplicateIndex, FunctionLocation};
pub use lang::{BranchKind, Language, LanguageSpec};
pub use metrics::{
    maintainability_index, maintainability_index_from_averages, CcLevel, CognitiveLevel,
    DepthLevel, MiLevel,
};
pub use walk::{WalkError, Walker};

use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

/// Analyze a single file.
pub fn analyze_file(path: PathBuf, language: Language) -> Result<SingleFileResult, WalkError> {
    let source = fs::read(&path).map_err(|e| WalkError::ParseFailed(e.to_string()))?;
    let mut walker = Walker::new(language)?;
    walker.analyze(path, &source)
}

/// Analyze a single file, auto-detecting language from extension.
pub fn analyze_file_auto(path: PathBuf) -> Result<SingleFileResult, WalkError> {
    let language = Language::from_path(&path)
        .ok_or_else(|| WalkError::LanguageSetupFailed("Unknown file extension".to_string()))?;
    analyze_file(path, language)
}

/// Analyze multiple files in parallel.
pub fn analyze_files(
    paths: Vec<PathBuf>,
    language: Option<Language>,
) -> Vec<Result<SingleFileResult, WalkError>> {
    paths
        .into_par_iter()
        .map(|path| {
            let lang = language.or_else(|| Language::from_path(&path));
            match lang {
                Some(l) => analyze_file(path, l),
                None => Err(WalkError::LanguageSetupFailed(format!(
                    "Unknown language for: {}",
                    path.display()
                ))),
            }
        })
        .collect()
}

/// Analyze a directory with glob pattern.
pub fn analyze_directory(
    dir: &Path,
    pattern: &str,
    language: Option<Language>,
) -> Vec<SingleFileResult> {
    let glob_pattern = dir.join(pattern);
    let pattern_str = glob_pattern.to_string_lossy();

    let paths: Vec<PathBuf> = glob::glob(&pattern_str)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .filter(|p| p.is_file() && !is_skippable(p))
        .collect();

    analyze_files(paths, language)
        .into_iter()
        .filter_map(|r| r.ok())
        .collect()
}

const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "dist",
    "build",
    ".next",
    ".cache",
    "target",
    ".git",
    "__pycache__",
    ".venv",
    "venv",
];

pub fn is_skippable(path: &Path) -> bool {
    path.components().any(|c| {
        if let std::path::Component::Normal(s) = c {
            SKIP_DIRS.iter().any(|skip| s == *skip)
        } else {
            false
        }
    })
}

/// Collect results into SoA structures.
pub fn collect_results(results: Vec<SingleFileResult>) -> (FileResults, FunctionResults, StringInterner) {
    let mut file_results = FileResults::with_capacity(results.len());
    let mut fn_results = FunctionResults::with_capacity(results.len() * 10);
    let mut interner = StringInterner::with_capacity(results.len() * 256);

    for (file_idx, result) in results.into_iter().enumerate() {
        file_results.push(
            result.path.clone(),
            result.loc,
            result.cc_max,
            result.cc_sum,
            result.cognitive_max,
            result.cognitive_sum,
            result.depth_max,
            result.imports,
            result.exports,
            result.mi,
            result.function_count,
            result.dup_count,
        );

        for func in result.functions {
            let (name_offset, name_len) = if let Some(ref name) = func.name {
                interner.intern(name)
            } else {
                (0, 0)
            };

            fn_results.push(
                file_idx as u32,
                name_offset,
                name_len,
                func.start_line,
                func.end_line,
                func.loc,
                func.cc,
                func.cognitive,
                func.depth,
                func.fingerprint,
            );
        }
    }

    (file_results, fn_results, interner)
}

/// Build cross-file duplicate index from function results.
pub fn build_duplicate_index(
    fn_results: &FunctionResults,
    threshold: f32,
) -> DuplicateIndex {
    let mut index = DuplicateIndex::new(threshold);

    for i in fn_results.indices() {
        let location = FunctionLocation {
            file_idx: fn_results.file_idx[i],
            fn_idx: i as u32,
        };

        // Use fingerprint as the token set (simplified - real implementation
        // would extract structure tokens during walk)
        let tokens = vec![fn_results.fingerprint[i]];

        index.add(location, fn_results.fingerprint[i], &tokens);
    }

    index
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skippable_paths() {
        assert!(is_skippable(Path::new("foo/node_modules/bar.js")));
        assert!(is_skippable(Path::new("project/target/debug/main")));
        assert!(!is_skippable(Path::new("src/main.rs")));
    }

    #[test]
    fn language_detection() {
        assert_eq!(
            Language::from_path(Path::new("test.rs")),
            Some(Language::Rust)
        );
        assert_eq!(
            Language::from_path(Path::new("test.py")),
            Some(Language::Python)
        );
        assert_eq!(Language::from_path(Path::new("test.txt")), None);
    }
}
