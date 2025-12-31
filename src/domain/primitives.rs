/**
# Purpose

Domain primitives - foundational value objects with enforced invariants.
These are the building blocks that make invalid states unrepresentable.

# Model

Core primitives:
- BytePos: validated byte position in source text
- Span: validated byte range with start < end invariant
- SourceText: UTF-8 validated source code
- LanguageId: enum of supported languages
- Symbol: validated identifier for imports/exports

# Invariants

- BytePos is always non-negative
- Span always has start ≤ end
- SourceText is always valid UTF-8
- LanguageId is always from supported set
- Symbol is never empty

# Boundary

Constructors enforce all invariants via Result types.
No implicit validation - all creation goes through explicit constructors.
*/

#[derive(Debug, Clone, PartialEq)]
pub enum DomainError {
    InvalidBytePosition(String),
    InvalidSpan(String),
    InvalidUtf8(String),
    UnsupportedLanguage(String),
    EmptySymbol(String),
    InternalError(String),
}

impl std::fmt::Display for DomainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DomainError::InvalidBytePosition(msg) => write!(f, "Invalid byte position: {}", msg),
            DomainError::InvalidSpan(msg) => write!(f, "Invalid span: {}", msg),
            DomainError::InvalidUtf8(msg) => write!(f, "Invalid UTF-8: {}", msg),
            DomainError::UnsupportedLanguage(msg) => write!(f, "Unsupported language: {}", msg),
            DomainError::EmptySymbol(msg) => write!(f, "Empty symbol: {}", msg),
            DomainError::InternalError(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for DomainError {}

/// Byte position in source text (0-based, validated)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BytePos(u32);

impl BytePos {
    pub const ZERO: Self = BytePos(0);

    pub fn new(pos: u32) -> Result<Self, DomainError> {
        Ok(BytePos(pos))
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }

    pub fn as_usize(self) -> usize {
        self.0 as usize
    }

    pub fn offset(self, offset: u32) -> Result<Self, DomainError> {
        let new_pos = self
            .0
            .checked_add(offset)
            .ok_or_else(|| DomainError::InvalidBytePosition("overflow".to_string()))?;
        Self::new(new_pos)
    }
}

/// Byte range in source text [start, end) with validated invariants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    start: BytePos,
    end: BytePos,
}

impl Span {
    pub fn new(start: BytePos, end: BytePos) -> Result<Self, DomainError> {
        if end < start {
            return Err(DomainError::InvalidSpan("end before start".to_string()));
        }
        Ok(Span { start, end })
    }

    pub fn empty_at(pos: BytePos) -> Self {
        Self {
            start: pos,
            end: pos,
        }
    }

    pub fn start(self) -> BytePos {
        self.start
    }

    pub fn end(self) -> BytePos {
        self.end
    }

    pub fn len(self) -> u32 {
        self.end.as_u32() - self.start.as_u32()
    }

    pub fn is_empty(self) -> bool {
        self.start == self.end
    }

    pub fn contains(self, pos: BytePos) -> bool {
        pos >= self.start && pos < self.end
    }

    pub fn contains_span(self, other: Span) -> bool {
        other.start >= self.start && other.end <= self.end
    }
}

/// UTF-8 validated source text
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceText(String);

