use clap::{App, Arg};
use regex::Regex;
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process;

fn main() -> io::Result<()> {
    let matches = App::new("SQL Formatter")
        .version("0.1.0")
        .author("Your Name")
        .about("Formats SQL files with perfect column alignment")
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .help("Enable verbose output")
        )
        .arg(
            Arg::with_name("files")
                .multiple(true)
                .help("SQL files to format")
        )
        .arg(
            Arg::with_name("all")
                .short("a")
                .long("all")
                .help("Format all SQL files in current directory and subdirectories")
        )
        .get_matches();

    let verbose = matches.is_present("verbose");
    
    if verbose {
        println!("Verbose mode enabled");
    }
    
    // Determine which files to format
    if matches.is_present("all") {
        if verbose {
            println!("Finding all SQL files in current directory");
        }
        
        format_all_sql_files(verbose)?;
    } else if let Some(files) = matches.values_of("files") {
        let files: Vec<&str> = files.collect();
        
        if verbose {
            println!("Formatting specified files: {:?}", files);
        }
        
        for file in files {
            if let Err(e) = format_file(file, verbose) {
                eprintln!("Error formatting {}: {}", file, e);
            }
        }
    } else {
        format_all_sql_files(verbose)?;
    }
    
    Ok(())
}

fn format_all_sql_files(verbose: bool) -> io::Result<()> {
    let current_dir = env::current_dir()?;
    
    if verbose {
        println!("Searching for SQL files in: {}", current_dir.display());
    }
    
    let mut sql_files = Vec::new();
    walk_directory(&current_dir, &mut sql_files, verbose)?;
    
    if verbose {
        println!("Found {} SQL files", sql_files.len());
    }
    
    for file in sql_files {
        if let Err(e) = format_file(&file.to_string_lossy(), verbose) {
            eprintln!("Error formatting {}: {}", file.display(), e);
        }
    }
    
    Ok(())
}

fn walk_directory(dir: &Path, sql_files: &mut Vec<std::path::PathBuf>, verbose: bool) -> io::Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                let dir_name = path.file_name().unwrap().to_string_lossy();
                if dir_name != "target" && dir_name != ".git" {
                    walk_directory(&path, sql_files, verbose)?;
                }
            } else if let Some(ext) = path.extension() {
                if ext == "sql" {
                    if verbose {
                        println!("Found SQL file: {}", path.display());
                    }
                    sql_files.push(path);
                }
            }
        }
    }
    
    Ok(())
}

fn format_file(path: &str, verbose: bool) -> io::Result<()> {
    if verbose {
        println!("Reading file: {}", path);
    }
    
    let content = fs::read_to_string(path)?;
    let original_content = content.clone();
    
    // Format SQL
    let formatted = format_sql_inserts(&content, verbose);
    
    // Check if content changed
    if original_content != formatted {
        if verbose {
            println!("Content changed, writing to file");
        }
        fs::write(path, formatted)?;
        println!("Formatted: {}", path);
    } else if verbose {
        println!("No changes needed for: {}", path);
    }
    
    Ok(())
}

fn format_sql_inserts(sql: &str, verbose: bool) -> String {
    if verbose {
        println!("Formatting INSERT statements");
    }
    
    // Find all INSERT statements - improved regex to better match complete statements
    let insert_regex = Regex::new(r"(?is)(INSERT\s+INTO\s+[\w\.]+\s*\([^)]+\))\s*VALUES\s*\n?\s*\(([^;]+)(?:;|\)\)\)|$)").unwrap();
    
    let mut result = String::from(sql);
    let mut offset = 0;
    
    for captures in insert_regex.captures_iter(sql) {
        if verbose {
            println!("Found INSERT statement to format");
        }
        
        let full_match = captures.get(0).unwrap();
        let start_pos = full_match.start();
        let end_pos = full_match.end();
        let match_len = end_pos - start_pos;
        
        let header = captures.get(1).unwrap().as_str();
        let values_section = captures.get(2).unwrap().as_str();
        
        // Format the INSERT statement
        let formatted_insert = format_insert_statement(header, values_section, verbose);
        
        // Replace the original with the formatted version
        result.replace_range(
            (start_pos + offset)..(end_pos + offset),
            &formatted_insert
        );
        
        // Adjust offset for future replacements
        offset += formatted_insert.len() - match_len;
    }
    
    // Clean up any trailing parentheses
    result = Regex::new(r"\)\)\);?").unwrap().replace(&result, ");").to_string();
    
    result
}

fn format_insert_statement(header: &str, values_section: &str, verbose: bool) -> String {
    if verbose {
        println!("Formatting INSERT values section");
        println!("Values section: {}", values_section);
    }
    
    // Cleanup the values section first - remove any extra trailing parentheses
    let clean_values = values_section.trim_end_matches(')');
    
    // Split values into rows by finding closing and opening parentheses patterns
    let row_regex = Regex::new(r"\)\s*,\s*\(").unwrap();
    
    // Make sure we have the values properly enclosed in parentheses for splitting
    let rows_text = if clean_values.contains("),") {
        format!("({})", clean_values)
    } else {
        format!("({}", clean_values)
    };
    
    let mut rows: Vec<&str> = row_regex.split(&rows_text).collect();
    
    if verbose {
        println!("Found {} value rows", rows.len());
    }
    
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
    
    if verbose {
        println!("Found {} columns", column_count);
    }
    
    let mut column_widths = vec![0; column_count];
    
    for row in &values_per_row {
        for (i, value) in row.iter().enumerate() {
            if i < column_widths.len() {
                column_widths[i] = column_widths[i].max(value.len());
            }
        }
    }
    
    if verbose {
        println!("Column widths: {:?}", column_widths);
    }
    
    // Format the rows with proper alignment
    let mut formatted_rows = Vec::new();
    
    for row in values_per_row {
        let mut formatted_row = String::from("(");
        
        for (i, value) in row.iter().enumerate() {
            if i > 0 {
                formatted_row.push_str(", ");
            }
            
            // Right-align numbers, left-align everything else
            if (value.parse::<i64>().is_ok() || value.parse::<f64>().is_ok()) && !value.starts_with('\'') {
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