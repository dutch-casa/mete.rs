use crate::domain::events::{EventValidator, StructuralEvent};
use crate::domain::metrics::{DuplicateGroup, DuplicateInstance, FileMetrics, NodeMetrics};
/**
# Purpose

SourceCode aggregate - the root entity for structural analysis.
Implements the aggregate root pattern with consistency boundary enforcement.

# Model

Aggregate responsibilities:
- Encapsulates source text and language context
- Manages the event stream processing lifecycle
- Enforces consistency invariants across operations
- Provides the narrow waist interface for analysis

# Invariants

- Source text is always valid UTF-8
- Language is always supported
- Event streams are well-formed and balanced
- Computed metrics are consistent with events
- Aggregate state is never partially invalid

# Boundary

Aggregate is the sole entry point for domain operations.
All internal state is encapsulated and protected.
External access is only through validated domain operations.
*/
use crate::domain::primitives::{BytePos, DomainError, LanguageId, SourceText};
use crate::domain::rules::{
    AggregationRules, CognitiveComplexityRules, ComplexityRules, DuplicationRules,
};

/// SourceCode aggregate root - the primary domain entity
#[derive(Debug, Clone)]
pub struct SourceCode {
    text: SourceText,
    language: LanguageId,
    events: Vec<StructuralEvent>,
    metrics: Option<FileMetrics>,
    node_metrics: Vec<NodeMetrics>,
}

impl SourceCode {
    pub fn new(text: String, language: &str) -> Result<Self, DomainError> {
        let source_text = SourceText::new(text)?;
        let lang_id = LanguageId::from_str(language)?;

        Ok(Self {
            text: source_text,
            language: lang_id,
            events: Vec::new(),
            metrics: None,
            node_metrics: Vec::new(),
        })
    }

    pub fn text(&self) -> &SourceText {
        &self.text
    }

    pub fn language(&self) -> LanguageId {
        self.language
    }

    pub fn events(&self) -> &[StructuralEvent] {
        &self.events
    }

    pub fn metrics(&self) -> Option<&FileMetrics> {
        self.metrics.as_ref()
    }

    pub fn node_metrics(&self) -> &[NodeMetrics] {
        &self.node_metrics
    }

    pub fn add_events(&mut self, events: Vec<StructuralEvent>) -> Result<(), DomainError> {
        let mut validator = EventValidator::new();

        for event in &events {
            validator.validate(event)?;
        }

        if !validator.is_balanced() {
            return Err(DomainError::InvalidSpan(
                "unbalanced event stream".to_string(),
            ));
        }

        self.events.extend(events);
        self.invalidate_metrics();

        Ok(())
    }

    pub fn compute_metrics(&mut self) -> Result<&FileMetrics, DomainError> {
        if self.metrics.is_none() {
            let actual_loc = self.text.as_str().lines().count() as u32;
            let file_metrics = AggregationRules::compute_file_metrics(
                &self.events,
                &self.node_metrics,
                actual_loc,
            )?;
            self.metrics = Some(file_metrics);
        }

        Ok(self.metrics.as_ref().unwrap())
    }

    pub fn compute_node_metrics(&mut self) -> Result<&[NodeMetrics], DomainError> {
        if self.node_metrics.is_empty() && !self.events.is_empty() {
            self.node_metrics = self.extract_node_metrics()?;
        }

        Ok(&self.node_metrics)
    }

    pub fn analyze_cursor(&self, cursor_pos: BytePos) -> Option<&NodeMetrics> {
        self.node_metrics
            .iter()
            .find(|node| node.contains_position(cursor_pos))
    }

    pub fn validate_consistency(&self) -> Result<(), DomainError> {
        if let Some(metrics) = &self.metrics {
            let actual_loc = self.text.as_str().lines().count() as u32;
            let computed_metrics = AggregationRules::compute_file_metrics(
                &self.events,
                &self.node_metrics,
                actual_loc,
            )?;

            if metrics != &computed_metrics {
                return Err(DomainError::InternalError(
                    "metrics inconsistency detected".to_string(),
                ));
            }
        }

        Ok(())
    }

    pub fn get_complexity_functions(&self) -> Vec<&NodeMetrics> {
        self.node_metrics
            .iter()
            .filter(|node| node.is_complex())
            .collect()
    }

    pub fn get_large_functions(&self) -> Vec<&NodeMetrics> {
        self.node_metrics
            .iter()
            .filter(|node| node.is_large())
            .collect()
    }

    pub fn get_deeply_nested_functions(&self) -> Vec<&NodeMetrics> {
        self.node_metrics
            .iter()
            .filter(|node| node.is_deeply_nested())
            .collect()
    }

    pub fn get_duplicate_groups(&self) -> Vec<DuplicateGroup> {
        use std::collections::HashMap;

        let mut fingerprint_map: HashMap<u64, Vec<DuplicateInstance>> = HashMap::new();

        for node in &self.node_metrics {
            let fp = node.fingerprint.as_u64();
            let start_line = self.byte_to_line(node.span.start().as_u32());
            let end_line = self.byte_to_line(node.span.end().as_u32());

            fingerprint_map
                .entry(fp)
                .or_default()
                .push(DuplicateInstance {
                    name: node.name.clone(),
                    start_line,
                    end_line,
                    span: node.span,
                });
        }

        fingerprint_map
            .into_iter()
            .filter(|(_, instances)| instances.len() > 1)
            .map(|(fingerprint, instances)| DuplicateGroup {
                fingerprint,
                instances,
            })
            .collect()
    }

