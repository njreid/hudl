//! Hudl formatter that converts KDL back to Hudl syntax
//!
//! This module handles:
//! - Custom indentation (respecting editor tab-width settings)
//! - Inverse pre-parsing (unquoting CSS selectors, unwrapping raw strings)
//! - Context-aware formatting of CEL expressions vs string literals
//! - Proto block formatting via `buf format`

use kdl::{KdlDocument, KdlNode, KdlValue, KdlEntry};
use std::process::{Command, Stdio};

/// Formatting options
#[derive(Debug, Clone)]
pub struct FormatOptions {
    /// Number of spaces per indent level
    pub indent_size: usize,
    /// Whether to use spaces (true) or tabs (false)
    pub use_spaces: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            indent_size: 4,
            use_spaces: true,
        }
    }
}

impl FormatOptions {
    pub fn new(tab_size: u32, insert_spaces: bool) -> Self {
        Self {
            indent_size: tab_size as usize,
            use_spaces: insert_spaces,
        }
    }

    fn indent(&self, level: usize) -> String {
        if self.use_spaces {
            " ".repeat(self.indent_size * level)
        } else {
            "\t".repeat(level)
        }
    }
}

/// Format a KdlDocument back to Hudl syntax
pub fn format(doc: &KdlDocument, options: &FormatOptions) -> String {
    let mut output = String::new();

    // Preserve document-level leading content (proto blocks, comments)
    if let Some(fmt) = doc.format() {
        let leading = &fmt.leading;
        if !leading.is_empty() {
            let processed = process_leading_comments(leading);
            output.push_str(&processed);
            if !processed.ends_with('\n') {
                output.push('\n');
            }
        }
    }

    for (i, node) in doc.nodes().iter().enumerate() {
        format_node(&mut output, node, 0, options, i == 0);
    }

    // Preserve document-level trailing content
    if let Some(fmt) = doc.format() {
        let trailing = &fmt.trailing;
        if !trailing.is_empty() && !trailing.chars().all(|c| c.is_whitespace()) {
            output.push_str(trailing);
        }
    }

    output
}

/// Format protobuf content using `clang-format`
/// Returns the formatted content, or the original if clang-format is not available or fails
fn format_proto_content(proto_content: &str) -> String {
    use std::io::Write;

    // clang-format reads from stdin with --assume-filename to set the language
    let clang_format_paths = [
        "clang-format",
        "/usr/bin/clang-format",
        "/usr/local/bin/clang-format",
    ];

    for clang_path in &clang_format_paths {
        let child = Command::new(clang_path)
            .args(["--assume-filename=x.proto"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();

        match child {
            Ok(mut child) => {
                // Write proto content to stdin
                if let Some(mut stdin) = child.stdin.take() {
                    if stdin.write_all(proto_content.as_bytes()).is_err() {
                        continue;
                    }
                }

                // Read formatted output
                match child.wait_with_output() {
                    Ok(output) if output.status.success() => {
                        return String::from_utf8_lossy(&output.stdout).to_string();
                    }
                    _ => continue,
                }
            }
            Err(_) => continue,
        }
    }

    // clang-format not available, return original
    proto_content.to_string()
}

/// Process leading comments to ensure proper spacing:
/// - Format proto blocks (/** ... */) using buf format
/// - Newline between multiline comments (/* */) and single-line comments (//)
/// - No extra blank lines between single-line comments and nodes
fn process_leading_comments(content: &str) -> String {
    let mut result = String::new();
    let mut in_proto_block = false;
    let mut proto_content = String::new();
    let mut after_multiline = false;
    let mut added_blank_after_multiline = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Detect proto block start (/** but not /*** which is decorative)
        if trimmed.starts_with("/**") && !trimmed.starts_with("/***") {
            in_proto_block = true;
            proto_content.clear();
            after_multiline = false;
            added_blank_after_multiline = false;

            // Check if it closes on the same line
            if trimmed.len() > 3 && trimmed[3..].contains("*/") {
                // Single-line proto block - probably empty or very short
                result.push_str(line);
                result.push('\n');
                in_proto_block = false;
                after_multiline = true;
            }
            continue;
        }

        if in_proto_block {
            // Check for end of proto block
            if trimmed.contains("*/") {
                in_proto_block = false;
                after_multiline = true;

                // Format the proto content
                let formatted_proto = format_proto_content(&proto_content);

                // Output the formatted proto block
                result.push_str("/**\n");
                for proto_line in formatted_proto.lines() {
                    result.push_str(proto_line);
                    result.push('\n');
                }
                result.push_str("*/\n");
            } else {
                // Accumulate proto content
                proto_content.push_str(line);
                proto_content.push('\n');
            }
            continue;
        }

        // Handle regular multiline comments (/* but not /**)
        if trimmed.starts_with("/*") && !trimmed.starts_with("/**") {
            result.push_str(line);
            result.push('\n');
            if !trimmed.ends_with("*/") {
                // Multi-line, need to keep reading
                // For simplicity, just pass through
            }
            after_multiline = trimmed.ends_with("*/");
            continue;
        }

        // Skip blank lines after multiline comment (we'll add our own)
        if trimmed.is_empty() {
            continue;
        }

        // Add blank line between multiline comment and single-line comments
        if after_multiline && !added_blank_after_multiline {
            result.push('\n');
            added_blank_after_multiline = true;
        }

        result.push_str(line);
        result.push('\n');
    }

    result
}

