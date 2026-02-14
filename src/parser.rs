use kdl::{KdlDocument, KdlError};

pub fn parse(input: &str) -> Result<KdlDocument, String> {
    let normalized = pre_parse(input);
    normalized.parse().map_err(|e: KdlError| {
        // kdl-rs errors already have good Display impl with line/col
        format!("KDL parse error: {}", e)
    })
}

pub fn pre_parse(input: &str) -> String {
    // Pre-parse in a string-aware manner
    let mut result = String::with_capacity(input.len() * 2);
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        // Handle comments - pass through unchanged
        if c == '/' && i + 1 < chars.len() {
            if chars[i + 1] == '/' {
                // Line comment - pass through to end of line
                while i < chars.len() && chars[i] != '\n' {
                    result.push(chars[i]);
                    i += 1;
                }
                continue;
            } else if chars[i + 1] == '*' {
                // Block comment - pass through to */
                result.push(chars[i]);
                i += 1;
                result.push(chars[i]);
                i += 1;
                while i + 1 < chars.len() {
                    result.push(chars[i]);
                    if chars[i] == '*' && chars[i + 1] == '/' {
                        i += 1;
                        result.push(chars[i]);
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                continue;
            }
        }

        // Handle quoted strings - pass through unchanged (but handle backticks inside)
        if c == '"' {
            result.push(c);
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    // Escape sequence
                    result.push(chars[i]);
                    i += 1;
                    result.push(chars[i]);
                    i += 1;
                } else {
                    result.push(chars[i]);
                    i += 1;
                }
            }
            if i < chars.len() {
                result.push(chars[i]); // closing quote
                i += 1;
            }
            continue;
        }

        // Handle raw strings #"..."# - pass through unchanged
        if c == '#' && i + 1 < chars.len() && chars[i + 1] == '"' {
            result.push(c);
            i += 1;
            result.push(chars[i]); // opening quote
            i += 1;
            while i < chars.len() {
                if chars[i] == '"' && i + 1 < chars.len() && chars[i + 1] == '#' {
                    result.push(chars[i]);
                    i += 1;
                    result.push(chars[i]);
                    i += 1;
                    break;
                }
                result.push(chars[i]);
                i += 1;
            }
            continue;
        }

        // Handle standalone backtick expressions - wrap in raw strings
        if c == '`' {
            let start = i;
            i += 1;
            while i < chars.len() && chars[i] != '`' {
                i += 1;
            }
            if i < chars.len() {
                let expr: String = chars[start..=i].iter().collect();
                result.push_str(&format!("#\"{}\"#", expr));
                i += 1;
            } else {
                result.push('`');
            }
            continue;
        }

        // Handle #content special token
        if c == '#' && i + 8 <= chars.len() {
            let next8: String = chars[i..i+8].iter().collect();
            if next8 == "#content" {
                let prev = if i > 0 { Some(chars[i - 1]) } else { None };
                if prev.is_none() || prev == Some('\n') || prev == Some(' ') || prev == Some('\t') ||
                   prev == Some('{') || prev == Some(';') || prev == Some('}') {
                    result.push_str("__hudl_content");
                    i += 8;
                    continue;
                }
            }
        }

        // Handle identifiers, paths, selectors, and keywords
        if is_ident_start(c) || (c == '#' && i + 1 < chars.len() && is_ident_start(chars[i+1])) || (c == '.' && i + 1 < chars.len() && (is_ident_start(chars[i+1]) || chars[i+1] == '/')) {
            let prev = if i > 0 { Some(chars[i - 1]) } else { None };
            if prev.is_none() || prev == Some('\n') || prev == Some(' ') || prev == Some('\t') ||
               prev == Some('{') || prev == Some(';') || prev == Some('}') {
                
                // Collect the full identifier/path/selector chain
                let start = i;
                while i < chars.len() && is_selector_char(chars[i]) {
                    // Stop if we see ~>
                    if chars[i] == '~' && i + 1 < chars.len() && chars[i + 1] == '>' {
                        break;
                    }
                    i += 1;
                }
                let full_ident: String = chars[start..i].iter().collect();

                // Exclude KDL keywords and #content from quoting
                if full_ident == "#true" || full_ident == "#false" || full_ident == "#null" || full_ident == "#content" {
                    // Just push it, but let #content be transformed
                    if full_ident == "#content" {
                        result.push_str("__hudl_content");
                    } else {
                        result.push_str(&full_ident);
                    }
                    continue;
                }

                // Check for keywords
                let keywords = ["if", "else", "each", "switch", "case", "default", "import", "el", "css"];
                if keywords.contains(&full_ident.as_str()) {
                    result.push_str("__hudl_");
                    result.push_str(&full_ident);
                    continue;
                }

                // Quote if it's not a plain identifier
                // Plain identifier: only letters, numbers, _, - and doesn't start with digit/path
                let is_plain = !full_ident.contains('/') && 
                              !full_ident.contains('.') && 
                              !full_ident.contains('&') && 
                              !full_ident.contains('#') &&
                              !full_ident.contains(':') &&
                              is_ident_start(full_ident.chars().next().unwrap_or(' '));

                if !is_plain {
                    result.push('"');
                    result.push_str(&full_ident);
                    result.push('"');
                } else {
                    result.push_str(&full_ident);
                }
                continue;
            }
        }

        // Handle } else -> }\nelse for KDL compatibility
        if c == '}' {
            result.push(c);
            let mut k = i + 1;
            // Skip whitespace
            while k < chars.len() && (chars[k] == ' ' || chars[k] == '\t') {
                k += 1;
            }
            // Check for 'else'
            if k + 4 <= chars.len() {
                let next4: String = chars[k..k+4].iter().collect();
                if next4 == "else" && (k + 4 >= chars.len() || !is_ident_char(chars[k + 4])) {
                    result.push('\n');
                    i = k - 1;
                }
            }
            i += 1;
            continue;
        }

        // Handle tilde attribute: ~on:click="value" or ~on:click~once="value"
        if c == '~' {
            let prev = if i > 0 { Some(chars[i - 1]) } else { None };
            // Check if this is an inline tilde attribute (after whitespace)
            if prev == Some(' ') || prev == Some('\t') || prev == Some('\n') {
                let start = i;
                i += 1; // skip the ~
                while i < chars.len() && is_tilde_attr_char(chars[i]) {
                    i += 1;
                }
                let full_name: String = chars[start..i].iter().collect();
                
                let is_plain = is_ident_start(full_name.chars().nth(1).unwrap_or(' ')) && 
                              full_name[1..].chars().all(is_ident_char);

                if !is_plain {
                    result.push('"');
                    result.push_str(&full_name);
                    result.push('"');
                } else {
                    result.push_str(&full_name);
                }
                continue;
            }
            // Check if this is ~> binding shorthand (after element name)
            if i + 1 < chars.len() && chars[i + 1] == '>' {
                i += 2; // skip ~>
                // Collect signal name (up to next ~ or whitespace)
                let signal_start = i;
                while i < chars.len() && chars[i] != '~' && !chars[i].is_whitespace() {
                    i += 1;
                }
                let signal_name: String = chars[signal_start..i].iter().collect();
                // Collect optional modifiers (~debounce:300ms etc.)
                let mut modifiers = String::new();
                while i < chars.len() && chars[i] == '~' {
                    let mod_start = i;
                    i += 1; // skip ~
                    while i < chars.len() && is_tilde_attr_char(chars[i]) {
                        i += 1;
                    }
                    modifiers.push_str(&chars[mod_start..i].iter().collect::<String>());
                }
                result.push(' ');
                result.push_str(&format!("\"~bind{}\"=\"{}\"", modifiers, signal_name));
                continue;
            }
        }

        // Handle property values: key=value
        if c == '=' {
            let prev = if i > 0 { Some(chars[i - 1]) } else { None };
            if prev.map_or(false, |p| is_ident_char(p) || p == ':' || p == '.' || p == '~') {
                let mut k = i + 1;
                while k < chars.len() && chars[k].is_whitespace() {
                    k += 1;
                }
                if k < chars.len() && (chars[k] == '"' || chars[k] == '`') {
                    result.push('=');
                    i = k;
                    continue;
                }

                if i + 1 < chars.len() && is_ident_start(chars[i + 1]) {
                    result.push('=');
                    i += 1;
                    if chars[i] == '"' || (chars[i] == '#' && i + 1 < chars.len() && chars[i + 1] == '"') {
                        // Already handled
                    } else {
                        result.push('"');
                        while i < chars.len() && is_ident_char(chars[i]) {
                            result.push(chars[i]);
                            i += 1;
                        }
                        result.push('"');
                        continue;
                    }
                }
            }
        }

        // Handle numeric values with units
        if c.is_ascii_digit() {
            let prev = if i > 0 { Some(chars[i - 1]) } else { None };
            if prev.is_none() || prev == Some(' ') || prev == Some('\t') ||
               prev == Some('{') || prev == Some(';') || prev == Some('\n') || prev == Some('}') {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                if i < chars.len() && chars[i].is_ascii_alphabetic() {
                    result.push('_');
                    for c in &chars[start..i] {
                        result.push(*c);
                    }
                    while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '%') {
                        result.push(chars[i]);
                        i += 1;
                    }
                    continue;
                } else {
                    for c in &chars[start..i] {
                        result.push(*c);
                    }
                    continue;
                }
            }
        }

        result.push(c);
        i += 1;
    }

    result
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

