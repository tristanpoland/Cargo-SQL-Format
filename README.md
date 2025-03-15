# sql-fmt

A SQL formatter that aligns columns in INSERT statements for perfect readability.

## Features

- Perfectly aligns columns in INSERT statements for better readability
- Right-aligns numbers and left-aligns text
- Maintains SQL syntax highlighting in editors
- Simple command-line interface
- Integrates with `cargo fmt`
- Cross-platform (Windows, macOS, Linux)

## Example

This tool transforms messy SQL:

```sql
INSERT INTO routes (domain_id, host, path, app_id, weight, https_only, created_at)
VALUES (1, 'api', '', 1, 100, 1, '2022-05-20 10:00:00'), (1, 'app', '', 2, 100, 1, '2022-05-20 12:15:00'), (1, 'auth', '', 3, 100, 1, '2022-05-20 14:45:00'), (2, NULL, '/api/v1', 1, 100, 1, '2022-05-21 09:30:00'), (2, NULL, '/auth', 3, 100, 1, '2022-05-21 11:15:00');
```

Into beautifully formatted and aligned SQL:

```sql
INSERT INTO routes (domain_id, host, path, app_id, weight, https_only, created_at)
VALUES
(1, 'api'        , ''       ,  1, 100, 1, '2022-05-20 10:00:00'),
(1, 'app'        , ''       ,  2, 100, 1, '2022-05-20 12:15:00'),
(1, 'auth'       , ''       ,  3, 100, 1, '2022-05-20 14:45:00'),
(2, NULL         , '/api/v1',  1, 100, 1, '2022-05-21 09:30:00'),
(2, NULL         , '/auth'  ,  3, 100, 1, '2022-05-21 11:15:00'),
(3, 'api-staging', ''       ,  5, 100, 1, '2022-05-22 10:00:00'),
(3, 'app-staging', ''       ,  6, 100, 1, '2022-05-22 12:15:00'),
(4, 'api-dev'    , ''       ,  7, 100, 0, '2022-05-23 09:30:00'),
(4, 'app-dev'    , ''       ,  8, 100, 0, '2022-05-23 11:45:00'),
(5, 'data'       , ''       ,  9, 100, 1, '2022-06-25 11:15:00'),
(5, 'ml'         , ''       , 10, 100, 1, '2022-06-25 15:00:00'),
(5, 'api'        , ''       , 11, 100, 1, '2022-06-26 09:45:00'),
(6, 'data-staging', ''      , 12, 100, 1, '2022-06-26 12:30:00'),
(6, 'api-staging', ''       , 13, 100, 1, '2022-06-26 15:45:00'),
(7, 'code'       , ''       , 16, 100, 1, '2022-07-30 10:15:00');
```

## Installation

### From Source

1. Clone the repository:
   ```
   git clone https://github.com/tristanpoland/Cargo-SQL-Format.git
   cd sql-fmt
   ```

2. Build the project:
   ```
   cargo build --release
   ```

3. Add to your PATH:
   ```
   # On Unix/Linux/macOS
   cp target/release/sql-fmt ~/.local/bin/

   # On Windows (PowerShell)
   Copy-Item .\target\release\sql-fmt.exe -Destination ~\bin\
   ```

### With Cargo

```
cargo install sql-fmt
```

## Usage

### Basic Usage

Format a specific SQL file:

```
sql-fmt path/to/your/file.sql
```

Format all SQL files in the current directory (recursively):

```
sql-fmt --all
```

Enable verbose output for debugging:

```
sql-fmt -v path/to/your/file.sql
```

### Integration with Cargo

To integrate with `cargo fmt`, add the following to your `.cargo/config.toml` file:

```toml
[alias]
fmt = "fmt -- && run --package sql-fmt"
```

Now when you run `cargo fmt`, it will run both Rust formatting and SQL formatting.

## How It Works

The formatter:

1. Parses SQL files to locate INSERT statements
2. Splits the VALUES section into rows and columns
3. Calculates the optimal width for each column
4. Right-aligns numeric values and left-aligns text values
5. Formats each value with perfect grid alignment
6. Writes the updated SQL back to the file

## License

This project is licensed under the MIT License - see the LICENSE file for details.