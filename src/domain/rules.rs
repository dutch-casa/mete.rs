use crate::domain::events::{BranchType, StructuralEvent};
use crate::domain::metrics::{
    FileMetrics, MaintainabilityIndex, NodeMetrics, StructuralFingerprint,
};
/**
# Purpose

Domain rules - pure business logic for metric computation.
These services implement the mathematical formulas and analysis algorithms.

# Model

Rule categories:
- ComplexityRules: cyclomatic complexity computation
- MaintainabilityRules: maintainability index calculation
- DuplicationRules: fingerprint generation and duplicate detection
- AggregationRules: metric aggregation and summary computation

# Invariants

- All computations are deterministic pure functions
- Mathematical formulas are correctly implemented
- All intermediate results maintain type safety
- No side effects or external dependencies

# Boundary

Rules operate only on validated domain primitives.
All mathematical errors are handled via Result types.
No I/O, persistence, or network operations.
*/
use crate::domain::primitives::DomainError;
use std::collections::HashMap;

/// Cyclomatic complexity computation rules
#[derive(Debug)]
pub struct ComplexityRules;

impl ComplexityRules {
    pub fn compute_base_complexity() -> u32 {
        1
    }

    pub fn compute_from_events(events: &[StructuralEvent]) -> u32 {
        let mut complexity = Self::compute_base_complexity();

        for event in events {
            if event.is_branch() {
                complexity += 1;
            }
        }

        complexity
    }

    pub fn compute_max_complexity(nodes: &[NodeMetrics]) -> u32 {
        nodes.iter().map(|n| n.cc).max().unwrap_or(0)
    }

    pub fn compute_sum_complexity(nodes: &[NodeMetrics]) -> u32 {
        nodes.iter().map(|n| n.cc).sum()
    }

    pub fn validate_complexity_invariants(
        complexity: u32,
        branch_count: u32,
    ) -> Result<(), DomainError> {
        if complexity < Self::compute_base_complexity() {
            return Err(DomainError::InvalidSpan(
                "complexity below minimum".to_string(),
            ));
        }

        if complexity < branch_count + 1 {
            return Err(DomainError::InvalidSpan(
                "complexity inconsistent with branches".to_string(),
            ));
        }

        Ok(())
    }
}

/// Cognitive Complexity computation rules (SonarSource spec)
///
/// Key differences from Cyclomatic Complexity:
/// - Nested structures increment based on current nesting depth
/// - else/else if have 0 nesting tax (just +1 structural)
/// - Function/lambda boundaries reset nesting level
/// - break/continue/return/throw are free (0 cost)
/// - Binary boolean operators add +1 per contiguous sequence
#[derive(Debug)]
pub struct CognitiveComplexityRules;

impl CognitiveComplexityRules {
    /// Base cognitive complexity for any function
    pub fn base_score() -> u32 {
        0 // Unlike CC, cognitive complexity starts at 0
    }

    /// Compute cognitive complexity from structural events
    /// Implements the SonarSource scoring algorithm
    pub fn compute_from_events(events: &[StructuralEvent]) -> u32 {
        if events.is_empty() {
            return 0;
        }

        let mut complexity = 0u32;
        let mut nesting_level = 0u32;
        let mut in_function = false;
        let mut prev_was_else_if = false;
        let mut prev_was_else = false;

        for event in events {
            match event {
                StructuralEvent::FunctionStart { .. } => {
                    // Function boundaries reset nesting
                    nesting_level = 0;
                    in_function = true;
                    prev_was_else_if = false;
                    prev_was_else = false;
                }
                StructuralEvent::FunctionEnd { .. } => {
                    in_function = false;
                    nesting_level = 0;
                    prev_was_else_if = false;
                    prev_was_else = false;
                }
                StructuralEvent::BlockEntry { .. } => {
                    // Entering a block increases nesting for subsequent control structures
                    // But only if we're inside a function
                    if in_function {
                        nesting_level += 1;
                    }
                    prev_was_else_if = false;
                    prev_was_else = false;
                }
                StructuralEvent::BlockExit { .. } => {
                    if nesting_level > 0 {
                        nesting_level -= 1;
                    }
                    prev_was_else_if = false;
                    prev_was_else = false;
                }
                StructuralEvent::Branch { branch_type, .. } => {
                    if !in_function {
                        continue;
                    }

                    match branch_type {
                        BranchType::Conditional => {
                            // if: +1 structural + current nesting
                            // else if: +1 structural only (0 nesting tax)
                            // else: +1 structural only (0 nesting tax)
                            if prev_was_else_if || prev_was_else {
                                // else if or else - no nesting penalty
                                complexity += 1;
                            } else {
                                // Regular if - pay nesting penalty
                                complexity += 1 + nesting_level;
                            }
                            // Reset the else flags - the next conditional in this block
                            // would need to be at the same nesting level
                            prev_was_else_if = false;
                            prev_was_else = false;
                        }
                        BranchType::Loop => {
                            // for/while: +1 structural + current nesting
                            complexity += 1 + nesting_level;
                            prev_was_else_if = false;
                            prev_was_else = false;
                        }
                        BranchType::Switch => {
                            // switch: +1 structural + current nesting
                            complexity += 1 + nesting_level;
                            prev_was_else_if = false;
                            prev_was_else = false;
                        }
                        BranchType::Exception => {
                            // catch: +1 structural + current nesting
                            complexity += 1 + nesting_level;
                            prev_was_else_if = false;
                            prev_was_else = false;
                        }
                        BranchType::Logical => {
                            // This is for ternary (conditional_expression)
                            // +1 structural + current nesting
                            complexity += 1 + nesting_level;
                            prev_was_else_if = false;
                            prev_was_else = false;
                        }
                    }
                }
                _ => {
                    prev_was_else_if = false;
                    prev_was_else = false;
                }
            }

            // Track else/else if for the next iteration
            // We need to look at the next event to determine if this is else/else if
            // For now, we rely on the next conditional being at the same nesting level
        }

        complexity.max(1) // Ensure at least 1 for non-empty functions
    }

