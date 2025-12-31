pub mod application;
/**
# Purpose

Structural metrics engine - pure domain-driven code quality analysis.
Implements DDD with deep modules pattern for clean architecture.

# Model

Three-layer architecture:
- Domain: pure business logic and invariants
- Application: use cases and orchestration
- Infrastructure: external adapters and I/O

Deep modules provide:
- Small public surface area (narrow waist)
- Large internal capability
- Stable contracts hiding volatility

# Invariants

- All domain operations are pure and deterministic
- Infrastructure concerns are isolated from domain
- Application layer orchestrates without business logic
- Error handling flows through Result types

# Boundary

Public API exposes only application use cases.
Domain is accessed only through application layer.
Infrastructure is plugable at domain boundaries.

# Non-goals

- No direct domain access from external code
- No infrastructure in domain layer
- No business logic in application layer
- No framework dependencies in core domain

# Complexity

- Analysis: O(n) where n = source text size
- Memory: O(f) where f = unique fingerprints
- Hot loop: event stream processing
- All operations are bounded and predictable

# Examples

```rust,ignore
let request = AnalyzeRequest::new(text, language)?;
let response = AnalysisService::analyze(request)?;
```
*/
pub mod domain;
pub mod infrastructure;

pub use application::{
    AnalysisService, AnalyzeRequest, AnalyzeResponse, DuplicateGroupDto, DuplicateInstanceDto,
    FileMetricsDto, NodeMetricsDto, WantFlags,
};
pub use domain::{FileMetrics, NodeMetrics, SourceCode, StructuralEvent};
pub use infrastructure::TreeSitterAdapter;

use domain::primitives::DomainError;

/// Main analysis entry point - narrow waist interface
pub fn analyze(text: String, language: String) -> Result<AnalyzeResponse, DomainError> {
    let request = AnalyzeRequest::new(text, language)?;
    AnalysisService::analyze(request)
}

/// Re-export for backward compatibility
pub type Error = DomainError;