/// Context for formatting entries based on their position
#[derive(Debug, Clone, Copy, PartialEq)]
enum EntryContext {
    /// First argument to `each` - bare variable name
    EachVarName,
    /// Second argument to `each` - CEL expression with backticks
    EachExpression,
    /// Argument to `switch` - bare variable name
    SwitchExpression,
    /// Regular content (text or CEL expression)
    Content,
    /// Property value (like class="foo")
    Property,
}

fn format_node(output: &mut String, node: &KdlNode, depth: usize, options: &FormatOptions, is_first_top_level: bool) {
    let indent = options.indent(depth);

    // Preserve leading comments for this node
    if let Some(fmt) = node.format() {
        let leading = &fmt.leading;
        if !leading.is_empty() {
            let trimmed = leading.trim_start_matches(|c: char| c == ' ' || c == '\t');
            if !trimmed.is_empty() {
                if is_first_top_level && depth == 0 {
                    // First top-level node: process comments with proper spacing
                    let processed = process_leading_comments(trimmed);
                    output.push_str(&processed);
                } else if depth == 0 {
                    // Other top-level nodes
                    for line in trimmed.lines() {
                        let line_trimmed = line.trim();
                        if !line_trimmed.is_empty() {
                            output.push_str(&indent);
                            output.push_str(line_trimmed);
                            output.push('\n');
                        }
                    }
                } else {
                    // Nested nodes: just preserve comments with proper indentation
                    for line in trimmed.lines() {
                        let line_trimmed = line.trim();
                        if line_trimmed.starts_with("//") || line_trimmed.starts_with("/*") {
                            output.push_str(&indent);
                            output.push_str(line_trimmed);
                            output.push('\n');
                        }
                    }
                }
            }
        }
    }

    // Format node name - apply inverse pre-parsing
    let name = inverse_preparse_name(node.name().value());

    output.push_str(&indent);
    output.push_str(&name);

    // Determine context based on node type
    let node_name = node.name().value();
    let is_each = node_name == "each";
    let is_switch = node_name == "switch";

    // Format entries (arguments and properties)
    let mut arg_index = 0;
    for entry in node.entries() {
        output.push(' ');

        let context = if entry.name().is_some() {
            // Named property
            EntryContext::Property
        } else if is_each {
            // each <varname> <expression>
            let ctx = if arg_index == 0 {
                EntryContext::EachVarName
            } else {
                EntryContext::EachExpression
            };
            arg_index += 1;
            ctx
        } else if is_switch && arg_index == 0 {
            arg_index += 1;
            EntryContext::SwitchExpression
        } else {
            arg_index += 1;
            EntryContext::Content
        };

        format_entry(output, entry, context);
    }

    // Format children
    if let Some(children) = node.children() {
        if children.nodes().is_empty() {
            output.push_str(" {}");
        } else {
            output.push_str(" {\n");
            for child in children.nodes() {
                format_node(output, child, depth + 1, options, false);
            }
            output.push_str(&indent);
            output.push('}');
        }
    }

    output.push('\n');
}

