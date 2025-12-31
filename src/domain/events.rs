/**
# Purpose

Structural events - value objects representing abstract syntax tree events.
These are the pure data structures that flow through the analysis pipeline.

# Model

Event types represent structural patterns:
- BlockEntry/BlockExit: nesting depth changes
- Branch: cyclomatic complexity increments
- Import/Export: module interface changes
- FunctionStart/FunctionEnd: function boundary events
- CursorPosition: optional cursor context

# Invariants

- All events are immutable value objects
- BlockEntry/BlockExit events are balanced in well-formed sequences
- FunctionStart/FunctionEnd events are properly nested
- Spans are always valid (start ≤ end)
- Symbol names are never empty

# Boundary

Events are created by adapters from Tree-sitter ASTs.
Core logic only consumes events, never creates them.
All validation happens at event creation boundaries.
*/
use crate::domain::primitives::{BytePos, DomainError, Span, Symbol};

/// Structural events from abstract syntax tree analysis
#[derive(Debug, Clone, PartialEq)]
pub enum StructuralEvent {
    BlockEntry { span: Span },
    BlockExit { span: Span },
    Branch { span: Span, branch_type: BranchType },
    Import { symbol: Symbol, span: Span },
    Export { symbol: Symbol, span: Span },
    FunctionStart { name: Option<String>, span: Span },
    FunctionEnd { span: Span },
    CursorPosition { position: BytePos },
}

/// Types of branching constructs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BranchType {
    Conditional,
    Loop,
    Switch,
    Exception,
    Logical,
}

impl StructuralEvent {
    pub fn block_entry(start: BytePos, end: BytePos) -> Result<Self, DomainError> {
        let span = Span::new(start, end)?;
        Ok(StructuralEvent::BlockEntry { span })
    }

    pub fn block_exit(start: BytePos, end: BytePos) -> Result<Self, DomainError> {
        let span = Span::new(start, end)?;
        Ok(StructuralEvent::BlockExit { span })
    }

    pub fn branch(
        start: BytePos,
        end: BytePos,
        branch_type: BranchType,
    ) -> Result<Self, DomainError> {
        let span = Span::new(start, end)?;
        Ok(StructuralEvent::Branch { span, branch_type })
    }

    pub fn import(symbol: String, start: BytePos, end: BytePos) -> Result<Self, DomainError> {
        let symbol = Symbol::new(symbol)?;
        let span = Span::new(start, end)?;
        Ok(StructuralEvent::Import { symbol, span })
    }

    pub fn export(symbol: String, start: BytePos, end: BytePos) -> Result<Self, DomainError> {
        let symbol = Symbol::new(symbol)?;
        let span = Span::new(start, end)?;
        Ok(StructuralEvent::Export { symbol, span })
    }

    pub fn function_start(
        name: Option<String>,
        start: BytePos,
        end: BytePos,
    ) -> Result<Self, DomainError> {
        let span = Span::new(start, end)?;
        Ok(StructuralEvent::FunctionStart { name, span })
    }

    pub fn function_end(start: BytePos, end: BytePos) -> Result<Self, DomainError> {
        let span = Span::new(start, end)?;
        Ok(StructuralEvent::FunctionEnd { span })
    }

    pub fn cursor_position(position: u32) -> Result<Self, DomainError> {
        let pos = BytePos::new(position)?;
        Ok(StructuralEvent::CursorPosition { position: pos })
    }

    pub fn span(&self) -> Option<Span> {
        match self {
            StructuralEvent::BlockEntry { span }
            | StructuralEvent::BlockExit { span }
            | StructuralEvent::Branch { span, .. }
            | StructuralEvent::Import { span, .. }
            | StructuralEvent::Export { span, .. }
            | StructuralEvent::FunctionStart { span, .. }
            | StructuralEvent::FunctionEnd { span } => Some(*span),
            StructuralEvent::CursorPosition { .. } => None,
        }
    }

    pub fn is_block_entry(&self) -> bool {
        matches!(self, StructuralEvent::BlockEntry { .. })
    }

    pub fn is_block_exit(&self) -> bool {
        matches!(self, StructuralEvent::BlockExit { .. })
    }

    pub fn is_branch(&self) -> bool {
        matches!(self, StructuralEvent::Branch { .. })
    }

    pub fn is_function_start(&self) -> bool {
        matches!(self, StructuralEvent::FunctionStart { .. })
    }

    pub fn is_function_end(&self) -> bool {
        matches!(self, StructuralEvent::FunctionEnd { .. })
    }

    pub fn is_import(&self) -> bool {
        matches!(self, StructuralEvent::Import { .. })
    }

    pub fn is_export(&self) -> bool {
        matches!(self, StructuralEvent::Export { .. })
    }

    pub fn is_cursor(&self) -> bool {
        matches!(self, StructuralEvent::CursorPosition { .. })
    }
}

/// Event stream validator for well-formed sequences
#[derive(Debug, Default)]
pub struct EventValidator {
    block_depth: u32,
    function_depth: u32,
}

impl EventValidator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate(&mut self, event: &StructuralEvent) -> Result<(), DomainError> {
        match event {
            StructuralEvent::BlockEntry { .. } => {
                self.block_depth += 1;
            }
            StructuralEvent::BlockExit { .. } => {
                if self.block_depth == 0 {
                    return Err(DomainError::InvalidSpan(
                        "unbalanced block exit".to_string(),
                    ));
                }
                self.block_depth -= 1;
            }
            StructuralEvent::FunctionStart { .. } => {
                self.function_depth += 1;
            }
            StructuralEvent::FunctionEnd { .. } => {
                if self.function_depth == 0 {
                    return Err(DomainError::InvalidSpan(
                        "unbalanced function exit".to_string(),
                    ));
                }
                self.function_depth -= 1;
            }
            _ => {}
        }
        Ok(())
    }

    pub fn is_balanced(&self) -> bool {
        self.block_depth == 0 && self.function_depth == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_creation() {
        let start = BytePos::new(10).unwrap();
        let end = BytePos::new(20).unwrap();

        assert!(StructuralEvent::block_entry(start, end).is_ok());
        assert!(StructuralEvent::block_exit(start, end).is_ok());
        assert!(StructuralEvent::branch(start, end, BranchType::Conditional).is_ok());
    }

    #[test]
    fn event_validation() {
        let mut validator = EventValidator::new();
        let start = BytePos::new(10).unwrap();
        let end = BytePos::new(20).unwrap();

        let entry = StructuralEvent::block_entry(start, end).unwrap();
        let exit = StructuralEvent::block_exit(start, end).unwrap();

        assert!(validator.validate(&entry).is_ok());
        assert!(validator.validate(&exit).is_ok());
        assert!(validator.is_balanced());

        assert!(validator.validate(&exit).is_err());
    }

    #[test]
    fn function_nesting() {
        let mut validator = EventValidator::new();
        let start = BytePos::new(10).unwrap();
        let end = BytePos::new(20).unwrap();

        let func_start =
            StructuralEvent::function_start(Some("test".to_string()), start, end).unwrap();
        let func_end = StructuralEvent::function_end(start, end).unwrap();

        assert!(validator.validate(&func_start).is_ok());
        assert!(validator.validate(&func_end).is_ok());
        assert!(validator.is_balanced());

        assert!(validator.validate(&func_end).is_err());
    }
}
