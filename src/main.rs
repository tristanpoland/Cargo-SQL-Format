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

    /// Write changes back to files instead of printing to stdout
    #[clap(short, long)]
    write: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    // Expand the glob pattern to get matching files
    let paths = glob(&args.input)?;
    
    for entry in paths {
        match entry {
            Ok(path) => {
                if path.extension().map_or(false, |ext| ext == "sql") {
                    process_file(&path, args.write)?;
                }
            }
            Err(e) => eprintln!("Error matching glob pattern: {}", e),
        }
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
    
    if write_to_file {
        // Write back to the file
        let mut output_file = File::create(path)?;
        output_file.write_all(formatted_content.as_bytes())?;
        println!("Formatted and saved: {}", path.display());
    } else {
        // Print to stdout
        println!("Formatted SQL:\n{}", formatted_content);
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
    // Split the statement into parts
    let parts: Vec<&str> = stmt.split("VALUES").collect();
    if parts.len() != 2 {
        return stmt.to_string(); // Return as is if it doesn't match our expectation
    }
    
    let insert_part = parts[0].trim();
    let values_part = parts[1].trim();
    
    // Format the VALUES part
    // First, identify the individual value tuples
    let re_values = Regex::new(r"\(\s*([^)]+)\s*\)").unwrap();
    let mut formatted_values = String::from("VALUES");
    
    for cap in re_values.captures_iter(values_part) {
        if let Some(values_match) = cap.get(0) {
            let values_tuple = values_match.as_str();
            formatted_values.push_str("\n    ");
            formatted_values.push_str(values_tuple);
            formatted_values.push_str(",");
        }
    }
    
    // Remove the last comma and add semicolon if present in original
    if formatted_values.ends_with(",") {
        formatted_values.pop();
        if values_part.trim_end().ends_with(";") {
            formatted_values.push(';');
        }
    }
    
    // Combine the parts
    format!("{}\n{}", insert_part, formatted_values)
}