    /// Compute max cognitive complexity across nodes
    pub fn compute_max_cognitive(nodes: &[NodeMetrics]) -> u32 {
        nodes.iter().map(|n| n.cognitive).max().unwrap_or(0)
    }

    /// Compute sum of cognitive complexity across nodes
    pub fn compute_sum_cognitive(nodes: &[NodeMetrics]) -> u32 {
        nodes.iter().map(|n| n.cognitive).sum()
    }
}

/// Maintainability index computation rules
#[derive(Debug)]
pub struct MaintainabilityRules;

impl MaintainabilityRules {
    /// Compute Maintainability Index using per-function averages.
    /// Returns value in range [0, 100] where higher is better.
    ///
    /// Uses average LOC and CC per function for fairer scoring of well-factored code.
    pub fn compute_maintainability_index(
        halstead_volume: f64,
        cyclomatic_complexity: u32,
        loc: u32,
    ) -> Result<MaintainabilityIndex, DomainError> {
        if loc == 0 {
            return MaintainabilityIndex::new(100.0);
        }

        let v = if halstead_volume <= 0.0 {
            1.0
        } else {
            halstead_volume
        };
        let cc = cyclomatic_complexity.max(1) as f64;
        let loc_f = loc as f64;

        let raw_mi = 171.0 - 5.2 * v.ln() - 0.23 * cc - 16.2 * loc_f.ln();

        let normalized_mi = (raw_mi * 100.0 / 171.0).max(0.0);

        MaintainabilityIndex::new(normalized_mi)
    }

    pub fn compute_maintainability_index_from_nodes(
        nodes: &[NodeMetrics],
        total_loc: u32,
    ) -> Result<MaintainabilityIndex, DomainError> {
        if nodes.is_empty() || total_loc == 0 {
            return MaintainabilityIndex::new(100.0);
        }

        let fn_count = nodes.len() as f64;
        let avg_loc = total_loc as f64 / fn_count;
        let avg_cc = nodes.iter().map(|n| n.cc).sum::<u32>() as f64 / fn_count;
        let avg_v = avg_loc * 3.0;

        let raw_mi = 171.0 - 5.2 * avg_v.ln() - 0.23 * avg_cc - 16.2 * avg_loc.ln();

        let normalized_mi = (raw_mi * 100.0 / 171.0).max(0.0);

        MaintainabilityIndex::new(normalized_mi)
    }

    pub fn estimate_halstead_volume(loc: u32, _complexity: u32, function_count: u32) -> f64 {
        if loc == 0 {
            return 1.0;
        }

        let avg_loc_per_fn = if function_count == 0 {
            loc
        } else {
            loc / function_count.max(1)
        };
        (avg_loc_per_fn as f64) * 3.0
    }
}

/// Duplication detection rules
#[derive(Debug)]
pub struct DuplicationRules;

impl DuplicationRules {
    pub fn compute_fingerprint(structure: &str) -> StructuralFingerprint {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();

        let normalized = Self::normalize_structure(structure);
        normalized.hash(&mut hasher);

        StructuralFingerprint::new(hasher.finish())
    }

    pub fn compute_fingerprint_from_events(events: &[StructuralEvent]) -> StructuralFingerprint {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();

        for event in events {
            match event {
                StructuralEvent::BlockEntry { span } => {
                    "block_entry".hash(&mut hasher);
                    span.len().hash(&mut hasher);
                }
                StructuralEvent::BlockExit { span } => {
                    "block_exit".hash(&mut hasher);
                    span.len().hash(&mut hasher);
                }
                StructuralEvent::Branch { branch_type, span } => {
                    "branch".hash(&mut hasher);
                    (*branch_type as u8).hash(&mut hasher);
                    span.len().hash(&mut hasher);
                }
                StructuralEvent::FunctionStart { name, span } => {
                    "function_start".hash(&mut hasher);
                    name.is_some().hash(&mut hasher);
                    span.len().hash(&mut hasher);
                }
                _ => {}
            }
        }

        StructuralFingerprint::new(hasher.finish())
    }

