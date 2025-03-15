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
    let lines: Vec<&str> = sql.lines().collect();
    let mut result = Vec::new();
    let mut i = 0;
    
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        
        // Check if this is an INSERT statement
        if trimmed.to_uppercase().starts_with("INSERT INTO") {
            let mut insert_statement = Vec::new();
            insert_statement.push(line.to_string());
            
            // Find the end of this INSERT statement
            let mut j = i + 1;
            let mut found_end = false;
            let mut in_quotes = false;
            let mut quote_char = ' ';
            let mut escaped = false;
            let mut brace_level = 0;
            
            // Count opening parentheses in the first line
            for c in line.chars() {
                if !in_quotes && c == '(' {
                    brace_level += 1;
                } else if !in_quotes && c == ')' {
                    brace_level -= 1;
                } else if !escaped && (c == '\'' || c == '"') {
                    if !in_quotes {
                        in_quotes = true;
                        quote_char = c;
                    } else if c == quote_char {
                        in_quotes = false;
                    }
                }
                escaped = c == '\\';
            }
            
            // Look for the end of the statement
            while j < lines.len() && !found_end {
                let next_line = lines[j];
                
                for c in next_line.chars() {
                    if !in_quotes && c == '(' {
                        brace_level += 1;
                    } else if !in_quotes && c == ')' {
                        brace_level -= 1;
                    } else if !escaped && (c == '\'' || c == '"') {
                        if !in_quotes {
                            in_quotes = true;
                            quote_char = c;
                        } else if c == quote_char {
                            in_quotes = false;
                        }
                    } else if !in_quotes && c == ';' && brace_level == 0 {
                        found_end = true;
                        break;
                    }
                    escaped = c == '\\' && !escaped;
                }
                
                insert_statement.push(next_line.to_string());
                j += 1;
                
                if found_end || (brace_level == 0 && !in_quotes && next_line.trim().ends_with(';')) {
                    break;
                }
            }
            
            // Format the INSERT statement
            let formatted_insert = format_insert_statement(&insert_statement);
            result.extend(formatted_insert);
            
            i = j;
        } else {
            // Not an INSERT statement, keep it as is
            result.push(line.to_string());
            i += 1;
        }
    }
    
    result.join("\n")
}

