use crate::domain::entropy::SymbolDistribution;
use crate::domain::events::BranchType;
use crate::domain::primitives::{BytePos, DomainError, LanguageId, SourceText};
use crate::domain::StructuralEvent;
use tree_sitter::{Node, Parser};

pub struct TreeSitterAdapter {
    parser: Parser,
    #[allow(dead_code)]
    language: LanguageId,
    spec: LanguageSpec,
}

impl TreeSitterAdapter {
    pub fn new(language: LanguageId) -> Result<Self, DomainError> {
        let mut parser = Parser::new();
        let (ts_language, spec) = Self::get_language_and_spec(language)?;

        parser.set_language(&ts_language).map_err(|e| {
            DomainError::UnsupportedLanguage(format!("Failed to set language: {}", e))
        })?;

        Ok(Self {
            parser,
            language,
            spec,
        })
    }

    fn get_language_and_spec(
        language: LanguageId,
    ) -> Result<(tree_sitter::Language, LanguageSpec), DomainError> {
        match language {
            LanguageId::Rust => Ok((tree_sitter_rust::LANGUAGE.into(), LanguageSpec::rust())),
            LanguageId::TypeScript => Ok((
                tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
                LanguageSpec::typescript(),
            )),
            LanguageId::JavaScript => Ok((
                tree_sitter_javascript::LANGUAGE.into(),
                LanguageSpec::javascript(),
            )),
            LanguageId::Python => Ok((tree_sitter_python::LANGUAGE.into(), LanguageSpec::python())),
            LanguageId::Go => Ok((tree_sitter_go::LANGUAGE.into(), LanguageSpec::go())),
            LanguageId::Java => Ok((tree_sitter_java::LANGUAGE.into(), LanguageSpec::java())),
            LanguageId::CSharp => {
                Ok((tree_sitter_c_sharp::LANGUAGE.into(), LanguageSpec::csharp()))
            }
            LanguageId::Elixir => Ok((tree_sitter_elixir::LANGUAGE.into(), LanguageSpec::elixir())),
            LanguageId::Cpp => Ok((tree_sitter_cpp::LANGUAGE.into(), LanguageSpec::cpp())),
        }
    }

    pub fn parse_to_events(
        &mut self,
        text: &SourceText,
    ) -> Result<Vec<StructuralEvent>, DomainError> {
        let source = text.as_str();
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| DomainError::InvalidUtf8("Failed to parse source".to_string()))?;

