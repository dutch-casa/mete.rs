//! Data-oriented structures using Struct of Arrays (SoA) layout.
//!
//! Principle: Data dominates. Flat arrays, no heap-per-item allocations.

use std::path::PathBuf;

/// Per-file results stored contiguously in parallel arrays.
/// SoA layout for cache-friendly iteration.
#[derive(Debug, Default)]
pub struct FileResults {
    // Parallel arrays - one entry per file
    pub paths: Vec<PathBuf>,
    pub loc: Vec<u32>,
    pub cc_max: Vec<u32>,
    pub cc_sum: Vec<u32>,
    pub cognitive_max: Vec<u32>,
    pub cognitive_sum: Vec<u32>,
    pub depth_max: Vec<u32>,
    pub imports: Vec<u32>,
    pub exports: Vec<u32>,
    pub mi: Vec<u8>, // 0-100, no wrapper
    pub function_count: Vec<u32>,
    pub dup_count: Vec<u32>,
}

impl FileResults {
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            paths: Vec::with_capacity(cap),
            loc: Vec::with_capacity(cap),
            cc_max: Vec::with_capacity(cap),
            cc_sum: Vec::with_capacity(cap),
            cognitive_max: Vec::with_capacity(cap),
            cognitive_sum: Vec::with_capacity(cap),
            depth_max: Vec::with_capacity(cap),
            imports: Vec::with_capacity(cap),
            exports: Vec::with_capacity(cap),
            mi: Vec::with_capacity(cap),
            function_count: Vec::with_capacity(cap),
            dup_count: Vec::with_capacity(cap),
        }
    }

    pub fn len(&self) -> usize {
        self.paths.len()
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Push a complete file result. All arrays grow together.
    #[allow(clippy::too_many_arguments)]
    pub fn push(
        &mut self,
        path: PathBuf,
        loc: u32,
        cc_max: u32,
        cc_sum: u32,
        cognitive_max: u32,
        cognitive_sum: u32,
        depth_max: u32,
        imports: u32,
        exports: u32,
        mi: u8,
        function_count: u32,
        dup_count: u32,
    ) {
        self.paths.push(path);
        self.loc.push(loc);
        self.cc_max.push(cc_max);
        self.cc_sum.push(cc_sum);
        self.cognitive_max.push(cognitive_max);
        self.cognitive_sum.push(cognitive_sum);
        self.depth_max.push(depth_max);
        self.imports.push(imports);
        self.exports.push(exports);
        self.mi.push(mi);
        self.function_count.push(function_count);
        self.dup_count.push(dup_count);
    }

    /// Merge another FileResults into this one (for parallel collection).
    pub fn extend(&mut self, other: Self) {
        self.paths.extend(other.paths);
        self.loc.extend(other.loc);
        self.cc_max.extend(other.cc_max);
        self.cc_sum.extend(other.cc_sum);
        self.cognitive_max.extend(other.cognitive_max);
        self.cognitive_sum.extend(other.cognitive_sum);
        self.depth_max.extend(other.depth_max);
        self.imports.extend(other.imports);
        self.exports.extend(other.exports);
        self.mi.extend(other.mi);
        self.function_count.extend(other.function_count);
        self.dup_count.extend(other.dup_count);
    }

    /// Iterator over file indices.
    pub fn indices(&self) -> impl Iterator<Item = usize> {
        0..self.paths.len()
    }
}

/// Per-function results for function-level commands.
/// Stores functions from all files with file index reference.
#[derive(Debug, Default)]
pub struct FunctionResults {
    pub file_idx: Vec<u32>,     // Which file this function belongs to
    pub name_offset: Vec<u32>,  // Offset into interned string table
    pub name_len: Vec<u16>,     // Length of name in string table
    pub start_line: Vec<u32>,
    pub end_line: Vec<u32>,
    pub loc: Vec<u32>,
    pub cc: Vec<u32>,
    pub cognitive: Vec<u32>,
    pub depth: Vec<u32>,
    pub fingerprint: Vec<u64>, // For cross-file duplicate detection
}

