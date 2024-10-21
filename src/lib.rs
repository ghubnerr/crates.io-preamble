use std::env;
use std::path::Path;
use std::process;

mod c_analyzer;
use c_analyzer::import_extractor::{AnalyzerError, CFileAnalyzer};

pub fn extract_import_summaries(
    file_path: &str,
) -> Result<Vec<c_analyzer::import_extractor::HeaderSummary>, AnalyzerError> {
    let mut analyzer = CFileAnalyzer::new();

    // Add default include paths
    analyzer.add_include_path(
        Path::new("/Library/Developer/CommandLineTools/usr/lib/clang/15.0.0/include").to_path_buf(),
    );
    analyzer.add_include_path(
        Path::new("/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/usr/include").to_path_buf(),
    );
    analyzer.add_include_path(
        Path::new("/Library/Developer/CommandLineTools/usr/include").to_path_buf(),
    );

    // Keep the original paths as fallbacks
    analyzer.add_include_path(Path::new("/usr/include").to_path_buf());
    analyzer.add_include_path(Path::new("/usr/local/include").to_path_buf());

    // Convert the file path string to a Path
    let path = Path::new(file_path);

    // Extract the parent directory of the file
    if let Some(parent_dir) = path.parent() {
        // Add the parent directory to the include paths
        analyzer.add_include_path(parent_dir.to_path_buf());
    }

    // Analyze the C file and get the summaries
    let summaries = analyzer.analyze_c_file(path)?;

    Ok(summaries)
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <file_path>", args[0]);
        process::exit(1);
    }

    let file_path = &args[1];

    match extract_import_summaries(file_path) {
        Ok(summaries) => {
            println!("Successfully extracted import summaries:");
            for (index, summary) in summaries.iter().enumerate() {
                println!("\nSummary {}:", index + 1);
                println!("Path: {:?}", summary.path);
                println!("Description: {}", summary.description);
                println!("Functions: {}", summary.functions.len());
                println!("Types: {}", summary.types.len());
                println!("Macros: {}", summary.macros.len());
            }
        }
        Err(e) => {
            println!("Error extracting import summaries");
            process::exit(1);
        }
    }
}