fn format_entry(output: &mut String, entry: &KdlEntry, context: EntryContext) {
    if let Some(name) = entry.name() {
        output.push_str(name.value());
        output.push('=');
    }
    format_value(output, entry.value(), context);
}

fn format_value(output: &mut String, value: &KdlValue, context: EntryContext) {
    match value {
        KdlValue::String(s) => {
            // Check if this is a pre-parsed backtick expression (wrapped as `expr`)
            let (is_backtick_wrapped, inner) = if s.starts_with('`') && s.ends_with('`') && s.len() >= 2 {
                (true, &s[1..s.len()-1])
            } else {
                (false, s.as_str())
            };

            match context {
                EntryContext::EachVarName | EntryContext::SwitchExpression => {
                    // Bare identifier - no quotes or backticks
                    // If it was wrapped in backticks by pre-parser, unwrap it
                    output.push_str(inner);
                }
                EntryContext::EachExpression => {
                    // CEL expression - always use backticks
                    if is_backtick_wrapped {
                        // Already has backticks from pre-parser, output as-is
                        output.push_str(s);
                    } else {
                        output.push('`');
                        output.push_str(s);
                        output.push('`');
                    }
                }
                EntryContext::Content => {
                    if is_backtick_wrapped {
                        // Was originally a backtick expression, keep it that way
                        output.push_str(s);
                    } else {
                        // Was originally a quoted string, keep it quoted
                        output.push('"');
                        output.push_str(&escape_string(s));
                        output.push('"');
                    }
                }
                EntryContext::Property => {
                    if is_backtick_wrapped {
                        // Property with CEL expression
                        output.push_str(s);
                    } else {
                        // Regular string property
                        output.push('"');
                        output.push_str(&escape_string(s));
                        output.push('"');
                    }
                }
            }
        }
        KdlValue::Integer(i) => {
            output.push_str(&i.to_string());
        }
        KdlValue::Float(f) => {
            output.push_str(&f.to_string());
        }
        KdlValue::Bool(b) => {
            output.push_str(if *b { "true" } else { "false" });
        }
        KdlValue::Null => {
            output.push_str("null");
        }
    }
}

/// Check if a string looks like a CEL expression (for backtick formatting)
///
/// Note: This is currently unused since we rely on the pre-parser's backtick
/// markers, but kept for potential future use.
///
/// Heuristic:
/// - Simple identifiers (title, userName, item_count) -> backticks (likely variable refs)
/// - Field access (user.name) -> backticks
/// - Function calls (items.size()) -> backticks
/// - Expressions with operators (a + b) -> backticks
/// - Human-readable text with spaces ("Hello World") -> quotes
/// - HTML content -> quotes
#[allow(dead_code)]
fn is_backtick_expression(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    // Must start with a letter or underscore (like a variable)
    let first = s.chars().next().unwrap();
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }

    // If it contains spaces without operators, it's human-readable text
    if s.contains(' ') {
        let has_operators = s.contains('+') || s.contains('-') || s.contains('*') ||
                           s.contains('/') || s.contains('?') || s.contains(':') ||
                           s.contains("==") || s.contains("!=") || s.contains("&&") ||
                           s.contains("||") || s.contains('<') || s.contains('>');
        if !has_operators {
            return false;
        }
    }

    // If it looks like HTML, it's a string
    if s.contains("</") || s.contains("/>") || (s.contains('<') && s.contains('>') && !s.contains("==")) {
        return false;
    }

    // Must only contain valid identifier/expression characters
    s.chars().all(|c|
        c.is_ascii_alphanumeric() ||
        c == '_' || c == '.' || c == '(' || c == ')' ||
        c == '[' || c == ']' || c == ' ' || c == '+' ||
        c == '-' || c == '*' || c == '/' || c == '=' ||
        c == '!' || c == '?' || c == ':' || c == ',' ||
        c == '\'' || c == '"' || c == '&' || c == '|' ||
        c == '<' || c == '>'
    )
}