impl SourceText {
    pub fn new(text: String) -> Result<Self, DomainError> {
        Ok(SourceText(text))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DomainError> {
        let text = std::str::from_utf8(bytes).map_err(|_| {
            DomainError::InvalidUtf8("source text contains invalid UTF-8".to_string())
        })?;
        Ok(SourceText(text.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn byte_at(&self, pos: BytePos) -> Result<u8, DomainError> {
        let pos_usize = pos.as_usize();
        if pos_usize >= self.len() {
            return Err(DomainError::InvalidBytePosition(
                "position out of bounds".to_string(),
            ));
        }
        Ok(self.0.as_bytes()[pos_usize])
    }

    pub fn substring(&self, span: Span) -> Result<&str, DomainError> {
        let start = span.start().as_usize();
        let end = span.end().as_usize();
        if end > self.len() {
            return Err(DomainError::InvalidSpan("span out of bounds".to_string()));
        }
        Ok(&self.0[start..end])
    }
}

/// Supported programming languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageId {
    Rust,
    TypeScript,
    Python,
    Go,
    Java,
    CSharp,
    JavaScript,
    Cpp,
    Elixir,
}

impl LanguageId {
    pub fn from_str(lang: &str) -> Result<Self, DomainError> {
        match lang.to_lowercase().as_str() {
            "rust" | "rs" => Ok(LanguageId::Rust),
            "typescript" | "ts" => Ok(LanguageId::TypeScript),
            "python" | "py" => Ok(LanguageId::Python),
            "go" | "golang" => Ok(LanguageId::Go),
            "java" => Ok(LanguageId::Java),
            "csharp" | "c#" | "cs" => Ok(LanguageId::CSharp),
            "javascript" | "js" => Ok(LanguageId::JavaScript),
            "cpp" | "c++" | "cxx" => Ok(LanguageId::Cpp),
            "elixir" | "ex" | "exs" => Ok(LanguageId::Elixir),
            _ => Err(DomainError::UnsupportedLanguage(lang.to_string())),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            LanguageId::Rust => "rust",
            LanguageId::TypeScript => "typescript",
            LanguageId::Python => "python",
            LanguageId::Go => "go",
            LanguageId::Java => "java",
            LanguageId::CSharp => "csharp",
            LanguageId::JavaScript => "javascript",
            LanguageId::Cpp => "cpp",
            LanguageId::Elixir => "elixir",
        }
    }
}

/// Validated symbol identifier for imports/exports
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Symbol(String);

impl Symbol {
    pub fn new(name: String) -> Result<Self, DomainError> {
        if name.is_empty() {
            return Err(DomainError::EmptySymbol(
                "symbol name cannot be empty".to_string(),
            ));
        }
        let valid = name.chars().all(|c| {
            c.is_alphanumeric()
                || c == '_'
                || c == '.'
                || c == ':'
                || c == '/'
                || c == '@'
                || c == '-'
                || c == '*'
                || c == '{'
                || c == '}'
                || c == ' '
                || c == '"'
                || c == '\''
        });
        if !valid {
            return Err(DomainError::EmptySymbol(format!(
                "symbol contains invalid characters: {}",
                name
            )));
        }
        Ok(Symbol(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_pos_validation() {
        assert!(BytePos::new(0).is_ok());
        assert!(BytePos::new(100).is_ok());
    }

    #[test]
    fn span_invariants() {
        let start = BytePos::new(10).unwrap();
        let end = BytePos::new(20).unwrap();

        assert!(Span::new(start, end).is_ok());
        assert_eq!(Span::new(start, end).unwrap().len(), 10);

        assert!(Span::new(end, start).is_err());
    }

    #[test]
    fn source_text_utf8_validation() {
        assert!(SourceText::new("hello".to_string()).is_ok());

        let invalid_utf8: Vec<u8> = vec![0xFF, 0xFE];
        assert!(SourceText::from_bytes(&invalid_utf8).is_err());

        let valid_utf8 = "hello world".as_bytes();
        assert!(SourceText::from_bytes(valid_utf8).is_ok());
    }

    #[test]
    fn language_id_parsing() {
        assert!(matches!(LanguageId::from_str("rust"), Ok(LanguageId::Rust)));
        assert!(matches!(LanguageId::from_str("RS"), Ok(LanguageId::Rust)));
        assert!(LanguageId::from_str("unknown").is_err());
    }

    #[test]
    fn symbol_validation() {
        assert!(Symbol::new("valid_name".to_string()).is_ok());
        assert!(Symbol::new("valid.name".to_string()).is_ok());
        assert!(Symbol::new("valid-name".to_string()).is_ok());
        assert!(Symbol::new("@scope/package".to_string()).is_ok());
        assert!(Symbol::new("".to_string()).is_err());
        assert!(Symbol::new("invalid\nname".to_string()).is_err());
    }
}