    fn byte_to_line(&self, byte_offset: u32) -> u32 {
        let text = self.text.as_str();
        let offset = byte_offset as usize;
        text[..offset.min(text.len())].matches('\n').count() as u32 + 1
    }

    fn invalidate_metrics(&mut self) {
        self.metrics = None;
        self.node_metrics.clear();
    }

    fn extract_node_metrics(&self) -> Result<Vec<NodeMetrics>, DomainError> {
        let mut nodes = Vec::new();
        let mut function_stack: Vec<(Option<String>, crate::domain::primitives::Span, usize)> =
            Vec::new();

        for (idx, event) in self.events.iter().enumerate() {
            match event {
                StructuralEvent::FunctionStart { name, span } => {
                    function_stack.push((name.clone(), *span, idx));
                }
                StructuralEvent::FunctionEnd { .. } => {
                    if let Some((name, span, start_idx)) = function_stack.pop() {
                        let function_events = &self.events[start_idx..=idx];
                        let fingerprint =
                            DuplicationRules::compute_fingerprint_from_events(function_events);

                        let loc = self.estimate_loc_for_span(span).max(1);
                        let cc = ComplexityRules::compute_from_events(function_events).max(1);
                        let cognitive =
                            CognitiveComplexityRules::compute_from_events(function_events).max(1);
                        let depth = self.estimate_depth_for_events(function_events);

                        let node =
                            NodeMetrics::new(name, span, loc, cc, cognitive, depth, fingerprint)?;
                        nodes.push(node);
                    }
                }
                _ => {}
            }
        }

        Ok(nodes)
    }

    fn estimate_loc_for_span(&self, span: crate::domain::primitives::Span) -> u32 {
        let substring = self.text.substring(span).unwrap_or("");
        substring.lines().count() as u32
    }

    fn estimate_depth_for_events(&self, events: &[StructuralEvent]) -> u32 {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::events::BranchType;

    #[test]
    fn source_code_creation() {
        let source = SourceCode::new("fn main() {}".to_string(), "rust");
        assert!(source.is_ok());

        let source = SourceCode::new("fn main() {}".to_string(), "rust").unwrap();
        assert_eq!(source.language(), LanguageId::Rust);
        assert!(!source.text().is_empty());
    }

    #[test]
    fn event_stream_validation() {
        let mut source = SourceCode::new("fn test() {}".to_string(), "rust").unwrap();

        let valid_events = vec![
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
            StructuralEvent::function_end(BytePos::new(0).unwrap(), BytePos::new(10).unwrap())
                .unwrap(),
        ];

        assert!(source.add_events(valid_events).is_ok());
        assert_eq!(source.events().len(), 3);
    }

    #[test]
    fn unbalanced_events_rejection() {
        let mut source = SourceCode::new("fn test() {}".to_string(), "rust").unwrap();

        let unbalanced_events = vec![StructuralEvent::function_start(
            Some("test".to_string()),
            BytePos::new(0).unwrap(),
            BytePos::new(10).unwrap(),
        )
        .unwrap()];

        assert!(source.add_events(unbalanced_events).is_err());
    }

    #[test]
    fn metrics_computation() {
        let mut source = SourceCode::new("fn test() { if true { } }".to_string(), "rust").unwrap();

        let events = vec![
            StructuralEvent::function_start(
                Some("test".to_string()),
                BytePos::new(0).unwrap(),
                BytePos::new(20).unwrap(),
            )
            .unwrap(),
            StructuralEvent::branch(
                BytePos::new(12).unwrap(),
                BytePos::new(14).unwrap(),
                BranchType::Conditional,
            )
            .unwrap(),
            StructuralEvent::function_end(BytePos::new(0).unwrap(), BytePos::new(20).unwrap())
                .unwrap(),
        ];

        source.add_events(events).unwrap();
        let metrics = source.compute_metrics().unwrap();

        assert!(metrics.loc > 0);
        assert!(metrics.cc_sum >= 1);
        assert!(metrics.mi.as_f64() >= 0.0);
        assert!(metrics.mi.as_f64() <= 100.0);
    }

    #[test]
    fn consistency_validation() {
        let mut source = SourceCode::new("fn test() {}".to_string(), "rust").unwrap();

        let events = vec![
            StructuralEvent::function_start(
                Some("test".to_string()),
                BytePos::new(0).unwrap(),
                BytePos::new(10).unwrap(),
            )
            .unwrap(),
            StructuralEvent::function_end(BytePos::new(0).unwrap(), BytePos::new(10).unwrap())
                .unwrap(),
        ];

        source.add_events(events).unwrap();
        source.compute_metrics().unwrap();

        assert!(source.validate_consistency().is_ok());
    }
}
