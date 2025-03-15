use regex::Regex;
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process;

// Global verbose flag
static mut VERBOSE: bool = false;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    
    // Set verbose mode
    let verbose = args.iter().any(|arg| arg == "-v" || arg == "--verbose");
    unsafe { VERBOSE = verbose; }
    
    // Log startup info in verbose mode
    log_verbose(&format!("Starting SQL formatter with args: {:?}", args));
    
    // Handle arguments
    let mut files_to_format = Vec::new();
    let mut is_sql_fmt_command = false;
    
    for (i, arg) in args.iter().enumerate() {
        if i == 0 {
            // Skip the program name
            continue;
        }
        
        if arg == "sql-fmt" {
            is_sql_fmt_command = true;
            continue;
        }
        
        if arg == "-v" || arg == "--verbose" {
            // Already handled
            continue;
        }
        
        if !arg.starts_with("-") {
            files_to_format.push(arg.clone());
        }
    }
    
    log_verbose(&format!("Files to format: {:?}", files_to_format));
    
    if is_sql_fmt_command && files_to_format.is_empty() || 
       (!is_sql_fmt_command && files_to_format.is_empty() && args.len() > 1) {
        // Format all SQL files
        log_verbose("No specific files provided, formatting all SQL files");
        format_all_sql_files()?;
    } else if !files_to_format.is_empty() {
        // Format specific files
        for file in files_to_format {
            log_verbose(&format!("Formatting file: {}", file));
            if let Err(e) = format_file(&file) {
                eprintln!("Error formatting {}: {}", file, e);
            }
        }
    } else {
        print_usage();
    }
    
    Ok(())
}

fn print_usage() {
    eprintln!("SQL Formatter - Format SQL files with proper alignment");
    eprintln!("");
    eprintln!("Usage:");
    eprintln!("  cargo-sql-fmt [files...]           Format specific SQL files");
    eprintln!("  cargo-sql-fmt                      Format all SQL files in current directory");
    eprintln!("  cargo sql-fmt [files...]           Same as above, when used as cargo subcommand");
    eprintln!("");
    eprintln!("Options:");
    eprintln!("  -v, --verbose                      Enable verbose logging");
}

fn log_verbose(message: &str) {
    unsafe {
        if VERBOSE {
            eprintln!("[SQL-FMT] {}", message);
        }
    }
}

fn format_all_sql_files() -> io::Result<()> {
    let current_dir = env::current_dir()?;
    log_verbose(&format!("Searching for SQL files in: {}", current_dir.display()));
    
    // Use a more reliable approach to find all SQL files
    let mut sql_files = Vec::new();
    walk_directory(&current_dir, &mut sql_files)?;
    
    log_verbose(&format!("Found {} SQL files", sql_files.len()));
    
    // Format each SQL file
    for file in sql_files {
        log_verbose(&format!("Formatting: {}", file.display()));
        if let Err(e) = format_file(&file.to_string_lossy()) {
            eprintln!("Error formatting {}: {}", file.display(), e);
        }
    }
    
    Ok(())
}

fn walk_directory(dir: &Path, sql_files: &mut Vec<std::path::PathBuf>) -> io::Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            // Skip target and .git directories
            if path.is_dir() {
                let dir_name = path.file_name().unwrap().to_string_lossy();
                if dir_name != "target" && dir_name != ".git" {
                    walk_directory(&path, sql_files)?;
                }
            } else if let Some(ext) = path.extension() {
                if ext == "sql" {
                    sql_files.push(path);
                }
            }
        }
    }
    
    Ok(())
}

