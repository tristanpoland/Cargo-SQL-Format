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
    // Split the SQL by INSERT statements to process each separately
    let insert_pattern = Regex::new(r"(?i)INSERT\s+INTO\s+[^\(]+\([^\)]+\)\s*VALUES\s*\n").unwrap();
    let mut result = String::new();
    let mut last_end = 0;
    
    for insert_match in insert_pattern.find_iter(sql) {
        // Add everything before this INSERT statement
        result.push_str(&sql[last_end..insert_match.start()]);
        
        // Get the INSERT statement header (everything before VALUES)
        let insert_header = &sql[insert_match.start()..insert_match.end()];
        result.push_str(insert_header);
        
        // Find the end of this INSERT statement's values
        let values_start = insert_match.end();
        let mut values_end = values_start;
        let mut paren_count = 0;
        let mut in_string = false;
        let mut values_lines = Vec::new();
        let mut current_line = String::new();
        
        // Extract value lines
        for (i, c) in sql[values_start..].char_indices() {
            if c == '\'' && (i == 0 || &sql[values_start + i - 1..values_start + i] != "\\") {
                in_string = !in_string;
            }
            
            if !in_string {
                if c == '(' {
                    paren_count += 1;
                } else if c == ')' {
                    paren_count -= 1;
                }
            }
            
            current_line.push(c);
            
            if c == '\n' || (c == ';' && paren_count == 0) {
                if !current_line.trim().is_empty() {
                    values_lines.push(current_line);
                    current_line = String::new();
                }
                
                if c == ';' && paren_count == 0 {
                    values_end = values_start + i + 1;
                    break;
                }
            }
        }
        
        // If we didn't find a semicolon, include the rest of the content
        if values_end == values_start {
            values_end = sql.len();
            // Add any remaining content
            if !current_line.is_empty() {
                values_lines.push(current_line);
            }
        }
        
        // Format value lines (ONLY align columns, don't change syntax)
        if !values_lines.is_empty() {
            let formatted_values = format_values(&values_lines);
            result.push_str(&formatted_values);
        }
        
        last_end = values_end;
    }
    
    // Add any remaining SQL after the last INSERT
    if last_end < sql.len() {
        result.push_str(&sql[last_end..]);
    }
    
    // If no INSERT statements were found, return the original SQL
    if result.is_empty() {
        return sql.to_string();
    }
    
    result
}

fn format_values(lines: &[String]) -> String {
    // Find columns in each line by splitting between parentheses
    let mut all_columns = Vec::new();
    
    for line in lines {
        let trimmed = line.trim();
        
        // Skip if not a value line
        if !trimmed.starts_with('(') {
            continue;
        }
        
        // Extract content between parentheses
        let open_paren = trimmed.find('(').unwrap_or(0);
        let close_paren = trimmed.rfind(')').unwrap_or(trimmed.len());
        
        if open_paren < close_paren {
            let content = &trimmed[open_paren + 1..close_paren];
            let mut columns = Vec::new();
            let mut current = String::new();
            let mut in_string = false;
            let mut paren_level = 0;
            
            // Split by commas, but respect strings and nested parentheses
            for c in content.chars() {
                if c == '\'' && (current.is_empty() || current.chars().last().unwrap() != '\\') {
                    in_string = !in_string;
                }
                
                if !in_string {
                    if c == '(' {
                        paren_level += 1;
                    } else if c == ')' {
                        paren_level -= 1;
                    }
                }
                
                if c == ',' && !in_string && paren_level == 0 {
                    columns.push(current.trim().to_string());
                    current = String::new();
                } else {
                    current.push(c);
                }
            }
            
            if !current.trim().is_empty() {
                columns.push(current.trim().to_string());
            }
            
            all_columns.push(columns);
        }
    }
    
    // Find max width for each column
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
    let mut line_idx = 0;
    
    for line in lines {
        let trimmed = line.trim();
        
        if trimmed.starts_with('(') && line_idx < all_columns.len() {
            // Format value line
            let columns = &all_columns[line_idx];
            let indent = if line.starts_with(' ') {
                line.chars().take_while(|&c| c == ' ').count()
            } else {
                4 // Default indent
            };
            
            result.push_str(&" ".repeat(indent));
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
            line_idx += 1;
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