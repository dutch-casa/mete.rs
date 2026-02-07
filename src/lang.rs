//! Language specifications for tree-sitter parsing.
//!
//! Static data tables mapping AST node kinds to structural categories.
//! Pure data, no logic.

use std::path::Path;

/// Supported programming languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
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

impl Language {
    /// Parse language from string (case-insensitive).
    pub fn from_name(s: &str) -> Option<Self> {
        const ALIASES: &[(&str, Language)] = &[
            ("rust", Language::Rust),
            ("rs", Language::Rust),
            ("typescript", Language::TypeScript),
            ("ts", Language::TypeScript),
            ("tsx", Language::TypeScript),
            ("python", Language::Python),
            ("py", Language::Python),
            ("go", Language::Go),
            ("golang", Language::Go),
            ("java", Language::Java),
            ("csharp", Language::CSharp),
            ("c#", Language::CSharp),
            ("cs", Language::CSharp),
            ("javascript", Language::JavaScript),
            ("js", Language::JavaScript),
            ("jsx", Language::JavaScript),
            ("cpp", Language::Cpp),
            ("c++", Language::Cpp),
            ("cxx", Language::Cpp),
            ("cc", Language::Cpp),
            ("hpp", Language::Cpp),
            ("h", Language::Cpp),
            ("elixir", Language::Elixir),
            ("ex", Language::Elixir),
            ("exs", Language::Elixir),
        ];

        let lower = s.to_lowercase();
        ALIASES
            .iter()
            .find(|(alias, _)| *alias == lower.as_str())
            .map(|(_, lang)| *lang)
    }

    /// Detect language from file extension.
    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|e| e.to_str())
            .and_then(Self::from_name)
    }

    /// Get tree-sitter language object.
    pub fn tree_sitter_language(&self) -> tree_sitter::Language {
        match self {
            Language::Rust => tree_sitter_rust::LANGUAGE.into(),
            Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Language::Python => tree_sitter_python::LANGUAGE.into(),
            Language::Go => tree_sitter_go::LANGUAGE.into(),
            Language::Java => tree_sitter_java::LANGUAGE.into(),
            Language::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
            Language::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Language::Cpp => tree_sitter_cpp::LANGUAGE.into(),
            Language::Elixir => tree_sitter_elixir::LANGUAGE.into(),
        }
    }

    /// Get language spec with node kind mappings.
    pub fn spec(&self) -> LanguageSpec {
        match self {
            Language::Rust => LanguageSpec::RUST,
            Language::TypeScript => LanguageSpec::TYPESCRIPT,
            Language::Python => LanguageSpec::PYTHON,
            Language::Go => LanguageSpec::GO,
            Language::Java => LanguageSpec::JAVA,
            Language::CSharp => LanguageSpec::CSHARP,
            Language::JavaScript => LanguageSpec::JAVASCRIPT,
            Language::Cpp => LanguageSpec::CPP,
            Language::Elixir => LanguageSpec::ELIXIR,
        }
    }

    /// Canonical name.
    pub fn name(&self) -> &'static str {
        match self {
            Language::Rust => "rust",
            Language::TypeScript => "typescript",
            Language::Python => "python",
            Language::Go => "go",
            Language::Java => "java",
            Language::CSharp => "csharp",
            Language::JavaScript => "javascript",
            Language::Cpp => "cpp",
            Language::Elixir => "elixir",
        }
    }
}

impl std::str::FromStr for Language {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_name(s).ok_or(())
    }
}

/// Language-specific AST node kind mappings.
/// All data is static - no runtime allocation.
#[derive(Debug, Clone, Copy)]
pub struct LanguageSpec {
    pub function_nodes: &'static [&'static str],
    pub branch_nodes: &'static [&'static str],
    pub block_nodes: &'static [&'static str],
    pub import_nodes: &'static [&'static str],
    pub export_nodes: &'static [&'static str],
    pub boolean_and_nodes: &'static [&'static str],
    pub boolean_or_nodes: &'static [&'static str],
    pub else_nodes: &'static [&'static str],
}

impl LanguageSpec {
    /// Check if node kind is a function.
    #[inline]
    pub fn is_function(&self, kind: &str) -> bool {
        self.function_nodes.contains(&kind)
    }

    /// Check if node kind is a branch (affects CC).
    #[inline]
    pub fn is_branch(&self, kind: &str) -> bool {
        self.branch_nodes.contains(&kind)
    }

    /// Check if node kind is a block (affects nesting depth).
    #[inline]
    pub fn is_block(&self, kind: &str) -> bool {
        self.block_nodes.contains(&kind)
    }

    /// Check if node kind is an import.
    #[inline]
    pub fn is_import(&self, kind: &str) -> bool {
        self.import_nodes.contains(&kind)
    }