        let mut events = Vec::new();
        let mut cursor = tree.walk();
        self.walk_tree_iterative(&mut events, &mut cursor, source.as_bytes())?;
        Ok(events)
    }

    fn walk_tree_iterative(
        &self,
        events: &mut Vec<StructuralEvent>,
        cursor: &mut tree_sitter::TreeCursor,
        source: &[u8],
    ) -> Result<(), DomainError> {
        loop {
            let node = cursor.node();
            let kind = node.kind();
            let is_function = self.spec.function_nodes.contains(&kind);
            let is_block = self.spec.block_nodes.contains(&kind);

            self.process_node_entry(events, &node, kind, source)?;

            if cursor.goto_first_child() {
                continue;
            }

            self.process_node_exit(events, &node, is_function, is_block)?;

            loop {
                if cursor.goto_next_sibling() {
                    break;
                }
                if !cursor.goto_parent() {
                    return Ok(());
                }
                let parent = cursor.node();
                let parent_kind = parent.kind();
                self.process_node_exit(
                    events,
                    &parent,
                    self.spec.function_nodes.contains(&parent_kind),
                    self.spec.block_nodes.contains(&parent_kind),
                )?;
            }
        }
    }

    fn process_node_entry(
        &self,
        events: &mut Vec<StructuralEvent>,
        node: &Node,
        kind: &str,
        source: &[u8],
    ) -> Result<(), DomainError> {
        let start_byte = node.start_byte() as u32;
        let end_byte = node.end_byte() as u32;
        let start = BytePos::new(start_byte)?;
        let end = BytePos::new(end_byte)?;

        match kind {
            k if self.spec.function_nodes.contains(&k) => {
                let name = self.extract_function_name(node, source);
                events.push(StructuralEvent::function_start(name, start, end)?);
            }
            k if self.spec.block_nodes.contains(&k) => {
                events.push(StructuralEvent::block_entry(start, end)?);
            }
            k if self.spec.branch_nodes.contains(&k) => {
                events.push(StructuralEvent::branch(
                    start,
                    end,
                    self.classify_branch(k),
                )?);
            }
            k if self.spec.import_nodes.contains(&k) => {
                if let Some(symbol) = self.extract_import_symbol(node, source) {
                    events.push(StructuralEvent::import(symbol, start, end)?);
                }
            }
            k if self.spec.export_nodes.contains(&k) => {
                if let Some(symbol) = self.extract_export_symbol(node, source) {
                    events.push(StructuralEvent::export(symbol, start, end)?);
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn process_node_exit(
        &self,
        events: &mut Vec<StructuralEvent>,
        node: &Node,
        is_function: bool,
        is_block: bool,
    ) -> Result<(), DomainError> {
        if !is_function && !is_block {
            return Ok(());
        }

        let start_byte = node.start_byte() as u32;
        let end_byte = node.end_byte() as u32;
        let start = BytePos::new(start_byte)?;
        let end = BytePos::new(end_byte)?;

        match (is_block, is_function) {
            (true, true) => {
                events.push(StructuralEvent::block_exit(start, end)?);
                events.push(StructuralEvent::function_end(start, end)?);
            }
            (true, false) => {
                events.push(StructuralEvent::block_exit(start, end)?);
            }
            (false, true) => {
                events.push(StructuralEvent::function_end(start, end)?);
            }
            (false, false) => {}
        }

        Ok(())
    }

    fn extract_function_name(&self, node: &Node, source: &[u8]) -> Option<String> {
        for child in node.children(&mut node.walk()) {
            if child.kind() == "identifier" || child.kind() == "name" {
                return Some(child.utf8_text(source).ok()?.to_string());
            }
        }
        None
    }

    fn extract_import_symbol(&self, node: &Node, source: &[u8]) -> Option<String> {
        for child in node.children(&mut node.walk()) {
            let kind = child.kind();
            if kind == "identifier"
                || kind == "scoped_identifier"
                || kind == "dotted_name"
                || kind == "alias"
            {
                return Some(child.utf8_text(source).ok()?.to_string());
            }
        }
        None
    }

    fn extract_export_symbol(&self, node: &Node, source: &[u8]) -> Option<String> {
        for child in node.children(&mut node.walk()) {
            if child.kind() == "identifier" || child.kind() == "name" {
                return Some(child.utf8_text(source).ok()?.to_string());
            }
        }
        None
    }

    fn classify_branch(&self, kind: &str) -> BranchType {
        match kind {
            "if_expression" | "if_statement" | "if" | "conditional_expression" | "unless" => {
                BranchType::Conditional
            }
            "for_expression" | "for_statement" | "for" | "for_in_statement"
            | "while_expression" | "while_statement" | "while" | "loop_expression" => {
                BranchType::Loop
            }
            "match_expression" | "switch_statement" | "case" | "case_clause" | "cond" | "with" => {
                BranchType::Switch
            }
            "try_expression" | "try_statement" | "rescue" | "catch_clause" | "except_clause"
            | "elif_clause" => BranchType::Exception,
            _ => BranchType::Conditional,
        }
    }

    /// Extract symbol distribution for entropy calculation
    /// Walks AST and collects node types, excluding comments and punctuation
    pub fn extract_symbol_distribution(
        &mut self,
        text: &SourceText,
    ) -> Result<SymbolDistribution, DomainError> {
        let source = text.as_str();
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| DomainError::InvalidUtf8("Failed to parse source".to_string()))?;

        let mut distribution = SymbolDistribution::with_capacity(128);
        let mut cursor = tree.walk();

        self.walk_for_entropy_iterative(&mut distribution, &mut cursor, source.as_bytes())?;

        Ok(distribution)
    }

    fn walk_for_entropy_iterative(
        &self,
        distribution: &mut SymbolDistribution,
        cursor: &mut tree_sitter::TreeCursor,
        _source: &[u8],
    ) -> Result<(), DomainError> {
        let mut stack: Vec<tree_sitter::Node> = Vec::new();

        loop {
            let node = cursor.node();
            let kind = node.kind();

            if Self::is_valid_symbol(kind) {
                distribution.insert(kind.to_string());
            }

            if cursor.goto_first_child() {
                stack.push(node);
                continue;
            }

            while !cursor.goto_next_sibling() {
                match stack.pop() {
                    Some(parent_node) => cursor.reset(parent_node),
                    None => return Ok(()),
                }
            }
        }
    }

    /// Check if a node kind is a valid structural symbol
    /// Excludes comments and punctuation
    fn is_valid_symbol(kind: &str) -> bool {
        // Exclude comments
        if kind == "comment"
            || kind == "line_comment"
            || kind == "block_comment"
            || kind == "doc_comment"
        {
            return false;
        }

        // Exclude punctuation (syntax glue, not user choices)
        let punctuation = [
            ";", ",", ".", "(", ")", "[", "]", "{", "}", "<", ">", ":", "::", "->", "=>", "=", "+",
            "-", "*", "/", "%", "!", "~", "&", "|", "^", "?", "@", "#", "$", "_", "\\", "'", "\"",
            "`",
        ];
        if punctuation.contains(&kind) {
            return false;
        }

        // Include keywords and identifiers (these represent logic choices)
        // Exclude only noise tokens that add no structural information
        true
    }
}

pub struct LanguageSpec {
    pub function_nodes: &'static [&'static str],
    pub branch_nodes: &'static [&'static str],
    pub block_nodes: &'static [&'static str],
    pub import_nodes: &'static [&'static str],
    pub export_nodes: &'static [&'static str],
}

