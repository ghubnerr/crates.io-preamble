use regex;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile;

pub struct CFileAnalyzer {
    include_paths: Vec<PathBuf>,
    parsed_files: HashMap<PathBuf, ParsedFile>,
    import_graph: HashMap<PathBuf, Vec<PathBuf>>,
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
    name: String,
    return_type: String,
    parameters: Vec<(String, String)>, // (type, name)
}

#[derive(Clone, Debug)]
pub struct TypeDef {
    name: String,
    definition: String,
}

#[derive(Clone, Debug)]
pub struct Macro {
    name: String,
    definition: String,
    parameters: Option<String>,
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
        println!("Creating new CFileAnalyzer");
        CFileAnalyzer {
            include_paths: vec![],
            parsed_files: HashMap::new(),
            import_graph: HashMap::new(),
        }
    }

    pub fn add_include_path(&mut self, path: PathBuf) {
        println!("Adding include path: {:?}", path);
        self.include_paths.push(path);
    }

    pub fn parse_file(&mut self, path: &Path) -> Result<(), AnalyzerError> {
        println!("Parsing file: {:?}", path);
        let content = fs::read_to_string(path)?;
        let parsed_file = self.parse_content(path, &content)?;

        println!("Inserting parsed file into parsed_files");
        self.parsed_files.insert(path.to_path_buf(), parsed_file);
        self.update_import_graph(path);

        println!("File parsed successfully: {:?}", path);
        Ok(())
    }

    fn parse_content(&self, path: &Path, content: &str) -> Result<ParsedFile, AnalyzerError> {
        println!("Parsing content of file: {:?}", path);
        let imports = self.extract_imports(content);
        let functions = self.extract_functions(content);
        let types = self.extract_types(content);
        let macros = self.extract_macros(content);

        println!(
            "Content parsed: {} imports, {} functions, {} types, {} macros",
            imports.len(),
            functions.len(),
            types.len(),
            macros.len()
        );
        Ok(ParsedFile {
            path: path.to_path_buf(),
            imports,
            functions,
            types,
            macros,
        })
    }

    fn extract_imports(&self, content: &str) -> Vec<Import> {
        println!("Extracting imports");
        let imports: Vec<_> = content
            .lines()
            .filter(|line| line.trim().starts_with("#include"))
            .map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                let path = parts[1]
                    .trim_matches(|c| c == '"' || c == '<' || c == '>')
                    .to_string();
                let is_system = line.contains('<');
                println!("Found import: {} (system: {})", path, is_system);
                Import { path, is_system }
            })
            .collect();
        println!("Extracted {} imports", imports.len());
        imports
    }

    fn extract_functions(&self, content: &str) -> Vec<Function> {
        println!("Extracting functions");
        let mut functions = Vec::new();
        let re = regex::Regex::new(r"(\w+)\s+(\w+)\((.*?)\);").unwrap();

        for cap in re.captures_iter(content) {
            let return_type = cap[1].to_string();
            let name = cap[2].to_string();
            let params_str = &cap[3];
            let parameters = self.extract_parameters(params_str);

            println!("Found function: {} (return type: {})", name, return_type);
            functions.push(Function {
                name,
                return_type,
                parameters,
            });
        }
        println!("Extracted {} functions", functions.len());
        functions
    }

    fn extract_parameters(&self, params_str: &str) -> Vec<(String, String)> {
        println!("Extracting parameters from: {}", params_str);
        let params: Vec<_> = params_str
            .split(',')
            .map(|p| p.trim())
            .filter(|p| !p.is_empty())
            .map(|param| {
                let parts: Vec<&str> = param.split_whitespace().collect();
                if parts.len() == 2 {
                    println!("Found parameter: {} {}", parts[0], parts[1]);
                    (parts[0].to_string(), parts[1].to_string())
                } else {
                    println!("Special case parameter: void");
                    ("void".to_string(), "".to_string())
                }
            })
            .collect();
        println!("Extracted {} parameters", params.len());
        params
    }

    fn extract_macros(&self, content: &str) -> Vec<Macro> {
        println!("Extracting macros");
        let mut macros = Vec::new();
        let re = regex::Regex::new(r"(?m)^\s*#\s*define\s+(\w+)(\([^)]*\))?\s*(.*)").unwrap();

        for cap in re.captures_iter(content) {
            println!("Found macro: {}", cap[1].to_string());
            macros.push(Macro {
                name: cap[1].to_string(),
                parameters: cap.get(2).map(|m| m.as_str().to_string()),
                definition: cap[3].trim().to_string(),
            });
        }

        println!("Extracted {} macros", macros.len());
        macros
    }

    fn extract_types(&self, content: &str) -> Vec<TypeDef> {
        println!("Extracting types");
        let mut types = Vec::new();
        let re =
            regex::Regex::new(r"typedef\s+(?:struct\s+\w+\s*\{[^\}]*\}\s+|\w+\s+)(\w+);").unwrap();

        for cap in re.captures_iter(content) {
            println!("Found type: {}", cap[1].to_string());
            types.push(TypeDef {
                definition: cap.get(0).unwrap().as_str().to_string(),
                name: cap[1].to_string(),
            });
        }
        println!("Extracted {} types", types.len());
        types
    }

    fn update_import_graph(&mut self, path: &Path) {
        println!("Updating import graph for: {:?}", path);
        if let Some(parsed_file) = self.parsed_files.get(path) {
            let imports: Vec<PathBuf> = parsed_file
                .imports
                .iter()
                .filter_map(|import| self.resolve_import(path, import))
                .collect();
            println!("Found {} resolved imports", imports.len());
            self.import_graph.insert(path.to_path_buf(), imports);
        }
        println!("Import graph updated");
    }

    fn resolve_import(&self, current_file: &Path, import: &Import) -> Option<PathBuf> {
        println!(
            "Resolving import: {} for file: {:?}",
            import.path, current_file
        );
        for include_path in &self.include_paths {
            let mut candidate_path = include_path.clone();
            candidate_path.push(&import.path);

            if candidate_path.exists() {
                println!("Resolved import to: {:?}", candidate_path);
                return Some(candidate_path);
            }
        }
        println!("Could not resolve import: {}", import.path);
        None
    }

    fn generate_description(&self, file: &ParsedFile) -> String {
        println!("Generating description for file: {:?}", file.path);
        let description = format!(
            "Header file containing {} functions, {} types, and {} macros",
            file.functions.len(),
            file.types.len(),
            file.macros.len(),
        );
        println!("Generated description: {}", description);
        description
    }

    pub fn analyze_c_file(
        &mut self,
        c_file_path: &Path,
    ) -> Result<Vec<HeaderSummary>, AnalyzerError> {
        println!("Starting analysis of C file: {:?}", c_file_path);
        let mut analyzed_headers = HashSet::new();
        let mut summaries = Vec::new();

        self.analyze_file_recursive(c_file_path, &mut analyzed_headers, &mut summaries)?;

        println!(
            "Analysis complete. Found {} header summaries",
            summaries.len()
        );
        Ok(summaries)
    }

    pub fn analyze_file_recursive(
        &mut self,
        file_path: &Path,
        analyzed_headers: &mut HashSet<PathBuf>,
        summaries: &mut Vec<HeaderSummary>,
    ) -> Result<(), AnalyzerError> {
        println!("Analyzing file: {:?}", file_path);
        if analyzed_headers.contains(file_path) {
            println!("File already analyzed, skipping: {:?}", file_path);
            return Ok(());
        }

        self.parse_file(file_path)?;
        analyzed_headers.insert(file_path.to_path_buf());
        println!("File parsed and added to analyzed headers: {:?}", file_path);

        let mut imports_to_analyze = Vec::new();
        let mut summary = None;

        if let Some(parsed_file) = self.parsed_files.get(file_path) {
            println!("Found parsed file for: {:?}", file_path);
            for import in &parsed_file.imports {
                if !import.is_system {
                    if let Some(resolved_path) = self.resolve_import(file_path, import) {
                        println!("Found non-system import to analyze: {:?}", resolved_path);
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
            println!("Created summary for file: {:?}", file_path);
        }

        for import_path in imports_to_analyze {
            println!("Recursively analyzing import: {:?}", import_path);
            self.analyze_file_recursive(&import_path, analyzed_headers, summaries)?;
        }

        if let Some(summary) = summary {
            summaries.push(summary);
            println!("Added summary to results for file: {:?}", file_path);
        }

        println!("Completed analysis of file: {:?}", file_path);
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

    // Test  imports
    #[test]
    fn test_extract_imports() {
        let analyzer = CFileAnalyzer::new();
        let content = r#"
            #include <stdio.h>
            #include "my_header.h"
        "#;

        let imports = analyzer.extract_imports(content);
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].path, "stdio.h");
        assert!(imports[0].is_system);
        assert_eq!(imports[1].path, "my_header.h");
        assert!(!imports[1].is_system);
    }

    #[test]
    fn test_extract_functions() {
        let analyzer = CFileAnalyzer::new();
        let content = r#"
            int add(int a, int b);
            void say_hello();
        "#;

        let functions = analyzer.extract_functions(content);
        assert_eq!(functions.len(), 2);
        assert_eq!(functions[0].name, "add");
        assert_eq!(functions[0].return_type, "int");
        assert_eq!(
            functions[0].parameters[0],
            ("int".to_string(), "a".to_string())
        );

        assert_eq!(functions[1].name, "say_hello");
        assert_eq!(functions[1].return_type, "void");
        assert!(functions[1].parameters.is_empty());
    }

    #[test]
    fn test_extract_types() {
        let analyzer = CFileAnalyzer::new();
        let content = r#"
            typedef int myint;
            typedef struct MyStruct {
                int x;
                int y;
            } MyStruct;
        "#;

        let types = analyzer.extract_types(content);
        assert_eq!(types.len(), 2);
        assert_eq!(types[0].name, "myint");
        assert_eq!(types[1].name, "MyStruct");
    }

    #[test]
    fn test_extract_macros() {
        let analyzer = CFileAnalyzer::new();
        let content = r#"
        #define MAX 100
        #define SQUARE(x) ((x) * (x))
    "#;

        let macros = analyzer.extract_macros(content);
        assert_eq!(macros.len(), 2);
        assert_eq!(macros[0].name, "MAX");
        assert_eq!(macros[0].definition, "100");
        assert_eq!(macros[1].name, "SQUARE");
        assert_eq!(macros[1].definition, "((x) * (x))");
    }

    #[test]
    fn test_parse_file() {
        let mut analyzer = CFileAnalyzer::new();

        let mut tmp_file = NamedTempFile::new().unwrap();
        write!(
            tmp_file,
            r#"
            #include <stdio.h>
            typedef int myint;
            void say_hello();
            #define MAX 100
        "#
        )
        .unwrap();

        let path = tmp_file.path().to_path_buf();
        analyzer.parse_file(path.as_path()).unwrap();

        assert!(analyzer.parsed_files.contains_key(&path));
        let parsed_file = analyzer.parsed_files.get(&path).unwrap();
        assert_eq!(parsed_file.functions.len(), 1);
        assert_eq!(parsed_file.types.len(), 1);
        assert_eq!(parsed_file.macros.len(), 1);
    }

    #[test]
    fn test_update_import_graph() {
        let mut analyzer = CFileAnalyzer::new();

        let mut tmp_file = NamedTempFile::new().unwrap();
        write!(
            tmp_file,
            r#"
            #include "my_header.h"
        "#
        )
        .unwrap();
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

        let resolved = analyzer.resolve_import(Path::new("main.c"), &import);
        assert!(resolved.is_none());
    }
}
