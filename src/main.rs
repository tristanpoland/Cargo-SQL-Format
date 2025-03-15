use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use glob::glob;
use regex::Regex;
use clap::Parser;

/// SQL formatter that focuses on formatting INSERT statements
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Input files or glob pattern (e.g., "*.sql" or "queries/*.sql")
    #[clap(required = true)]
    input: String,

    /// Print to stdout instead of writing back to files
    #[clap(short, long)]
    print: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    // Expand the glob pattern to get matching files
    let paths = glob(&args.input)?;
    let mut processed_files = 0;
    
    for entry in paths {
        match entry {
            Ok(path) => {
                // Only process files with .sql extension
                if path.extension().map_or(false, |ext| ext.to_str().unwrap_or("") == "sql") {
                    process_file(&path, !args.print)?;
                    processed_files += 1;
                }
            }
            Err(e) => eprintln!("Error matching glob pattern: {}", e),
        }
    }
    
    if processed_files == 0 {
        println!("No SQL files found matching pattern: {}", args.input);
    } else {
        println!("Processed {} SQL file(s)", processed_files);
    }
    
    Ok(())
}

fn process_file(path: &PathBuf, write_to_file: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("Processing file: {}", path.display());
    
    // Read file content
    let mut file = File::open(path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    
    // Format the content
    let formatted_content = format_sql(&content);
    
    // Check if content actually changed
    if content == formatted_content {
        println!("No changes needed for: {}", path.display());
        return Ok(());
    }
    
    if write_to_file {
        // Write back to the file
        let mut output_file = File::create(path)?;
        output_file.write_all(formatted_content.as_bytes())?;
        println!("Formatted and saved: {}", path.display());
    } else {
        // Print to stdout
        println!("Formatted SQL for {}:\n{}", path.display(), formatted_content);
    }
    
    Ok(())
}

fn format_sql(sql: &str) -> String {
    // Regex to match INSERT statements
    let insert_regex = Regex::new(r"(?i)(INSERT\s+INTO\s+\w+\s*\([^)]+\))\s*VALUES\s*").unwrap();
    
    // Split the SQL by lines to process each statement
    let lines: Vec<&str> = sql.lines().collect();
    let mut formatted_lines = Vec::new();
    let mut i = 0;
    
    while i < lines.len() {
        let line = lines[i];
        
        // Check if this line contains an INSERT statement
        if insert_regex.is_match(line) {
            let mut insert_statement = line.to_string();
            
            // Look ahead for VALUES clause and format it
            let mut j = i + 1;
            while j < lines.len() && !lines[j].trim().starts_with(";") {
                insert_statement.push_str("\n");
                insert_statement.push_str(lines[j]);
                j += 1;
            }
            
            // If we found a semicolon, include it
            if j < lines.len() && lines[j].trim().starts_with(";") {
                insert_statement.push_str("\n");
                insert_statement.push_str(lines[j]);
                j += 1;
            }
            
            // Format the collected INSERT statement
            let formatted_insert = format_insert_statement(&insert_statement);
            formatted_lines.push(formatted_insert);
            
            // Skip the lines we've processed
            i = j;
        } else {
            // Keep non-INSERT lines as is
            formatted_lines.push(line.to_string());
            i += 1;
        }
    }
    
    formatted_lines.join("\n")
}

fn format_insert_statement(stmt: &str) -> String {
    // First, check if this is a multi-line INSERT statement
    let lines: Vec<&str> = stmt.lines().collect();
    if lines.len() <= 1 {
        return stmt.to_string(); // Already a single line, return as is
    }
    
    // Find the INSERT INTO part and VALUES part
    let insert_pattern = Regex::new(r"(?i)^(INSERT\s+INTO\s+\w+\s*\([^)]+\))").unwrap();
    let values_pattern = Regex::new(r"(?i)\bVALUES\s*").unwrap();
    
    let mut insert_part = String::new();
    let mut full_stmt = stmt.to_string();
    
    // Extract INSERT INTO part
    if let Some(insert_match) = insert_pattern.find(full_stmt.as_str()) {
        insert_part = insert_match.as_str().to_string();
        // Remove the INSERT part from the full statement
        full_stmt = full_stmt[insert_match.end()..].to_string();
    } else {
        return stmt.to_string(); // Couldn't find INSERT INTO part
    }
    
    // Format VALUES
    let mut values_keyword_pos = 0;
    if let Some(values_match) = values_pattern.find(full_stmt.as_str()) {
        values_keyword_pos = values_match.end();
    } else {
        return stmt.to_string(); // Couldn't find VALUES keyword
    }
    
    let values_keyword = &full_stmt[..values_keyword_pos];
    let values_data = &full_stmt[values_keyword_pos..];
    
    // Parse individual value tuples
    let value_tuples_pattern = Regex::new(r"\(\s*[^)]+\s*\)").unwrap();
    let mut value_tuples = Vec::new();
    
    for value_match in value_tuples_pattern.find_iter(values_data) {
        value_tuples.push(value_match.as_str().to_string());
    }
    
    // Check for trailing semicolon
    let has_semicolon = stmt.trim_end().ends_with(';');
    
    // Build the formatted statement
    let mut formatted = format!("{} {}", insert_part.trim(), values_keyword.trim());
    
    for (i, tuple) in value_tuples.iter().enumerate() {
        if i == 0 {
            formatted.push_str("\n    ");
        } else {
            formatted.push_str(",\n    ");
        }
        formatted.push_str(tuple.trim());
    }
    
    // Add semicolon if needed
    if has_semicolon {
        formatted.push(';');
    }
    
    formatted
}