/// Inverse pre-parse a node name back to Hudl syntax
fn inverse_preparse_name(name: &str) -> String {
    // If the name is quoted and looks like a CSS selector, unquote it
    // The KDL parser already gives us the unquoted value, but we stored
    // selectors as quoted strings like "div#root.container"

    // Check if this looks like a CSS selector that should be unquoted
    if is_css_selector(name) {
        return name.to_string();
    }

    // Check if it needs quoting for KDL compatibility but not for Hudl
    // (names with special chars that aren't valid KDL identifiers)
    name.to_string()
}

/// Check if a name looks like a CSS selector (should be displayed without quotes in Hudl)
fn is_css_selector(name: &str) -> bool {
    // CSS selectors have: tag names, #id, .class, &reference
    // e.g., "div#root.container", ".my-class", "&my-id"
    if name.is_empty() {
        return false;
    }

    // Must start with a letter, #, ., or &
    let first = name.chars().next().unwrap();
    if !first.is_ascii_alphabetic() && first != '#' && first != '.' && first != '&' {
        return false;
    }

    // Must only contain valid selector characters
    name.chars().all(|c|
        c.is_ascii_alphanumeric() ||
        c == '_' || c == '-' || c == '.' || c == '#' || c == '&' || c == ':'
    )
}

fn escape_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            _ => result.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    #[test]
    fn test_format_simple() {
        let input = r#"el { div "hello" }"#;
        let doc = parse(input).unwrap();
        let options = FormatOptions::new(2, true);
        let formatted = format(&doc, &options);
        assert!(formatted.contains("el {"));
        assert!(formatted.contains("  div")); // 2-space indent
    }

    #[test]
    fn test_format_preserves_selectors() {
        let input = r#"el { div#root.container { h1 "hello" } }"#;
        let doc = parse(input).unwrap();
        let options = FormatOptions::default();
        let formatted = format(&doc, &options);
        // The selector should appear without extra quotes in output
        assert!(formatted.contains("div#root.container"));
    }

    #[test]
    fn test_format_backtick_expressions() {
        let input = r#"el { h1 `title` }"#;
        let doc = parse(input).unwrap();
        let options = FormatOptions::default();
        let formatted = format(&doc, &options);
        assert!(formatted.contains("`title`"));
    }

    #[test]
    fn test_format_custom_indent() {
        let input = r#"el { div { span "text" } }"#;
        let doc = parse(input).unwrap();

        let options_2 = FormatOptions::new(2, true);
        let formatted_2 = format(&doc, &options_2);
        assert!(formatted_2.contains("  div")); // 2-space indent

        let options_4 = FormatOptions::new(4, true);
        let formatted_4 = format(&doc, &options_4);
        assert!(formatted_4.contains("    div")); // 4-space indent
    }

    #[test]
    fn test_is_css_selector() {
        assert!(is_css_selector("div"));
        assert!(is_css_selector("div#root"));
        assert!(is_css_selector("div.container"));
        assert!(is_css_selector("div#root.container"));
        assert!(is_css_selector(".my-class"));
        assert!(is_css_selector("#my-id"));
        assert!(is_css_selector("&reference"));
        assert!(is_css_selector("hover:bg-blue-500"));

        assert!(!is_css_selector(""));
        assert!(!is_css_selector("123"));
        assert!(!is_css_selector("hello world")); // spaces
    }

    #[test]
    fn test_is_backtick_expression() {
        assert!(is_backtick_expression("title"));
        assert!(is_backtick_expression("user.name"));
        assert!(is_backtick_expression("items.size()"));
        assert!(is_backtick_expression("x + y"));
        assert!(is_backtick_expression("a ? b : c"));

        assert!(!is_backtick_expression(""));
        assert!(!is_backtick_expression("Hello World")); // Human-readable text
        assert!(!is_backtick_expression("<div>")); // HTML
    }

    #[test]
    fn test_format_each_loop() {
        // each <varname> <expression> - varname is bare, expression has backticks
        let input = r#"el { each item `items` { li `item` } }"#;
        let doc = parse(input).unwrap();
        let options = FormatOptions::new(2, true);
        let formatted = format(&doc, &options);
        // First arg (item) should be bare, second arg (items) should have backticks
        assert!(formatted.contains("each item `items`"));
        // Content should also have backticks
        assert!(formatted.contains("li `item`"));
    }

    #[test]
    fn test_format_switch() {
        // switch <expression> - expression is bare
        let input = r#"el { switch status { case "active" { span "Active" } } }"#;
        let doc = parse(input).unwrap();
        let options = FormatOptions::new(2, true);
        let formatted = format(&doc, &options);
        // switch arg should be bare
        assert!(formatted.contains("switch status"));
    }

    #[test]
    fn test_format_simple_hudl_content() {
        // Test formatting similar to simple.hudl
        let input = r#"el {
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
        let doc = parse(input).unwrap();
        let options = FormatOptions::new(2, true);
        let formatted = format(&doc, &options);

        // Check 2-space indentation
        assert!(formatted.contains("  div#root.container"));
        assert!(formatted.contains("    h1"));
        assert!(formatted.contains("      each"));

        // Check backtick expressions are preserved
        assert!(formatted.contains("`title`"));
        assert!(formatted.contains("`description`"));
        assert!(formatted.contains("each feat `features`"));
        assert!(formatted.contains("`feat`"));

        // Check selector is unquoted
        assert!(formatted.contains("div#root.container"));
    }

    #[test]
    fn test_format_preserves_string_literals() {
        // Quoted strings should stay quoted, not become backticks
        let input = r#"el { h1 "Hello World" }"#;
        let doc = parse(input).unwrap();
        let options = FormatOptions::default();
        let formatted = format(&doc, &options);
        assert!(formatted.contains("\"Hello World\""));
        assert!(!formatted.contains("`Hello World`"));
    }

    #[test]
    fn test_format_preserves_comments() {
        let input = r#"// name: MyComponent
// data: MyData
el {
    div "content"
}"#;
        let doc = parse(input).unwrap();
        let options = FormatOptions::new(2, true);
        let formatted = format(&doc, &options);
        // Comments should be preserved
        assert!(formatted.contains("// name: MyComponent"));
        assert!(formatted.contains("// data: MyData"));
    }

    #[test]
    fn test_format_preserves_proto_block() {
        let input = r#"/**
message MyData {
    string name = 1;
}
*/

// name: Test
// data: MyData
el {
    div `name`
}"#;
        let doc = parse(input).unwrap();
        let options = FormatOptions::new(2, true);
        let formatted = format(&doc, &options);
        // Proto block should be preserved
        assert!(formatted.contains("/**"));
        assert!(formatted.contains("message MyData"));
        assert!(formatted.contains("*/"));
        // Should have blank line between multiline comment and single-line comments
        assert!(formatted.contains("*/\n\n//"), "Should have blank line between */ and //");
        // Single-line comments should be preserved
        assert!(formatted.contains("// name: Test"));
        assert!(formatted.contains("// data: MyData"));
    }
}
