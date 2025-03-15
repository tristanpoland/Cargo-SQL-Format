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
    
    // Process line by line
    for line in sql.lines() {
        let trimmed = line.trim();
        
        // Check if this line is part of an INSERT statement
        if line_contains_insert(trimmed) {
            // Format the INSERT line
            let formatted_line = format_insert_line(line);
            result.push_str(&formatted_line);
            result.push('\n');
        } else if line_is_values_line(trimmed) {
            // This is a VALUES line (standalone VALUES keyword)
            result.push_str(line);
            result.push('\n');
        } else if line_is_values_row(trimmed) {
            // This is a values row line (starting with parenthesis)
            let formatted_values = format_values_line(line);
            result.push_str(&formatted_values);
            result.push('\n');
        } else {
            // Not a line we want to format, keep as is
            result.push_str(line);
            result.push('\n');
        }
    }
    
    // Remove trailing newline if the original doesn't have one
    if !sql.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }
    
    result
}

fn line_contains_insert(line: &str) -> bool {
    line.to_uppercase().contains("INSERT INTO")
}

fn line_is_values_line(line: &str) -> bool {
    line.to_uppercase() == "VALUES"
}

fn line_is_values_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('(') && (trimmed.ends_with("),") || trimmed.ends_with(");") || trimmed.ends_with(")"))
}

fn format_insert_line(line: &str) -> String {
    // Find the column list in the INSERT statement
    if let (Some(start_idx), Some(end_idx)) = (line.find('('), line.rfind(')')) {
        let before = &line[0..start_idx + 1]; // Include the opening parenthesis
        let after = &line[end_idx..]; // Include the closing parenthesis
        let columns = &line[start_idx + 1..end_idx];
        
        // Format the columns
        let formatted_columns = format_column_list(columns);
        
        format!("{}{}{}", before, formatted_columns, after)
    } else {
        // No columns found or incomplete line, return as is
        line.to_string()
    }
}

fn format_column_list(columns: &str) -> String {
    let mut formatted = String::new();
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = ' ';
    let mut escaped = false;
    
    // Split by commas, respecting quotes
    for c in columns.chars() {
        if !escaped && (c == '\'' || c == '"') {
            current.push(c);
            if !in_quotes {
                in_quotes = true;
                quote_char = c;
            } else if c == quote_char {
                in_quotes = false;
            }
        } else if c == ',' && !in_quotes {
            tokens.push(current.trim().to_string());
            current = String::new();
        } else {
            current.push(c);
        }
        
        escaped = !escaped && c == '\\';
    }
    
    // Add the last token if there is one
    if !current.trim().is_empty() {
        tokens.push(current.trim().to_string());
    }
    
    // Join with comma and space
    formatted = tokens.join(", ");
    
    formatted
}

fn format_values_line(line: &str) -> String {
    let mut result = String::new();
    let mut current_pos = 0;
    let mut in_quotes = false;
    let mut quote_char = ' ';
    let mut escaped = false;
    let mut paren_level = 0;
    
    for (i, c) in line.chars().enumerate() {
        if !escaped && (c == '\'' || c == '"') {
            if !in_quotes {
                in_quotes = true;
                quote_char = c;
            } else if c == quote_char {
                in_quotes = false;
            }
            result.push(c);
        } else if c == '(' && !in_quotes {
            paren_level += 1;
            result.push(c);
        } else if c == ')' && !in_quotes {
            paren_level -= 1;
            result.push(c);
        } else if c == ',' && !in_quotes && paren_level <= 1 {
            // This is a comma at the top level of VALUES list
            result.push(',');
            
            // Check if the next character is already a space
            if i + 1 < line.len() && line.chars().nth(i + 1) != Some(' ') {
                result.push(' ');
            }
        } else {
            result.push(c);
        }
        
        escaped = !escaped && c == '\\';
    }
    
    result
}