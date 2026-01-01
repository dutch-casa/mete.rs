/**
# Purpose

Structural entropy - Shannon entropy measurement for code complexity.
Measures syntactic structure based on the unpredictability of AST node distributions.

# Model

Entropy components:
- SymbolCount: total valid AST nodes (N) - must be > 0
- SymbolFrequency: counts of each node type (map) - must have entries
- ShannonEntropy: computed entropy in bits (H) - 0 <= H <= log2(N)
- MetricMass: entropy × log₁₀(N) - normalizes for file size

# Invariants

- NodeCount is always positive (> 0)
- ShannonEntropy is always in range [0, log2(N)]
- MetricMass is positive for non-empty files
- Symbol frequencies sum to NodeCount
- Filtering excludes comments and punctuation tokens

# Algorithm

H(F) = -Σ P(xᵢ) × log₂ P(xᵢ)
M = H(F) × log₁₀(N)

Where:
- N = total valid nodes
- P(xᵢ) = frequency(xᵢ) / N
- H(F) = entropy in bits
- M = metric mass for size-normalized comparison

# Interpretation

H ≈ 2.0: Very simple, repetitive code
H > 6.0: Extremely complex, varied code (likely "God Class")
High M: Large AND complex files (most confusing)
Low M: Small files regardless of complexity
*/
use crate::domain::primitives::DomainError;
use std::collections::HashMap;
use std::fmt;

/// Total count of valid AST nodes in a file
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeCount(u32);

impl NodeCount {
    /// Minimum valid node count
    pub const MIN: u32 = 1;

    pub fn new(count: u32) -> Result<Self, DomainError> {
        if count == 0 {
            return Err(DomainError::InternalError(
                "node count cannot be zero".to_string(),
            ));
        }
        Ok(NodeCount(count))
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }

    pub fn as_usize(self) -> usize {
        self.0 as usize
    }

    /// Maximum possible entropy for this count (uniform distribution)
    pub fn max_entropy(self) -> f64 {
        (self.0 as f64).log2()
    }
}

/// Frequency of a specific node type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SymbolFrequency(u32);

impl SymbolFrequency {
    pub fn new(freq: u32) -> Result<Self, DomainError> {
        if freq == 0 {
            return Err(DomainError::InternalError(
                "symbol frequency cannot be zero".to_string(),
            ));
        }
        Ok(SymbolFrequency(freq))
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }
}

/// Map of node type names to their frequencies
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolDistribution(HashMap<String, NodeCount>);

impl SymbolDistribution {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(HashMap::with_capacity(capacity))
    }

    pub fn insert(&mut self, node_type: String) {
        self.0
            .entry(node_type)
            .and_modify(|count| count.0 += 1)
            .or_insert_with(|| NodeCount(1));
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn total_nodes(&self) -> NodeCount {
        let sum = self.0.values().map(|n| n.as_u32()).sum::<u32>();
        // SAFETY: sum is always >= 1 when distribution is non-empty
        // If empty, we return NodeCount(1) to avoid division by zero
        NodeCount(if sum == 0 { 1 } else { sum })
    }

    pub fn unique_symbols(&self) -> usize {
        self.0.len()
    }

    pub fn frequencies(&self) -> &HashMap<String, NodeCount> {
        &self.0
    }

    pub fn most_common(&self, n: usize) -> Vec<(&String, &NodeCount)> {
        let mut entries: Vec<_> = self
            .0
            .iter()
            .filter(|(_, count)| count.as_u32() > 0)
            .collect();
        entries.sort_by(|a, b| b.1.cmp(a.1));
        entries.into_iter().take(n).collect()
    }
}

impl Default for SymbolDistribution {
    fn default() -> Self {
        Self::new()
    }
}

/// Shannon entropy value in bits
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ShannonEntropy(f64);

impl ShannonEntropy {
    /// Minimum possible entropy (single symbol repeated)
    pub const MIN: f64 = 0.0;

    pub fn new(value: f64) -> Result<Self, DomainError> {
        if value.is_nan() || value.is_infinite() {
            return Err(DomainError::InternalError(
                "invalid entropy value".to_string(),
            ));
        }
        if value < 0.0 {
            return Err(DomainError::InternalError(
                "entropy cannot be negative".to_string(),
            ));
        }
        Ok(ShannonEntropy(value))
    }

    pub fn as_f64(self) -> f64 {
        self.0
    }

    pub fn is_simple(self) -> bool {
        self.0 <= 2.0
    }

    pub fn is_complex(self) -> bool {
        self.0 > 6.0
    }
}

impl fmt::Display for ShannonEntropy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}", self.0)
    }
}

/// Metric mass - entropy normalized by file size
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct MetricMass(f64);

impl MetricMass {
    pub fn new(value: f64) -> Result<Self, DomainError> {
        if value.is_nan() || value.is_infinite() {
            return Err(DomainError::InternalError(
                "invalid metric mass".to_string(),
            ));
        }
        if value < 0.0 {
            return Err(DomainError::InternalError(
                "metric mass cannot be negative".to_string(),
            ));
        }
        Ok(MetricMass(value))
    }

    pub fn as_f64(self) -> f64 {
        self.0
    }
}

