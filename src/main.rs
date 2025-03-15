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
        .about("Formats SQL files with conservative column alignment")
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
        .arg(
            Arg::with_name("backup")
                .short("b")
                .long("backup")
                .help("Create backup files before formatting")
        )
        .get_matches();

    let verbose = matches.is_present("verbose");
    let dry_run = matches.is_present("dry-run");
    let backup = matches.is_present("backup");
    
    if verbose {
        println!("Verbose mode enabled");
    }
    
    if dry_run {
        println!("Dry run mode enabled - no files will be modified");
    }
    
    if backup {
        println!("Backup mode enabled - creating .bak files before formatting");
    }
    
    // Determine which files to format
    if matches.is_present("all") {
        if verbose {
            println!("Finding all SQL files in current directory");
        }
        
        format_all_sql_files(verbose, dry_run, backup)?;
    } else if let Some(files) = matches.values_of("files") {
        let files: Vec<&str> = files.collect();
        
        if verbose {
            println!("Formatting specified files: {:?}", files);
        }
        
        for file in files {
            if let Err(e) = format_file(file, verbose, dry_run, backup) {
                eprintln!("Error formatting {}: {}", file, e);
            }
        }
    } else {
        // Default to reading from stdin and writing to stdout if no files specified
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        
        let formatted = format_sql(&input, verbose);
        io::stdout().write_all(formatted.as_bytes())?;
    }
    
    Ok(())
}

