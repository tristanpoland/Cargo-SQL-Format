use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::error::Error;
use std::cmp::max;

use clap::Parser;
use glob::glob;

#[derive(Parser)]
#[clap(name = "SQL Formatter", about = "Formats SQL files with aligned columns")]
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

#[derive(Debug)]
struct InsertStatement {
    header: String,
    values_keyword: String,
    rows: Vec<Vec<String>>,
    terminator: String,
}

fn format_sql(sql: &str) -> String {
    let mut result = String::new();
    let mut current_insert: Option<InsertStatement> = None;
    let mut buffer = Vec::new();
    
    // First pass: collect all INSERT statements
    for line in sql.lines() {
        let trimmed = line.trim();
        
        if line_contains_insert(trimmed) {
            // Start of a new INSERT statement
            if let Some(insert) = current_insert.take() {
                // Format the previous INSERT statement
                let formatted = format_insert_statement(insert);
                result.push_str(&formatted);
            }
            
            // Extract column names
            let header = line.to_string();
            current_insert = Some(InsertStatement {
                header,
                values_keyword: String::new(),
                rows: Vec::new(),
                terminator: String::new(),
            });
        } else if let Some(ref mut insert) = current_insert {
            if line_is_values_line(trimmed) {
                // This is the VALUES line
                insert.values_keyword = line.to_string();
            } else if line_is_values_row(trimmed) {
                // This is a values row
                let values = parse_values_row(line);
                insert.rows.push(values);
                
                // Check if this is the last row (has terminator)
                if trimmed.ends_with(");") {
                    insert.terminator = ");".to_string();
                }
            } else if !trimmed.is_empty() {
                // Other line that's part of the INSERT statement
                buffer.push(line.to_string());
            }
        } else {
            // Not part of an INSERT statement
            result.push_str(line);
            result.push('\n');
        }
    }
    
    // Format the last INSERT statement if any
    if let Some(insert) = current_insert {
        let formatted = format_insert_statement(insert);
        result.push_str(&formatted);
    }
    
    // Add any remaining lines
    for line in buffer {
        result.push_str(&line);
        result.push('\n');
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
    line.trim().to_uppercase() == "VALUES"
}

fn line_is_values_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('(') && (trimmed.ends_with("),") || trimmed.ends_with(");") || trimmed.ends_with(')'))
}

fn parse_values_row(line: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = ' ';
    let mut escaped = false;
    let mut paren_level = 0;
    let mut first_paren_found = false;
    
    for c in line.chars() {
        if !escaped && (c == '\'' || c == '"') {
            current.push(c);
            if !in_quotes {
                in_quotes = true;
                quote_char = c;
            } else if c == quote_char {
                in_quotes = false;
            }
        } else if c == '(' && !in_quotes {
            if !first_paren_found {
                first_paren_found = true;
                // Skip the opening parenthesis of the row
            } else {
                current.push(c);
                paren_level += 1;
            }
        } else if c == ')' && !in_quotes {
            if paren_level == 0 {
                // This is the closing parenthesis of the row
                if !current.trim().is_empty() {
                    values.push(current.trim().to_string());
                    current = String::new();
                }
            } else {
                current.push(c);
                paren_level -= 1;
            }
        } else if c == ',' && !in_quotes && paren_level == 0 {
            values.push(current.trim().to_string());
            current = String::new();
        } else {
            current.push(c);
        }
        
        escaped = !escaped && c == '\\';
    }
    
    // Add the last value if there is one
    if !current.trim().is_empty() {
        values.push(current.trim().to_string());
    }
    
    values
}

fn format_insert_statement(insert: InsertStatement) -> String {
    let mut result = String::new();
    
    // Add header
    result.push_str(&insert.header);
    result.push('\n');
    
    // Add VALUES keyword
    result.push_str(&insert.values_keyword);
    result.push('\n');
    
    // Calculate the maximum width for each column
    let num_columns = insert.rows.iter().map(|row| row.len()).max().unwrap_or(0);
    let mut column_widths = vec![0; num_columns];
    
    for row in &insert.rows {
        for (i, value) in row.iter().enumerate() {
            if i < num_columns {
                column_widths[i] = max(column_widths[i], value.len());
            }
        }
    }
    
    // Format and add each row
    for (i, row) in insert.rows.iter().enumerate() {
        result.push('(');
        
        for (j, value) in row.iter().enumerate() {
            result.push_str(value);
            
            // Add padding and comma if not the last column
            if j < row.len() - 1 {
                let padding = column_widths[j] - value.len() + 1;
                for _ in 0..padding {
                    result.push(' ');
                }
                result.push(',');
                result.push(' ');
            }
        }
        
        // Add row terminator
        if i == insert.rows.len() - 1 {
            result.push_str(&insert.terminator);
        } else {
            result.push_str("),");
        }
        
        result.push('\n');
    }
    
    result
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