    /// Check if node kind is an export.
    #[inline]
    pub fn is_export(&self, kind: &str) -> bool {
        self.export_nodes.contains(&kind)
    }

    /// Check if node kind is boolean AND.
    #[inline]
    pub fn is_boolean_and(&self, kind: &str) -> bool {
        self.boolean_and_nodes.contains(&kind)
    }

    /// Check if node kind is boolean OR.
    #[inline]
    pub fn is_boolean_or(&self, kind: &str) -> bool {
        self.boolean_or_nodes.contains(&kind)
    }

    /// Check if node kind is else/else-if.
    #[inline]
    pub fn is_else(&self, kind: &str) -> bool {
        self.else_nodes.contains(&kind)
    }

    /// Classify branch type for cognitive complexity.
    pub fn classify_branch(&self, kind: &str) -> BranchKind {
        if self.is_boolean_and(kind) {
            return BranchKind::BooleanAnd;
        }
        if self.is_boolean_or(kind) {
            return BranchKind::BooleanOr;
        }
        if self.is_else(kind) {
            return BranchKind::Else;
        }

        // Pattern match on common branch kinds
        match kind {
            "if_expression" | "if_statement" | "if" | "conditional_expression" | "unless" => {
                BranchKind::Conditional
            }
            "for_expression" | "for_statement" | "for" | "for_in_statement"
            | "while_expression" | "while_statement" | "while" | "loop_expression"
            | "do_statement" => BranchKind::Loop,
            "match_expression" | "switch_statement" | "case" | "cond" | "with" => BranchKind::Switch,
            "case_clause" | "match_arm" => BranchKind::SwitchCase,
            "try_expression" | "try_statement" | "rescue" | "catch_clause" | "except_clause"
            | "elif_clause" => BranchKind::Exception,
            _ => BranchKind::Conditional,
        }
    }

    pub const RUST: Self = Self {
        function_nodes: &["function_item", "impl_item", "closure_expression"],
        branch_nodes: &[
            "if_expression",
            "match_expression",
            "for_expression",
            "while_expression",
            "loop_expression",
        ],
        block_nodes: &["block", "declaration_list"],
        import_nodes: &["use_declaration"],
        export_nodes: &["function_item", "struct_item", "enum_item", "trait_item"],
        boolean_and_nodes: &["binary_expression"],
        boolean_or_nodes: &[],
        else_nodes: &["else_clause"],
    };

    pub const TYPESCRIPT: Self = Self {
        function_nodes: &[
            "function_declaration",
            "method_definition",
            "arrow_function",
            "function_expression",
        ],
        branch_nodes: &[
            "if_statement",
            "switch_statement",
            "for_statement",
            "for_in_statement",
            "while_statement",
            "do_statement",
            "conditional_expression",
        ],
        block_nodes: &["statement_block", "class_body"],
        import_nodes: &["import_statement", "import_specifier"],
        export_nodes: &["export_statement"],
        boolean_and_nodes: &[],
        boolean_or_nodes: &[],
        else_nodes: &["else_clause"],
    };

    pub const JAVASCRIPT: Self = Self {
        function_nodes: &[
            "function_declaration",
            "method_definition",
            "arrow_function",
            "function_expression",
        ],
        branch_nodes: &[
            "if_statement",
            "switch_statement",
            "for_statement",
            "for_in_statement",
            "while_statement",
            "do_statement",
            "conditional_expression",
        ],
        block_nodes: &["statement_block", "class_body"],
        import_nodes: &["import_statement", "call_expression"],
        export_nodes: &["export_statement"],
        boolean_and_nodes: &[],
        boolean_or_nodes: &[],
        else_nodes: &["else_clause"],
    };

    pub const PYTHON: Self = Self {
        function_nodes: &["function_definition", "class_definition", "lambda"],
        branch_nodes: &[
            "if_statement",
            "elif_clause",
            "for_statement",
            "while_statement",
            "try_statement",
            "except_clause",
            "conditional_expression",
        ],
        block_nodes: &["block", "class_body"],
        import_nodes: &["import_statement", "import_from_statement"],
        export_nodes: &[],
        boolean_and_nodes: &["and_operator"],
        boolean_or_nodes: &["or_operator"],
        else_nodes: &["else_clause"],
    };

    pub const GO: Self = Self {
        function_nodes: &["function_declaration", "method_declaration", "func_literal"],
        branch_nodes: &[
            "if_statement",
            "switch_statement",
            "for_statement",
            "select_statement",
            "type_switch_statement",
        ],
        block_nodes: &["block"],
        import_nodes: &["import_declaration", "import_spec"],
        export_nodes: &["function_declaration", "type_declaration"],
        boolean_and_nodes: &[],
        boolean_or_nodes: &[],
        else_nodes: &["else_clause"],
    };