fn format_all_sql_files(verbose: bool, dry_run: bool, backup: bool) -> io::Result<()> {
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
        if let Err(e) = format_file(&file.to_string_lossy(), verbose, dry_run, backup) {
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

fn format_file(path: &str, verbose: bool, dry_run: bool, backup: bool) -> io::Result<()> {
    if verbose {
        println!("Reading file: {}", path);
    }
    
    let content = fs::read_to_string(path)?;
    let original_content = content.clone();
    
    // Create backup if requested
    if backup && !dry_run {
        let backup_path = format!("{}.bak", path);
        if verbose {
            println!("Creating backup: {}", backup_path);
        }
        fs::write(&backup_path, &content)?;
    }
    
    // Format SQL
    let formatted = format_sql(&content, verbose);
    
    // Check if content changed
    if original_content != formatted {
        if verbose {
            println!("Content changed");
            
            if dry_run {
                println!("Dry run - not writing changes to file");
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

// The main formatting function that dispatches to specific formatters
// Column type enum for better formatting decisions
#[derive(Debug, Clone, Copy, PartialEq)]
enum ColumnType {
    String,
    Number,
    Null,
    Function,
    Unknown
}

fn format_sql(sql: &str, verbose: bool) -> String {
    if verbose {
        println!("Formatting SQL");
    }
    
    // Find INSERT statements first
    let mut result = String::from(sql);
    
    // Only attempt to format INSERTs with VALUES
    result = format_inserts(&result, verbose);
    
    result
}

// Format INSERT statements specifically
fn format_inserts(sql: &str, verbose: bool) -> String {
    if verbose {
        println!("Looking for INSERT statements with VALUES");
    }
    
    // Find all INSERT statements with an improved regex
    let insert_regex = Regex::new(r#"(?is)(INSERT\s+INTO\s+[\w\.`\[\]"]+\s*\([^)]+\))\s*VALUES\s*"#).unwrap();
    
    let mut result = String::from(sql);
    
    // Process all INSERT statements
    let mut matches = Vec::new();
    for capture in insert_regex.captures_iter(sql) {
        let full_match = capture.get(0).unwrap();
        let insert_header = capture.get(1).unwrap().as_str();
        let start_pos = full_match.start();
        let end_header_pos = full_match.end();
        
        // Find the end of the VALUES clause
        // This needs to handle multiple rows and potential comments
        if let Some(values_section) = find_values_section(&result[end_header_pos..], verbose) {
            matches.push((
                insert_header.to_string(),
                start_pos,
                end_header_pos,
                values_section
            ));
        }
    }
    
    // Process matches in reverse order to maintain correct string offsets
    for (i, (header, start_pos, end_header_pos, values_section)) in matches.into_iter().enumerate().rev() {
        if verbose {
            println!("Formatting INSERT statement {}", i + 1);
        }
        
        let new_sql = try_format_insert_values(&header, &values_section, verbose);
        
        // Replace only if formatting was successful
        if !new_sql.is_empty() {
            // Check if the values section ends with a comma (multi-statement)
            let is_multi_statement = values_section.trim_end().ends_with(',');
            // If this is a multi-statement insert, keep the trailing comma
            let final_sql = if is_multi_statement && !new_sql.ends_with(',') {
                format!("{},", new_sql.trim_end_matches(';'))
            } else {
                new_sql
            };
            
            result.replace_range(
                start_pos..(end_header_pos + values_section.len()),
                &final_sql
            );
        }
    }
    
    result
}

// Find the VALUES section in the SQL string
fn find_values_section(sql_fragment: &str, verbose: bool) -> Option<String> {
    let mut in_string = false;
    let mut current_string_delimiter = '\0';
    let mut paren_depth = 0;
    let mut escaped = false;
    let mut in_comment = false;
    let mut in_multi_comment = false;
    
    let mut values_section = String::new();
    let chars: Vec<char> = sql_fragment.chars().collect();
    
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        
        // Handle comments
        if !in_string {
            // Check for comment start
            if !in_comment && !in_multi_comment && i < chars.len() - 1 && c == '-' && chars[i + 1] == '-' {
                in_comment = true;
                values_section.push(c);
                values_section.push(chars[i + 1]);
                i += 2;
                continue;
            }
            
            // Check for multi-line comment start
            if !in_comment && !in_multi_comment && i < chars.len() - 1 && c == '/' && chars[i + 1] == '*' {
                in_multi_comment = true;
                values_section.push(c);
                values_section.push(chars[i + 1]);
                i += 2;
                continue;
            }
            
            // End of single line comment
            if in_comment && c == '\n' {
                in_comment = false;
                values_section.push(c);
                i += 1;
                continue;
            }
            
            // End of multi-line comment
            if in_multi_comment && i < chars.len() - 1 && c == '*' && chars[i + 1] == '/' {
                in_multi_comment = false;
                values_section.push(c);
                values_section.push(chars[i + 1]);
                i += 2;
                continue;
            }
        }
        
        // Skip processing if in a comment
        if in_comment || in_multi_comment {
            values_section.push(c);
            i += 1;
            continue;
        }
        
        match c {
            '\\' => {
                escaped = !escaped;
            },
            '\'' | '"' | '`' => {
                if !escaped {
                    if !in_string {
                        in_string = true;
                        current_string_delimiter = c;
                    } else if c == current_string_delimiter {
                        in_string = false;
                    }
                }
                escaped = false;
            },
            '(' => {
                if !in_string {
                    paren_depth += 1;
                }
                escaped = false;
            },
            ')' => {
                if !in_string {
                    paren_depth -= 1;
                    
                    // Detect the end of VALUES if we've closed all parentheses
                    // and we're followed by a semicolon or end of input
                    if paren_depth <= 0 {
                        // Include the closing parenthesis
                        values_section.push(c);
                        i += 1;
                        
                        // Look for a semicolon or another INSERT statement
                        while i < chars.len() {
                            let next_char = chars[i];
                            if next_char == ';' {
                                values_section.push(next_char);
                                i += 1;
                                break;
                            } else if !next_char.is_whitespace() {
                                // If not a semicolon and not whitespace, we stop
                                break;
                            }
                            values_section.push(next_char);
                            i += 1;
                        }
                        
                        return Some(values_section);
                    }
                }
                escaped = false;
            },
            _ => {
                escaped = false;
            }
        }
        
        values_section.push(c);
        i += 1;
    }
    
    // If we reached end of input without finding proper closing
    if verbose && paren_depth > 0 {
        println!("Warning: Unclosed parentheses in VALUES section");
    }
    
    // Return what we have if we found anything with at least one row
    if !values_section.is_empty() && paren_depth <= 0 {
        Some(values_section)
    } else {
        None
    }
}

// Try to format the values of an INSERT statement
// This is deliberately conservative to avoid corrupting data
fn try_format_insert_values(header: &str, values_section: &str, verbose: bool) -> String {
    if verbose {
        println!("Attempting to format INSERT VALUES section");
    }
    
    // Step 1: Try to parse the values section into rows
    let rows = parse_insert_rows(values_section, verbose);
    
    if rows.is_empty() {
        if verbose {
            println!("Warning: Could not parse any rows from VALUES section");
        }
        return String::new(); // Return empty string to indicate formatting should be skipped
    }
    
    // Step 2: Check if all rows have the same number of columns
    // This is a safety check - if not, we should avoid formatting
    let first_row_cols = rows[0].len();
    let all_same_cols = rows.iter().all(|row| row.len() == first_row_cols);
    
    if !all_same_cols {
        if verbose {
            println!("Warning: Not all rows have the same number of columns. Skipping formatting.");
        }
        return String::new();
    }
    
    // Step 3: Analyze column types across all rows
    let mut col_types = vec![ColumnType::Unknown; first_row_cols]; 
    
    for row in &rows {
        for (i, col) in row.iter().enumerate() {
            if i < col_types.len() {
                let col_str = col.trim();
                
                // Determine column type based on content
                if col_str.starts_with('\'') && col_str.ends_with('\'') {
                    // String literal
                    col_types[i] = ColumnType::String;
                } else if col_str.parse::<f64>().is_ok() || col_str.parse::<i64>().is_ok() {
                    // Numeric value
                    col_types[i] = ColumnType::Number;
                } else if col_str.to_uppercase() == "NULL" {
                    // NULL value
                    col_types[i] = ColumnType::Null;
                } else if col_str.contains('(') && col_str.contains(')') {
                    // Function call or expression
                    col_types[i] = ColumnType::Function;
                }
            }
        }
    }
    
    // Step 4: Find the max length of each column across all rows
    let mut col_widths = vec![0; first_row_cols];
    
    for row in &rows {
        for (i, col) in row.iter().enumerate() {
            if i < col_widths.len() {
                col_widths[i] = col_widths[i].max(col.len());
            }
        }
    }
    
    // Step 5: Format each row with consistent spacing
    let mut formatted_rows = Vec::new();
    
    for row in rows {
        let mut formatted_cols = Vec::new();
        
        for (i, col) in row.iter().enumerate() {
            let col_str = col.trim();
            
            match col_types[i] {
                ColumnType::Function => {
                    // Don't pad function calls or complex expressions - preserve exactly
                    formatted_cols.push(col_str.to_string());
                },
                ColumnType::Number | ColumnType::Null => {
                    // Right-align numbers and NULLs
                    formatted_cols.push(format!("{:>width$}", col_str, width = col_widths[i]));
                },
                _ => {
                    // Left-align everything else (strings, etc.)
                    formatted_cols.push(format!("{:<width$}", col_str, width = col_widths[i]));
                }
            }
        }
        
        formatted_rows.push(format!("    ({})", formatted_cols.join(", ")));
    }
    
    // Combine everything with proper layout
    let mut result = String::new();
    result.push_str(header);
    result.push_str("\nVALUES\n");
    result.push_str(&formatted_rows.join(",\n"));
    
    // Ensure correct SQL termination
    if !result.trim().ends_with(';') {
        result.push(';');
    }
    
    result
}

// Parse the rows of an INSERT VALUES section
fn parse_insert_rows(values_section: &str, verbose: bool) -> Vec<Vec<String>> {
    if verbose {
        println!("Parsing INSERT rows");
    }
    
    let mut rows = Vec::new();
    let mut paren_depth = 0;
    let mut in_string = false;
    let mut current_string_delimiter = '\0';
    let mut escaped = false;
    let mut in_comment = false;
    let mut in_multi_comment = false;
    let mut current_row = Vec::new();
    let mut current_column = String::new();
    
    let chars: Vec<char> = values_section.chars().collect();
    let mut i = 0;
    
    // Skip any initial whitespace or comments before the first row
    while i < chars.len() {
        let c = chars[i];
        
        // Check for start of a row
        if c == '(' {
            break;
        }
        
        // Check for comment start
        if i < chars.len() - 1 && c == '-' && chars[i + 1] == '-' {
            in_comment = true;
            i += 2;
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            if i < chars.len() {
                i += 1; // Skip the newline
            }
            in_comment = false;
            continue;
        }
        
        // Check for multi-line comment start
        if i < chars.len() - 1 && c == '/' && chars[i + 1] == '*' {
            in_multi_comment = true;
            i += 2;
            while i < chars.len() - 1 && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            if i < chars.len() - 1 {
                i += 2; // Skip the */
            }
            in_multi_comment = false;
            continue;
        }
        
        i += 1;
    }
    
    while i < chars.len() {
        let c = chars[i];
        
        // Handle comments
        if !in_string {
            // Check for comment start
            if !in_comment && !in_multi_comment && i < chars.len() - 1 && c == '-' && chars[i + 1] == '-' {
                in_comment = true;
                i += 2;
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                if i < chars.len() {
                    i += 1; // Skip the newline
                }
                in_comment = false;
                continue;
            }
            
            // Check for multi-line comment start
            if !in_comment && !in_multi_comment && i < chars.len() - 1 && c == '/' && chars[i + 1] == '*' {
                in_multi_comment = true;
                i += 2;
                while i < chars.len() - 1 && !(chars[i] == '*' && chars[i + 1] == '/') {
                    i += 1;
                }
                if i < chars.len() - 1 {
                    i += 2; // Skip the */
                }
                in_multi_comment = false;
                continue;
            }
        }
        
        match c {
            '\\' => {
                current_column.push(c);
                escaped = !escaped;
            },
            '\'' | '"' | '`' => {
                current_column.push(c);
                if !escaped {
                    if !in_string {
                        in_string = true;
                        current_string_delimiter = c;
                    } else if c == current_string_delimiter {
                        in_string = false;
                    }
                }
                escaped = false;
            },
            '(' => {
                if !in_string {
                    paren_depth += 1;
                    if paren_depth == 1 {
                        // Start of a new row
                        current_row = Vec::new();
                        current_column = String::new();
                        i += 1;
                        continue;
                    }
                }
                current_column.push(c);
                escaped = false;
            },
            ')' => {
                if !in_string {
                    paren_depth -= 1;
                    if paren_depth == 0 {
                        // End of a row
                        if !current_column.trim().is_empty() {
                            current_row.push(current_column.trim().to_string());
                        }
                        rows.push(current_row.clone());
                        current_row = Vec::new();
                        current_column = String::new();
                        i += 1;
                        continue;
                    }
                }
                current_column.push(c);
                escaped = false;
            },
            ',' => {
                if !in_string && paren_depth == 1 {
                    // Column separator
                    current_row.push(current_column.trim().to_string());
                    current_column = String::new();
                } else {
                    current_column.push(c);
                }
                escaped = false;
            },
            ';' => {
                if !in_string && paren_depth == 0 {
                    // End of statement
                    break;
                } else {
                    current_column.push(c);
                }
                escaped = false;
            },
            _ => {
                current_column.push(c);
                escaped = false;
            }
        }
        
        i += 1;
    }
    
    // Add the last row if not already added
    if !current_row.is_empty() && current_row.iter().any(|s| !s.trim().is_empty()) {
        if !current_column.trim().is_empty() {
            current_row.push(current_column.trim().to_string());
        }
        rows.push(current_row);
    }
    
    if verbose {
        println!("Found {} rows", rows.len());
    }
    
    rows
}