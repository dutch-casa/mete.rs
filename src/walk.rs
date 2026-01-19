//! Single-pass tree-sitter AST walker.
//!
//! Computes metrics during traversal - no intermediate event representation.
//! Key insight: Metrics are counters updated during walk, not collected then processed.

use crate::data::{FunctionData, SingleFileResult, WalkState};
use crate::lang::{BranchKind, Language, LanguageSpec};
use crate::metrics;
use std::collections::HashMap;
use std::path::PathBuf;
use tree_sitter::{Node, Parser};

/// Error type for walk operations.
#[derive(Debug)]
pub enum WalkError {
    ParseFailed(String),
    LanguageSetupFailed(String),
}

impl std::fmt::Display for WalkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WalkError::ParseFailed(msg) => write!(f, "Parse failed: {}", msg),
            WalkError::LanguageSetupFailed(msg) => write!(f, "Language setup failed: {}", msg),
        }
    }
}

impl std::error::Error for WalkError {}

/// Single-pass AST walker.
/// Collects all metrics in one traversal.
pub struct Walker {
    parser: Parser,
    spec: LanguageSpec,
}

impl Walker {
    /// Create a new walker for the given language.
    pub fn new(language: Language) -> Result<Self, WalkError> {
        let mut parser = Parser::new();
        let ts_lang = language.tree_sitter_language();

        parser.set_language(&ts_lang).map_err(|e| {
            WalkError::LanguageSetupFailed(format!("Failed to set language: {}", e))
        })?;

        Ok(Self {
            parser,
            spec: language.spec(),
        })
    }

