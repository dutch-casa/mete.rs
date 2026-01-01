/**
# Purpose

Quality metrics - value objects representing computed code quality measures.
These are the deterministic outputs of the structural analysis process.

# Model

Metric categories:
- FileMetrics: aggregate measures for entire source file
- NodeMetrics: per-function/block measures
- MaintainabilityIndex: composite quality score (0-100)
- StructuralFingerprint: hash for duplication detection

# Invariants

- All metric values are non-negative except MaintainabilityIndex (0-100)
- Cyclomatic complexity ≥ branch count
- MaintainabilityIndex is always clamped to [0, 100]
- Fingerprint hashes are stable and deterministic
- LOC counts are accurate line counts (not character counts)

# Boundary

Metrics are computed by domain services from validated events.
All mathematical invariants are enforced during computation.
No external dependencies or time-based calculations.
*/
use crate::domain::primitives::{DomainError, Span};

/// Maintainability Index - composite quality score (0-100)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MaintainabilityIndex(u8);

impl MaintainabilityIndex {
    pub const MIN: Self = Self(0);
    pub const MAX: Self = Self(100);

    pub fn new(value: f64) -> Result<Self, DomainError> {
        if value.is_nan() || value.is_infinite() {
            return Err(DomainError::InvalidSpan(
                "invalid maintainability index".to_string(),
            ));
        }
        let clamped = value.clamp(0.0, 100.0);
        Ok(MaintainabilityIndex(clamped as u8))
    }

    pub fn as_f64(self) -> f64 {
        self.0 as f64
    }

    pub fn as_u8(self) -> u8 {
        self.0
    }

    pub fn is_excellent(self) -> bool {
        self.0 >= 85
    }

    pub fn is_good(self) -> bool {
        self.0 >= 70
    }

    pub fn is_moderate(self) -> bool {
        self.0 >= 50
    }

    pub fn is_poor(self) -> bool {
        self.0 < 50
    }
}

/// Structural fingerprint for duplication detection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StructuralFingerprint(u64);

impl StructuralFingerprint {
    pub fn new(hash: u64) -> Self {
        StructuralFingerprint(hash)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }

    pub fn from_structure(structure_hash: u64) -> Self {
        Self::new(structure_hash)
    }
}

/// File-level quality metrics
#[derive(Debug, Clone, PartialEq)]
pub struct FileMetrics {
    pub loc: u32,
    pub cc_max: u32,
    pub cc_sum: u32,
    pub cognitive_max: u32,
    pub cognitive_sum: u32,
    pub depth_max: u32,
    pub fan_in: u32,
    pub fan_out: u32,
    pub exports: u32,
    pub mi: MaintainabilityIndex,
    pub dup_blocks: u32,
    pub functions_count: u32,
}

impl FileMetrics {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        loc: u32,
        cc_max: u32,
        cc_sum: u32,
        cognitive_max: u32,
        cognitive_sum: u32,
        depth_max: u32,
        fan_in: u32,
        fan_out: u32,
        exports: u32,
        mi: MaintainabilityIndex,
        dup_blocks: u32,
        functions_count: u32,
    ) -> Result<Self, DomainError> {
        if cc_max > cc_sum {
            return Err(DomainError::InvalidSpan(
                "max complexity exceeds sum".to_string(),
            ));
        }

        if cognitive_max > cognitive_sum {
            return Err(DomainError::InvalidSpan(
                "max cognitive complexity exceeds sum".to_string(),
            ));
        }

        Ok(FileMetrics {
            loc,
            cc_max,
            cc_sum,
            cognitive_max,
            cognitive_sum,
            depth_max,
            fan_in,
            fan_out,
            exports,
            mi,
            dup_blocks,
            functions_count,
        })
    }

    pub fn with_fan_in(mut self, fan_in: u32) -> Self {
        self.fan_in = fan_in;
        self
    }

    pub fn stability_index(&self) -> f64 {
        let total = self.fan_in + self.fan_out;
        if total == 0 {
            0.0
        } else {
            self.fan_out as f64 / total as f64
        }
    }

    pub fn complexity_density(&self) -> f64 {
        if self.loc == 0 {
            0.0
        } else {
            self.cc_sum as f64 / self.loc as f64
        }
    }

    pub fn duplication_ratio(&self) -> f64 {
        if self.loc == 0 {
            0.0
        } else {
            self.dup_blocks as f64 / self.loc as f64
        }
    }

    pub fn is_high_complexity(&self) -> bool {
        self.cc_max > 10 || self.complexity_density() > 0.2
    }

    pub fn is_deeply_nested(&self) -> bool {
        self.depth_max > 5
    }

    pub fn has_high_fan_out(&self) -> bool {
        self.fan_out > 20
    }

    pub fn is_stable(&self) -> bool {
        self.stability_index() < 0.5
    }
}

