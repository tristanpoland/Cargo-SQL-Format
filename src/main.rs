use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::error::Error;

use clap::Parser;
use glob::glob;

#[derive(Parser)]
#[clap(name = "SQL Formatter", about = "Formats SQL files without changing syntax")]
struct Cli {
    /// Path to SQL file or glob pattern to match multiple files
    #[clap(name = "PATH")]
    path: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let paths = expand_glob(&cli.path)?;
    
    for path in paths {
        println!("Processing file: {}", path.display());
        match format_sql_file(&path) {
            Ok(_) => println!("Successfully formatted {}", path.display()),
            Err(e) => eprintln!("Error formatting {}: {}", path.display(), e),
        }
    }
    
    Ok(())
}

fn expand_glob(pattern: &str) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut paths = Vec::new();
    
    for entry in glob(pattern)? {
        match entry {
            Ok(path) => {
                if path.is_file() && path.extension().map_or(false, |ext| ext == "sql") {
                    paths.push(path);
                }
            },
            Err(e) => eprintln!("Error with glob pattern: {}", e),
        }
    }
    
    if paths.is_empty() {
        return Err("No SQL files found with the given pattern".into());
    }
    
    Ok(paths)
}

fn format_sql_file(path: &Path) -> Result<(), Box<dyn Error>> {
    // Read the file content
    let mut file = File::open(path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;

    // Format the SQL content
    let formatted_content = format_sql(&content);

    // Write back to the file
    let mut file = File::create(path)?;
    file.write_all(formatted_content.as_bytes())?;

    Ok(())
}

fn format_sql(sql: &str) -> String {
    let mut result = String::new();
    let lines: Vec<&str> = sql.lines().collect();
    
    // Process each line
    for i in 0..lines.len() {
        let line = lines[i];
        
        // Apply formatting to this line
        let formatted_line = format_line(line);
        result.push_str(&formatted_line);
        
        // Add newline if not the last line
        if i < lines.len() - 1 {
            result.push('\n');
        }
    }
    
    result
}

fn format_line(line: &str) -> String {
    let mut result = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut prev_char: Option<char> = None;
    
    let chars: Vec<char> = line.chars().collect();
    
    for i in 0..chars.len() {
        let c = chars[i];
        
        // Handle quotes (carefully handling escaping)
        if c == '\'' && (prev_char != Some('\\') || (prev_char == Some('\\') && i >= 2 && chars[i-2] == '\\')) {
            in_single_quote = !in_single_quote;
        } else if c == '"' && (prev_char != Some('\\') || (prev_char == Some('\\') && i >= 2 && chars[i-2] == '\\')) {
            in_double_quote = !in_double_quote;
        }
        
        // Always add the current character
        result.push(c);
        
        // If we're not in a quote and we see a comma
        if !in_single_quote && !in_double_quote && c == ',' {
            // Check the next character
            if i + 1 < chars.len() {
                let next_char = chars[i + 1];
                // If next character is not a space or newline, add a space
                if next_char != ' ' && next_char != '\n' && next_char != '\t' {
                    result.push(' ');
                }
            }
        }
        
        prev_char = Some(c);
    }
    
    result
}