    /// Analyze a file and return metrics.
    pub fn analyze(&mut self, path: PathBuf, source: &[u8]) -> Result<SingleFileResult, WalkError> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| WalkError::ParseFailed("Failed to parse source".to_string()))?;

        let mut state = WalkState::default();
        let mut functions: Vec<FunctionData> = Vec::new();
        let mut function_stack: Vec<FunctionContext> = Vec::new();

        // Count actual lines (excluding blanks, including comments)
        let loc = metrics::count_lines(source);
        state.loc = loc;

        let mut cursor = tree.walk();
        self.walk_iterative(&mut state, &mut functions, &mut function_stack, &mut cursor, source);

        // Compute file-level metrics
        let (cc_max, cc_sum, cognitive_max, cognitive_sum) = if functions.is_empty() {
            (state.cc.max(1), state.cc.max(1), state.cognitive, state.cognitive)
        } else {
            let cc_max = functions.iter().map(|f| f.cc).max().unwrap_or(1);
            let cc_sum = functions.iter().map(|f| f.cc).sum::<u32>().max(1);
            let cognitive_max = functions.iter().map(|f| f.cognitive).max().unwrap_or(0);
            let cognitive_sum = functions.iter().map(|f| f.cognitive).sum();
            (cc_max, cc_sum, cognitive_max, cognitive_sum)
        };

        // Count duplicates within file
        let dup_count = count_within_file_duplicates(&functions);

        // Compute MI
        let mi = if functions.is_empty() {
            metrics::maintainability_index(
                metrics::estimate_halstead_volume(loc),
                cc_sum,
                loc,
            )
        } else {
            let fn_count = functions.len() as f64;
            let avg_loc = functions.iter().map(|f| f.loc).sum::<u32>() as f64 / fn_count;
            let avg_cc = cc_sum as f64 / fn_count;
            metrics::maintainability_index_from_averages(avg_loc, avg_cc, functions.len() as u32)
        };

        Ok(SingleFileResult {
            path,
            loc,
            cc_max,
            cc_sum,
            cognitive_max,
            cognitive_sum,
            depth_max: state.max_depth,
            imports: state.imports,
            exports: state.exports,
            mi,
            function_count: functions.len() as u32,
            dup_count,
            functions,
        })
    }

    /// Iterative tree walk (avoids stack overflow on deeply nested code).
    fn walk_iterative(
        &self,
        state: &mut WalkState,
        functions: &mut Vec<FunctionData>,
        function_stack: &mut Vec<FunctionContext>,
        cursor: &mut tree_sitter::TreeCursor,
        source: &[u8],
    ) {
        loop {
            let node = cursor.node();
            let kind = node.kind();

            // Process node entry
            self.process_entry(state, functions, function_stack, &node, kind, source);

            // Descend to first child
            if cursor.goto_first_child() {
                continue;
            }

            // Process node exit
            self.process_exit(state, functions, function_stack, &node, kind, source);

            // Try siblings and parents
            loop {
                if cursor.goto_next_sibling() {
                    break;
                }
                if !cursor.goto_parent() {
                    return;
                }
                let parent = cursor.node();
                let parent_kind = parent.kind();
                self.process_exit(state, functions, function_stack, &parent, parent_kind, source);
            }
        }
    }

    fn process_entry(
        &self,
        state: &mut WalkState,
        _functions: &mut Vec<FunctionData>,
        function_stack: &mut Vec<FunctionContext>,
        node: &Node,
        kind: &str,
        source: &[u8],
    ) {
        if self.spec.is_function(kind) {
            self.handle_function_entry(state, function_stack, node);
        }

        if self.spec.is_block(kind) {
            state.enter_block();
            if state.in_function {
                state.update_fingerprint(hash_kind("block"));
            }
        }

        if self.spec.is_branch(kind) {
            self.handle_branch_entry(state, kind);
        }

        if self.spec.is_import(kind) {
            state.imports += 1;
        }

        if self.spec.is_export(kind) {
            state.exports += 1;
        }

        if !self.spec.is_boolean_and(kind) && !self.spec.is_boolean_or(kind) {
            state.reset_bool_chain();
        }

        // Include identifiers and literals in fingerprint for better duplicate detection
        if state.in_function {
            if is_identifier_or_literal(kind) {
                if let Ok(text) = node.utf8_text(source) {
                    state.update_fingerprint(hash_content(text));
                }
            }
        }
    }

    fn handle_function_entry(
        &self,
        state: &mut WalkState,
        function_stack: &mut Vec<FunctionContext>,
        node: &Node,
    ) {
        let start_line = node.start_position().row as u32 + 1;

        if state.in_function {
            function_stack.push(FunctionContext {
                name: None,
                start_line: state.fn_start_line,
                cc: state.fn_cc,
                cognitive: state.fn_cognitive,
                max_depth: state.fn_max_depth,
                depth_at_start: state.fn_depth_at_start,
                fingerprint: state.fingerprint_hash,
            });
        }

        state.start_function(node.start_byte() as u32, start_line);
        state.update_fingerprint(hash_kind("function"));
    }

    fn handle_branch_entry(&self, state: &mut WalkState, kind: &str) {
        let branch_kind = self.spec.classify_branch(kind);

        if branch_kind.adds_cc() {
            state.record_branch();
        }

        self.update_cognitive_for_branch(state, branch_kind);

        if state.in_function {
            state.update_fingerprint(hash_kind(kind));
        }
    }

    fn update_cognitive_for_branch(&self, state: &mut WalkState, branch_kind: BranchKind) {
        match branch_kind {
            BranchKind::Conditional | BranchKind::Loop | BranchKind::Switch | BranchKind::Exception => {
                state.record_cognitive(branch_kind.adds_nesting_penalty());
                state.enter_cognitive_nesting();
            }
            BranchKind::SwitchCase | BranchKind::Else => {
                state.cognitive += 1;
                if state.in_function {
                    state.fn_cognitive += 1;
                }
                if matches!(branch_kind, BranchKind::Else) {
                    state.enter_cognitive_nesting();
                }
            }
            BranchKind::BooleanAnd | BranchKind::BooleanOr => {
                let is_and = matches!(branch_kind, BranchKind::BooleanAnd);
                if state.record_bool_op(is_and) {
                    state.cognitive += 1;
                    if state.in_function {
                        state.fn_cognitive += 1;
                    }
                }
            }
        }
    }

    /// Process a node on exit (ascending from it).
    fn process_exit(
        &self,
        state: &mut WalkState,
        functions: &mut Vec<FunctionData>,
        function_stack: &mut Vec<FunctionContext>,
        node: &Node,
        kind: &str,
        source: &[u8],
    ) {
        // Block exit
        if self.spec.is_block(kind) {
            state.exit_block();
        }

        // Branch exit - decrease cognitive nesting
        if self.spec.is_branch(kind) {
            let branch_kind = self.spec.classify_branch(kind);
            if branch_kind.increases_nesting() {
                state.exit_cognitive_nesting();
            }
        }

        // Function end
        if self.spec.is_function(kind) {
            let name = self.extract_function_name(node, source);
            let end_line = node.end_position().row as u32 + 1;
            let start_line = state.fn_start_line;
            let loc = (end_line - start_line + 1).max(1);

            let (cc, cognitive, depth, fingerprint) = state.end_function();

            functions.push(FunctionData {
                name,
                start_line,
                end_line,
                loc,
                cc,
                cognitive,
                depth,
                fingerprint,
            });

            // Restore parent function context if nested
            if let Some(ctx) = function_stack.pop() {
                state.in_function = true;
                state.fn_start_line = ctx.start_line;
                state.fn_cc = ctx.cc;
                state.fn_cognitive = ctx.cognitive;
                state.fn_max_depth = ctx.max_depth;
                state.fn_depth_at_start = ctx.depth_at_start;
                state.fingerprint_hash = ctx.fingerprint;
            }
        }
    }

    /// Extract function name from AST node.
    fn extract_function_name(&self, node: &Node, source: &[u8]) -> Option<String> {
        const NAME_KINDS: &[&str] = &["identifier", "name", "property_identifier"];

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if NAME_KINDS.contains(&child.kind()) {
                if let Ok(text) = child.utf8_text(source) {
                    return Some(text.to_string());
                }
            }
        }
        None
    }
}

