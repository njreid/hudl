use kdl::{KdlDocument, KdlError};
use regex::Regex;

pub fn parse(input: &str) -> Result<KdlDocument, KdlError> {
    let normalized = pre_parse(input);
    // println!("--- NORMALIZED ---\n{}\n------------------", normalized);
    normalized.parse()
}

fn pre_parse(input: &str) -> String {
    let mut result = input.to_string();

    // Wrap backtick expressions in raw strings to preserve inner quotes
    // e.g., `user.name == "admin"` becomes #"`user.name == "admin"`"#
    result = wrap_backtick_expressions(&result);

    let digit_regex = Regex::new(r"(\s|[{;]|^)([0-9]+[a-zA-Z%]+)").unwrap();
    result = digit_regex.replace_all(&result, "${1}_${2}").to_string();

    let else_regex = Regex::new(r"\}\s*else").unwrap();
    result = else_regex.replace_all(&result, "}\nelse").to_string();

    let prop_regex = Regex::new(r"(\w+)=([a-zA-Z_\-][a-zA-Z0-9_\-]*)").unwrap();
    result = prop_regex.replace_all(&result, "$1=\"$2\"").to_string();

    let selector_regex = Regex::new(r"(^|[\s{};])([&.][\w.&-]*|[a-zA-Z_\-][\w\-]*[\.][\w.&-]*)").unwrap();
    result = selector_regex.replace_all(&result, "$1\"$2\"").to_string();

    let at_rule_regex = Regex::new(r"(^|[\s{};])(@[a-zA-Z_\-]+)").unwrap();
    result = at_rule_regex.replace_all(&result, "$1\"$2\"").to_string();

    result
}

/// Wrap standalone backtick expressions in KDL raw strings.
/// Handles expressions that are NOT already inside a quoted string.
fn wrap_backtick_expressions(input: &str) -> String {
    let mut result = String::with_capacity(input.len() * 2);
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    let mut in_string = false;
    let mut string_char = '"';

    while i < chars.len() {
        let c = chars[i];

        // Track string state (but not raw strings for simplicity)
        if !in_string && (c == '"' || c == '\'') {
            in_string = true;
            string_char = c;
            result.push(c);
            i += 1;
            continue;
        }

        if in_string && c == string_char && (i == 0 || chars[i - 1] != '\\') {
            in_string = false;
            result.push(c);
            i += 1;
            continue;
        }

        // Handle backtick expressions outside of strings
        if !in_string && c == '`' {
            // Find matching closing backtick
            let start = i;
            i += 1;
            while i < chars.len() && chars[i] != '`' {
                i += 1;
            }
            if i < chars.len() {
                // Found closing backtick - wrap in raw string
                let expr: String = chars[start..=i].iter().collect();
                result.push_str(&format!("#\"{}\"#", expr));
                i += 1;
            } else {
                // No closing backtick - just push the opening one
                result.push('`');
            }
            continue;
        }

        result.push(c);
        i += 1;
    }

    result
}
