use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use glob::glob;
use regex::Regex;
use clap::Parser;

/// SQL formatter that focuses on formatting INSERT statements with column alignment
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Input files or glob pattern (e.g., "*.sql" or "queries/*.sql")
    #[clap(required = true)]
    input: String,

    /// Print to stdout instead of writing back to files
    #[clap(short, long)]
    print: bool,
    
    /// Force formatting even if no changes are detected
    #[clap(short, long)]
    force: bool,
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
                    process_file(&path, !args.print, args.force)?;
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

fn process_file(path: &PathBuf, write_to_file: bool, force: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("Processing file: {}", path.display());
    
    // Read file content
    let mut file = File::open(path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    
    // Format the content
    let formatted_content = format_sql(&content);
    
    // Check if content actually changed (ignoring whitespace differences at line ends)
    let content_normalized = normalize_whitespace(&content);
    let formatted_normalized = normalize_whitespace(&formatted_content);
    let content_changed = content_normalized != formatted_normalized;
    
    if !content_changed && !force {
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

// Normalize whitespace for comparison to ignore formatting-only changes
fn normalize_whitespace(s: &str) -> String {
    let mut result = String::new();
    let mut prev_char_is_whitespace = false;
    
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_char_is_whitespace {
                result.push(' ');
                prev_char_is_whitespace = true;
            }
        } else {
            result.push(c);
            prev_char_is_whitespace = false;
        }
    }
    
    result
}

fn format_sql(sql: &str) -> String {
    let mut lines = sql.lines().collect::<Vec<_>>();
    let mut result = Vec::new();
    let mut i = 0;
    
    while i < lines.len() {
        let line = lines[i].trim();
        
        // Look for INSERT statements
        if line.to_uppercase().starts_with("INSERT INTO") {
            let mut insert_block = Vec::new();
            insert_block.push(line.to_string());
            
            // Find the VALUES keyword
            let mut j = i + 1;
            let mut values_found = false;
            
            while j < lines.len() {
                let next_line = lines[j].trim();
                insert_block.push(next_line.to_string());
                
                // Check if this line contains the VALUES keyword
                if next_line.to_uppercase().contains("VALUES") {
                    values_found = true;
                    break;
                }
                
                j += 1;
            }
            
            // If VALUES is found, collect all value tuples
            if values_found {
                j += 1;
                while j < lines.len() {
                    let next_line = lines[j].trim();
                    
                    // Stop when we reach a line that doesn't look like part of the INSERT statement
                    if next_line.is_empty() || (next_line.starts_with("--") && !next_line.contains("(")) || 
                       (next_line.to_uppercase().starts_with("INSERT") || 
                        next_line.to_uppercase().starts_with("UPDATE") || 
                        next_line.to_uppercase().starts_with("DELETE") || 
                        next_line.to_uppercase().starts_with("CREATE") || 
                        next_line.to_uppercase().starts_with("ALTER") || 
                        next_line.to_uppercase().starts_with("DROP")) && !next_line.contains(",") {
                        break;
                    }
                    
                    insert_block.push(next_line.to_string());
                    
                    // If we reach a line with a semicolon and no comma, we're done with this INSERT statement
                    if next_line.ends_with(";") && !next_line.ends_with(",;") {
                        j += 1;
                        break;
                    }
                    
                    j += 1;
                }
                
                // Format the entire INSERT statement
                let formatted_insert = format_insert_statement(&insert_block.join(" "));
                result.push(formatted_insert);
                
                // Skip ahead to the next part of the file
                i = j;
                continue;
            }
        }
        
        // For non-INSERT lines, just add them as is
        result.push(lines[i].to_string());
        i += 1;
    }
    
    result.join("\n")
}

fn format_insert_statement(stmt: &str) -> String {
    // Clean up extra whitespace
    let stmt = stmt.trim().replace("\n", " ");
    
    // Split into INSERT part and VALUES part
    let parts: Vec<&str> = stmt.split("VALUES").collect();
    if parts.len() != 2 {
        return stmt.to_string(); // Not a standard INSERT ... VALUES format
    }
    
    let insert_part = parts[0].trim();
    let values_part = parts[1].trim();
    
    // Extract value tuples
    let tuple_regex = Regex::new(r"\(\s*([^)]+)\s*\)").unwrap();
    let mut tuples = Vec::new();
    
    for cap in tuple_regex.captures_iter(values_part) {
        if let Some(inside_tuple) = cap.get(1) {
            let values = parse_values(inside_tuple.as_str());
            tuples.push(values);
        }
    }
    
    if tuples.is_empty() {
        return stmt.to_string(); // No tuples found
    }
    
    // Find max width for each column
    let num_columns = tuples.iter().map(|v| v.len()).max().unwrap_or(0);
    let mut column_widths = vec![0; num_columns];
    
    for tuple in &tuples {
        for (i, value) in tuple.iter().enumerate() {
            if i < column_widths.len() {
                column_widths[i] = column_widths[i].max(value.len());
            }
        }
    }
    
    // Format the statement
    let mut result = String::new();
    result.push_str(insert_part);
    result.push_str("\nVALUES\n");
    
    for (i, tuple) in tuples.iter().enumerate() {
        if i > 0 {
            result.push_str(",\n");
        }
        
        result.push_str("    (");
        
        for (j, value) in tuple.iter().enumerate() {
            if j > 0 {
                result.push_str(", ");
            }
            
            result.push_str(value);
            
            // Add padding, except for the last column
            if j < tuple.len() - 1 {
                let padding = " ".repeat(column_widths[j].saturating_sub(value.len()));
                result.push_str(&padding);
            }
        }
        
        result.push(')');
    }
    
    // Add semicolon if the original statement had one
    if values_part.trim_end().ends_with(';') {
        result.push(';');
    }
    
    result
}

fn parse_values(values_str: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut escape_next = false;
    
    for c in values_str.chars() {
        match c {
            '\\' if !escape_next => {
                escape_next = true;
                current.push(c);
            },
            '\'' if !escape_next => {
                in_quotes = !in_quotes;
                current.push(c);
            },
            ',' if !in_quotes && !escape_next => {
                result.push(current.trim().to_string());
                current = String::new();
            },
            _ => {
                if escape_next {
                    escape_next = false;
                }
                current.push(c);
            }
        }
    }
    
    if !current.trim().is_empty() {
        result.push(current.trim().to_string());
    }
    
    result
}