fn format_insert_statement(lines: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    
    if lines.is_empty() {
        return result;
    }
    
    // First line should contain INSERT INTO table (columns)
    let first_line = &lines[0];
    let mut formatted_first_line = String::new();
    
    // Format the column list if there's an opening parenthesis in the first line
    if let Some(open_paren_idx) = first_line.find('(') {
        // Handle columns on first line
        formatted_first_line.push_str(&first_line[..open_paren_idx + 1]);
        
        let mut column_list = String::new();
        let mut in_quotes = false;
        let mut quote_char = ' ';
        let mut escaped = false;
        let mut brace_level = 1; // We're already inside the first parenthesis
        
        for c in first_line[open_paren_idx + 1..].chars() {
            if !escaped && (c == '\'' || c == '"') {
                if !in_quotes {
                    in_quotes = true;
                    quote_char = c;
                } else if c == quote_char {
                    in_quotes = false;
                }
                column_list.push(c);
            } else if !in_quotes && c == '(' {
                brace_level += 1;
                column_list.push(c);
            } else if !in_quotes && c == ')' {
                brace_level -= 1;
                if brace_level == 0 {
                    // End of column list
                    let formatted_columns = format_column_list(&column_list);
                    formatted_first_line.push_str(&formatted_columns);
                    formatted_first_line.push(')');
                    
                    // Add the rest of the line after the closing parenthesis
                    let close_paren_idx = first_line[open_paren_idx + 1..].find(')').unwrap() + open_paren_idx + 1;
                    if close_paren_idx + 1 < first_line.len() {
                        formatted_first_line.push_str(&first_line[close_paren_idx + 1..]);
                    }
                    break;
                } else {
                    column_list.push(c);
                }
            } else if c == ',' && !in_quotes {
                column_list.push(',');
                column_list.push(' ');
            } else {
                column_list.push(c);
                escaped = c == '\\' && !escaped;
            }
        }
        
        // If we didn't close the parenthesis yet, the column list continues to the next lines
        if brace_level > 0 {
            // Just add what we've processed so far
            formatted_first_line.push_str(&column_list);
            result.push(formatted_first_line);
            
            // Process columns that continue to next lines
            let mut i = 1;
            while i < lines.len() && brace_level > 0 {
                let line = &lines[i];
                let mut formatted_line = String::new();
                let mut line_column_list = String::new();
                
                for c in line.chars() {
                    if !escaped && (c == '\'' || c == '"') {
                        if !in_quotes {
                            in_quotes = true;
                            quote_char = c;
                        } else if c == quote_char {
                            in_quotes = false;
                        }
                        line_column_list.push(c);
                    } else if !in_quotes && c == '(' {
                        brace_level += 1;
                        line_column_list.push(c);
                    } else if !in_quotes && c == ')' {
                        brace_level -= 1;
                        if brace_level == 0 {
                            // End of column list
                            let formatted_columns = format_column_list(&line_column_list);
                            formatted_line.push_str(&formatted_columns);
                            formatted_line.push(')');
                            
                            // Add the rest of the line
                            let close_paren_idx = line.find(')').unwrap();
                            if close_paren_idx + 1 < line.len() {
                                formatted_line.push_str(&line[close_paren_idx + 1..]);
                            }
                            break;
                        } else {
                            line_column_list.push(c);
                        }
                    } else if c == ',' && !in_quotes {
                        line_column_list.push(',');
                        line_column_list.push(' ');
                    } else {
                        line_column_list.push(c);
                        escaped = c == '\\' && !escaped;
                    }
                }
                
                if brace_level > 0 {
                    formatted_line.push_str(&line_column_list);
                }
                
                result.push(formatted_line);
                i += 1;
            }
            
            // Process VALUES section
            let mut found_values = false;
            while i < lines.len() {
                let line = &lines[i];
                let trimmed = line.trim();
                
                if !found_values && trimmed.to_uppercase() == "VALUES" {
                    found_values = true;
                    result.push(line.to_string());
                } else if found_values {
                    // Format values rows
                    let formatted_values_line = format_values_line(line);
                    result.push(formatted_values_line);
                } else {
                    result.push(line.to_string());
                }
                
                i += 1;
            }
        } else {
            // Column list ended on first line
            result.push(formatted_first_line);
            
            // Process the rest of the lines
            let mut found_values = false;
            for i in 1..lines.len() {
                let line = &lines[i];
                let trimmed = line.trim();
                
                if !found_values && trimmed.to_uppercase() == "VALUES" {
                    found_values = true;
                    result.push(line.to_string());
                } else if found_values {
                    // Format values rows
                    let formatted_values_line = format_values_line(line);
                    result.push(formatted_values_line);
                } else {
                    result.push(line.to_string());
                }
            }
        }
    } else {
        // No opening parenthesis, just add the first line and process the rest
        result.push(first_line.to_string());
        
        let mut found_values = false;
        for i in 1..lines.len() {
            let line = &lines[i];
            let trimmed = line.trim();
            
            if !found_values && trimmed.to_uppercase() == "VALUES" {
                found_values = true;
                result.push(line.to_string());
            } else if found_values {
                // Format values rows
                let formatted_values_line = format_values_line(line);
                result.push(formatted_values_line);
            } else {
                result.push(line.to_string());
            }
        }
    }
    
    result
}

fn format_column_list(columns: &str) -> String {
    let mut formatted_columns = String::new();
    let mut tokens = Vec::new();
    let mut current_token = String::new();
    let mut in_quotes = false;
    let mut quote_char = ' ';
    let mut escaped = false;
    
    // Tokenize the column list, preserving quoted strings exactly
    for c in columns.chars() {
        if !escaped && (c == '\'' || c == '"') {
            current_token.push(c);
            if !in_quotes {
                in_quotes = true;
                quote_char = c;
            } else if c == quote_char {
                in_quotes = false;
            }
        } else if !in_quotes && c == ',' {
            tokens.push(current_token.trim().to_string());
            current_token = String::new();
        } else {
            current_token.push(c);
            escaped = c == '\\' && !escaped;
        }
    }
    
    if !current_token.trim().is_empty() {
        tokens.push(current_token.trim().to_string());
    }
    
    // Join tokens with comma and space
    formatted_columns = tokens.join(", ");
    
    formatted_columns
}

fn format_values_line(line: &str) -> String {
    let mut result = String::new();
    let mut current_pos = 0;
    let mut in_quotes = false;
    let mut quote_char = ' ';
    let mut escaped = false;
    let mut brace_level = 0;
    
    // Find each comma outside of quotes and add a space after it if needed
    for (i, c) in line.chars().enumerate() {
        if !escaped && (c == '\'' || c == '"') {
            if !in_quotes {
                in_quotes = true;
                quote_char = c;
            } else if c == quote_char {
                in_quotes = false;
            }
            result.push(c);
        } else if !in_quotes && c == '(' {
            brace_level += 1;
            result.push(c);
        } else if !in_quotes && c == ')' {
            brace_level -= 1;
            result.push(c);
        } else if !in_quotes && c == ',' && brace_level == 1 {
            // This is a comma separating values in the VALUES list
            result.push(',');
            
            // Check if the next character is already a space
            let next_pos = i + 1;
            if next_pos < line.len() && line.chars().nth(next_pos) != Some(' ') {
                result.push(' ');
            }
        } else {
            result.push(c);
            escaped = c == '\\' && !escaped;
        }
    }
    
    result
}