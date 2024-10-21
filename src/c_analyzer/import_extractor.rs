use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::{Language, Node, Parser, TreeCursor};

pub struct CFileAnalyzer {
    include_paths: Vec<PathBuf>,
    parsed_files: HashMap<PathBuf, ParsedFile>,
    import_graph: HashMap<PathBuf, Vec<PathBuf>>,
    parser: Parser,
    source: String,
}

struct ParsedFile {
    path: PathBuf,
    imports: Vec<Import>,
    functions: Vec<Function>,
    types: Vec<TypeDef>,
    macros: Vec<Macro>,
}

#[derive(Clone, Debug)]
pub struct Import {
    path: String,
    is_system: bool,
}

#[derive(Clone, Debug)]
pub struct Function {
    pub name: String,
    pub return_type: String,
    pub parameters: Vec<(String, String)>, // (type, name)
}

#[derive(Clone, Debug)]
pub struct TypeDef {
    pub name: String,
    pub definition: String,
}

#[derive(Clone, Debug)]
pub struct Macro {
    pub name: String,
    pub definition: String,
    pub parameters: Option<String>,
}

#[derive(Clone, Debug)]
pub struct HeaderSummary {
    pub path: PathBuf,
    pub description: String,
    pub functions: Vec<Function>,
    pub types: Vec<TypeDef>,
    pub macros: Vec<Macro>,
}

#[derive(Debug)]
pub enum AnalyzerError {
    IoError(std::io::Error),
    ParseError(String),
    AnalysisError(String),
}

impl From<std::io::Error> for AnalyzerError {
    fn from(error: std::io::Error) -> Self {
        AnalyzerError::IoError(error)
    }
}

impl CFileAnalyzer {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(&Language::new(tree_sitter_c::LANGUAGE))
            .expect("Error setting language");

