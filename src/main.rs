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
        .arg(
            Arg::with_name("dry-run")
                .short("d")
                .long("dry-run")
                .help("Show formatting changes without modifying files")
        )
        .get_matches();

    let verbose = matches.is_present("verbose");
    let dry_run = matches.is_present("dry-run");
    
    if verbose {
        println!("Verbose mode enabled");
    }
    
    if dry_run {
        println!("Dry run mode enabled - no files will be modified");
    }
    
    // Determine which files to format
    if matches.is_present("all") {
        if verbose {
            println!("Finding all SQL files in current directory");
        }
        
        format_all_sql_files(verbose, dry_run)?;
    } else if let Some(files) = matches.values_of("files") {
        let files: Vec<&str> = files.collect();
        
        if verbose {
            println!("Formatting specified files: {:?}", files);
        }
        
        for file in files {
            if let Err(e) = format_file(file, verbose, dry_run) {
                eprintln!("Error formatting {}: {}", file, e);
            }
        }
    } else {
        // Default to reading from stdin and writing to stdout if no files specified
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        
        let formatted = format_sql_inserts(&input, verbose);
        io::stdout().write_all(formatted.as_bytes())?;
    }
    
    Ok(())
}

fn format_all_sql_files(verbose: bool, dry_run: bool) -> io::Result<()> {
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
        if let Err(e) = format_file(&file.to_string_lossy(), verbose, dry_run) {
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
                if dir_name != "target" && dir_name != ".git" && !dir_name.starts_with(".") {
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

fn format_file(path: &str, verbose: bool, dry_run: bool) -> io::Result<()> {
    if verbose {
        println!("Reading file: {}", path);
    }
    
    let content = fs::read_to_string(path)?;
    
    // Process SQL comments before formatting
    let (content_without_comments, comments) = extract_comments(&content);
    let original_content = content.clone();
    
    // Format SQL
    let mut formatted = format_sql_inserts(&content_without_comments, verbose);
    
    // Restore comments
    formatted = restore_comments(formatted, comments);
    
    // Check if content changed
    if original_content != formatted {
        if verbose {
            println!("Content changed");
            
            if dry_run {
                println!("Dry run - not writing changes to file");
                println!("Original:\n{}", original_content);
                println!("Formatted:\n{}", formatted);
            }
        }
        
        if !dry_run {
            fs::write(path, formatted)?;
            println!("Formatted: {}", path);
        } else {
            println!("Would format: {}", path);
        }
    } else if verbose {
        println!("No changes needed for: {}", path);
    }
    
    Ok(())
}

// Extract SQL comments to preserve them during formatting
fn extract_comments(sql: &str) -> (String, Vec<(usize, String)>) {
    let mut result = String::from(sql);
    let mut comments = Vec::new();
    
    // Extract single-line comments (-- comment)
    let line_comment_regex = Regex::new(r"--[^\n]*").unwrap();
    
    let mut offset = 0;
    for captures in line_comment_regex.captures_iter(sql) {
        let comment = captures.get(0).unwrap();
        let start = comment.start() - offset;
        let end = comment.end() - offset;
        let text = comment.as_str().to_string();
        
        comments.push((start, text.clone()));
        result.replace_range(start..end, &" ".repeat(end - start));
        offset += end - start - 1;
    }
    
    // Extract multi-line comments (/* comment */)
    let multi_comment_regex = Regex::new(r"/\*[\s\S]*?\*/").unwrap();
    
    let mut offset = 0;
    let mut ranges_to_replace = Vec::new();
    for captures in multi_comment_regex.captures_iter(&result) {
        let comment = captures.get(0).unwrap();
        let start = comment.start() - offset;
        let end = comment.end() - offset;
        let text = comment.as_str().to_string();
        
        comments.push((start, text.clone()));
        ranges_to_replace.push((start, end));
        offset += end - start - 1;
    }
    
    for (start, end) in ranges_to_replace {
        result.replace_range(start..end, &" ".repeat(end - start));
    }
    
    (result, comments)
}

// Restore comments to their proper locations
fn restore_comments(formatted: String, comments: Vec<(usize, String)>) -> String {
    let mut result = formatted;
    
    // Insert comments in reverse order to avoid offset issues
    for (pos, comment) in comments.into_iter().rev() {
        if pos <= result.len() {
            result.insert_str(pos, &comment);
        }
    }
    
    result
}

fn format_sql_inserts(sql: &str, verbose: bool) -> String {
    if verbose {
        println!("Formatting INSERT statements");
    }
    
    // Find all INSERT statements with improved regex
    // This regex looks for INSERT INTO statements followed by VALUES
    let insert_regex = Regex::new(r#"(?is)(INSERT\s+INTO\s+[\w\.`\[\]\"]+\s*\([^)]+\))\s*VALUES\s*\n?\s*\("#).unwrap();
    
    let mut result = String::from(sql);
    
    // Find all matches first
    let mut matches = Vec::new();
    for captures in insert_regex.captures_iter(&sql) {
        let header = captures.get(1).unwrap().as_str();
        let start_pos = captures.get(0).unwrap().start();
        let header_end_pos = start_pos + captures.get(0).unwrap().as_str().len();
        
        matches.push((header.to_string(), start_pos, header_end_pos));
    }
    
    // Process matches in reverse order to avoid offset issues
    for (index, (header, start_pos, header_end_pos)) in matches.iter().enumerate().rev() {
        if verbose {
            println!("Processing INSERT statement {}", index + 1);
        }
        
        // Find the end of the VALUES section
        let mut depth = 1; // Starting with one opening parenthesis
        let mut end_pos = *header_end_pos;
        let mut in_string = false;
        let mut string_delimiter = ' '; // Will be set to ' or " when a string starts
        let mut escaped = false;
        
        let chars: Vec<char> = result[*header_end_pos..].chars().collect();
        
        for (i, &c) in chars.iter().enumerate() {
            match c {
                '\\' => {
                    escaped = !escaped;
                },
                '\'' | '"' | '`' => {
                    if !escaped {
                        if !in_string {
                            // Start of a string
                            in_string = true;
                            string_delimiter = c;
                        } else if c == string_delimiter {
                            // End of a string
                            in_string = false;
                        }
                    }
                    escaped = false;
                },
                '(' => {
                    if !in_string {
                        depth += 1;
                    }
                    escaped = false;
                },
                ')' => {
                    if !in_string {
                        depth -= 1;
                        if depth == 0 {
                            // Found the end of the VALUES section
                            end_pos = *header_end_pos + i + 1;
                            
                            // Look for a semicolon or another closing paren
                            for j in i+1..chars.len() {
                                if chars[j] == ';' {
                                    end_pos = *header_end_pos + j + 1;
                                    break;
                                } else if !chars[j].is_whitespace() && chars[j] != ',' {
                                    break;
                                }
                            }
                            
                            break;
                        }
                    }
                    escaped = false;
                },
                _ => {
                    escaped = false;
                }
            }
        }
        
        if depth > 0 && verbose {
            println!("Warning: Could not find end of VALUES section for INSERT statement {}", index + 1);
            // If we can't find the end, skip this match to prevent breaking the SQL
            continue;
        }
        
        // Extract the VALUES section
        let values_section = &result[*header_end_pos..end_pos];
        
        // Format the INSERT statement
        let formatted_insert = format_insert_statement(header, values_section, verbose);
        
        // Replace the original with the formatted version
        result.replace_range(start_pos..&end_pos, &formatted_insert);
    }
    
    result
}

fn format_insert_statement(header: &str, values_section: &str, verbose: bool) -> String {
    if verbose {
        println!("Formatting INSERT values section");
    }
    
    // The values section starts with an opening parenthesis and includes all content up to the final closing parenthesis
    // It might contain nested function calls like JSON_ARRAY() with their own parentheses
    
    // First, let's identify the rows by tokenizing while respecting nested structures
    let mut rows = Vec::new();
    let mut current_row = String::new();
    let mut depth = 0;
    let mut in_string = false;
    let mut string_delimiter = ' '; // Will be set to ' or " when a string starts
    let mut escaped = false;
    
    for c in values_section.chars() {
        match c {
            '\\' => {
                current_row.push(c);
                escaped = !escaped;
            },
            '\'' | '"' | '`' => {
                current_row.push(c);
                if !escaped {
                    if !in_string {
                        // Start of a string
                        in_string = true;
                        string_delimiter = c;
                    } else if c == string_delimiter {
                        // End of a string
                        in_string = false;
                    }
                }
                escaped = false;
            },
            '(' => {
                current_row.push(c);
                if !in_string {
                    depth += 1;
                }
                escaped = false;
            },
            ')' => {
                current_row.push(c);
                if !in_string {
                    depth -= 1;
                    // If we've closed a top-level row parenthesis
                    if depth == 0 {
                        // Don't add empty rows
                        if !current_row.trim().is_empty() {
                            rows.push(current_row.trim().to_string());
                            current_row = String::new();
                        }
                    }
                }
                escaped = false;
            },
            ',' => {
                if !in_string && depth == 1 {
                    // End of a column value at the top level
                    current_row.push(c);
                } else if !in_string && depth == 0 {
                    // This is a comma between rows - skip it
                } else {
                    // This is a comma inside a string or a nested structure
                    current_row.push(c);
                }
                escaped = false;
            },
            _ => {
                // Skip leading whitespace at depth 0
                if !(depth == 0 && c.is_whitespace()) {
                    current_row.push(c);
                }
                escaped = false;
            }
        }
    }
    
    // Add the last row if it's not empty
    if !current_row.trim().is_empty() {
        rows.push(current_row.trim().to_string());
    }
    
    if verbose {
        println!("Identified {} rows", rows.len());
    }
    
    // Now that we have the rows, extract the column values from each row
    let mut values_per_row = Vec::new();
    
    for (i, row) in rows.iter().enumerate() {
        if verbose {
            println!("Processing row {}: {}", i + 1, row);
        }
        
        // Strip the outer parentheses
        let row_content = row.trim_start_matches('(').trim_end_matches(')').trim();
        
        // Split the row into columns
        let mut columns = Vec::new();
        let mut current_column = String::new();
        let mut depth = 0;
        let mut in_string = false;
        let mut string_delimiter = ' '; // Will be set to ' or " when a string starts
        let mut escaped = false;
        
        for c in row_content.chars() {
            match c {
                '\\' => {
                    current_column.push(c);
                    escaped = !escaped;
                },
                '\'' | '"' | '`' => {
                    current_column.push(c);
                    if !escaped {
                        if !in_string {
                            // Start of a string
                            in_string = true;
                            string_delimiter = c;
                        } else if c == string_delimiter {
                            // End of a string
                            in_string = false;
                        }
                    }
                    escaped = false;
                },
                '(' => {
                    current_column.push(c);
                    if !in_string {
                        depth += 1;
                    }
                    escaped = false;
                },
                ')' => {
                    current_column.push(c);
                    if !in_string {
                        depth -= 1;
                    }
                    escaped = false;
                },
                ',' => {
                    if in_string || depth > 0 {
                        // This comma is part of a string or a nested function call
                        current_column.push(c);
                    } else {
                        // This comma separates columns
                        columns.push(current_column.trim().to_string());
                        current_column = String::new();
                    }
                    escaped = false;
                },
                _ => {
                    current_column.push(c);
                    escaped = false;
                }
            }
        }
        
        // Add the last column
        if !current_column.trim().is_empty() {
            columns.push(current_column.trim().to_string());
        }
        
        if verbose {
            println!("Row {} has {} columns", i + 1, columns.len());
        }
        
        values_per_row.push(columns);
    }
    
    // Check if all rows have the same number of columns
    let row_column_counts: Vec<usize> = values_per_row.iter().map(|row| row.len()).collect();
    let all_same_width = row_column_counts.iter().all(|&count| count == row_column_counts[0]);
    
    if !all_same_width && verbose {
        println!("Warning: Not all rows have the same number of columns. Column counts: {:?}", row_column_counts);
    }
    
    // Find the maximum width for each column
    let column_count = values_per_row.iter().map(|row| row.len()).max().unwrap_or(0);
    
    if verbose {
        println!("Maximum column count: {}", column_count);
    }
    
    // Calculate column widths
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
            let value_trimmed = value.trim();
            let is_numeric = value_trimmed.parse::<i64>().is_ok() || 
                            value_trimmed.parse::<f64>().is_ok() || 
                            value_trimmed == "NULL" ||
                            value_trimmed.to_uppercase() == "NULL";
                            
            if is_numeric {
                formatted_row.push_str(&format!("{:>width$}", value_trimmed, width=column_widths[i]));
            } else {
                // For non-numeric values, preserve their exact representation
                let padding = " ".repeat(column_widths[i].saturating_sub(value_trimmed.len()));
                formatted_row.push_str(value_trimmed);
                formatted_row.push_str(&padding);
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