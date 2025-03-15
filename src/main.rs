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
    let mut formatted = content.clone();
    
    // Format different SQL statement types
    formatted = format_sql_inserts(&formatted);
    formatted = format_sql_creates(&formatted);
    formatted = format_sql_selects(&formatted);
    formatted = format_sql_updates(&formatted);
    formatted = format_sql_deletes(&formatted);
    formatted = format_sql_alters(&formatted);
    formatted = align_sql_commas(&formatted);
    
    if content != formatted {
        fs::write(path, formatted)?;
        println!("Formatted: {}", path);
    }
    
    Ok(())
}

// Format SQL INSERTs with aligned column grid
fn format_sql_inserts(sql: &str) -> String {
    // Find all INSERT statements
    let insert_regex = Regex::new(r"(?is)(INSERT\s+INTO\s+\w+(?:\.\w+)?\s*\([^)]+\))\s*VALUES\s*\n?\s*\(([^;]+)(?:;|$)").unwrap();
    
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

// Format CREATE TABLE statements with aligned columns
fn format_sql_creates(sql: &str) -> String {
    let create_regex = Regex::new(r"(?is)(CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?[^\s(]+\s*\()([^;]+)(\);)").unwrap();
    
    let mut result = String::from(sql);
    let mut offset = 0;
    
    for captures in create_regex.captures_iter(sql) {
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
    // Split the column definitions
    let col_lines: Vec<&str> = columns_section.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    
    // Parse column definitions to find name, type, and constraints
    let mut column_parts: Vec<Vec<String>> = Vec::new();
    
    for line in col_lines {
        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        if parts.len() >= 2 {
            let name = parts[0].trim();
            
            // Find the type and constraints
            let rest = parts[1].trim();
            let type_regex = Regex::new(r"^([A-Za-z0-9_]+(?:\s*\([^)]+\))?)(.*)$").unwrap();
            
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
    
    // Format the column definitions with proper alignment
    let mut formatted_columns = Vec::new();
    
    for parts in column_parts {
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
    let select_regex = Regex::new(r"(?is)(SELECT\s+)(.+?)(\s+FROM\s+)(.+?)(?:(?:\s+WHERE\s+)(.+?))?(?:(?:\s+GROUP\s+BY\s+)(.+?))?(?:(?:\s+HAVING\s+)(.+?))?(?:(?:\s+ORDER\s+BY\s+)(.+?))?(?:(?:\s+LIMIT\s+)(\d+))?(?:(?:\s+OFFSET\s+)(\d+))?(\s*;|\s*$)").unwrap();
    
    let mut result = String::from(sql);
    let mut offset = 0;
    
    for captures in select_regex.captures_iter(sql) {
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
    
    formatted
}

// Format UPDATE statements
fn format_sql_updates(sql: &str) -> String {
    let update_regex = Regex::new(r"(?is)(UPDATE\s+)(.+?)(\s+SET\s+)(.+?)(?:(?:\s+WHERE\s+)(.+?))?(\s*;|\s*$)").unwrap();
    
    let mut result = String::from(sql);
    let mut offset = 0;
    
    for captures in update_regex.captures_iter(sql) {
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
    
    // Format SET clauses with aligned equals signs
    let mut formatted_set_clauses = Vec::new();
    for clause in set_list {
        if let Some(equals_pos) = clause.find('=') {
            let (col, val) = clause.split_at(equals_pos);
            formatted_set_clauses.push(format!("{:<width$}{}", col, val, width=max_col_len));
        } else {
            formatted_set_clauses.push(clause.to_string());
        }
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
    
    formatted
}

// Format DELETE statements
fn format_sql_deletes(sql: &str) -> String {
    let delete_regex = Regex::new(r"(?is)(DELETE\s+FROM\s+)(.+?)(?:(?:\s+WHERE\s+)(.+?))?(\s*;|\s*$)").unwrap();
    
    let mut result = String::from(sql);
    let mut offset = 0;
    
    for captures in delete_regex.captures_iter(sql) {
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

// Format ALTER TABLE statements
fn format_sql_alters(sql: &str) -> String {
    let alter_regex = Regex::new(r"(?is)(ALTER\s+TABLE\s+)(.+?)(\s+)(.+?)(\s*;|\s*$)").unwrap();
    
    let mut result = String::from(sql);
    let mut offset = 0;
    
    for captures in alter_regex.captures_iter(sql) {
        let full_match = captures.get(0).unwrap();
        let start_pos = full_match.start();
        let end_pos = full_match.end();
        let match_len = end_pos - start_pos;
        
        // Extract all parts of the ALTER statement
        let alter_table = captures.get(1).unwrap().as_str();
        let table = captures.get(2).unwrap().as_str();
        let spacing = captures.get(3).unwrap().as_str();
        let action = captures.get(4).unwrap().as_str();
        let terminator = captures.get(5).unwrap().as_str();
        
        // Format the ALTER statement
        let formatted_alter = format!("{}{}{}{}{}", alter_table, table, spacing, action, terminator);
        
        // Replace the original with the formatted version
        result.replace_range(
            (start_pos + offset)..(end_pos + offset),
            &formatted_alter
        );
        
        // Adjust offset for future replacements
        offset += formatted_alter.len() - match_len;
    }
    
    result
}

// Align commas in lists to improve readability
fn align_sql_commas(sql: &str) -> String {
    // Comma at the end of the line - improve readability
    let comma_regex = Regex::new(r"(\S+)(\s*),(\s*)").unwrap();
    sql.to_string().replace(", ", ",\n  ")
}