fn format_file(path: &str) -> io::Result<()> {
    log_verbose(&format!("Reading file content: {}", path));
    
    // Read file content
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            log_verbose(&format!("Failed to read file: {}", e));
            return Err(e);
        }
    };
    
    // Get original content for comparison
    let original_content = content.clone();
    
    // Format SQL with detailed error handling
    log_verbose("Starting SQL formatting...");
    let mut formatted = content.clone();
    
    // Try to format different SQL statement types
    // If any formatting function fails, we'll log it but continue with others
    formatted = format_with_error_handling(format_sql_inserts, &formatted, "INSERT statements");
    formatted = format_with_error_handling(format_sql_creates, &formatted, "CREATE TABLE statements");
    formatted = format_with_error_handling(format_sql_selects, &formatted, "SELECT statements");
    formatted = format_with_error_handling(format_sql_updates, &formatted, "UPDATE statements");
    formatted = format_with_error_handling(format_sql_deletes, &formatted, "DELETE statements");
    
    // Check if content changed
    if original_content != formatted {
        log_verbose("Content was modified, writing changes to file");
        fs::write(path, formatted)?;
        println!("Formatted: {}", path);
    } else {
        log_verbose("No changes needed for this file");
    }
    
    Ok(())
}

fn format_with_error_handling<F>(formatter: F, content: &str, description: &str) -> String 
where F: Fn(&str) -> String + std::panic::UnwindSafe {
    match std::panic::catch_unwind(move || formatter(content)) {
        Ok(result) => result,
        Err(_) => {
            log_verbose(&format!("Error while formatting {}, skipping", description));
            content.to_string()
        }
    }
}