/// Node-level metrics for functions and blocks
#[derive(Debug, Clone, PartialEq)]
pub struct NodeMetrics {
    pub name: Option<String>,
    pub span: Span,
    pub loc: u32,
    pub cc: u32,
    pub cognitive: u32,
    pub depth: u32,
    pub fingerprint: StructuralFingerprint,
}

impl NodeMetrics {
    pub fn new(
        name: Option<String>,
        span: Span,
        loc: u32,
        cc: u32,
        cognitive: u32,
        depth: u32,
        fingerprint: StructuralFingerprint,
    ) -> Result<Self, DomainError> {
        if loc == 0 {
            return Err(DomainError::InvalidSpan(
                "node LOC cannot be zero".to_string(),
            ));
        }

        if cc == 0 {
            return Err(DomainError::InvalidSpan(
                "node complexity cannot be zero".to_string(),
            ));
        }

        if cognitive == 0 {
            return Err(DomainError::InvalidSpan(
                "node cognitive complexity cannot be zero".to_string(),
            ));
        }

        Ok(NodeMetrics {
            name,
            span,
            loc,
            cc,
            cognitive,
            depth,
            fingerprint,
        })
    }

    pub fn complexity_per_loc(&self) -> f64 {
        self.cc as f64 / self.loc as f64
    }

    pub fn is_complex(&self) -> bool {
        self.cc > 10 || self.complexity_per_loc() > 0.3
    }

    pub fn is_deeply_nested(&self) -> bool {
        self.depth > 3
    }

    pub fn is_large(&self) -> bool {
        self.loc > 50
    }

    pub fn contains_position(&self, position: crate::domain::primitives::BytePos) -> bool {
        self.span.contains(position)
    }
}

#[derive(Debug, Clone)]
pub struct DuplicateGroup {
    pub fingerprint: u64,
    pub instances: Vec<DuplicateInstance>,
}

#[derive(Debug, Clone)]
pub struct DuplicateInstance {
    pub name: Option<String>,
    pub start_line: u32,
    pub end_line: u32,
    pub span: Span,
}

/// Metrics aggregation for multiple nodes
#[derive(Debug, Clone, PartialEq)]
pub struct AggregatedMetrics {
    pub total_loc: u32,
    pub total_cc: u32,
    pub max_cc: u32,
    pub max_depth: u32,
    pub avg_cc: f64,
    pub avg_depth: f64,
    pub complex_functions: u32,
    pub large_functions: u32,
    pub deep_functions: u32,
}