impl FunctionResults {
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            file_idx: Vec::with_capacity(cap),
            name_offset: Vec::with_capacity(cap),
            name_len: Vec::with_capacity(cap),
            start_line: Vec::with_capacity(cap),
            end_line: Vec::with_capacity(cap),
            loc: Vec::with_capacity(cap),
            cc: Vec::with_capacity(cap),
            cognitive: Vec::with_capacity(cap),
            depth: Vec::with_capacity(cap),
            fingerprint: Vec::with_capacity(cap),
        }
    }

    pub fn len(&self) -> usize {
        self.file_idx.len()
    }

    pub fn is_empty(&self) -> bool {
        self.file_idx.is_empty()
    }

    /// Push a complete function result.
    #[allow(clippy::too_many_arguments)]
    pub fn push(
        &mut self,
        file_idx: u32,
        name_offset: u32,
        name_len: u16,
        start_line: u32,
        end_line: u32,
        loc: u32,
        cc: u32,
        cognitive: u32,
        depth: u32,
        fingerprint: u64,
    ) {
        self.file_idx.push(file_idx);
        self.name_offset.push(name_offset);
        self.name_len.push(name_len);
        self.start_line.push(start_line);
        self.end_line.push(end_line);
        self.loc.push(loc);
        self.cc.push(cc);
        self.cognitive.push(cognitive);
        self.depth.push(depth);
        self.fingerprint.push(fingerprint);
    }

    /// Merge another FunctionResults into this one.
    pub fn extend(&mut self, other: Self) {
        self.file_idx.extend(other.file_idx);
        self.name_offset.extend(other.name_offset);
        self.name_len.extend(other.name_len);
        self.start_line.extend(other.start_line);
        self.end_line.extend(other.end_line);
        self.loc.extend(other.loc);
        self.cc.extend(other.cc);
        self.cognitive.extend(other.cognitive);
        self.depth.extend(other.depth);
        self.fingerprint.extend(other.fingerprint);
    }

    /// Iterator over function indices.
    pub fn indices(&self) -> impl Iterator<Item = usize> {
        0..self.file_idx.len()
    }
}

/// String interning for function names.
/// Stores all names in a single contiguous buffer.
#[derive(Debug, Default)]
pub struct StringInterner {
    data: String,
}

impl StringInterner {
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            data: String::with_capacity(cap),
        }
    }

    /// Intern a string, returning (offset, length).
    pub fn intern(&mut self, s: &str) -> (u32, u16) {
        let offset = self.data.len() as u32;
        let len = s.len().min(u16::MAX as usize) as u16;
        self.data.push_str(&s[..len as usize]);
        (offset, len)
    }

    /// Retrieve a string by offset and length.
    pub fn get(&self, offset: u32, len: u16) -> &str {
        let start = offset as usize;
        let end = start + len as usize;
        &self.data[start..end]
    }

    /// Total bytes stored.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Accumulator updated during single-pass AST walk.
/// No allocations during traversal.
#[derive(Debug, Default, Clone)]
pub struct WalkState {
    // File-level counters
    pub cc: u32,
    pub cognitive: u32,
    pub depth: u32,
    pub max_depth: u32,
    pub imports: u32,
    pub exports: u32,
    pub loc: u32,

    // Function tracking
    pub fn_start_byte: u32,
    pub fn_start_line: u32,
    pub fn_depth_at_start: u32,
    pub fn_cc: u32,
    pub fn_cognitive: u32,
    pub fn_max_depth: u32,
    pub in_function: bool,

    // Cognitive complexity tracking
    pub cognitive_nesting: u32,
    pub last_bool_op: Option<bool>, // true = AND, false = OR

    // Fingerprint accumulator (rolling hash)
    pub fingerprint_hash: u64,
}

impl WalkState {
    /// Reset for a new file.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Enter a block scope.
    pub fn enter_block(&mut self) {
        self.depth += 1;
        self.max_depth = self.max_depth.max(self.depth);
        if self.in_function {
            self.fn_max_depth = self.fn_max_depth.max(self.depth - self.fn_depth_at_start);
        }
    }

