use crate::domain::primitives::{BytePos, DomainError};
/**
# Purpose

Application layer - use cases and orchestration without business logic.
Implements clean architecture separation between domain and infrastructure.

# Model

Use cases:
- AnalysisService: primary analysis orchestration
- Request/Response DTOs: external data transfer
- Command/Query separation: read/write operation separation

# Invariants

- Application layer contains no business logic
- All domain operations go through domain layer
- External concerns are isolated in infrastructure
- Error handling is consistent across use cases

# Boundary

Accepts external DTOs and converts to domain objects.
Delegates all business logic to domain layer.
Converts domain results back to external DTOs.

# Non-goals

- No metric computation logic
- No validation that belongs to domain
- No infrastructure concerns
- No framework-specific code
*/
use crate::domain::{SourceCode, StructuralEvent};

/// Analysis request DTO - external data transfer object
#[derive(Debug, Clone)]
pub struct AnalyzeRequest {
    pub version: u32,
    pub language: String,
    pub text: String,
    pub cursor_byte: Option<u32>,
    pub want: WantFlags,
}

impl AnalyzeRequest {
    pub fn new(text: String, language: String) -> Result<Self, DomainError> {
        Self::with_options(text, language, None, WantFlags::all())
    }

    pub fn with_options(
        text: String,
        language: String,
        cursor_byte: Option<u32>,
        want: WantFlags,
    ) -> Result<Self, DomainError> {
        if text.is_empty() {
            return Err(DomainError::InvalidUtf8("empty source text".to_string()));
        }

        Ok(Self {
            version: 1,
            language,
            text,
            cursor_byte,
            want,
        })
    }

    pub fn validate(&self) -> Result<(), DomainError> {
        if self.version != 1 {
            return Err(DomainError::UnsupportedLanguage(format!(
                "version {} not supported",
                self.version
            )));
        }

        if let Some(cursor) = self.cursor_byte {
            if cursor >= self.text.len() as u32 {
                return Err(DomainError::InvalidBytePosition(
                    "cursor out of bounds".to_string(),
                ));
            }
        }

        Ok(())
    }
}

/// Flags for requested metrics
#[derive(Debug, Clone, PartialEq)]
pub struct WantFlags {
    pub file: bool,
    pub cursor: bool,
    pub functions: bool,
}

impl WantFlags {
    pub fn all() -> Self {
        Self {
            file: true,
            cursor: true,
            functions: true,
        }
    }

    pub fn file_only() -> Self {
        Self {
            file: true,
            cursor: false,
            functions: false,
        }
    }

    pub fn with_functions(mut self) -> Self {
        self.functions = true;
        self
    }

    pub fn with_cursor(mut self) -> Self {
        self.cursor = true;
        self
    }
}

/// Analysis response DTO - external data transfer object
#[derive(Debug, Clone, serde::Serialize)]
pub struct AnalyzeResponse {
    pub version: u32,
    pub revision: u64,
    pub file: Option<FileMetricsDto>,
    pub cursor: Option<NodeMetricsDto>,
    pub functions: Option<Vec<NodeMetricsDto>>,
    pub duplicates: Option<Vec<DuplicateGroupDto>>,
}

impl AnalyzeResponse {
    pub fn new(version: u32, revision: u64) -> Self {
        Self {
            version,
            revision,
            file: None,
            cursor: None,
            functions: None,
            duplicates: None,
        }
    }

    pub fn with_file_metrics(mut self, metrics: FileMetricsDto) -> Self {
        self.file = Some(metrics);
        self
    }

    pub fn with_cursor_metrics(mut self, metrics: NodeMetricsDto) -> Self {
        self.cursor = Some(metrics);
        self
    }

    pub fn with_function_metrics(mut self, metrics: Vec<NodeMetricsDto>) -> Self {
        self.functions = Some(metrics);
        self
    }

    pub fn with_duplicates(mut self, duplicates: Vec<DuplicateGroupDto>) -> Self {
        self.duplicates = Some(duplicates);
        self
    }
}

/// File metrics DTO - external data transfer object
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FileMetricsDto {
    pub loc: u32,
    pub cc_max: u32,
    pub cc_sum: u32,
    pub cognitive_max: u32,
    pub cognitive_sum: u32,
    pub depth_max: u32,
    pub fan_in: u32,
    pub fan_out: u32,
    pub exports: u32,
    pub mi: f64,
    pub stability: f64,
    pub dup_blocks: u32,
    pub functions_count: u32,
}

impl From<crate::domain::metrics::FileMetrics> for FileMetricsDto {
    fn from(metrics: crate::domain::metrics::FileMetrics) -> Self {
        Self {
            loc: metrics.loc,
            cc_max: metrics.cc_max,
            cc_sum: metrics.cc_sum,
            cognitive_max: metrics.cognitive_max,
            cognitive_sum: metrics.cognitive_sum,
            depth_max: metrics.depth_max,
            fan_in: metrics.fan_in,
            fan_out: metrics.fan_out,
            exports: metrics.exports,
            mi: metrics.mi.as_f64(),
            stability: metrics.stability_index(),
            dup_blocks: metrics.dup_blocks,
            functions_count: metrics.functions_count,
        }
    }
}

