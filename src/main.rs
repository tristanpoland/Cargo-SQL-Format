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
    let mut formatted = String::new();
    let mut in_insert = false;
    let mut in_values = false;
    let mut in_values_list = false;
    let mut paren_level = 0;
    let mut quote_char = None;
    let mut last_char: Option<char> = None;
    let mut buffer = String::new();
    
    // Split the SQL by lines to process line by line
    for line in sql.lines() {
        let mut line_buffer = String::new();
        let trimmed = line.trim();

        // Check if we're entering an INSERT statement
        if !in_insert && trimmed.to_uppercase().starts_with("INSERT INTO") {
            in_insert = true;
            in_values = false;
        }
        
        // Check if we're starting the VALUES section
        if in_insert && !in_values && trimmed.to_uppercase() == "VALUES" {
            in_values = true;
            in_values_list = true;
            line_buffer.push_str(trimmed);
            buffer.push_str(&line_buffer);
            buffer.push('\n');
            continue;
        }
        
        // Parse the line character by character
        for (i, c) in trimmed.char_indices() {
            // Track quotes (strings)
            if (c == '\'' || c == '"') && last_char != Some('\\') {
                if let Some(q) = quote_char {
                    if q == c {
                        quote_char = None;
                    }
                } else {
                    quote_char = Some(c);
                }
            }
            
            // Track parentheses level (when not in a string)
            if quote_char.is_none() {
                if c == '(' {
                    paren_level += 1;
                } else if c == ')' {
                    paren_level -= 1;
                }
            }
            
            // Add the character to our buffer
            line_buffer.push(c);
            
            // Track the last character for escaping
            last_char = Some(c);
            
            // If we're in VALUES and looking at a comma that's not in a string and at top level
            if in_values_list && c == ',' && quote_char.is_none() && paren_level == 1 {
                // Format the comma with a space after it for VALUES entries
                if i < trimmed.len() - 1 && trimmed.chars().nth(i + 1) != Some(' ') {
                    line_buffer.push(' ');
                }
            }
        }
        
        // Handle the end of the INSERT statement
        if in_insert && trimmed.ends_with(';') {
            in_insert = false;
            in_values = false;
            in_values_list = false;
        }
        
        buffer.push_str(&line_buffer);
        buffer.push('\n');
    }
    
    // Format INSERT column list
    let mut result = buffer;
    result = format_insert_columns(&result);
    
    result
}