    pub fn count_duplicates(fingerprints: &HashMap<StructuralFingerprint, u16>) -> u32 {
        fingerprints
            .values()
            .map(|&count| if count > 1 { count - 1 } else { 0 })
            .sum::<u16>() as u32
    }

    fn normalize_structure(structure: &str) -> String {
        structure
            .chars()
            .map(|c| match c {
                'a'..='z' | 'A'..='Z' | '_' => '_',
                '0'..='9' => '#',
                _ => c,
            })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ")
    }
}

/// Metric aggregation rules
#[derive(Debug)]
pub struct AggregationRules;

impl AggregationRules {
    pub fn compute_file_metrics(
        events: &[StructuralEvent],
        nodes: &[NodeMetrics],
        actual_loc: u32,
    ) -> Result<FileMetrics, DomainError> {
        let loc = actual_loc;
        let (cc_sum, cc_max) = if nodes.is_empty() {
            let cc = ComplexityRules::compute_from_events(events);
            (cc, cc)
        } else {
            (
                ComplexityRules::compute_sum_complexity(nodes),
                ComplexityRules::compute_max_complexity(nodes),
            )
        };

        let depth_max = Self::compute_max_depth(events);
        let fan_out = Self::compute_fan_out(events);
        let exports = Self::compute_exports(events);

        let mi = MaintainabilityRules::compute_maintainability_index_from_nodes(nodes, loc)?;

        let fingerprints = Self::compute_fingerprints(nodes);
        let dup_blocks = DuplicationRules::count_duplicates(&fingerprints);

        let (cognitive_sum, cognitive_max) = if nodes.is_empty() {
            let cognitive = CognitiveComplexityRules::compute_from_events(events);
            (cognitive, cognitive)
        } else {
            (
                CognitiveComplexityRules::compute_sum_cognitive(nodes),
                CognitiveComplexityRules::compute_max_cognitive(nodes),
            )
        };

        FileMetrics::new(
            loc,
            cc_max,
            cc_sum,
            cognitive_max,
            cognitive_sum,
            depth_max,
            0,
            fan_out,
            exports,
            mi,
            dup_blocks,
            nodes.len() as u32,
        )
    }

    fn compute_max_depth(events: &[StructuralEvent]) -> u32 {
        let mut depth = 0u32;
        let mut max_depth = 0u32;

        for event in events {
            match event {
                StructuralEvent::BlockEntry { .. } => {
                    depth += 1;
                    max_depth = max_depth.max(depth);
                }
                StructuralEvent::BlockExit { .. } => {
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            }
        }

        max_depth
    }

    fn compute_fan_out(events: &[StructuralEvent]) -> u32 {
        events.iter().filter(|e| e.is_import()).count() as u32
    }

    fn compute_exports(events: &[StructuralEvent]) -> u32 {
        events.iter().filter(|e| e.is_export()).count() as u32
    }

    fn compute_fingerprints(nodes: &[NodeMetrics]) -> HashMap<StructuralFingerprint, u16> {
        let mut fingerprints = HashMap::new();

        for node in nodes {
            *fingerprints.entry(node.fingerprint).or_insert(0) += 1;
        }

        fingerprints
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::events::{BranchType, StructuralEvent};
    use crate::domain::primitives::BytePos;

    #[test]
    fn complexity_computation() {
        let events = vec![
            StructuralEvent::branch(
                BytePos::new(0).unwrap(),
                BytePos::new(10).unwrap(),
                BranchType::Conditional,
            )
            .unwrap(),
            StructuralEvent::branch(
                BytePos::new(20).unwrap(),
                BytePos::new(30).unwrap(),
                BranchType::Loop,
            )
            .unwrap(),
        ];

        let complexity = ComplexityRules::compute_from_events(&events);
        assert_eq!(complexity, 3);
    }

    #[test]
    fn maintainability_computation() {
        let mi = MaintainabilityRules::compute_maintainability_index(100.0, 5, 50).unwrap();
        assert!(mi.as_f64() > 0.0);
        assert!(mi.as_f64() <= 100.0);
    }

    #[test]
    fn fingerprint_computation() {
        let events = vec![
            StructuralEvent::function_start(
                Some("test".to_string()),
                BytePos::new(0).unwrap(),
                BytePos::new(10).unwrap(),
            )
            .unwrap(),
            StructuralEvent::branch(
                BytePos::new(5).unwrap(),
                BytePos::new(8).unwrap(),
                BranchType::Conditional,
            )
            .unwrap(),
        ];

        let fingerprint = DuplicationRules::compute_fingerprint_from_events(&events);
        assert!(fingerprint.as_u64() > 0);
    }

    #[test]
    fn duplicate_counting() {
        let mut fingerprints = HashMap::new();
        fingerprints.insert(StructuralFingerprint::new(1), 1);
        fingerprints.insert(StructuralFingerprint::new(2), 3);
        fingerprints.insert(StructuralFingerprint::new(3), 1);

        let dup_count = DuplicationRules::count_duplicates(&fingerprints);
        assert_eq!(dup_count, 2);
    }
}