impl LanguageSpec {
    pub fn rust() -> Self {
        Self {
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
        }
    }

    pub fn typescript() -> Self {
        Self {
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
        }
    }

    pub fn javascript() -> Self {
        Self {
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
        }
    }

    pub fn python() -> Self {
        Self {
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
        }
    }

    pub fn go() -> Self {
        Self {
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
        }
    }

    pub fn elixir() -> Self {
        Self {
            function_nodes: &["call", "def", "defp", "defmodule", "anonymous_function"],
            branch_nodes: &[
                "if", "unless", "case", "cond", "with", "try", "rescue", "for",
            ],
            block_nodes: &["do_block", "block"],
            import_nodes: &["import", "alias", "use", "require"],
            export_nodes: &["def"],
        }
    }

    pub fn java() -> Self {
        Self {
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
        }
    }

    pub fn csharp() -> Self {
        Self {
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
        }
    }

    pub fn cpp() -> Self {
        Self {
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_creation_rust() {
        let adapter = TreeSitterAdapter::new(LanguageId::Rust);
        assert!(adapter.is_ok());
    }

    #[test]
    fn adapter_creation_elixir() {
        let adapter = TreeSitterAdapter::new(LanguageId::Elixir);
        assert!(adapter.is_ok());
    }

    #[test]
    fn parse_rust_function() {
        let mut adapter = TreeSitterAdapter::new(LanguageId::Rust).unwrap();
        let text = SourceText::new("fn main() { if true { } }".to_string()).unwrap();

        let events = adapter.parse_to_events(&text);
        assert!(events.is_ok());

        let events = events.unwrap();
        assert!(!events.is_empty());

        let has_function = events.iter().any(|e| e.is_function_start());
        assert!(has_function);
    }

    #[test]
    fn parse_elixir_function() {
        let mut adapter = TreeSitterAdapter::new(LanguageId::Elixir).unwrap();
        let text =
            SourceText::new("defmodule Test do\n  def hello, do: :world\nend".to_string()).unwrap();

        let events = adapter.parse_to_events(&text);
        assert!(events.is_ok());
    }

    #[test]
    fn parse_python_function() {
        let mut adapter = TreeSitterAdapter::new(LanguageId::Python).unwrap();
        let text = SourceText::new("def hello():\n    if True:\n        pass".to_string()).unwrap();

        let events = adapter.parse_to_events(&text);
        assert!(events.is_ok());

        let events = events.unwrap();
        let has_function = events.iter().any(|e| e.is_function_start());
        assert!(has_function);
    }

    #[test]
    fn adapter_creation_java() {
        let adapter = TreeSitterAdapter::new(LanguageId::Java);
        assert!(adapter.is_ok());
    }

    #[test]
    fn adapter_creation_csharp() {
        let adapter = TreeSitterAdapter::new(LanguageId::CSharp);
        assert!(adapter.is_ok());
    }

    #[test]
    fn parse_java_class() {
        let mut adapter = TreeSitterAdapter::new(LanguageId::Java).unwrap();
        let text = SourceText::new(
            "public class Test {\n    public void foo() {\n        if (true) { }\n    }\n}"
                .to_string(),
        )
        .unwrap();

        let events = adapter.parse_to_events(&text);
        assert!(events.is_ok());

        let events = events.unwrap();
        let has_function = events.iter().any(|e| e.is_function_start());
        assert!(has_function);
    }

    #[test]
    fn parsecsharp_class() {
        let mut adapter = TreeSitterAdapter::new(LanguageId::CSharp).unwrap();
        let text = SourceText::new(
            "public class Test {\n    public void Foo() {\n        if (true) { }\n    }\n}"
                .to_string(),
        )
        .unwrap();

        let events = adapter.parse_to_events(&text);
        assert!(events.is_ok());

        let events = events.unwrap();
        let has_function = events.iter().any(|e| e.is_function_start());
        assert!(has_function);
    }
}