impl AggregatedMetrics {
    pub fn from_nodes(nodes: &[NodeMetrics]) -> Self {
        if nodes.is_empty() {
            return Self {
                total_loc: 0,
                total_cc: 0,
                max_cc: 0,
                max_depth: 0,
                avg_cc: 0.0,
                avg_depth: 0.0,
                complex_functions: 0,
                large_functions: 0,
                deep_functions: 0,
            };
        }

        let total_loc: u32 = nodes.iter().map(|n| n.loc).sum();
        let total_cc: u32 = nodes.iter().map(|n| n.cc).sum();
        let max_cc = nodes.iter().map(|n| n.cc).max().unwrap_or(0);
        let max_depth = nodes.iter().map(|n| n.depth).max().unwrap_or(0);

        let avg_cc = total_cc as f64 / nodes.len() as f64;
        let avg_depth = nodes.iter().map(|n| n.depth).sum::<u32>() as f64 / nodes.len() as f64;

        let complex_functions = nodes.iter().filter(|n| n.is_complex()).count() as u32;
        let large_functions = nodes.iter().filter(|n| n.is_large()).count() as u32;
        let deep_functions = nodes.iter().filter(|n| n.is_deeply_nested()).count() as u32;

        Self {
            total_loc,
            total_cc,
            max_cc,
            max_depth,
            avg_cc,
            avg_depth,
            complex_functions,
            large_functions,
            deep_functions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::primitives::{BytePos, Span};

    #[test]
    fn maintainability_index_validation() {
        assert!(MaintainabilityIndex::new(50.0).is_ok());
        assert!(MaintainabilityIndex::new(-10.0).is_ok());
        assert_eq!(MaintainabilityIndex::new(-10.0).unwrap().as_u8(), 0);
        assert!(MaintainabilityIndex::new(150.0).is_ok());
        assert_eq!(MaintainabilityIndex::new(150.0).unwrap().as_u8(), 100);
        assert!(MaintainabilityIndex::new(f64::NAN).is_err());
    }

    #[test]
    fn file_metrics_invariants() {
        let mi = MaintainabilityIndex::new(75.0).unwrap();

        assert!(FileMetrics::new(100, 5, 10, 3, 8, 3, 0, 2, 1, mi, 0, 2).is_ok());
        assert!(FileMetrics::new(100, 15, 10, 3, 8, 3, 0, 2, 1, mi, 0, 2).is_err());
        assert!(FileMetrics::new(100, 5, 10, 10, 8, 3, 0, 2, 1, mi, 0, 2).is_err());
    }

    #[test]
    fn stability_index_calculation() {
        let mi = MaintainabilityIndex::new(75.0).unwrap();

        let stable = FileMetrics::new(100, 5, 10, 3, 8, 2, 10, 1, 0, mi, 0, 2).unwrap();
        assert!(stable.stability_index() < 0.5);
        assert!(stable.is_stable());

        let unstable = FileMetrics::new(100, 5, 10, 3, 8, 2, 2, 10, 0, mi, 0, 2).unwrap();
        assert!(unstable.stability_index() > 0.5);
        assert!(!unstable.is_stable());

        let isolated = FileMetrics::new(100, 5, 10, 3, 8, 2, 0, 0, 0, mi, 0, 2).unwrap();
        assert_eq!(isolated.stability_index(), 0.0);
    }

    #[test]
    fn node_metrics_validation() {
        let span = Span::new(BytePos::new(0).unwrap(), BytePos::new(10).unwrap()).unwrap();
        let fingerprint = StructuralFingerprint::new(12345);

        assert!(NodeMetrics::new(Some("test".to_string()), span, 10, 3, 4, 2, fingerprint).is_ok());
        assert!(NodeMetrics::new(Some("test".to_string()), span, 0, 3, 4, 2, fingerprint).is_err());
        assert!(
            NodeMetrics::new(Some("test".to_string()), span, 10, 0, 4, 2, fingerprint).is_err()
        );
        assert!(
            NodeMetrics::new(Some("test".to_string()), span, 10, 3, 0, 2, fingerprint).is_err()
        );
    }

    #[test]
    fn aggregated_metrics() {
        let span = Span::new(BytePos::new(0).unwrap(), BytePos::new(10).unwrap()).unwrap();
        let fingerprint = StructuralFingerprint::new(12345);

        let nodes = vec![
            NodeMetrics::new(Some("f1".to_string()), span, 10, 3, 4, 2, fingerprint).unwrap(),
            NodeMetrics::new(Some("f2".to_string()), span, 20, 5, 6, 3, fingerprint).unwrap(),
        ];

        let agg = AggregatedMetrics::from_nodes(&nodes);
        assert_eq!(agg.total_loc, 30);
        assert_eq!(agg.total_cc, 8);
        assert_eq!(agg.max_cc, 5);
        assert_eq!(agg.avg_cc, 4.0);
    }
}