fn format_insert_columns(sql: &str) -> String {
    let mut result = String::new();
    let mut in_column_list = false;
    let mut paren_level = 0;
    let mut quote_char = None;
    let mut column_text = String::new();
    
    for line in sql.lines() {
        let mut line_buffer = String::new();
        let mut skip_rest = false;
        
        // Check if this line contains an INSERT INTO statement
        if line.trim().to_uppercase().starts_with("INSERT INTO") {
            // Find the opening parenthesis for columns
            if let Some(open_paren_idx) = line.find('(') {
                in_column_list = true;
                paren_level = 1;
                
                // Add everything before the open parenthesis
                line_buffer.push_str(&line[..open_paren_idx + 1]);
                
                // Process the rest of the line to find column names
                for c in line[open_paren_idx + 1..].chars() {
                    if quote_char.is_some() {
                        // In string, just add the character
                        column_text.push(c);
                        if c == *quote_char.as_ref().unwrap() {
                            quote_char = None;
                        }
                    } else if c == '\'' || c == '"' {
                        // Start of string
                        column_text.push(c);
                        quote_char = Some(c);
                    } else if c == '(' {
                        // Nested parenthesis
                        paren_level += 1;
                        column_text.push(c);
                    } else if c == ')' {
                        // Close parenthesis
                        paren_level -= 1;
                        if paren_level == 0 {
                            // End of column list
                            in_column_list = false;
                            
                            // Format the column list
                            let formatted_columns = format_column_list(&column_text);
                            line_buffer.push_str(&formatted_columns);
                            line_buffer.push(')');
                            
                            // Check if there's more to this line
                            let rest_idx = line[open_paren_idx + 1..].find(')');
                            if let Some(idx) = rest_idx {
                                let rest = &line[open_paren_idx + 1 + idx + 1..];
                                if !rest.trim().is_empty() {
                                    line_buffer.push_str(rest);
                                }
                            }
                            
                            skip_rest = true;
                            break;
                        } else {
                            column_text.push(c);
                        }
                    } else if c == ',' {
                        // Comma in column list
                        column_text.push(c);
                        // If next character is not a space, add one
                        let next_idx = column_text.len();
                        if next_idx < column_text.len() && column_text.chars().nth(next_idx) != Some(' ') {
                            column_text.push(' ');
                        }
                    } else {
                        // Other character, just add it
                        column_text.push(c);
                    }
                }
                
                if !skip_rest && !in_column_list {
                    // Column list ended exactly at end of line
                    let formatted_columns = format_column_list(&column_text);
                    line_buffer.push_str(&formatted_columns);
                    line_buffer.push(')');
                }
            } else {
                // No opening parenthesis on this line, just add it as is
                line_buffer.push_str(line);
            }
        } else if in_column_list {
            // Continuing a column list from previous line
            for c in line.chars() {
                if quote_char.is_some() {
                    // In string, just add the character
                    column_text.push(c);
                    if c == *quote_char.as_ref().unwrap() {
                        quote_char = None;
                    }
                } else if c == '\'' || c == '"' {
                    // Start of string
                    column_text.push(c);
                    quote_char = Some(c);
                } else if c == '(' {
                    // Nested parenthesis
                    paren_level += 1;
                    column_text.push(c);
                } else if c == ')' {
                    // Close parenthesis
                    paren_level -= 1;
                    if paren_level == 0 {
                        // End of column list
                        in_column_list = false;
                        
                        // Format the column list
                        let formatted_columns = format_column_list(&column_text);
                        line_buffer.push_str(&formatted_columns);
                        line_buffer.push(')');
                        
                        // Add the rest of the line
                        if let Some(rest_idx) = line.find(')') {
                            let rest = &line[rest_idx + 1..];
                            if !rest.trim().is_empty() {
                                line_buffer.push_str(rest);
                            }
                        }
                        
                        skip_rest = true;
                        break;
                    } else {
                        column_text.push(c);
                    }
                } else if c == ',' {
                    // Comma in column list
                    column_text.push(c);
                    // If next character is not a space, add one
                    let next_idx = column_text.len();
                    if next_idx < column_text.len() && column_text.chars().nth(next_idx) != Some(' ') {
                        column_text.push(' ');
                    }
                } else {
                    // Other character, just add it
                    column_text.push(c);
                }
            }
            
            if !skip_rest && !in_column_list {
                // Column list ended exactly at end of line
                let formatted_columns = format_column_list(&column_text);
                line_buffer.push_str(&formatted_columns);
                line_buffer.push(')');
            }
        } else {
            // Not in a column list, just add the line as is
            line_buffer.push_str(line);
        }
        
        result.push_str(&line_buffer);
        result.push('\n');
    }
    
    result
}

fn format_column_list(columns: &str) -> String {
    let mut result = String::new();
    let mut in_string = false;
    let mut quote_char = None;
    let mut paren_level = 0;
    let mut column_start = 0;
    let mut formatted_columns = Vec::new();
    
    // Split columns while respecting quotes and parentheses
    for (i, c) in columns.char_indices() {
        if quote_char.is_some() {
            // In string
            if c == *quote_char.as_ref().unwrap() && (i == 0 || columns.chars().nth(i - 1) != Some('\\')) {
                quote_char = None;
                in_string = false;
            }
        } else if c == '\'' || c == '"' {
            // Start of string
            quote_char = Some(c);
            in_string = true;
        } else if !in_string {
            if c == '(' {
                paren_level += 1;
            } else if c == ')' {
                paren_level -= 1;
            } else if c == ',' && paren_level == 0 {
                // End of a column
                let column = columns[column_start..i].trim();
                formatted_columns.push(column.to_string());
                column_start = i + 1;
            }
        }
    }
    
    // Add the last column
    if column_start < columns.len() {
        let column = columns[column_start..].trim();
        formatted_columns.push(column.to_string());
    }
    
    // Join columns with comma and space
    result = formatted_columns.join(", ");
    
    result
}