    pub const JAVA: Self = Self {
        function_nodes: &[
            "method_declaration",
            "constructor_declaration",
            "class_declaration",
            "lambda_expression",
        ],
        branch_nodes: &[
            "if_statement",
            "while_statement",
            "for_statement",
            "do_statement",
            "switch_statement",
            "try_statement",
            "catch_clause",
            "ternary_expression",
        ],
        block_nodes: &["block"],
        import_nodes: &["import_declaration"],
        export_nodes: &[
            "class_declaration",
            "interface_declaration",
            "method_declaration",
        ],
        boolean_and_nodes: &[],
        boolean_or_nodes: &[],
        else_nodes: &["else"],
    };

    pub const CSHARP: Self = Self {
        function_nodes: &[
            "method_declaration",
            "constructor_declaration",
            "class_declaration",
            "struct_declaration",
            "lambda_expression",
            "local_function_statement",
        ],
        branch_nodes: &[
            "if_statement",
            "while_statement",
            "for_statement",
            "foreach_statement",
            "switch_statement",
            "try_statement",
            "catch_clause",
            "ternary_expression",
        ],
        block_nodes: &["block", "switch_body"],
        import_nodes: &["using_directive"],
        export_nodes: &[
            "class_declaration",
            "struct_declaration",
            "interface_declaration",
            "method_declaration",
            "delegate_declaration",
        ],
        boolean_and_nodes: &[],
        boolean_or_nodes: &[],
        else_nodes: &["else_clause"],
    };

    pub const CPP: Self = Self {
        function_nodes: &[
            "function_definition",
            "method_definition",
            "lambda_expression",
            "class_specifier",
        ],
        branch_nodes: &[
            "if_statement",
            "while_statement",
            "for_statement",
            "switch_statement",
            "try_statement",
            "catch_clause",
        ],
        block_nodes: &["compound_statement"],
        import_nodes: &[],
        export_nodes: &["function_definition", "class_specifier", "struct_specifier"],
        boolean_and_nodes: &[],
        boolean_or_nodes: &[],
        else_nodes: &["else_clause"],
    };

    pub const ELIXIR: Self = Self {
        function_nodes: &["call", "def", "defp", "defmodule", "anonymous_function"],
        branch_nodes: &[
            "if", "unless", "case", "cond", "with", "try", "rescue", "for",
        ],
        block_nodes: &["do_block", "block"],
        import_nodes: &["import", "alias", "use", "require"],
        export_nodes: &["def"],
        boolean_and_nodes: &["and"],
        boolean_or_nodes: &["or"],
        else_nodes: &["else"],
    };
}

/// Branch classification for complexity calculations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchKind {
    Conditional,
    Loop,
    Switch,
    SwitchCase,
    Exception,
    Else,
    BooleanAnd,
    BooleanOr,
}

impl BranchKind {
    /// Does this branch add to cyclomatic complexity?
    #[inline]
    pub fn adds_cc(&self) -> bool {
        !matches!(self, Self::SwitchCase | Self::Else)
    }

    /// Does this branch add nesting penalty for cognitive complexity?
    #[inline]
    pub fn adds_nesting_penalty(&self) -> bool {
        matches!(
            self,
            Self::Conditional | Self::Loop | Self::Switch | Self::Exception
        )
    }

    /// Does this branch increase cognitive nesting level?
    #[inline]
    pub fn increases_nesting(&self) -> bool {
        matches!(
            self,
            Self::Conditional | Self::Loop | Self::Switch | Self::Exception | Self::Else
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_from_str() {
        assert_eq!(Language::from_name("rust"), Some(Language::Rust));
        assert_eq!(Language::from_name("RS"), Some(Language::Rust));
        assert_eq!(Language::from_name("TypeScript"), Some(Language::TypeScript));
        assert_eq!(Language::from_name("unknown"), None);
    }

    #[test]
    fn language_from_path() {
        use std::path::PathBuf;

        assert_eq!(
            Language::from_path(&PathBuf::from("test.rs")),
            Some(Language::Rust)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("test.py")),
            Some(Language::Python)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("test.txt")),
            None
        );
    }

    #[test]
    fn spec_lookups() {
        let spec = LanguageSpec::RUST;
        assert!(spec.is_function("function_item"));
        assert!(!spec.is_function("if_expression"));
        assert!(spec.is_branch("if_expression"));
        assert!(spec.is_block("block"));
    }

    #[test]
    fn branch_classification() {
        let spec = LanguageSpec::RUST;
        assert_eq!(
            spec.classify_branch("if_expression"),
            BranchKind::Conditional
        );
        assert_eq!(
            spec.classify_branch("for_expression"),
            BranchKind::Loop
        );
    }
}
