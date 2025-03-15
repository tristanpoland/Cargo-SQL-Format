use regex::Regex;
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::Command;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    
    // Handle both `cargo sql-fmt` and `cargo-sql-fmt` invocations
    if args.len() >= 2 && args[1] == "sql-fmt" {
        // Called as `cargo sql-fmt`
        let args = &args[2..];
        if args.is_empty() {
            // Format all SQL files in the project
            format_all_sql_files()?;
        } else {
            // Format specific files
            for file in args {
                format_file(file)?;
            }
        }
    } else if args.len() >= 1 && (args[0].ends_with("cargo-sql-fmt") || args[0].ends_with("cargo-sql-fmt.exe")) {
        // Called directly as `cargo-sql-fmt`
        let args = &args[1..];
        if args.is_empty() {
            // Format all SQL files in the project
            format_all_sql_files()?;
        } else {
            // Format specific files
            for file in args {
                format_file(file)?;
            }
        }
    } else {
        eprintln!("Usage: cargo sql-fmt [files...]");
        eprintln!("If no files are specified, all SQL files in the project will be formatted.");
    }
    
    Ok(())
}

fn format_all_sql_files() -> io::Result<()> {
    let output = if cfg!(windows) {
        Command::new("cmd")
            .args(["/C", "dir /B /S *.sql"])
            .output()?
    } else {
        Command::new("sh")
            .args(["-c", "find . -name \"*.sql\" -type f"])
            .output()?
    };
    
    if output.status.success() {
        let file_list = String::from_utf8_lossy(&output.stdout);
        for file in file_list.lines() {
            if !file.contains("target/") && !file.is_empty() {
                println!("Formatting {}", file);
                format_file(file)?;
            }
        }
    }
    
    Ok(())
}

fn format_file(path: &str) -> io::Result<()> {
    let content = fs::read_to_string(path)?;
    let formatted = format_sql_inserts(&content);
    
    if content != formatted {
        fs::write(path, formatted)?;
        println!("Formatted: {}", path);
    }
    
    Ok(())
}

fn format_sql_inserts(sql: &str) -> String {
    // Find all INSERT statements
    let insert_regex = Regex::new(r"(?is)(INSERT\s+INTO\s+\w+\s*\([^)]+\))\s*VALUES\s*\n?\s*\(([^;]+)(?:;|$)").unwrap();
    
    let mut result = String::from(sql);
    let mut offset = 0;
    
    for captures in insert_regex.captures_iter(sql) {
        let full_match = captures.get(0).unwrap();
        let start_pos = full_match.start();
        let end_pos = full_match.end();
        let match_len = end_pos - start_pos;
        
        let header = captures.get(1).unwrap().as_str();
        let values_section = captures.get(2).unwrap().as_str();
        
        // Format the INSERT statement
        let formatted_insert = format_insert_statement(header, values_section);
        
        // Replace the original with the formatted version
        result.replace_range(
            (start_pos + offset)..(end_pos + offset),
            &formatted_insert
        );
        
        // Adjust offset for future replacements
        offset += formatted_insert.len() - match_len;
    }
    
    result
}

fn format_insert_statement(header: &str, values_section: &str) -> String {
    // Split values into rows by finding closing and opening parentheses patterns
    let row_regex = Regex::new(r"\)\s*,\s*\(").unwrap();
    let rows_text = if values_section.contains("),") {
        format!("({})", values_section)
    } else {
        format!("({}", values_section)
    };
    
    let mut rows: Vec<&str> = row_regex.split(&rows_text).collect();
    
    // Clean up rows (remove trailing/leading parentheses)
    for i in 0..rows.len() {
        rows[i] = rows[i].trim();
        if rows[i].ends_with(')') {
            rows[i] = &rows[i][..rows[i].len() - 1];
        }
        if rows[i].starts_with('(') {
            rows[i] = &rows[i][1..];
        }
    }
    
    // Parse each row into values, properly handling quoted strings and functions
    let mut values_per_row: Vec<Vec<String>> = Vec::new();
    for row in rows {
        let mut values = Vec::new();
        let mut current_value = String::new();
        let mut in_quote = false;
        let mut in_function = 0; // Nested function depth counter
        let mut escaped = false;
        
        for c in row.chars() {
            match c {
                '\\' => {
                    current_value.push(c);
                    escaped = true;
                },
                '\'' => {
                    current_value.push(c);
                    if !escaped {
                        in_quote = !in_quote;
                    }
                    escaped = false;
                },
                '(' => {
                    current_value.push(c);
                    if !in_quote {
                        in_function += 1;
                    }
                    escaped = false;
                },
                ')' => {
                    current_value.push(c);
                    if !in_quote && in_function > 0 {
                        in_function -= 1;
                    }
                    escaped = false;
                },
                ',' => {
                    if in_quote || in_function > 0 {
                        current_value.push(c);
                    } else {
                        values.push(current_value.trim().to_string());
                        current_value = String::new();
                    }
                    escaped = false;
                },
                _ => {
                    current_value.push(c);
                    escaped = false;
                }
            }
        }
        
        if !current_value.is_empty() {
            values.push(current_value.trim().to_string());
        }
        
        values_per_row.push(values);
    }
    
    // Find the maximum width for each column
    let column_count = values_per_row.iter().map(|row| row.len()).max().unwrap_or(0);
    let mut column_widths = vec![0; column_count];
    
    for row in &values_per_row {
        for (i, value) in row.iter().enumerate() {
            if i < column_widths.len() {
                column_widths[i] = column_widths[i].max(value.len());
            }
        }
    }
    
    // Format the rows with proper alignment
    let mut formatted_rows = Vec::new();
    
    for row in values_per_row {
        let mut formatted_row = String::from("(");
        
        for (i, value) in row.iter().enumerate() {
            if i > 0 {
                formatted_row.push_str(", ");
            }
            
            // Right-align numbers, POINTs, and numeric functions; left-align everything else
            if value.starts_with("POINT(") || 
               (value.parse::<f64>().is_ok() && !value.starts_with('\'')) || 
               value.parse::<i64>().is_ok() || 
               value == "0" || value == "1" {
                formatted_row.push_str(&format!("{:>width$}", value, width=column_widths[i]));
            } else {
                formatted_row.push_str(&format!("{:<width$}", value, width=column_widths[i]));
            }
        }
        
        formatted_row.push_str("),");
        formatted_rows.push(formatted_row);
    }
    
    // Combine everything with proper layout
    let mut result = String::new();
    result.push_str(header);
    result.push_str("\nVALUES\n");
    result.push_str(&formatted_rows.join("\n"));
    
    // Fix the last row (remove trailing comma, add semicolon)
    if result.ends_with(",") {
        result.pop();
        result.push(';');
    }
    
    result
}