fn is_tilde_attr_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '~' || c == ':' || c == '.'
}

fn is_selector_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '&' || c == ':' || c == '#' || c == '/' || c == '~'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backtick_wrapping() {
        let result = pre_parse("span `name`");
        assert!(result.contains("#\"`name`\"#"));
    }

    #[test]
    fn test_backtick_with_inner_quotes() {
        let result = pre_parse(r#"span `active ? "yes" : "no"`"#);
        assert!(result.contains("#\"`active ? \"yes\" : \"no\"`\"#"));
    }

    #[test]
    fn test_else_handling() {
        let result = pre_parse("} else {");
        assert!(result.contains("}\n__hudl_else"));
    }

    #[test]
    fn test_selector_shorthand() {
        let result = pre_parse("#main.container { }");
        assert!(result.contains("\"#main.container\""));
    }

    #[test]
    fn test_tilde_attr() {
        let result = pre_parse("button ~on:click=\"handler()\"");
        assert!(result.contains("\"~on:click\""));
    }

    #[test]
    fn test_binding_shorthand() {
        let result = pre_parse("input~>username");
        assert!(result.contains("\"~bind\"=\"username\""));
    }

    #[test]
    fn test_numeric_unit() {
        let result = pre_parse("div 10px");
        assert!(result.contains("_10px"));
    }
}
