use crate::domain::StructuralEvent;
use crate::domain::primitives::{LanguageId, DomainError, BytePos, SourceText};
use crate::domain::events::BranchType;
use tree_sitter::{Language, Parser, Node};

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
        
        parser.set_language(&ts_language)
            .map_err(|e| DomainError::UnsupportedLanguage(format!("Failed to set language: {}", e)))?;
        
        Ok(Self { parser, language, spec })
    }
    
    fn get_language_and_spec(language: LanguageId) -> Result<(Language, LanguageSpec), DomainError> {
        match language {
            LanguageId::Rust => Ok((
                tree_sitter_rust::LANGUAGE.into(),
                LanguageSpec::rust()
            )),
            LanguageId::TypeScript => Ok((
                tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
                LanguageSpec::typescript()
            )),
            LanguageId::JavaScript => Ok((
                tree_sitter_javascript::LANGUAGE.into(),
                LanguageSpec::javascript()
            )),
            LanguageId::Python => Ok((
                tree_sitter_python::LANGUAGE.into(),
                LanguageSpec::python()
            )),
            LanguageId::Go => Ok((
                tree_sitter_go::LANGUAGE.into(),
                LanguageSpec::go()
            )),
            LanguageId::Elixir => Ok((
                tree_sitter_elixir::LANGUAGE.into(),
                LanguageSpec::elixir()
            )),
            _ => Err(DomainError::UnsupportedLanguage(format!("{:?}", language))),
        }
    }
    
    pub fn parse_to_events(&mut self, text: &SourceText) -> Result<Vec<StructuralEvent>, DomainError> {
        let source = text.as_str();
        let tree = self.parser.parse(source, None)
            .ok_or_else(|| DomainError::InvalidUtf8("Failed to parse source".to_string()))?;
        
        let mut events = Vec::new();
        let mut cursor = tree.walk();
        
        self.walk_tree(&mut events, &mut cursor, source.as_bytes())?;
        
        Ok(events)
    }
    
    fn walk_tree(
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
                self.walk_tree(events, cursor, source)?;
                cursor.goto_parent();
            }
            
            self.process_node_exit(events, &node, is_function, is_block)?;
            
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        
        Ok(())
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
        
        if self.spec.function_nodes.contains(&kind) {
            let name = self.extract_function_name(node, source);
            let start = BytePos::new(start_byte)?;
            let end = BytePos::new(end_byte)?;
            events.push(StructuralEvent::function_start(name, start, end)?);
        }
        
        if self.spec.block_nodes.contains(&kind) {
            let start = BytePos::new(start_byte)?;
            let end = BytePos::new(end_byte)?;
            events.push(StructuralEvent::block_entry(start, end)?);
        }
        
        if self.spec.branch_nodes.contains(&kind) {
            let branch_type = self.classify_branch(kind);
            let start = BytePos::new(start_byte)?;
            let end = BytePos::new(end_byte)?;
            events.push(StructuralEvent::branch(start, end, branch_type)?);
        }
        
        if self.spec.import_nodes.contains(&kind) {
            if let Some(symbol) = self.extract_import_symbol(node, source) {
                let start = BytePos::new(start_byte)?;
                let end = BytePos::new(end_byte)?;
                events.push(StructuralEvent::import(symbol, start, end)?);
            }
        }
        
        if self.spec.export_nodes.contains(&kind) {
            if let Some(symbol) = self.extract_export_symbol(node, source) {
                let start = BytePos::new(start_byte)?;
                let end = BytePos::new(end_byte)?;
                events.push(StructuralEvent::export(symbol, start, end)?);
            }
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
        let start_byte = node.start_byte() as u32;
        let end_byte = node.end_byte() as u32;
        
        if is_block {
            let start = BytePos::new(start_byte)?;
            let end = BytePos::new(end_byte)?;
            events.push(StructuralEvent::block_exit(start, end)?);
        }
        
        if is_function {
            let start = BytePos::new(start_byte)?;
            let end = BytePos::new(end_byte)?;
            events.push(StructuralEvent::function_end(start, end)?);
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
            if kind == "identifier" || kind == "scoped_identifier" || kind == "dotted_name" || kind == "alias" {
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
            "if_expression" | "if_statement" | "if" | "conditional_expression" | "unless" => BranchType::Conditional,
            "for_expression" | "for_statement" | "for" | "for_in_statement" | 
            "while_expression" | "while_statement" | "while" | "loop_expression" => BranchType::Loop,
            "match_expression" | "switch_statement" | "case" | "case_clause" | "cond" | "with" => BranchType::Switch,
            "try_expression" | "try_statement" | "rescue" | "catch_clause" | "except_clause" | "elif_clause" => BranchType::Exception,
            _ => BranchType::Conditional,
        }
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
                "if_expression", "match_expression", "for_expression", 
                "while_expression", "loop_expression"
            ],
            block_nodes: &["block", "declaration_list"],
            import_nodes: &["use_declaration"],
            export_nodes: &["function_item", "struct_item", "enum_item", "trait_item"],
        }
    }
    
    pub fn typescript() -> Self {
        Self {
            function_nodes: &["function_declaration", "method_definition", "arrow_function", "function_expression"],
            branch_nodes: &[
                "if_statement", "switch_statement", "for_statement", 
                "for_in_statement", "while_statement", "do_statement",
                "conditional_expression"
            ],
            block_nodes: &["statement_block", "class_body"],
            import_nodes: &["import_statement", "import_specifier"],
            export_nodes: &["export_statement"],
        }
    }
    
    pub fn javascript() -> Self {
        Self {
            function_nodes: &["function_declaration", "method_definition", "arrow_function", "function_expression"],
            branch_nodes: &[
                "if_statement", "switch_statement", "for_statement", 
                "for_in_statement", "while_statement", "do_statement",
                "conditional_expression"
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
                "if_statement", "elif_clause", "for_statement", 
                "while_statement", "try_statement", "except_clause",
                "conditional_expression"
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
                "if_statement", "switch_statement", "for_statement",
                "select_statement", "type_switch_statement"
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
                "if", "unless", "case", "cond", "with",
                "try", "rescue", "for"
            ],
            block_nodes: &["do_block", "block"],
            import_nodes: &["import", "alias", "use", "require"],
            export_nodes: &["def"],
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
        let text = SourceText::new("defmodule Test do\n  def hello, do: :world\nend".to_string()).unwrap();
        
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
}