        CFileAnalyzer {
            include_paths: vec![],
            parsed_files: HashMap::new(),
            import_graph: HashMap::new(),
            parser,
            source: String::new(),
        }
    }

    pub fn add_include_path(&mut self, path: PathBuf) {
        self.include_paths.push(path);
    }

    pub fn parse_file(&mut self, path: &Path) -> Result<(), AnalyzerError> {
        self.source = fs::read_to_string(path)?;
        let tree = self
            .parser
            .parse(&self.source, None)
            .ok_or_else(|| AnalyzerError::ParseError("Failed to parse file".to_string()))?;
        let root_node = tree.root_node();

        let parsed_file = self.parse_content(path, &root_node)?;
        self.parsed_files.insert(path.to_path_buf(), parsed_file);
        self.update_import_graph(path);

        Ok(())
    }

    fn parse_content(&self, path: &Path, node: &Node) -> Result<ParsedFile, AnalyzerError> {
        let imports = self.extract_imports(node);
        let functions = self.extract_functions(node);
        let types = self.extract_types(node);
        let macros = self.extract_macros(node);

        Ok(ParsedFile {
            path: path.to_path_buf(),
            imports,
            functions,
            types,
            macros,
        })
    }

    fn extract_imports(&self, node: &Node) -> Vec<Import> {
        let mut imports = Vec::new();
        let cursor = node.walk();

        self.traverse_tree(cursor, |node| {
            if node.kind() == "preproc_include" {
                if let Some(path) = node.child_by_field_name("path") {
                    let mut path_text = path
                        .utf8_text(self.source.as_bytes())
                        .unwrap_or("")
                        .to_string();
                    let is_system = path.kind() == "system_lib_string";

                    // Remove surrounding angle brackets or quotation marks
                    path_text = path_text
                        .trim_start_matches('<')
                        .trim_start_matches('"')
                        .trim_end_matches('>')
                        .trim_end_matches('"')
                        .to_string();

                    imports.push(Import {
                        path: path_text,
                        is_system,
                    });
                }
            }
        });

        imports
    }

    fn extract_functions(&self, node: &Node) -> Vec<Function> {
        let mut functions = Vec::new();
        let cursor = node.walk();

        self.traverse_tree(cursor, |node| {
            if node.kind() == "function_definition" || node.kind() == "declaration" {
                if let Some(declarator) = node.child_by_field_name("declarator") {
                    if let Some(name) = self.get_function_name(&declarator) {
                        let return_type = self.get_return_type(node);
                        let parameters = self.get_parameters(&declarator);
                        functions.push(Function {
                            name,
                            return_type,
                            parameters,
                        });
                    }
                }
            }
        });

        functions
    }

    fn extract_types(&self, node: &Node) -> Vec<TypeDef> {
        let mut types = Vec::new();
        let cursor = node.walk();

        self.traverse_tree(cursor, |node| {
            if node.kind() == "type_definition" {
                // Handle typedef cases
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name_text = name_node
                        .utf8_text(self.source.as_bytes())
                        .unwrap_or("")
                        .to_string();

                    let definition_node = node.child_by_field_name("type");
                    let definition_text = definition_node
                        .and_then(|n| n.utf8_text(self.source.as_bytes()).ok())
                        .unwrap_or("")
                        .to_string();

                    types.push(TypeDef {
                        name: name_text,
                        definition: definition_text,
                    });
                }
            } else if node.kind() == "struct_specifier" || node.kind() == "enum_specifier" {
                // Handle struct and enum specifiers
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name_text = name_node
                        .utf8_text(self.source.as_bytes())
                        .unwrap_or("")
                        .to_string();

                    let definition_text = node
                        .utf8_text(self.source.as_bytes())
                        .unwrap_or("")
                        .to_string();

                    types.push(TypeDef {
                        name: name_text,
                        definition: definition_text,
                    });
                }
            }
        });

        types
    }

    fn extract_macros(&self, node: &Node) -> Vec<Macro> {
        let mut macros = Vec::new();
        let cursor = node.walk();

        self.traverse_tree(cursor, |node| {
            // Debug: Print the kind of each node to understand what the tree contains
            // println!("Node kind: {}", node.kind());

            if node.kind() == "preproc_def" || node.kind() == "preproc_function_def" {
                if let Some(name) = node.child_by_field_name("name") {
                    let name_text = name
                        .utf8_text(self.source.as_bytes())
                        .unwrap_or("")
                        .to_string();

                    // Handle both parameterized and non-parameterized macros
                    let parameters = self.get_macro_parameters(node);
                    let definition = self.get_macro_definition(node);

                    macros.push(Macro {
                        name: name_text,
                        parameters,
                        definition,
                    });
                }
            }
        });

        macros
    }

    fn traverse_tree<F>(&self, mut cursor: TreeCursor, mut f: F)
    where
        F: FnMut(&Node),
    {
        let mut stack = Vec::new();
        loop {
            f(&cursor.node());

            if cursor.goto_first_child() {
                stack.push(cursor.clone());
            } else {
                while !cursor.goto_next_sibling() {
                    if let Some(parent) = stack.pop() {
                        cursor = parent;
                    } else {
                        return; // We've finished traversing the tree
                    }
                }
            }
        }
    }

    fn get_function_name(&self, declarator: &Node) -> Option<String> {
        declarator
            .child_by_field_name("declarator")?
            .utf8_text(self.source.as_bytes())
            .ok()
            .map(|s| s.to_string())
    }

    fn get_return_type(&self, function_node: &Node) -> String {
        function_node
            .child_by_field_name("type")
            .and_then(|n| n.utf8_text(self.source.as_bytes()).ok())
            .unwrap_or("")
            .to_string()
    }

    fn get_parameters(&self, declarator: &Node) -> Vec<(String, String)> {
        let mut parameters = Vec::new();
        if let Some(param_list) = declarator.child_by_field_name("parameters") {
            for param in param_list.children(&mut param_list.walk()) {
                if param.kind() == "parameter_declaration" {
                    let param_type = param
                        .child_by_field_name("type")
                        .and_then(|n| n.utf8_text(self.source.as_bytes()).ok())
                        .unwrap_or("")
                        .to_string();
                    let param_name = param
                        .child_by_field_name("declarator")
                        .and_then(|n| n.utf8_text(self.source.as_bytes()).ok())
                        .unwrap_or("")
                        .to_string();
                    parameters.push((param_type, param_name));
                }
            }
        }
        parameters
    }

    fn get_macro_parameters(&self, macro_node: &Node) -> Option<String> {
        macro_node
            .child_by_field_name("parameters")
            .and_then(|n| n.utf8_text(self.source.as_bytes()).ok())
            .map(|s| s.to_string())
    }

    fn get_macro_definition(&self, macro_node: &Node) -> String {
        macro_node
            .child_by_field_name("value")
            .and_then(|n| n.utf8_text(self.source.as_bytes()).ok())
            .unwrap_or("")
            .to_string()
    }

    fn update_import_graph(&mut self, path: &Path) {
        if let Some(parsed_file) = self.parsed_files.get(path) {
            let imports: Vec<PathBuf> = parsed_file
                .imports
                .iter()
                .filter_map(|import| self.resolve_import(path, import))
                .collect();
            self.import_graph.insert(path.to_path_buf(), imports);
        }
    }

    fn resolve_import(&self, current_file: &Path, import: &Import) -> Option<PathBuf> {
        for include_path in &self.include_paths {
            let mut candidate_path = include_path.clone();
            candidate_path.push(&import.path);

            if candidate_path.exists() {
                return Some(candidate_path);
            }
        }
        None
    }

    fn generate_description(&self, file: &ParsedFile) -> String {
        format!(
            "Header file containing {} functions, {} types, and {} macros",
            file.functions.len(),
            file.types.len(),
            file.macros.len(),
        )
    }

    pub fn analyze_c_file(
        &mut self,
        c_file_path: &Path,
    ) -> Result<Vec<HeaderSummary>, AnalyzerError> {
        let mut analyzed_headers = HashSet::new();
        let mut summaries = Vec::new();

        self.analyze_file_recursive(c_file_path, &mut analyzed_headers, &mut summaries)?;

        Ok(summaries)
    }

    pub fn analyze_file_recursive(
        &mut self,
        file_path: &Path,
        analyzed_headers: &mut HashSet<PathBuf>,
        summaries: &mut Vec<HeaderSummary>,
    ) -> Result<(), AnalyzerError> {
        if analyzed_headers.contains(file_path) {
            return Ok(());
        }

        self.parse_file(file_path)?;
        analyzed_headers.insert(file_path.to_path_buf());

        let mut imports_to_analyze = Vec::new();
        let mut summary = None;

        if let Some(parsed_file) = self.parsed_files.get(file_path) {
            for import in &parsed_file.imports {
                if !import.is_system {
                    if let Some(resolved_path) = self.resolve_import(file_path, import) {
                        imports_to_analyze.push(resolved_path);
                    }
                }
            }

            summary = Some(HeaderSummary {
                path: file_path.to_path_buf(),
                description: self.generate_description(parsed_file),
                functions: parsed_file.functions.clone(),
                types: parsed_file.types.clone(),
                macros: parsed_file.macros.clone(),
            });
        }

        for import_path in imports_to_analyze {
            self.analyze_file_recursive(&import_path, analyzed_headers, summaries)?;
        }

        if let Some(summary) = summary {
            summaries.push(summary);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    #[test]
    fn test_new_analyzer() {
        let analyzer = CFileAnalyzer::new();
        assert!(analyzer.include_paths.is_empty());
        assert!(analyzer.parsed_files.is_empty());
        assert!(analyzer.import_graph.is_empty());
    }

    #[test]
    fn test_add_include_path() {
        let mut analyzer = CFileAnalyzer::new();
        let include_path = PathBuf::from("/usr/include");
        analyzer.add_include_path(include_path.clone());
        assert_eq!(analyzer.include_paths.len(), 1);
        assert_eq!(analyzer.include_paths[0], include_path);
    }

    #[test]
    fn test_extract_imports() {
        let mut analyzer = CFileAnalyzer::new();
        let content = r#"
            #include <stdio.h>
            #include "my_header.h"
        "#;
        let mut tmp_file = NamedTempFile::new().unwrap();
        write!(tmp_file, "{}", content).unwrap();
        let path = tmp_file.path().to_path_buf();
        analyzer.parse_file(path.as_path()).unwrap();

        let parsed_file = analyzer.parsed_files.get(&path).unwrap();
        assert_eq!(parsed_file.imports.len(), 2);
        assert_eq!(parsed_file.imports[0].path, "stdio.h");
        assert!(parsed_file.imports[0].is_system);
        assert_eq!(parsed_file.imports[1].path, "my_header.h");
        assert!(!parsed_file.imports[1].is_system);
    }

    #[test]
    fn test_extract_functions() {
        let mut analyzer = CFileAnalyzer::new();
        let content = r#"
            int add(int a, int b);
            void say_hello(void);
        "#;
        let mut tmp_file = NamedTempFile::new().unwrap();
        write!(tmp_file, "{}", content).unwrap();
        let path = tmp_file.path().to_path_buf();
        analyzer.parse_file(path.as_path()).unwrap();

        let parsed_file = analyzer.parsed_files.get(&path).unwrap();
        assert_eq!(parsed_file.functions.len(), 2);
        assert_eq!(parsed_file.functions[0].name, "add");
        assert_eq!(parsed_file.functions[0].return_type, "int");
        assert_eq!(
            parsed_file.functions[0].parameters[0],
            ("int".to_string(), "a".to_string())
        );
        assert_eq!(parsed_file.functions[1].name, "say_hello");
        assert_eq!(parsed_file.functions[1].return_type, "void");
        assert_eq!(
            parsed_file.functions[1].parameters[0],
            ("void".to_string(), "".to_string())
        );
    }

    #[test]
    fn test_extract_types() {
        let mut analyzer = CFileAnalyzer::new();
        let content = r#"
            typedef int myint;
            typedef struct MyStruct {
                int x;
                int y;
            } MyStruct;
        "#;
        let mut tmp_file = NamedTempFile::new().unwrap();
        write!(tmp_file, "{}", content).unwrap();
        let path = tmp_file.path().to_path_buf();
        analyzer.parse_file(path.as_path()).unwrap();

        let parsed_file = analyzer.parsed_files.get(&path).unwrap();
        assert_eq!(parsed_file.types.len(), 2);
        assert_eq!(parsed_file.types[0].name, "myint");
        assert_eq!(parsed_file.types[1].name, "MyStruct");
    }

    #[test]
    fn test_extract_macros() {
        let mut analyzer = CFileAnalyzer::new();
        let content = r#"
        #define MAX 100
        #define SQUARE(x) ((x) * (x))
        "#;
        let mut tmp_file = NamedTempFile::new().unwrap();
        write!(tmp_file, "{}", content).unwrap();
        let path = tmp_file.path().to_path_buf();
        analyzer.parse_file(path.as_path()).unwrap();

        let parsed_file = analyzer.parsed_files.get(&path).unwrap();
        assert_eq!(parsed_file.macros.len(), 2);
        assert_eq!(parsed_file.macros[0].name, "MAX");
        assert_eq!(parsed_file.macros[0].definition, "100");
        assert_eq!(parsed_file.macros[1].name, "SQUARE");
        assert_eq!(parsed_file.macros[1].definition, "((x) * (x))");
    }

    #[test]
    fn test_parse_file() {
        let mut analyzer = CFileAnalyzer::new();

        let content = r#"
            #include <stdio.h>
            typedef int myint;
            void say_hello(void);
            #define MAX 100
        "#;
        let mut tmp_file = NamedTempFile::new().unwrap();
        write!(tmp_file, "{}", content).unwrap();
        let path = tmp_file.path().to_path_buf();
        analyzer.parse_file(path.as_path()).unwrap();

        assert!(analyzer.parsed_files.contains_key(&path));
        let parsed_file = analyzer.parsed_files.get(&path).unwrap();
        assert_eq!(parsed_file.imports.len(), 1);
        assert_eq!(parsed_file.functions.len(), 1);
        assert_eq!(parsed_file.types.len(), 1);
        assert_eq!(parsed_file.macros.len(), 1);
    }

    #[test]
    fn test_update_import_graph() {
        let mut analyzer = CFileAnalyzer::new();

        let content = r#"
            #include "my_header.h"
        "#;
        let mut tmp_file = NamedTempFile::new().unwrap();
        write!(tmp_file, "{}", content).unwrap();
        let path = tmp_file.path().to_path_buf();

        analyzer.parse_file(path.as_path()).unwrap();
        analyzer.update_import_graph(path.as_path());

        assert!(analyzer.import_graph.contains_key(&path));
    }

    #[test]
    fn test_resolve_import() {
        let mut analyzer = CFileAnalyzer::new();
        let include_path = PathBuf::from("/usr/local/include");
        analyzer.add_include_path(include_path.clone());

        let import = Import {
            path: "my_header.h".to_string(),
            is_system: false,
        };

        // Test resolution (assuming the path exists)
        let resolved = analyzer.resolve_import(Path::new("main.c"), &import);
        assert!(resolved.is_none());
    }
}
