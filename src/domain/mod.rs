pub mod events;
pub mod metrics;
pub mod primitives;
pub mod rules;
/**
# Purpose

Core domain module for structural metrics analysis.
Implements Domain-Driven Design with deep modules pattern.
Encapsulates all business logic and invariants for code quality metrics.

# Model

Domain consists of:
- SourceCode aggregate: root entity for analysis
- StructuralEvents: value objects representing AST events
- QualityMetrics: value objects for computed metrics
- AnalysisRules: domain services for metric computation

Deep modules provide:
- Small public surface area (narrow waist)
- Large internal capability
- Stable contracts hiding implementation volatility

# Invariants

- All metrics are deterministic functions of structural events
- Cyclomatic complexity ≥ branch count
- Maintainability Index ∈ [0, 100]
- Fingerprint hashes are stable and collision-resistant
- SourceCode aggregate maintains consistency boundaries

# Boundary

Accepts only validated domain primitives:
- SourceText (UTF-8 validated)
- LanguageId (enum of supported languages)
- StructuralEvents (well-formed sequence)

Rejects external concerns:
- I/O operations
- Persistence
- Network communication
- Time-dependent operations

# Non-goals

- No semantic analysis or type checking
- No language-specific parsing logic
- No external system integration
- No UI/presentation concerns

# Complexity

- Event processing: O(n) where n = number of events
- Metric computation: O(1) per event (amortized)
- Fingerprinting: O(m) where m = node size
- Memory: O(f) where f = unique fingerprints
- Hot loop: structural event stream processing

# Examples

```rust,ignore
let source = SourceCode::new(text, language)?;
let events = StructuralEvent::from_ast(&source);
let metrics = source.compute_metrics(&events)?;
```
*/
pub mod source_code;

pub use events::StructuralEvent;
pub use metrics::{DuplicateGroup, DuplicateInstance, FileMetrics, MaintainabilityIndex, NodeMetrics, StructuralFingerprint};
pub use primitives::{BytePos, LanguageId, SourceText, Span};
pub use source_code::SourceCode;