// Format SQL INSERTs with aligned column grid
fn format_sql_inserts(sql: &str) -> String {
    log_verbose("Formatting INSERT statements");
    
    // Find all INSERT statements
    let insert_regex = match Regex::new(r"(?is)(INSERT\s+INTO\s+\w+(?:\.\w+)?\s*\([^)]+\))\s*VALUES\s*\n?\s*\(([^;]+)(?:;|$)") {
        Ok(re) => re,
        Err(e) => {
            log_verbose(&format!("Regex error: {}", e));
            return sql.to_string();
        }
    };
    
    let mut result = String::from(sql);
    let mut offset = 0;
    
    for captures in insert_regex.captures_iter(sql) {
        log_verbose("Found an INSERT statement to format");
        
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
    log_verbose(&format!("Formatting INSERT statement with header: {}", header));
    
    // Split values into rows by finding closing and opening parentheses patterns
    let row_regex = match Regex::new(r"\)\s*,\s*\(") {
        Ok(re) => re,
        Err(e) => {
            log_verbose(&format!("Row regex error: {}", e));
            return format!("{}\nVALUES\n({})", header, values_section);
        }
    };
    
    let rows_text = if values_section.contains("),") {
        format!("({})", values_section)
    } else {
        format!("({}", values_section)
    };
    
    let mut rows: Vec<&str> = row_regex.split(&rows_text).collect();
    log_verbose(&format!("Split into {} rows", rows.len()));
    
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
    
    for (row_idx, row) in rows.iter().enumerate() {
        log_verbose(&format!("Processing row {}: {}", row_idx + 1, row));
        
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
        
        log_verbose(&format!("Row {} split into {} values", row_idx + 1, values.len()));
        values_per_row.push(values);
    }
    
    // Find the maximum width for each column
    let column_count = values_per_row.iter().map(|row| row.len()).max().unwrap_or(0);
    log_verbose(&format!("Found {} columns", column_count));
    
    let mut column_widths = vec![0; column_count];
    
    for row in &values_per_row {
        for (i, value) in row.iter().enumerate() {
            if i < column_widths.len() {
                column_widths[i] = column_widths[i].max(value.len());
            }
        }
    }
    
    log_verbose(&format!("Column widths: {:?}", column_widths));
    
    // Format the rows with proper alignment
    let mut formatted_rows = Vec::new();
    
    for (row_idx, row) in values_per_row.iter().enumerate() {
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
        
        log_verbose(&format!("Formatted row {}: {}", row_idx + 1, formatted_rows.last().unwrap()));
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
    
    log_verbose("INSERT statement formatting complete");
    result
}

// Format CREATE TABLE statements with aligned columns
fn format_sql_creates(sql: &str) -> String {
    log_verbose("Formatting CREATE TABLE statements");
    
    // Find all CREATE statements
    let create_regex = match Regex::new(r"(?is)(CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?[^\s(]+\s*\()([^;]+)(\);)") {
        Ok(re) => re,
        Err(e) => {
            log_verbose(&format!("CREATE regex error: {}", e));
            return sql.to_string();
        }
    };
    
    let mut result = String::from(sql);
    let mut offset = 0;
    
    for captures in create_regex.captures_iter(sql) {
        log_verbose("Found a CREATE TABLE statement to format");
        
        let full_match = captures.get(0).unwrap();
        let start_pos = full_match.start();
        let end_pos = full_match.end();
        let match_len = end_pos - start_pos;
        
        let header = captures.get(1).unwrap().as_str();
        let columns_section = captures.get(2).unwrap().as_str();
        let footer = captures.get(3).unwrap().as_str();
        
        // Format the CREATE TABLE statement
        let formatted_create = format_create_statement(header, columns_section, footer);
        
        // Replace the original with the formatted version
        result.replace_range(
            (start_pos + offset)..(end_pos + offset),
            &formatted_create
        );
        
        // Adjust offset for future replacements
        offset += formatted_create.len() - match_len;
    }
    
    result
}

fn format_create_statement(header: &str, columns_section: &str, footer: &str) -> String {
    log_verbose("Formatting CREATE statement columns");
    
    // Split the column definitions
    let col_lines: Vec<&str> = columns_section.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    log_verbose(&format!("Found {} column definitions", col_lines.len()));
    
    // Parse column definitions to find name, type, and constraints
    let mut column_parts: Vec<Vec<String>> = Vec::new();
    
    for line in col_lines {
        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        if parts.len() >= 2 {
            let name = parts[0].trim();
            
            // Find the type and constraints
            let rest = parts[1].trim();
            let type_regex = match Regex::new(r"^([A-Za-z0-9_]+(?:\s*\([^)]+\))?)(.*)$") {
                Ok(re) => re,
                Err(e) => {
                    log_verbose(&format!("Type regex error: {}", e));
                    column_parts.push(vec![line.to_string(), "".to_string(), "".to_string()]);
                    continue;
                }
            };
            
            if let Some(type_caps) = type_regex.captures(rest) {
                let col_type = type_caps.get(1).unwrap().as_str().trim();
                let constraints = type_caps.get(2).map_or("", |m| m.as_str().trim());
                
                column_parts.push(vec![
                    name.to_string(),
                    col_type.to_string(),
                    constraints.to_string()
                ]);
            } else {
                // If not parsed correctly, just use the raw parts
                column_parts.push(vec![
                    name.to_string(),
                    rest.to_string(),
                    "".to_string()
                ]);
            }
        } else {
            // Special cases like PRIMARY KEY constraints
            column_parts.push(vec![
                line.to_string(),
                "".to_string(),
                "".to_string()
            ]);
        }
    }
    
    // Find the maximum width for each column part
    let mut name_width = 0;
    let mut type_width = 0;
    
    for parts in &column_parts {
        if !parts[0].starts_with("PRIMARY KEY") && !parts[0].starts_with("FOREIGN KEY") {
            name_width = name_width.max(parts[0].len());
            type_width = type_width.max(parts[1].len());
        }
    }
    
    log_verbose(&format!("Column name width: {}, type width: {}", name_width, type_width));
    
    // Format the column definitions with proper alignment
    let mut formatted_columns = Vec::new();
    
    for (i, parts) in column_parts.iter().enumerate() {
        if parts[0].starts_with("PRIMARY KEY") || parts[0].starts_with("FOREIGN KEY") || parts[0].starts_with("CONSTRAINT") {
            // Special case for constraints
            formatted_columns.push(format!("  {}", parts[0]));
        } else {
            // Regular column definition
            let formatted_line = format!(
                "  {:<name_width$} {:<type_width$} {}",
                parts[0],
                parts[1],
                parts[2],
                name_width=name_width,
                type_width=type_width
            ).trim_end().to_string();
            
            formatted_columns.push(formatted_line);
        }
        
        log_verbose(&format!("Formatted column {}: {}", i + 1, formatted_columns.last().unwrap()));
    }
    
    // Combine everything with proper layout
    format!(
        "{}\n{}\n{}",
        header,
        formatted_columns.join(",\n"),
        footer
    )
}

// Format SELECT statements with proper indentation and alignment
fn format_sql_selects(sql: &str) -> String {
    log_verbose("Formatting SELECT statements");
    
    let select_regex = match Regex::new(r"(?is)(SELECT\s+)(.+?)(\s+FROM\s+)(.+?)(?:(?:\s+WHERE\s+)(.+?))?(?:(?:\s+GROUP\s+BY\s+)(.+?))?(?:(?:\s+HAVING\s+)(.+?))?(?:(?:\s+ORDER\s+BY\s+)(.+?))?(?:(?:\s+LIMIT\s+)(\d+))?(?:(?:\s+OFFSET\s+)(\d+))?(\s*;|\s*$)") {
        Ok(re) => re,
        Err(e) => {
            log_verbose(&format!("SELECT regex error: {}", e));
            return sql.to_string();
        }
    };
    
    let mut result = String::from(sql);
    let mut offset = 0;
    
    for captures in select_regex.captures_iter(sql) {
        log_verbose("Found a SELECT statement to format");
        
        let full_match = captures.get(0).unwrap();
        let start_pos = full_match.start();
        let end_pos = full_match.end();
        let match_len = end_pos - start_pos;
        
        // Extract all parts of the SELECT statement
        let select_keyword = captures.get(1).unwrap().as_str();
        let columns = captures.get(2).unwrap().as_str();
        let from_keyword = captures.get(3).unwrap().as_str();
        let tables = captures.get(4).unwrap().as_str();
        
        let where_clause = captures.get(5).map_or("", |m| m.as_str());
        let group_by = captures.get(6).map_or("", |m| m.as_str());
        let having = captures.get(7).map_or("", |m| m.as_str());
        let order_by = captures.get(8).map_or("", |m| m.as_str());
        let limit = captures.get(9).map_or("", |m| m.as_str());
        let offset_val = captures.get(10).map_or("", |m| m.as_str());
        let terminator = captures.get(11).unwrap().as_str();
        
        // Format the SELECT statement
        let formatted_select = format_select_statement(
            select_keyword, columns, from_keyword, tables,
            where_clause, group_by, having, order_by,
            limit, offset_val, terminator
        );
        
        // Replace the original with the formatted version
        result.replace_range(
            (start_pos + offset)..(end_pos + offset),
            &formatted_select
        );
        
        // Adjust offset for future replacements
        offset += formatted_select.len() - match_len;
    }
    
    result
}

fn format_select_statement(
    select_keyword: &str,
    columns: &str,
    from_keyword: &str,
    tables: &str,
    where_clause: &str,
    group_by: &str,
    having: &str,
    order_by: &str,
    limit: &str,
    offset_val: &str,
    terminator: &str
) -> String {
    // Format column list with proper alignment
    log_verbose("Formatting SELECT statement columns");
    
    let column_list = columns.split(',')
        .map(|s| s.trim())
        .collect::<Vec<&str>>()
        .join(",\n       ");
    
    // Format table list
    let table_list = tables.split(',')
        .map(|s| s.trim())
        .collect::<Vec<&str>>()
        .join(",\n     ");
    
    // Start building the formatted statement
    let mut formatted = format!("{}{}",
        select_keyword,
        if column_list.contains('\n') {
            format!("\n       {}", column_list)
        } else {
            column_list
        }
    );
    
    // Add FROM clause
    formatted.push_str(&format!("{}{}", 
        from_keyword,
        if table_list.contains('\n') {
            format!("\n     {}", table_list)
        } else {
            table_list
        }
    ));
    
    // Add WHERE clause if present
    if !where_clause.is_empty() {
        formatted.push_str(&format!("\nWHERE {}", where_clause));
    }
    
    // Add GROUP BY clause if present
    if !group_by.is_empty() {
        formatted.push_str(&format!("\nGROUP BY {}", group_by));
    }
    
    // Add HAVING clause if present
    if !having.is_empty() {
        formatted.push_str(&format!("\nHAVING {}", having));
    }
    
    // Add ORDER BY clause if present
    if !order_by.is_empty() {
        formatted.push_str(&format!("\nORDER BY {}", order_by));
    }
    
    // Add LIMIT clause if present
    if !limit.is_empty() {
        formatted.push_str(&format!("\nLIMIT {}", limit));
    }
    
    // Add OFFSET clause if present
    if !offset_val.is_empty() {
        formatted.push_str(&format!("\nOFFSET {}", offset_val));
    }
    
    // Add terminator
    formatted.push_str(terminator);
    
    log_verbose("SELECT statement formatting complete");
    formatted
}

// Format UPDATE statements
fn format_sql_updates(sql: &str) -> String {
    log_verbose("Formatting UPDATE statements");
    
    let update_regex = match Regex::new(r"(?is)(UPDATE\s+)(.+?)(\s+SET\s+)(.+?)(?:(?:\s+WHERE\s+)(.+?))?(\s*;|\s*$)") {
        Ok(re) => re,
        Err(e) => {
            log_verbose(&format!("UPDATE regex error: {}", e));
            return sql.to_string();
        }
    };
    
    let mut result = String::from(sql);
    let mut offset = 0;
    
    for captures in update_regex.captures_iter(sql) {
        log_verbose("Found an UPDATE statement to format");
        
        let full_match = captures.get(0).unwrap();
        let start_pos = full_match.start();
        let end_pos = full_match.end();
        let match_len = end_pos - start_pos;
        
        // Extract all parts of the UPDATE statement
        let update_keyword = captures.get(1).unwrap().as_str();
        let table = captures.get(2).unwrap().as_str();
        let set_keyword = captures.get(3).unwrap().as_str();
        let set_clauses = captures.get(4).unwrap().as_str();
        let where_clause = captures.get(5).map_or("", |m| m.as_str());
        let terminator = captures.get(6).unwrap().as_str();
        
        // Format the UPDATE statement
        let formatted_update = format_update_statement(
            update_keyword, table, set_keyword, set_clauses, where_clause, terminator
        );
        
        // Replace the original with the formatted version
        result.replace_range(
            (start_pos + offset)..(end_pos + offset),
            &formatted_update
        );
        
        // Adjust offset for future replacements
        offset += formatted_update.len() - match_len;
    }
    
    result
}

fn format_update_statement(
    update_keyword: &str,
    table: &str,
    set_keyword: &str,
    set_clauses: &str,
    where_clause: &str,
    terminator: &str
) -> String {
    // Format SET clauses with proper alignment
    log_verbose("Formatting UPDATE statement SET clauses");
    
    let set_list = set_clauses.split(',')
        .map(|s| s.trim())
        .collect::<Vec<&str>>();
    
    // Find the longest column name for alignment
    let mut max_col_len = 0;
    for clause in &set_list {
        if let Some(equals_pos) = clause.find('=') {
            max_col_len = max_col_len.max(equals_pos);
        }
    }
    
    log_verbose(&format!("Maximum column name length: {}", max_col_len));
    
    // Format SET clauses with aligned equals signs
    let mut formatted_set_clauses = Vec::new();
    for (i, clause) in set_list.iter().enumerate() {
        if let Some(equals_pos) = clause.find('=') {
            let (col, val) = clause.split_at(equals_pos);
            formatted_set_clauses.push(format!("{:<width$}{}", col, val, width=max_col_len));
        } else {
            formatted_set_clauses.push(clause.to_string());
        }
        
        log_verbose(&format!("Formatted SET clause {}: {}", i + 1, formatted_set_clauses.last().unwrap()));
    }
    
    // Build the formatted statement
    let mut formatted = format!("{}{}{}", update_keyword, table, set_keyword);
    
    if formatted_set_clauses.len() > 1 {
        formatted.push_str(&format!("\n  {}", formatted_set_clauses.join(",\n  ")));
    } else if !formatted_set_clauses.is_empty() {
        formatted.push_str(&formatted_set_clauses[0]);
    }
    
    // Add WHERE clause if present
    if !where_clause.is_empty() {
        formatted.push_str(&format!("\nWHERE {}", where_clause));
    }
    
    // Add terminator
    formatted.push_str(terminator);
    
    log_verbose("UPDATE statement formatting complete");
    formatted
}

// Format DELETE statements
fn format_sql_deletes(sql: &str) -> String {
    log_verbose("Formatting DELETE statements");
    
    let delete_regex = match Regex::new(r"(?is)(DELETE\s+FROM\s+)(.+?)(?:(?:\s+WHERE\s+)(.+?))?(\s*;|\s*$)") {
        Ok(re) => re,
        Err(e) => {
            log_verbose(&format!("DELETE regex error: {}", e));
            return sql.to_string();
        }
    };
    
    let mut result = String::from(sql);
    let mut offset = 0;
    
    for captures in delete_regex.captures_iter(sql) {
        log_verbose("Found a DELETE statement to format");
        
        let full_match = captures.get(0).unwrap();
        let start_pos = full_match.start();
        let end_pos = full_match.end();
        let match_len = end_pos - start_pos;
        
        // Extract all parts of the DELETE statement
        let delete_from = captures.get(1).unwrap().as_str();
        let table = captures.get(2).unwrap().as_str();
        let where_clause = captures.get(3).map_or("", |m| m.as_str());
        let terminator = captures.get(4).unwrap().as_str();
        
        // Format the DELETE statement
        let formatted_delete = format!(
            "{}{}{}{}{}",
            delete_from,
            table,
            if !where_clause.is_empty() { "\nWHERE " } else { "" },
            where_clause,
            terminator
        );
        
        // Replace the original with the formatted version
        result.replace_range(
            (start_pos + offset)..(end_pos + offset),
            &formatted_delete
        );
        
        // Adjust offset for future replacements
        offset += formatted_delete.len() - match_len;
    }
    
    result
}

// Check if SQL is valid before attempting to format
fn is_valid_sql(sql: &str) -> bool {
    // Simple validation - check for basic SQL syntax
    // This doesn't guarantee SQL is valid, but catches obvious problems
    let has_sql_keywords = sql.to_lowercase().contains("select") || 
                          sql.to_lowercase().contains("insert") ||
                          sql.to_lowercase().contains("update") ||
                          sql.to_lowercase().contains("delete") ||
                          sql.to_lowercase().contains("create") ||
                          sql.to_lowercase().contains("alter");
    
    if !has_sql_keywords {
        log_verbose("Warning: File doesn't contain common SQL keywords");
    }
    
    // Count parentheses to check for basic balance
    let open_parens = sql.chars().filter(|&c| c == '(').count();
    let close_parens = sql.chars().filter(|&c| c == ')').count();
    
    if open_parens != close_parens {
        log_verbose(&format!("Warning: Unbalanced parentheses ({} open, {} close)", 
                          open_parens, close_parens));
        return false;
    }
    
    // Count quotes to check for matching quotes
    let quote_count = sql.chars().filter(|&c| c == '\'').count();
    if quote_count % 2 != 0 {
        log_verbose(&format!("Warning: Odd number of quotes ({})", quote_count));
        return false;
    }
    
    true
}

// Diagnostic function to print SQL structure
fn print_sql_structure(sql: &str) {
    if !unsafe { VERBOSE } {
        return;
    }
    
    log_verbose("SQL Structure Analysis:");
    log_verbose(&format!("Length: {} characters", sql.len()));
    
    // Look for statement types
    for stmt_type in &["SELECT", "INSERT", "UPDATE", "DELETE", "CREATE", "ALTER"] {
        let count = sql.to_uppercase().matches(stmt_type).count();
        log_verbose(&format!("  {} statements: {}", stmt_type, count));
    }
    
    // Check for balance
    let open_parens = sql.chars().filter(|&c| c == '(').count();
    let close_parens = sql.chars().filter(|&c| c == ')').count();
    log_verbose(&format!("  Parentheses: {} open, {} close", open_parens, close_parens));
    
    // Look for semicolons
    let semicolons = sql.chars().filter(|&c| c == ';').count();
    log_verbose(&format!("  Semicolons: {}", semicolons));
    
    // Look for VALUES keywords (relevant for INSERT statements)
    let values = sql.to_uppercase().matches("VALUES").count();
    log_verbose(&format!("  VALUES keywords: {}", values));
}