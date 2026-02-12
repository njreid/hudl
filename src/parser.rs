use kdl::{KdlDocument, KdlError};

pub fn parse(input: &str) -> Result<KdlDocument, String> {
    let normalized = pre_parse(input);
    if std::env::var("HUDL_DEBUG").is_ok() {
        eprintln!("--- NORMALIZED ---\n{}\n------------------", normalized);
    }
    normalized.parse().map_err(|e: KdlError| {
        // kdl-rs errors already have good Display impl with line/col
        format!("KDL parse error: {}", e)
    })
}

fn pre_parse(input: &str) -> String {
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

        // Handle selector shorthand: &id or .class at start of identifier position
        // Only transform if followed by word characters
        if (c == '&' || c == '.') && i + 1 < chars.len() && is_ident_start(chars[i + 1]) {
            // Check if we're at a position where a node name would be expected
            // (after whitespace, {, ;, or at start)
            let prev = if i > 0 { Some(chars[i - 1]) } else { None };
            if prev.is_none() || prev == Some('\n') || prev == Some(' ') || prev == Some('\t') ||
               prev == Some('{') || prev == Some(';') {
                // This is a selector shorthand - quote it
                result.push('"');
                result.push(c);
                i += 1;
                while i < chars.len() && is_selector_char(chars[i]) {
                    result.push(chars[i]);
                    i += 1;
                }
                result.push('"');
                continue;
            }
        }

        // Handle tag.class selector shorthand (e.g., div.foo.bar) or path-like identifiers
        if is_ident_start(c) || (c == '.' && i + 1 < chars.len() && chars[i+1] == '/') {
            let prev = if i > 0 { Some(chars[i - 1]) } else { None };
            if prev.is_none() || prev == Some('\n') || prev == Some(' ') || prev == Some('\t') ||
               prev == Some('{') || prev == Some(';') {
                
                // Collect the full identifier/path/selector chain
                let start = i;
                while i < chars.len() && (is_ident_char(chars[i]) || chars[i] == '/' || chars[i] == '.' || chars[i] == '&' || chars[i] == '#') {
                    i += 1;
                }
                let full_ident: String = chars[start..i].iter().collect();

                // Quote if it's not a plain identifier
                // Plain identifier: only letters, numbers, _, - and doesn't start with digit/path
                let is_plain = !full_ident.contains('/') && 
                              !full_ident.contains('.') && 
                              !full_ident.contains('&') && 
                              !full_ident.contains('#') &&
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
            i += 1;
            // Skip whitespace
            while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
                i += 1;
            }
            // Check for 'else'
            if i + 4 <= chars.len() {
                let next4: String = chars[i..i+4].iter().collect();
                if next4 == "else" && (i + 4 >= chars.len() || !is_ident_char(chars[i + 4])) {
                    result.push('\n');
                }
            }
            continue;
        }

        // Handle tilde attribute: ~on:click="value" or ~on:click~once="value"
        // The tilde starts an attribute name that can contain ~, :, and .
        if c == '~' {
            let prev = if i > 0 { Some(chars[i - 1]) } else { None };
            // Check if this is an inline tilde attribute (after whitespace)
            if prev == Some(' ') || prev == Some('\t') || prev == Some('\n') {
                // Collect the tilde attribute name
                let start = i;
                i += 1; // skip the ~
                while i < chars.len() && is_tilde_attr_char(chars[i]) {
                    i += 1;
                }
                let attr_name: String = chars[start..i].iter().collect();

                // Check if followed by = for property assignment
                if i < chars.len() && chars[i] == '=' {
                    result.push_str(&attr_name);
                    // Let the regular = handling take care of the value
                } else {
                    // No value - just the attribute name (boolean attribute)
                    result.push_str(&attr_name);
                }
                continue;
            }
            // Check if this is ~> binding shorthand (after element name)
            if i + 1 < chars.len() && chars[i + 1] == '>' {
                // element~>signal -> element ~bind="signal"
                // element~>signal~debounce:300ms -> element ~bind~debounce:300ms="signal"
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
                result.push_str(&format!(" ~bind{}=\"{}\"", modifiers, signal_name));
                continue;
            }
        }

        // Handle bare word property values: key=value -> key="value"
        if c == '=' && i + 1 < chars.len() && is_ident_start(chars[i + 1]) {
            // Check that we're in a property context (preceded by identifier)
            let mut j = i - 1;
            while j > 0 && is_ident_char(chars[j]) {
                j -= 1;
            }
            if j < i - 1 {
                // There's an identifier before =
                result.push(c);
                i += 1;
                // Check if the value is a raw string (already quoted)
                if chars[i] == '"' || (chars[i] == '#' && i + 1 < chars.len() && chars[i + 1] == '"') {
                    // Already quoted, pass through
                } else {
                    // Quote the bare value
                    result.push('"');
                    while i < chars.len() && is_ident_char(chars[i]) {
                        result.push(chars[i]);
                        i += 1;
                    }
                    result.push('"');
                }
                continue;
            }
        }

        // Handle numeric values with units: 10px -> _10px (for KDL compatibility)
        if c.is_ascii_digit() {
            let prev = if i > 0 { Some(chars[i - 1]) } else { None };
            if prev.is_none() || prev == Some(' ') || prev == Some('\t') ||
               prev == Some('{') || prev == Some(';') || prev == Some('\n') {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                // Check if followed by unit
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
                    // Just a number, push it
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

/// Check if a character is valid in a tilde attribute name (includes ~, :, .)
fn is_tilde_attr_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '~' || c == ':' || c == '.'
}

fn is_selector_char(c: char) -> bool {
    // Include ':' for Tailwind CSS modifiers like md:, hover:, focus:
    // Include '#' for ID selectors like div#root
    c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '&' || c == ':' || c == '#'
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Backtick Expression Tests ===

    #[test]
    fn test_backtick_wrapping() {
        let result = pre_parse("span `name`");
        assert!(result.contains("#\"`name`\"#"));
    }

    #[test]
    fn test_backtick_with_inner_quotes() {
        // CEL ternary with quoted strings must preserve the quotes
        let result = pre_parse(r#"span `active ? "yes" : "no"`"#);
        assert!(result.contains("#\"`active ? \"yes\" : \"no\"`\"#"));
    }

    #[test]
    fn test_multiple_backticks_same_line() {
        let result = pre_parse("span `first` `second`");
        assert!(result.contains("#\"`first`\"#"));
        assert!(result.contains("#\"`second`\"#"));
    }

    #[test]
    fn test_backtick_in_attribute() {
        let result = pre_parse("a href=`user.url`");
        // The backtick should be wrapped, but also the attribute value handling applies
        assert!(result.contains("#\"`user.url`\"#"));
    }

    // === String Passthrough Tests ===

    #[test]
    fn test_string_passthrough() {
        let result = pre_parse(r#"p "Hello & World""#);
        assert!(result.contains("\"Hello & World\""));
        // The & inside string should NOT trigger selector transformation
        assert!(!result.contains("\"&"));
    }

    #[test]
    fn test_string_with_ampersand_at_start() {
        let result = pre_parse(r#"p "&foo bar""#);
        assert!(result.contains("\"&foo bar\""));
    }

    #[test]
    fn test_string_with_dot_inside() {
        let result = pre_parse(r#"p "div.class is CSS""#);
        assert!(result.contains("\"div.class is CSS\""));
    }

    #[test]
    fn test_string_with_backticks_inside() {
        // Backticks inside quoted strings should NOT be transformed
        let result = pre_parse(r#"p "Use `code` for inline""#);
        assert!(result.contains("\"Use `code` for inline\""));
        // Should NOT have raw string wrapper
        assert!(!result.contains("#\""));
    }

    #[test]
    fn test_escape_sequences_in_strings() {
        let result = pre_parse(r#"p "Line1\nLine2""#);
        assert!(result.contains("\"Line1\\nLine2\""));
    }

    #[test]
    fn test_escaped_quote_in_string() {
        let result = pre_parse(r#"p "Say \"hello\"""#);
        assert!(result.contains("\"Say \\\"hello\\\"\""));
    }

    // === Raw String Passthrough Tests ===

    #[test]
    fn test_raw_string_passthrough() {
        let result = pre_parse("p #\"raw & string\"#");
        assert!(result.contains("#\"raw & string\"#"));
    }

    #[test]
    fn test_raw_string_with_quotes() {
        let result = pre_parse("p #\"contains quotes inside\"#");
        assert!(result.contains("#\"contains quotes inside\"#"));
    }

    // === Comment Tests ===

    #[test]
    fn test_line_comment_passthrough() {
        let result = pre_parse("// &foo.bar comment\ndiv");
        assert!(result.contains("// &foo.bar comment"));
        // The &foo.bar in comment should NOT be quoted
        assert!(!result.contains("\"&foo.bar\""));
    }

    #[test]
    fn test_block_comment_passthrough() {
        let result = pre_parse("/* &foo.bar */ div");
        assert!(result.contains("/* &foo.bar */"));
    }

    #[test]
    fn test_proto_block_passthrough() {
        let result = pre_parse("/** message Foo { } */");
        assert!(result.contains("/** message Foo { } */"));
    }

    #[test]
    fn test_proto_block_multiline() {
        let input = r#"/**
message User {
    string name = 1;
    string email = 2;
}
*/"#;
        let result = pre_parse(input);
        assert!(result.contains("message User"));
        assert!(result.contains("string name = 1"));
    }

    // === Selector Shorthand Tests ===

    #[test]
    fn test_selector_shorthand() {
        let result = pre_parse("&main.container { }");
        assert!(result.contains("\"&main.container\""));
    }

    #[test]
    fn test_class_only_selector() {
        let result = pre_parse(".container { }");
        assert!(result.contains("\".container\""));
    }

    #[test]
    fn test_id_only_selector() {
        let result = pre_parse("&header { }");
        assert!(result.contains("\"&header\""));
    }

    #[test]
    fn test_tag_with_class() {
        let result = pre_parse("div.foo.bar { }");
        assert!(result.contains("\"div.foo.bar\""));
    }

    #[test]
    fn test_tag_with_id() {
        let result = pre_parse("div&myId { }");
        assert!(result.contains("\"div&myId\""));
    }

    #[test]
    fn test_tag_with_hash_id() {
        // CSS-style ID selector with #
        let result = pre_parse("div#root { }");
        assert!(result.contains("\"div#root\""));
    }

    #[test]
    fn test_tag_with_hash_id_and_class() {
        // CSS-style ID selector with # followed by class
        let result = pre_parse("div#root.container { }");
        assert!(result.contains("\"div#root.container\""));
    }

    #[test]
    fn test_tag_with_class_and_id() {
        let result = pre_parse("div.foo&bar { }");
        assert!(result.contains("\"div.foo&bar\""));
    }

    #[test]
    fn test_plain_tag_no_transformation() {
        let result = pre_parse("div { }");
        // Plain div should NOT be quoted
        assert!(result.contains("div {"));
        assert!(!result.contains("\"div\""));
    }

    // === Else Handling Tests ===

    #[test]
    fn test_else_handling() {
        let result = pre_parse("} else {");
        assert!(result.contains("}\nelse"));
    }

    #[test]
    fn test_else_with_tabs() {
        let result = pre_parse("}\t\telse {");
        assert!(result.contains("}\nelse"));
    }

    #[test]
    fn test_else_if_not_transformed() {
        // "elsewhere" should not trigger else handling
        let result = pre_parse("} elsewhere {");
        assert!(!result.contains("}\nelsewhere"));
    }

    // === Attribute Value Tests ===

    #[test]
    fn test_attribute_value() {
        let result = pre_parse("input type=text");
        assert!(result.contains("type=\"text\""));
    }

    #[test]
    fn test_attribute_value_with_hyphen() {
        let result = pre_parse("div data-id=foo-bar");
        assert!(result.contains("data-id=\"foo-bar\""));
    }

    #[test]
    fn test_attribute_already_quoted() {
        let result = pre_parse(r#"input type="text""#);
        // Should not double-quote
        assert!(result.contains("type=\"text\""));
        assert!(!result.contains("type=\"\"text\"\""));
    }

    #[test]
    fn test_attribute_with_raw_string() {
        let result = pre_parse("div class=#\"foo bar\"#");
        // Should not transform raw string values
        assert!(result.contains("class=#\"foo bar\"#"));
    }

    // === Numeric Unit Tests ===

    #[test]
    fn test_numeric_with_unit() {
        let result = pre_parse("div 10px");
        assert!(result.contains("_10px"));
    }

    #[test]
    fn test_numeric_with_percent() {
        // % is not alphabetic, so 100% doesn't get underscore prefix
        // (only units like px, rem, em that start with letters do)
        let result = pre_parse("div 100%");
        assert!(result.contains("100%"));
    }

    #[test]
    fn test_numeric_with_rem() {
        let result = pre_parse("div 2rem");
        assert!(result.contains("_2rem"));
    }

    #[test]
    fn test_plain_number_no_transformation() {
        let result = pre_parse("div 42");
        // Plain number should NOT be prefixed
        assert!(result.contains(" 42"));
        assert!(!result.contains("_42"));
    }

    #[test]
    fn test_number_in_string_no_transformation() {
        let result = pre_parse(r#"p "width: 10px""#);
        // Number inside string should not be transformed
        assert!(result.contains("\"width: 10px\""));
        assert!(!result.contains("_10px"));
    }

    // === Integration Tests ===

    #[test]
    fn test_complex_template() {
        let input = r#"
// Component: Dashboard
div.container&main {
    if `user.isAdmin` {
        span.admin "Admin: `user.name`"
    } else {
        span "User: `user.name`"
    }
    p "Contact & Support"
}
"#;
        let result = pre_parse(input);

        // Selector should be quoted
        assert!(result.contains("\"div.container&main\""));
        // Standalone backticks (outside strings) should be wrapped in raw strings
        assert!(result.contains("#\"`user.isAdmin`\"#"));
        // Backticks INSIDE quoted strings should NOT be transformed
        // (they're handled by codegen for string interpolation)
        assert!(result.contains("\"Admin: `user.name`\""));
        assert!(result.contains("\"User: `user.name`\""));
        // Else should have newline
        assert!(result.contains("}\nelse"));
        // & in string should be preserved
        assert!(result.contains("\"Contact & Support\""));
        // Comment should be preserved
        assert!(result.contains("// Component: Dashboard"));
    }

    #[test]
    fn test_full_parse_with_ampersand_in_string() {
        // This was the original bug - ensure full parse works
        let input = r#"
el {
    p "Built with HUDL & Go"
}
"#;
        let result = parse(input);
        assert!(result.is_ok(), "Parse failed: {:?}", result.err());
    }

    #[test]
    fn test_full_parse_with_cel_ternary() {
        let input = r#"
el {
    span `active ? "yes" : "no"`
}
"#;
        let result = parse(input);
        assert!(result.is_ok(), "Parse failed: {:?}", result.err());
    }

    #[test]
    fn test_import_with_path() {
        let input = r#"
import {
    ./layout
}
el {
    div "hello"
}
"#;
        let result = parse(input);
        assert!(result.is_ok(), "Import path should be parseable (maybe after pre-parsing fixes): {:?}", result.err());
    }
}

#[cfg(test)]
mod extra_tests {
    use super::*;
    
    #[test]
    fn test_parse_simple_hudl() {
        let input = r#"/**
message SimpleData {
    string title = 1;
    string description = 2;
    repeated string features = 3;
}
*/

// name: Simple
// data: SimpleData

el {
    div#root.container {
        h1 `title`
        p `description`
        ul {
            each feat `features` {
                li `feat`
            }
        }
    }
}"#;
        
        let result = parse(input);
        match &result {
            Ok(doc) => println!("Success! {} nodes", doc.nodes().len()),
            Err(e) => println!("Error: {}", e),
        }
        assert!(result.is_ok(), "Parse failed: {:?}", result.err());
    }
}