impl fmt::Display for MetricMass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}", self.0)
    }
}

/// Result of entropy analysis for a single file
#[derive(Debug, Clone)]
pub struct EntropyResult {
    pub path: String,
    pub entropy: ShannonEntropy,
    pub metric_mass: MetricMass,
    pub node_count: NodeCount,
    pub unique_symbols: usize,
    pub distribution: SymbolDistribution,
}

impl EntropyResult {
    pub fn new(
        path: String,
        entropy: ShannonEntropy,
        metric_mass: MetricMass,
        node_count: NodeCount,
        distribution: SymbolDistribution,
    ) -> Self {
        Self {
            path,
            entropy,
            metric_mass,
            node_count,
            unique_symbols: distribution.unique_symbols(),
            distribution,
        }
    }

    pub fn complexity_level(&self) -> &str {
        if self.entropy.is_simple() {
            "simple"
        } else if self.entropy.is_complex() {
            "complex"
        } else {
            "moderate"
        }
    }
}

/// Entropy rules - pure computation of Shannon entropy
#[derive(Debug)]
pub struct EntropyRules;

impl EntropyRules {
    /// Compute Shannon entropy from symbol distribution
    pub fn compute_entropy(
        distribution: &SymbolDistribution,
    ) -> Result<ShannonEntropy, DomainError> {
        let total = distribution.total_nodes().as_u32() as f64;

        if total == 0.0 {
            return ShannonEntropy::new(0.0);
        }

        let mut entropy = 0.0f64;

        for count in distribution.frequencies().values() {
            let freq = count.as_u32() as f64;
            let probability = freq / total;

            if probability > 0.0 {
                entropy -= probability * probability.log2();
            }
        }

        ShannonEntropy::new(entropy)
    }

    /// Compute metric mass: H × log₁₀(N)
    pub fn compute_metric_mass(
        entropy: ShannonEntropy,
        node_count: NodeCount,
    ) -> Result<MetricMass, DomainError> {
        let n = node_count.as_u32() as f64;

        if n <= 0.0 {
            return MetricMass::new(0.0);
        }

        let log_n = n.log10();
        let mass = entropy.as_f64() * log_n;

        MetricMass::new(mass)
    }

    /// Full entropy analysis returning all metrics
    pub fn analyze(
        distribution: SymbolDistribution,
    ) -> Result<(ShannonEntropy, MetricMass, NodeCount), DomainError> {
        let node_count = distribution.total_nodes();
        let entropy = Self::compute_entropy(&distribution)?;
        let metric_mass = Self::compute_metric_mass(entropy, node_count)?;

        Ok((entropy, metric_mass, node_count))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_count_validation() {
        assert!(NodeCount::new(1).is_ok());
        assert!(NodeCount::new(100).is_ok());
        assert!(NodeCount::new(0).is_err());
    }

    #[test]
    fn symbol_frequency_validation() {
        assert!(SymbolFrequency::new(1).is_ok());
        assert!(SymbolFrequency::new(0).is_err());
    }

    #[test]
    fn entropy_computation_uniform() {
        // Uniform distribution: each of 4 symbols appears once
        let mut dist = SymbolDistribution::new();
        dist.insert("function_definition".to_string());
        dist.insert("identifier".to_string());
        dist.insert("block".to_string());
        dist.insert("return_statement".to_string());

        let entropy = EntropyRules::compute_entropy(&dist).unwrap();
        // log2(4) = 2.0, uniform means H = log2(4) = 2.0
        assert!((entropy.as_f64() - 2.0).abs() < 0.001);
    }

    #[test]
    fn entropy_computation_skewed() {
        // Skewed: one symbol dominates
        let mut dist = SymbolDistribution::new();
        for _ in 0..9 {
            dist.insert("identifier".to_string());
        }
        dist.insert("function_definition".to_string());

        let entropy = EntropyRules::compute_entropy(&dist).unwrap();
        // Should be less than uniform (log2(2) = 1.0)
        assert!(entropy.as_f64() < 1.0);
        assert!(entropy.as_f64() > 0.0);
    }

    #[test]
    fn metric_mass_computation() {
        let entropy = ShannonEntropy::new(2.0).unwrap();
        let node_count = NodeCount::new(100).unwrap();

        let mass = EntropyRules::compute_metric_mass(entropy, node_count).unwrap();
        // 2.0 * log10(100) = 2.0 * 2.0 = 4.0
        assert!((mass.as_f64() - 4.0).abs() < 0.001);
    }

    #[test]
    fn entropy_simple_classification() {
        let simple = ShannonEntropy::new(1.5).unwrap();
        let moderate = ShannonEntropy::new(4.0).unwrap();
        let complex = ShannonEntropy::new(7.0).unwrap();

        assert!(simple.is_simple());
        assert!(!moderate.is_simple());
        assert!(!moderate.is_complex());
        assert!(complex.is_complex());
    }

    #[test]
    fn symbol_distribution_insert() {
        let mut dist = SymbolDistribution::new();
        dist.insert("function_definition".to_string());
        dist.insert("function_definition".to_string());
        dist.insert("identifier".to_string());

        assert_eq!(dist.len(), 2);
        assert_eq!(dist.total_nodes().as_u32(), 3);
    }
}