/// Context for nested function tracking.
#[derive(Debug)]
struct FunctionContext {
    #[allow(dead_code)]
    name: Option<String>,
    start_line: u32,
    cc: u32,
    cognitive: u32,
    max_depth: u32,
    depth_at_start: u32,
    fingerprint: u64,
}

/// Simple string hash for fingerprint updates.
#[inline]
fn hash_kind(kind: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325; // FNV offset basis
    for byte in kind.bytes() {
        h ^= byte as u64;
        h = h.wrapping_mul(0x100000001b3); // FNV prime
    }
    h
}

/// Hash content (identifiers, literals) for fingerprinting.
#[inline]
fn hash_content(text: &str) -> u64 {
    let mut h: u64 = 0x811c9dc5; // Different seed than hash_kind
    for byte in text.bytes() {
        h ^= byte as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Check if node kind is an identifier or literal that should be fingerprinted.
#[inline]
fn is_identifier_or_literal(kind: &str) -> bool {
    matches!(
        kind,
        "identifier"
            | "field_identifier"
            | "type_identifier"
            | "property_identifier"
            | "string_literal"
            | "string"
            | "integer_literal"
            | "number"
            | "float_literal"
            | "boolean"
            | "true"
            | "false"
    )
}

/// Count duplicates within a single file (same fingerprint = duplicate).
fn count_within_file_duplicates(functions: &[FunctionData]) -> u32 {
    if functions.len() < 2 {
        return 0;
    }

    let mut fingerprint_counts: HashMap<u64, u32> = HashMap::new();
    for f in functions {
        *fingerprint_counts.entry(f.fingerprint).or_insert(0) += 1;
    }

    // Count how many are duplicates (count > 1)
    fingerprint_counts
        .values()
        .filter(|&&count| count > 1)
        .map(|&count| count - 1) // N copies means N-1 duplicates
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyze_simple_rust() {
        let source = b"fn main() { if true { } }";
        let mut walker = Walker::new(Language::Rust).unwrap();
        let result = walker.analyze(PathBuf::from("test.rs"), source).unwrap();

        assert_eq!(result.function_count, 1);
        assert!(result.cc_max >= 1); // At least base + if
    }

    #[test]
    fn analyze_nested_functions() {
        let source = b"fn outer() { fn inner() { } }";
        let mut walker = Walker::new(Language::Rust).unwrap();
        let result = walker.analyze(PathBuf::from("test.rs"), source).unwrap();

        // Should find 2 functions
        assert_eq!(result.function_count, 2);
    }

    #[test]
    fn analyze_python() {
        let source = b"def hello():\n    if True:\n        pass\n";
        let mut walker = Walker::new(Language::Python).unwrap();
        let result = walker.analyze(PathBuf::from("test.py"), source).unwrap();

        assert_eq!(result.function_count, 1);
    }

    #[test]
    fn hash_determinism() {
        // Same input should always produce same hash
        let h1 = hash_kind("function");
        let h2 = hash_kind("function");
        assert_eq!(h1, h2);

        // Different input should produce different hash
        let h3 = hash_kind("block");
        assert_ne!(h1, h3);
    }

    #[test]
    fn duplicate_detection() {
        let functions = vec![
            FunctionData {
                name: Some("a".to_string()),
                start_line: 1,
                end_line: 5,
                loc: 5,
                cc: 1,
                cognitive: 0,
                depth: 1,
                fingerprint: 12345,
            },
            FunctionData {
                name: Some("b".to_string()),
                start_line: 6,
                end_line: 10,
                loc: 5,
                cc: 1,
                cognitive: 0,
                depth: 1,
                fingerprint: 12345, // Same fingerprint = duplicate
            },
            FunctionData {
                name: Some("c".to_string()),
                start_line: 11,
                end_line: 15,
                loc: 5,
                cc: 1,
                cognitive: 0,
                depth: 1,
                fingerprint: 99999, // Different
            },
        ];

        let dup_count = count_within_file_duplicates(&functions);
        assert_eq!(dup_count, 1); // One duplicate pair
    }
}
