// crates/bsc-classic/src/diagnostic.rs
//! Rustc-style Compiler Diagnostic Engine
//! Provides formatted source-snippet diagnostics with carets (^^^^), line gutters,
//! and contextual help notes.

pub struct DiagnosticRenderer;

impl DiagnosticRenderer {
    /// Parses raw error string, extracts line/column information, and formats a rustc-like diagnostic.
    pub fn render(source: &str, raw_err: &str, stage: &str) -> String {
        let (line_opt, col_opt, clean_msg) = Self::extract_line_col(raw_err);

        let (err_code, stage_label) = match stage {
            "Lexer" => ("E0101", "Lexical Analysis"),
            "Parser" => ("E0201", "Syntax Analysis"),
            "Semantic" => ("E0301", "Semantic & Type Check"),
            "VM" => ("E0701", "Runtime VM Execution"),
            _ => ("E0001", "Compiler Pipeline"),
        };

        if let (Some(line), Some(col)) = (line_opt, col_opt) {
            let lines: Vec<&str> = source.lines().collect();
            let source_line = if line >= 1 && line <= lines.len() {
                lines[line - 1]
            } else {
                ""
            };

            // Calculate caret width
            let caret_width = Self::calculate_caret_width(source_line, col);
            let indent = if col > 0 { col - 1 } else { 0 };
            let caret_str = "^".repeat(caret_width.max(1));

            let help_note = Self::suggest_help(&clean_msg);

            let mut out = String::new();
            out.push_str(&format!("error[{}]: {} Error\n", err_code, stage_label));
            out.push_str(&format!(" --> input.lang:{}:{}\n", line, col));
            out.push_str("  |\n");
            out.push_str(&format!("{:>2} | {}\n", line, source_line));
            out.push_str(&format!("  | {}{}{}\n", " ".repeat(indent), caret_str, Self::format_inline_hint(&clean_msg)));
            out.push_str("  |\n");

            if let Some(help) = help_note {
                out.push_str(&format!("  = help: {}\n", help));
            }

            out
        } else {
            // Fallback for errors without line:col
            format!("error[{}]: {} Error\n  = details: {}\n", err_code, stage_label, raw_err)
        }
    }

    fn extract_line_col(err: &str) -> (Option<usize>, Option<usize>, String) {
        // Search for pattern "at <line>:<col>"
        if let Some(pos) = err.rfind(" at ") {
            let prefix = &err[..pos];
            let suffix = &err[pos + 4..];
            let parts: Vec<&str> = suffix.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).collect();
            if parts.len() >= 2 {
                if let (Ok(l), Ok(c)) = (parts[0].parse::<usize>(), parts[1].parse::<usize>()) {
                    return (Some(l), Some(c), prefix.trim().to_string());
                }
            }
        }
        (None, None, err.to_string())
    }

    fn calculate_caret_width(line: &str, col: usize) -> usize {
        if col == 0 || col > line.len() {
            return 1;
        }
        let rest = &line[col - 1..];
        let mut width = 0;
        let mut chars = rest.chars();
        if let Some(first) = chars.next() {
            width += 1;
            if first.is_alphanumeric() || first == '_' {
                for c in chars {
                    if c.is_alphanumeric() || c == '_' {
                        width += 1;
                    } else {
                        break;
                    }
                }
            }
        }
        width.max(1)
    }

    fn format_inline_hint(msg: &str) -> String {
        if msg.contains("Expected type") {
            " expected valid primitive type".to_string()
        } else if msg.contains("Expected ';'") {
            " expected ';' after statement".to_string()
        } else if msg.contains("Unexpected character") {
            " unexpected character".to_string()
        } else if msg.contains("Undeclared variable") || msg.contains("Undefined") {
            " not found in current scope".to_string()
        } else if msg.contains("Type mismatch") {
            " type mismatch occurred here".to_string()
        } else {
            format!(" {}", msg)
        }
    }

    fn suggest_help(msg: &str) -> Option<String> {
        if msg.contains("Expected type") {
            Some("classic language primitive types are `int`, `bool`, and `void`.".to_string())
        } else if msg.contains("Expected ';'") {
            Some("statements in classic language must terminate with a semicolon `;`.".to_string())
        } else if msg.contains("Expected 'fn'") {
            Some("functions are declared with the syntax: `fn <name>(<params>) -> <type> { ... }`.".to_string())
        } else if msg.contains("Type mismatch") {
            Some("ensure variable initializer or return value type strictly matches its declaration.".to_string())
        } else if msg.contains("Undeclared variable") || msg.contains("Undefined") {
            Some("declare variable before referencing it: `let <name>: <type> = <value>;`.".to_string())
        } else if msg.contains("Condition of") {
            Some("conditional expressions in `if` and `while` must evaluate to a boolean (`bool`).".to_string())
        } else if msg.contains("Unexpected character") {
            Some("remove unsupported symbols and verify UTF-8 encoding.".to_string())
        } else {
            None
        }
    }
}