    /// Exit a block scope.
    pub fn exit_block(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    /// Record a branch (adds to CC).
    pub fn record_branch(&mut self) {
        self.cc += 1;
        if self.in_function {
            self.fn_cc += 1;
        }
    }

    /// Record cognitive complexity increment.
    pub fn record_cognitive(&mut self, nesting_penalty: bool) {
        let inc = if nesting_penalty {
            1 + self.cognitive_nesting
        } else {
            1
        };
        self.cognitive += inc;
        if self.in_function {
            self.fn_cognitive += inc;
        }
    }

    /// Enter cognitive nesting level.
    pub fn enter_cognitive_nesting(&mut self) {
        self.cognitive_nesting += 1;
    }

    /// Exit cognitive nesting level.
    pub fn exit_cognitive_nesting(&mut self) {
        self.cognitive_nesting = self.cognitive_nesting.saturating_sub(1);
    }

    /// Record a boolean operator (for cognitive complexity chains).
    /// Returns true if this is a new sequence (adds complexity).
    pub fn record_bool_op(&mut self, is_and: bool) -> bool {
        let is_new = self.last_bool_op != Some(is_and);
        self.last_bool_op = Some(is_and);
        is_new
    }

    /// Reset boolean operator chain.
    pub fn reset_bool_chain(&mut self) {
        self.last_bool_op = None;
    }

    /// Start tracking a function.
    pub fn start_function(&mut self, start_byte: u32, start_line: u32) {
        self.in_function = true;
        self.fn_start_byte = start_byte;
        self.fn_start_line = start_line;
        self.fn_depth_at_start = self.depth;
        self.fn_cc = 1; // Base complexity
        self.fn_cognitive = 0;
        self.fn_max_depth = 0;
        self.fingerprint_hash = 0;
        self.cognitive_nesting = 0;
        self.last_bool_op = None;
    }

    /// End function tracking, returns (cc, cognitive, max_depth, fingerprint).
    pub fn end_function(&mut self) -> (u32, u32, u32, u64) {
        self.in_function = false;
        let result = (
            self.fn_cc,
            self.fn_cognitive,
            self.fn_max_depth,
            self.fingerprint_hash,
        );
        self.cognitive_nesting = 0;
        self.last_bool_op = None;
        result
    }

    /// Update rolling fingerprint hash.
    pub fn update_fingerprint(&mut self, kind_hash: u64) {
        // FNV-1a inspired rolling hash
        self.fingerprint_hash = self.fingerprint_hash.wrapping_mul(0x100000001b3);
        self.fingerprint_hash ^= kind_hash;
    }
}

/// Single file analysis result (used during parallel collection).
#[derive(Debug, Clone)]
pub struct SingleFileResult {
    pub path: PathBuf,
    pub loc: u32,
    pub cc_max: u32,
    pub cc_sum: u32,
    pub cognitive_max: u32,
    pub cognitive_sum: u32,
    pub depth_max: u32,
    pub imports: u32,
    pub exports: u32,
    pub mi: u8,
    pub function_count: u32,
    pub dup_count: u32,
    pub functions: Vec<FunctionData>,
}

/// Function data collected during analysis.
#[derive(Debug, Clone)]
pub struct FunctionData {
    pub name: Option<String>,
    pub start_line: u32,
    pub end_line: u32,
    pub loc: u32,
    pub cc: u32,
    pub cognitive: u32,
    pub depth: u32,
    pub fingerprint: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_results_push() {
        let mut results = FileResults::with_capacity(2);
        results.push(
            PathBuf::from("test.rs"),
            100, 5, 10, 3, 8, 3, 2, 1, 75, 2, 0,
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results.loc[0], 100);
        assert_eq!(results.mi[0], 75);
    }

    #[test]
    fn string_interner() {
        let mut interner = StringInterner::with_capacity(64);
        let (off1, len1) = interner.intern("hello");
        let (off2, len2) = interner.intern("world");

        assert_eq!(interner.get(off1, len1), "hello");
        assert_eq!(interner.get(off2, len2), "world");
    }

    #[test]
    fn walk_state_depth() {
        let mut state = WalkState::default();
        state.enter_block();
        state.enter_block();
        assert_eq!(state.depth, 2);
        assert_eq!(state.max_depth, 2);
        state.exit_block();
        assert_eq!(state.depth, 1);
        assert_eq!(state.max_depth, 2);
    }

    #[test]
    fn walk_state_function() {
        let mut state = WalkState::default();
        state.start_function(0, 1);
        state.record_branch();
        state.record_branch();
        let (cc, cog, depth, _) = state.end_function();
        assert_eq!(cc, 3); // 1 base + 2 branches
        assert_eq!(cog, 0);
        assert_eq!(depth, 0);
    }
}