/// Node metrics DTO - external data transfer object
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct NodeMetricsDto {
    pub name: Option<String>,
    pub span: SpanDto,
    pub loc: u32,
    pub cc: u32,
    pub cognitive: u32,
    pub depth: u32,
    pub fingerprint: u64,
}

impl From<crate::domain::metrics::NodeMetrics> for NodeMetricsDto {
    fn from(metrics: crate::domain::metrics::NodeMetrics) -> Self {
        Self {
            name: metrics.name.clone(),
            span: SpanDto::from(metrics.span),
            loc: metrics.loc,
            cc: metrics.cc,
            cognitive: metrics.cognitive,
            depth: metrics.depth,
            fingerprint: metrics.fingerprint.as_u64(),
        }
    }
}

/// Span DTO - external data transfer object
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SpanDto {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct DuplicateGroupDto {
    pub fingerprint: u64,
    pub instances: Vec<DuplicateInstanceDto>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct DuplicateInstanceDto {
    pub name: Option<String>,
    pub start_line: u32,
    pub end_line: u32,
    pub span: SpanDto,
}

impl SpanDto {
    pub fn new(start: u32, end: u32) -> Result<Self, DomainError> {
        if end < start {
            return Err(DomainError::InvalidSpan("end before start".to_string()));
        }
        Ok(Self { start, end })
    }
}

impl From<crate::domain::primitives::Span> for SpanDto {
    fn from(span: crate::domain::primitives::Span) -> Self {
        Self {
            start: span.start().as_u32(),
            end: span.end().as_u32(),
        }
    }
}

/// Primary analysis service - main use case orchestrator
#[derive(Debug)]
pub struct AnalysisService;

impl AnalysisService {
    pub fn analyze(request: AnalyzeRequest) -> Result<AnalyzeResponse, DomainError> {
        request.validate()?;

        let mut source = SourceCode::new(request.text.clone(), &request.language)?;

        let events = Self::generate_events(&source)?;
        source.add_events(events)?;

        let mut response = AnalyzeResponse::new(request.version, 1);

        source.compute_node_metrics()?;

        if request.want.file {
            let metrics = source.compute_metrics()?;
            response = response.with_file_metrics(FileMetricsDto::from(metrics.clone()));
        }

        if request.want.functions {
            let node_metrics = source.node_metrics();
            let dtos: Vec<NodeMetricsDto> = node_metrics
                .iter()
                .map(|m| NodeMetricsDto::from(m.clone()))
                .collect();
            response = response.with_function_metrics(dtos);
        }

        if request.want.cursor && request.cursor_byte.is_some() {
            let cursor_pos = BytePos::new(request.cursor_byte.unwrap())?;
            if let Some(node_metrics) = source.analyze_cursor(cursor_pos) {
                response = response.with_cursor_metrics(NodeMetricsDto::from(node_metrics.clone()));
            }
        }

        let dup_groups = source.get_duplicate_groups();
        if !dup_groups.is_empty() {
            let dup_dtos: Vec<DuplicateGroupDto> = dup_groups
                .into_iter()
                .map(|g| DuplicateGroupDto {
                    fingerprint: g.fingerprint,
                    instances: g
                        .instances
                        .into_iter()
                        .map(|i| DuplicateInstanceDto {
                            name: i.name,
                            start_line: i.start_line,
                            end_line: i.end_line,
                            span: SpanDto::from(i.span),
                        })
                        .collect(),
                })
                .collect();
            response = response.with_duplicates(dup_dtos);
        }

        Ok(response)
    }

    fn generate_events(source: &SourceCode) -> Result<Vec<StructuralEvent>, DomainError> {
        use crate::infrastructure::TreeSitterAdapter;

        let mut adapter = TreeSitterAdapter::new(source.language())?;
        adapter.parse_to_events(source.text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_validation() {
        let request = AnalyzeRequest::new("fn main() {}".to_string(), "rust".to_string());
        assert!(request.is_ok());

        let request = request.unwrap();
        assert!(request.validate().is_ok());
    }

    #[test]
    fn request_with_cursor() {
        let request = AnalyzeRequest::with_options(
            "fn main() {}".to_string(),
            "rust".to_string(),
            Some(5),
            WantFlags::file_only(),
        );
        assert!(request.is_ok());
    }

    #[test]
    fn invalid_cursor_position() {
        let request = AnalyzeRequest::with_options(
            "fn main() {}".to_string(),
            "rust".to_string(),
            Some(100),
            WantFlags::file_only(),
        );
        assert!(request.is_ok());

        let request = request.unwrap();
        assert!(request.validate().is_err());
    }

    #[test]
    fn want_flags() {
        let all = WantFlags::all();
        assert!(all.file && all.cursor && all.functions);

        let file_only = WantFlags::file_only();
        assert!(file_only.file && !file_only.cursor && !file_only.functions);

        let with_functions = file_only.with_functions();
        assert!(with_functions.file && with_functions.functions && !with_functions.cursor);
    }

    #[test]
    fn span_dto_conversion() {
        let span_dto = SpanDto::new(10, 20).unwrap();
        assert_eq!(span_dto.start, 10);
        assert_eq!(span_dto.end, 20);

        assert!(SpanDto::new(20, 10).is_err());
    }
}
