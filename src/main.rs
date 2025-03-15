use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use glob::glob;
use regex::Regex;
use clap::Parser;

/// SQL formatter that ONLY aligns columns in INSERT statements
/// Does not modify ANY syntax, only adds spacing
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
    
    // Handle both shell-style paths and glob patterns
    let input_path = Path::new(&args.input);
    let mut processed_files = 0;
    
    if input_path.exists() && input_path.is_file() {
        // Direct file path
        if input_path.extension().map_or(false, |ext| ext.to_str().unwrap_or("") == "sql") {
            process_file(input_path, !args.print, args.force)?;
            processed_files += 1;
        }
    } else if input_path.exists() && input_path.is_dir() {
        // Directory path - process all .sql files in it
        for entry in fs::read_dir(input_path)? {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |ext| ext.to_str().unwrap_or("") == "sql") {
                    process_file(&path, !args.print, args.force)?;
                    processed_files += 1;
                }
            }
        }
    } else {
        // Treat as glob pattern
        let paths = glob(&args.input)?;
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
    }
    
    if processed_files == 0 {
        println!("No SQL files found matching pattern: {}", args.input);
    } else {
        println!("Processed {} SQL file(s)", processed_files);
    }
    
    Ok(())
}

fn process_file(path: &Path, write_to_file: bool, force: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("Processing file: {}", path.display());
    
    // Read file content
    let mut file = File::open(path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    
    // Format the content - ONLY aligning columns, no syntax changes
    let formatted_content = format_sql(&content);
    
    // Check if content actually changed
    if content == formatted_content && !force {
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

fn format_sql(sql: &str) -> String {
    // Process each INSERT statement with VALUES separately
    let mut result = String::new();
    let mut last_pos = 0;
    
    // Use a more specific regex to find INSERT statements
    let insert_pattern = Regex::new(r"(?im)^INSERT\s+INTO\s+\w+\s*\([^)]+\)\s*VALUES\s*").unwrap();
    
    for insert_match in insert_pattern.find_iter(sql) {
        // Add everything up to this INSERT statement
        result.push_str(&sql[last_pos..insert_match.start()]);
        
        // Extract and add the INSERT header
        let header_end = insert_match.end();
        result.push_str(&sql[insert_match.start()..header_end]);
        
        // Find all VALUES rows for this INSERT statement
        let mut value_block_start = header_end;
        let mut value_block_end = header_end;
        let mut in_value_block = true;
        let mut values_lines = Vec::new();
        let mut current_line = String::new();
        let mut line_start = value_block_start;
        
        // Scan through the next part of SQL to extract VALUES rows
        for (i, c) in sql[header_end..].chars().enumerate() {
            if !in_value_block {
                break;
            }
            
            current_line.push(c);
            
            if c == ';' || c == '\n' {
                // End of a line
                let trimmed = current_line.trim();
                
                if !trimmed.is_empty() {
                    // Skip the VALUES keyword if it appears on its own line
                    if !(trimmed.eq_ignore_ascii_case("values") || 
                         trimmed.eq_ignore_ascii_case("values (")) {
                        values_lines.push((line_start, header_end + i + 1, current_line.clone()));
                    }
                }
                
                if c == ';' {
                    // End of values block
                    value_block_end = header_end + i + 1;
                    in_value_block = false;
                }
                
                current_line.clear();
                line_start = header_end + i + 1;
            } else if i > 0 && i + header_end < sql.len() && 
                      !trimmed_starts_with(
                          &sql[header_end + i..].chars().take(20).collect::<String>(), 
                          "("
                      ) &&
                      (c == 'I' || c == 'i') && 
                      trimmed_starts_with(
                          &sql[header_end + i..].chars().take(20).collect::<String>(), 
                          "INSERT"
                      ) {
                // Detected start of next INSERT statement
                value_block_end = header_end + i;
                in_value_block = false;
                break;
            }
        }
        
        // If we reached the end of the string without finding a semicolon
        if in_value_block && value_block_end == header_end {
            value_block_end = sql.len();
            
            // Add any remaining content if we didn't add it already
            if !current_line.is_empty() {
                values_lines.push((line_start, value_block_end, current_line));
            }
        }
        
        // Format the values lines
        if !values_lines.is_empty() {
            let formatted_values = format_values_lines(&values_lines);
            result.push_str(&formatted_values);
        }
        
        last_pos = value_block_end;
        
        // Safety check to prevent infinite loop
        if value_block_end <= insert_match.start() {
            break;
        }
    }
    
    // Add any remaining SQL after the last INSERT
    if last_pos < sql.len() {
        result.push_str(&sql[last_pos..]);
    }
    
    // If no changes were made, return the original SQL
    if result.is_empty() {
        return sql.to_string();
    }
    
    result
}

// Helper function to check if a string starts with another string (ignoring whitespace)
fn trimmed_starts_with(s: &str, prefix: &str) -> bool {
    s.trim_start().to_ascii_lowercase().starts_with(&prefix.to_ascii_lowercase())
}

fn format_values_lines(lines: &[(usize, usize, String)]) -> String {
    // Parse and extract column values
    let mut all_columns = Vec::new();
    
    for (_, _, line) in lines {
        let trimmed = line.trim();
        
        // Skip lines that don't start with a value tuple
        if !trimmed.starts_with('(') {
            continue;
        }
        
        // Extract content between parentheses
        let mut in_parens = false;
        let mut depth = 0;
        let mut columns = Vec::new();
        let mut current = String::new();
        let mut in_string = false;
        
        for c in trimmed.chars() {
            if c == '\'' && !in_string {
                in_string = true;
                current.push(c);
            } else if c == '\'' && in_string {
                // Check for escaped quotes
                if current.ends_with('\\') {
                    current.push(c);
                } else {
                    in_string = false;
                    current.push(c);
                }
            } else if c == '(' && !in_string {
                depth += 1;
                if depth == 1 {
                    in_parens = true;
                } else {
                    current.push(c);
                }
            } else if c == ')' && !in_string {
                depth -= 1;
                if depth == 0 {
                    in_parens = false;
                    if !current.trim().is_empty() {
                        columns.push(current.trim().to_string());
                    }
                    current.clear();
                } else {
                    current.push(c);
                }
            } else if c == ',' && depth == 1 && !in_string {
                columns.push(current.trim().to_string());
                current.clear();
            } else if in_parens {
                current.push(c);
            }
        }
        
        if !columns.is_empty() {
            all_columns.push(columns);
        }
    }
    
    // Find max width for each column
    if all_columns.is_empty() {
        // If we couldn't parse column values, return original lines
        return lines.iter().map(|(_, _, line)| line.clone()).collect::<String>();
    }
    
    let max_cols = all_columns.iter().map(|cols| cols.len()).max().unwrap_or(0);
    let mut col_widths = vec![0; max_cols];
    
    for columns in &all_columns {
        for (i, col) in columns.iter().enumerate() {
            if i < max_cols {
                col_widths[i] = col_widths[i].max(col.len());
            }
        }
    }
    
    // Format each line
    let mut result = String::new();
    let mut col_idx = 0;
    
    for (_, _, line) in lines {
        let trimmed = line.trim();
        
        if trimmed.starts_with('(') && col_idx < all_columns.len() {
            // Format this value line with column alignment
            let columns = &all_columns[col_idx];
            
            // Calculate indentation by counting spaces at beginning of line
            let indent = line.chars().take_while(|&c| c.is_whitespace()).count();
            let indent_spaces = if indent > 0 { indent } else { 4 };
            
            result.push_str(&" ".repeat(indent_spaces));
            result.push('(');
            
            for (i, col) in columns.iter().enumerate() {
                if i > 0 {
                    result.push_str(", ");
                }
                
                result.push_str(col);
                
                // Add padding for all but the last column
                if i < columns.len() - 1 {
                    let padding = col_widths[i].saturating_sub(col.len());
                    result.push_str(&" ".repeat(padding));
                }
            }
            
            result.push(')');
            
            // Preserve any trailing comma or semicolon
            if trimmed.ends_with(',') {
                result.push(',');
            } else if trimmed.ends_with(';') {
                result.push(';');
            }
            
            result.push('\n');
            col_idx += 1;
        } else {
            // Keep the line as is
            result.push_str(line);
            if !line.ends_with('\n') {
                result.push('\n');
            }
        }
    }
    
    result
}