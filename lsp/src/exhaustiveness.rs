//! Switch exhaustiveness checking for Hudl templates.
//!
//! Checks that switch statements on enum types cover all values,
//! or have a default case.

use std::collections::HashSet;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

/// Information about a switch statement found in the document
#[derive(Debug)]
pub struct SwitchInfo {
    pub expr: String,
    pub cases: Vec<String>,
    pub has_default: bool,
    pub line: u32,
    pub character: u32,
}

/// Enum definition extracted from proto block
#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: String,
    pub values: Vec<String>,
}

/// Check a switch statement for exhaustiveness against an enum.
///
/// Returns a diagnostic if the switch is non-exhaustive (missing enum values).
pub fn check_switch_exhaustiveness(
    switch_info: &SwitchInfo,
    enum_def: Option<&EnumDef>,
) -> Option<Diagnostic> {
    // If we don't know the enum, skip checking
    let enum_def = enum_def?;

    // If there's a default, it's always exhaustive
    if switch_info.has_default {
        return None;
    }

    // Check which enum values are covered
    let covered: HashSet<&str> = switch_info
        .cases
        .iter()
        .map(|c| c.trim())
        .collect();

    let missing: Vec<&String> = enum_def
        .values
        .iter()
        .filter(|v| !covered.contains(v.as_str()))
        .collect();

    if !missing.is_empty() {
        return Some(Diagnostic {
            range: Range {
                start: Position {
                    line: switch_info.line,
                    character: switch_info.character,
                },
                end: Position {
                    line: switch_info.line,
                    character: switch_info.character + 6, // "switch"
                },
            },
            severity: Some(DiagnosticSeverity::WARNING),
            message: format!(
                "Non-exhaustive switch: missing cases for {}. Add these cases or a default clause.",
                missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
            ),
            ..Default::default()
        });
    }

    None
}

/// Extract switch statements from document content.
pub fn extract_switches(content: &str) -> Vec<SwitchInfo> {
    let mut switches = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for (line_num, line) in lines.iter().enumerate() {
        // Find switch keyword (with backtick CEL expression)
        if let Some(pos) = line.find("switch ") {
            // Check it's not in a comment
            let before = &line[..pos];
            if before.contains("//") {
                continue;
            }

            // Extract the expression (between switch and {)
            let after = &line[pos + 7..]; // after "switch "
            let expr = if let Some(brace) = after.find('{') {
                after[..brace].trim().trim_matches('`').to_string()
            } else {
                after.trim().trim_matches('`').to_string()
            };

            // Find cases and default by scanning following lines
            let (cases, has_default) = extract_cases(&lines, line_num + 1);

            switches.push(SwitchInfo {
                expr,
                cases,
                has_default,
                line: line_num as u32,
                character: pos as u32,
            });
        }
    }

    switches
}

/// Extract case patterns from lines following a switch.
fn extract_cases(lines: &[&str], start_line: usize) -> (Vec<String>, bool) {
    let mut cases = Vec::new();
    let mut has_default = false;
    let mut brace_depth = 1;

    for line in lines.iter().skip(start_line) {
        // Track brace depth
        for c in line.chars() {
            match c {
                '{' => brace_depth += 1,
                '}' => brace_depth -= 1,
                _ => {}
            }
        }

        // Exit when we close the switch block
        if brace_depth <= 0 {
            break;
        }

        // Look for case (bare identifier for enum values)
        if let Some(pos) = line.find("case ") {
            let before = &line[..pos];
            if !before.contains("//") {
                let after = &line[pos + 5..];
                // Extract the pattern (until { or end of line)
                // Handle both bare identifiers and backtick expressions
                let pattern = if let Some(brace) = after.find('{') {
                    after[..brace].trim().trim_matches('`').to_string()
                } else {
                    after.trim().trim_matches('`').to_string()
                };
                cases.push(pattern);
            }
        }

        // Look for default
        if line.contains("default") && !line.contains("//") {
            has_default = true;
        }
    }

    (cases, has_default)
}

/// Extract enum definitions from proto blocks in the content.
pub fn extract_enums(content: &str) -> Vec<EnumDef> {
    let mut enums = Vec::new();

    // Simple regex-based extraction
    // In production, use a proper proto parser
    let enum_re = regex::Regex::new(r"enum\s+(\w+)\s*\{([^}]+)\}").unwrap();
    let value_re = regex::Regex::new(r"(\w+)\s*=\s*\d+").unwrap();

    for caps in enum_re.captures_iter(content) {
        let name = caps[1].to_string();
        let body = &caps[2];

        let values: Vec<String> = value_re
            .captures_iter(body)
            .map(|c| c[1].to_string())
            .collect();

        enums.push(EnumDef { name, values });
    }

    enums
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_switches_cel_syntax() {
        let content = r#"
switch `status` {
    case STATUS_ACTIVE {
        span "Active"
    }
    case STATUS_PENDING {
        span "Pending"
    }
}
"#;
        let switches = extract_switches(content);
        assert_eq!(switches.len(), 1);
        assert_eq!(switches[0].expr, "status");
        assert_eq!(switches[0].cases.len(), 2);
        assert_eq!(switches[0].cases[0], "STATUS_ACTIVE");
        assert_eq!(switches[0].cases[1], "STATUS_PENDING");
        assert!(!switches[0].has_default);
    }

    #[test]
    fn test_extract_switches_with_default() {
        let content = r#"
switch `status` {
    case STATUS_ACTIVE {
        span "Active"
    }
    default {
        span "Unknown"
    }
}
"#;
        let switches = extract_switches(content);
        assert_eq!(switches.len(), 1);
        assert!(switches[0].has_default);
    }

    #[test]
    fn test_extract_enums() {
        let content = r#"
/**
enum Status {
    STATUS_UNKNOWN = 0;
    STATUS_ACTIVE = 1;
    STATUS_PENDING = 2;
    STATUS_FAILED = 3;
}
*/
"#;
        let enums = extract_enums(content);
        assert_eq!(enums.len(), 1);
        assert_eq!(enums[0].name, "Status");
        assert_eq!(enums[0].values.len(), 4);
        assert!(enums[0].values.contains(&"STATUS_ACTIVE".to_string()));
        assert!(enums[0].values.contains(&"STATUS_PENDING".to_string()));
    }

    #[test]
    fn test_exhaustiveness_check_missing_cases() {
        let switch_info = SwitchInfo {
            expr: "status".to_string(),
            cases: vec!["STATUS_ACTIVE".to_string()],
            has_default: false,
            line: 5,
            character: 0,
        };

        let enum_def = EnumDef {
            name: "Status".to_string(),
            values: vec![
                "STATUS_UNKNOWN".to_string(),
                "STATUS_ACTIVE".to_string(),
                "STATUS_PENDING".to_string(),
            ],
        };

        let diagnostic = check_switch_exhaustiveness(&switch_info, Some(&enum_def));
        assert!(diagnostic.is_some());
        let diag = diagnostic.unwrap();
        assert!(diag.message.contains("STATUS_UNKNOWN"));
        assert!(diag.message.contains("STATUS_PENDING"));
    }

    #[test]
    fn test_exhaustiveness_check_with_default() {
        let switch_info = SwitchInfo {
            expr: "status".to_string(),
            cases: vec!["STATUS_ACTIVE".to_string()],
            has_default: true,
            line: 5,
            character: 0,
        };

        let enum_def = EnumDef {
            name: "Status".to_string(),
            values: vec![
                "STATUS_ACTIVE".to_string(),
                "STATUS_PENDING".to_string(),
            ],
        };

        let diagnostic = check_switch_exhaustiveness(&switch_info, Some(&enum_def));
        assert!(diagnostic.is_none()); // Default covers missing cases
    }

    #[test]
    fn test_exhaustiveness_check_complete() {
        let switch_info = SwitchInfo {
            expr: "status".to_string(),
            cases: vec![
                "STATUS_ACTIVE".to_string(),
                "STATUS_PENDING".to_string(),
            ],
            has_default: false,
            line: 5,
            character: 0,
        };

        let enum_def = EnumDef {
            name: "Status".to_string(),
            values: vec![
                "STATUS_ACTIVE".to_string(),
                "STATUS_PENDING".to_string(),
            ],
        };

        let diagnostic = check_switch_exhaustiveness(&switch_info, Some(&enum_def));
        assert!(diagnostic.is_none()); // All cases covered
    }
}
