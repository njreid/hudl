use tower_lsp::lsp_types::*;

use hudlc::ast::{DatastarAttr, Node, Root};

/// Known Datastar attribute categories and their valid modifiers
struct AttrCategory {
    valid_modifiers: &'static [&'static str],
}

/// Wildcard modifier entries end with `:*` meaning any suffix after `:` is valid
const EVENT_MODIFIERS: &[&str] = &[
    "once", "prevent", "stop", "capture", "outside", "passive",
    "window", "document", "debounce:*", "throttle:*",
];
const INTERSECT_EXTRA: &[&str] = &["half", "full"];
const FETCH_EXTRA: &[&str] = &[
    "swap:*", "settle:*", "target:*", "indicator:*", "header.*",
    "from:*", "scroll:*",
];
const LET_MODIFIERS: &[&str] = &["ifmissing"];
const PERSIST_MODIFIERS: &[&str] = &["session"];
const TELEPORT_MODIFIERS: &[&str] = &["prepend", "append"];
const SCROLL_MODIFIERS: &[&str] = &[
    "smooth", "instant", "hstart", "hcenter", "hend", "vstart", "vcenter", "vend",
];
const BIND_MODIFIERS: &[&str] = &["debounce:*", "throttle:*"];
const NO_MODIFIERS: &[&str] = &[];

/// Classify a Datastar attribute name and return the valid modifier set
fn classify_attr(name: &str) -> Option<AttrCategory> {
    if name == "on:intersect" {
        // Intersect gets event modifiers + its own extras
        Some(AttrCategory { valid_modifiers: &[] }) // handled specially
    } else if name.starts_with("on:fetch") {
        Some(AttrCategory { valid_modifiers: &[] }) // handled specially
    } else if name.starts_with("on:") {
        Some(AttrCategory { valid_modifiers: EVENT_MODIFIERS })
    } else if name.starts_with("let:") {
        Some(AttrCategory { valid_modifiers: LET_MODIFIERS })
    } else if name.starts_with('.') || name.starts_with("class:") {
        Some(AttrCategory { valid_modifiers: NO_MODIFIERS })
    } else {
        match name {
            "show" | "text" | "ref" => Some(AttrCategory { valid_modifiers: NO_MODIFIERS }),
            "bind" => Some(AttrCategory { valid_modifiers: BIND_MODIFIERS }),
            "persist" => Some(AttrCategory { valid_modifiers: PERSIST_MODIFIERS }),
            "teleport" => Some(AttrCategory { valid_modifiers: TELEPORT_MODIFIERS }),
            "scrollIntoView" => Some(AttrCategory { valid_modifiers: SCROLL_MODIFIERS }),
            _ => None, // Unknown attribute
        }
    }
}

/// Check if a modifier matches a modifier spec (supports `*` wildcard suffix)
fn modifier_matches(modifier: &str, spec: &str) -> bool {
    if spec.ends_with(":*") {
        let prefix = &spec[..spec.len() - 1]; // "debounce:"
        modifier == &spec[..spec.len() - 2] || modifier.starts_with(prefix)
    } else if spec.ends_with(".*") {
        let prefix = &spec[..spec.len() - 1]; // "header."
        modifier.starts_with(prefix)
    } else {
        modifier == spec
    }
}

/// Get valid modifiers for a given attribute name, combining sets as needed
fn get_valid_modifiers(name: &str) -> Vec<&'static str> {
    if name == "on:intersect" {
        let mut mods: Vec<&str> = EVENT_MODIFIERS.to_vec();
        mods.extend_from_slice(INTERSECT_EXTRA);
        mods
    } else if name == "on:fetch" {
        let mut mods: Vec<&str> = EVENT_MODIFIERS.to_vec();
        mods.extend_from_slice(FETCH_EXTRA);
        mods
    } else if let Some(cat) = classify_attr(name) {
        cat.valid_modifiers.to_vec()
    } else {
        Vec::new()
    }
}

/// Validate Datastar attributes in a template and return diagnostics
pub fn validate_datastar_attrs(content: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    let doc = match hudlc::parser::parse(content) {
        Ok(doc) => doc,
        Err(_) => return diagnostics, // Syntax errors handled elsewhere
    };

    let root = match hudlc::transformer::transform(&doc) {
        Ok(root) => root,
        Err(_) => return diagnostics,
    };

    // Collect all datastar attrs with their positions
    let mut attrs_to_check: Vec<&DatastarAttr> = Vec::new();
    collect_datastar_attrs_from_nodes(&root.nodes, &mut attrs_to_check);

    // For each attr, find its line number in source text and validate
    for attr in attrs_to_check {
        let (line, col) = find_attr_position(content, &attr.name);

        // Check if the attribute name is known
        if classify_attr(&attr.name).is_none() {
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position { line, character: col },
                    end: Position { line, character: col + attr.name.len() as u32 },
                },
                severity: Some(DiagnosticSeverity::WARNING),
                source: Some("hudl-datastar".to_string()),
                message: format!("Unknown Datastar attribute '~{}'", attr.name),
                ..Default::default()
            });
            continue;
        }

        // Validate modifiers
        let valid_mods = get_valid_modifiers(&attr.name);
        for modifier in &attr.modifiers {
            if !valid_mods.iter().any(|spec| modifier_matches(modifier, spec)) {
                diagnostics.push(Diagnostic {
                    range: Range {
                        start: Position { line, character: col },
                        end: Position { line, character: col + attr.name.len() as u32 + 1 + modifier.len() as u32 },
                    },
                    severity: Some(DiagnosticSeverity::WARNING),
                    source: Some("hudl-datastar".to_string()),
                    message: format!(
                        "Unknown modifier '~{}' on Datastar attribute '~{}'",
                        modifier, attr.name
                    ),
                    ..Default::default()
                });
            }
        }
    }

    diagnostics
}

/// Recursively collect DatastarAttr references from AST nodes
fn collect_datastar_attrs_from_nodes<'a>(
    nodes: &'a [Node],
    out: &mut Vec<&'a DatastarAttr>,
) {
    for node in nodes {
        match node {
            Node::Element(el) => {
                for attr in &el.datastar {
                    out.push(attr);
                }
                collect_datastar_attrs_from_nodes(&el.children, out);
            }
            Node::ControlFlow(cf) => match cf {
                hudlc::ast::ControlFlow::If { then_block, else_block, .. } => {
                    collect_datastar_attrs_from_nodes(then_block, out);
                    if let Some(eb) = else_block {
                        collect_datastar_attrs_from_nodes(eb, out);
                    }
                }
                hudlc::ast::ControlFlow::Each { body, .. } => {
                    collect_datastar_attrs_from_nodes(body, out);
                }
                hudlc::ast::ControlFlow::Switch { cases, default, .. } => {
                    for case in cases {
                        collect_datastar_attrs_from_nodes(&case.1, out);
                    }
                    if let Some(d) = default {
                        collect_datastar_attrs_from_nodes(d, out);
                    }
                }
            },
            Node::Text(_) | Node::ContentSlot => {}
        }
    }
}

/// Find the position (line, col) of a tilde attribute in source text.
/// Looks for `~name` or `name` inside tilde blocks.
fn find_attr_position(content: &str, attr_name: &str) -> (u32, u32) {
    // Try to find ~attrName (inline form)
    let inline_pattern = format!("~{}", attr_name);
    for (line_num, line) in content.lines().enumerate() {
        if let Some(col) = line.find(&inline_pattern) {
            return (line_num as u32, col as u32);
        }
    }

    // Try to find attrName inside a tilde block (block form: just the name at start of line)
    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with(attr_name) {
            let next_char = trimmed.chars().nth(attr_name.len());
            if next_char.map_or(true, |c| c == ' ' || c == '"' || c == '~' || c == '\t') {
                let col = line.len() - line.trim_start().len();
                return (line_num as u32, col as u32);
            }
        }
    }

    (0, 0)
}

/// Collect signal names from `let:*` attributes in the AST
pub fn collect_signals(root: &Root) -> Vec<String> {
    let mut signals = Vec::new();
    collect_signals_from_nodes(&root.nodes, &mut signals);
    signals.sort();
    signals.dedup();
    signals
}

fn collect_signals_from_nodes(nodes: &[Node], out: &mut Vec<String>) {
    for node in nodes {
        match node {
            Node::Element(el) => {
                for attr in &el.datastar {
                    if let Some(signal_name) = attr.name.strip_prefix("let:") {
                        out.push(signal_name.to_string());
                    }
                }
                collect_signals_from_nodes(&el.children, out);
            }
            Node::ControlFlow(cf) => match cf {
                hudlc::ast::ControlFlow::If { then_block, else_block, .. } => {
                    collect_signals_from_nodes(then_block, out);
                    if let Some(eb) = else_block {
                        collect_signals_from_nodes(eb, out);
                    }
                }
                hudlc::ast::ControlFlow::Each { body, .. } => {
                    collect_signals_from_nodes(body, out);
                }
                hudlc::ast::ControlFlow::Switch { cases, default, .. } => {
                    for case in cases {
                        collect_signals_from_nodes(&case.1, out);
                    }
                    if let Some(d) = default {
                        collect_signals_from_nodes(d, out);
                    }
                }
            },
            Node::Text(_) | Node::ContentSlot => {}
        }
    }
}

/// Get completion items for a position in a Hudl document
pub fn get_completions(content: &str, position: Position) -> Vec<CompletionItem> {
    let lines: Vec<&str> = content.lines().collect();
    let line_idx = position.line as usize;
    if line_idx >= lines.len() {
        return Vec::new();
    }

    let line = lines[line_idx];
    let col = position.character as usize;
    let before_cursor = if col <= line.len() { &line[..col] } else { line };

    // Detect context: inside tilde block?
    let in_tilde_block = is_in_tilde_block(content, position.line);

    // Check for action completion after @
    if before_cursor.ends_with('@') || before_cursor.contains("@") {
        let last_at = before_cursor.rfind('@');
        if let Some(_) = last_at {
            return action_completions();
        }
    }

    // Check for signal completion after $
    if before_cursor.ends_with('$') || (before_cursor.contains('$') && !before_cursor.ends_with(' ')) {
        // Parse document for signals
        if let Ok(doc) = hudlc::parser::parse(content) {
            if let Ok(root) = hudlc::transformer::transform(&doc) {
                let signals = collect_signals(&root);
                return signals.into_iter().map(|name| CompletionItem {
                    label: format!("${}", name),
                    kind: Some(CompletionItemKind::VARIABLE),
                    detail: Some("Datastar signal".to_string()),
                    insert_text: Some(name),
                    ..Default::default()
                }).collect();
            }
        }
        return Vec::new();
    }

    // Inside a tilde block: suggest attribute names
    if in_tilde_block {
        return attribute_completions();
    }

    // After ~ character: suggest attribute names
    if before_cursor.trim_start().starts_with('~') || before_cursor.ends_with('~') {
        return attribute_completions();
    }

    Vec::new()
}

/// Check if a line is inside a tilde block by scanning for `~ {` / `}` context
fn is_in_tilde_block(content: &str, target_line: u32) -> bool {
    let mut in_tilde = false;
    let mut brace_depth = 0;

    for (line_num, line) in content.lines().enumerate() {
        if line_num as u32 > target_line {
            break;
        }

        let trimmed = line.trim();

        if trimmed.starts_with("~ {") || trimmed == "~{" {
            in_tilde = true;
            brace_depth = 1;
            continue;
        }

        if in_tilde {
            for ch in trimmed.chars() {
                if ch == '{' {
                    brace_depth += 1;
                } else if ch == '}' {
                    brace_depth -= 1;
                    if brace_depth == 0 {
                        in_tilde = false;
                        break;
                    }
                }
            }
        }
    }

    in_tilde
}

/// Completions for Datastar attribute names (inside tilde blocks)
fn attribute_completions() -> Vec<CompletionItem> {
    let attrs = [
        ("on:click", "Event handler for click", "on:click"),
        ("on:submit", "Event handler for form submit", "on:submit"),
        ("on:input", "Event handler for input", "on:input"),
        ("on:change", "Event handler for change", "on:change"),
        ("on:keydown", "Event handler for keydown", "on:keydown"),
        ("on:keyup", "Event handler for keyup", "on:keyup"),
        ("on:load", "Event handler for load", "on:load"),
        ("on:intersect", "Intersection observer trigger", "on:intersect"),
        ("on:fetch", "Fetch event handler", "on:fetch"),
        ("show", "Conditionally show/hide element", "show"),
        ("text", "Set element text content", "text"),
        ("ref", "Element reference", "ref"),
        ("bind", "Two-way data binding", "bind"),
        ("let:", "Define a signal", "let:"),
        ("persist", "Persist signals to storage", "persist"),
        ("teleport", "Move element to another location", "teleport"),
        ("scrollIntoView", "Scroll element into view", "scrollIntoView"),
        (".class", "Toggle CSS class", "."),
        ("class:", "Toggle CSS class (alt syntax)", "class:"),
    ];

    attrs.iter().map(|(label, detail, insert)| CompletionItem {
        label: label.to_string(),
        kind: Some(CompletionItemKind::PROPERTY),
        detail: Some(detail.to_string()),
        insert_text: Some(insert.to_string()),
        ..Default::default()
    }).collect()
}

/// Completions for Datastar actions (after @)
fn action_completions() -> Vec<CompletionItem> {
    let actions = [
        ("@get", "HTTP GET request"),
        ("@post", "HTTP POST request"),
        ("@put", "HTTP PUT request"),
        ("@patch", "HTTP PATCH request"),
        ("@delete", "HTTP DELETE request"),
        ("@setAll", "Set all matching signals"),
        ("@toggleAll", "Toggle all matching signals"),
        ("@fit", "Fit element"),
        ("@peek", "Peek at signal value"),
        ("@clipboard", "Copy to clipboard"),
    ];

    actions.iter().map(|(label, detail)| CompletionItem {
        label: label.to_string(),
        kind: Some(CompletionItemKind::FUNCTION),
        detail: Some(detail.to_string()),
        insert_text: Some(label[1..].to_string()), // Insert without @
        ..Default::default()
    }).collect()
}

/// Find ranges of lines that are inside tilde blocks (for semantic tokens)
pub fn find_tilde_block_ranges(content: &str) -> Vec<(u32, u32)> {
    let mut ranges = Vec::new();
    let mut in_tilde = false;
    let mut start_line = 0u32;
    let mut brace_depth = 0;

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        if !in_tilde && (trimmed.starts_with("~ {") || trimmed == "~{") {
            in_tilde = true;
            start_line = line_num as u32;
            brace_depth = 1;
            continue;
        }

        if in_tilde {
            for ch in trimmed.chars() {
                if ch == '{' {
                    brace_depth += 1;
                } else if ch == '}' {
                    brace_depth -= 1;
                    if brace_depth == 0 {
                        ranges.push((start_line, line_num as u32));
                        in_tilde = false;
                        break;
                    }
                }
            }
        }
    }

    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_known_attrs() {
        assert!(classify_attr("on:click").is_some());
        assert!(classify_attr("on:intersect").is_some());
        assert!(classify_attr("on:fetch").is_some());
        assert!(classify_attr("let:count").is_some());
        assert!(classify_attr("show").is_some());
        assert!(classify_attr("text").is_some());
        assert!(classify_attr("ref").is_some());
        assert!(classify_attr("bind").is_some());
        assert!(classify_attr("persist").is_some());
        assert!(classify_attr("teleport").is_some());
        assert!(classify_attr("scrollIntoView").is_some());
        assert!(classify_attr(".active").is_some());
        assert!(classify_attr("class:active").is_some());
    }

    #[test]
    fn test_classify_unknown_attr() {
        assert!(classify_attr("foo").is_none());
        assert!(classify_attr("banana").is_none());
    }

    #[test]
    fn test_modifier_matches_exact() {
        assert!(modifier_matches("once", "once"));
        assert!(!modifier_matches("once", "prevent"));
    }

    #[test]
    fn test_modifier_matches_wildcard() {
        assert!(modifier_matches("debounce:300ms", "debounce:*"));
        assert!(modifier_matches("debounce", "debounce:*"));
        assert!(!modifier_matches("throttle:100ms", "debounce:*"));
    }

    #[test]
    fn test_modifier_matches_dot_wildcard() {
        assert!(modifier_matches("header.X-Custom", "header.*"));
        assert!(!modifier_matches("swap:morph", "header.*"));
    }

    #[test]
    fn test_validate_unknown_attr() {
        let content = r#"el {
    div {
        ~ {
            foo "bar"
        }
    }
}"#;
        let diags = validate_datastar_attrs(content);
        assert!(diags.iter().any(|d| d.message.contains("Unknown Datastar attribute '~foo'")));
    }

    #[test]
    fn test_validate_valid_attr_no_warning() {
        let content = r#"el {
    div {
        ~ {
            on:click "$count++"
        }
    }
}"#;
        let diags = validate_datastar_attrs(content);
        assert!(diags.is_empty(), "Expected no diagnostics, got: {:?}", diags);
    }

    #[test]
    fn test_validate_invalid_modifier() {
        let content = r#"el {
    div {
        ~ {
            show~bogus "$visible"
        }
    }
}"#;
        let diags = validate_datastar_attrs(content);
        assert!(diags.iter().any(|d| d.message.contains("Unknown modifier '~bogus'")));
    }

    #[test]
    fn test_validate_valid_modifier() {
        let content = r#"el {
    div {
        ~ {
            on:click~once~prevent "$doThing()"
        }
    }
}"#;
        let diags = validate_datastar_attrs(content);
        assert!(diags.is_empty(), "Expected no diagnostics, got: {:?}", diags);
    }

    #[test]
    fn test_collect_signals() {
        let content = r#"el {
    div {
        ~ {
            let:count 0
            let:name "'hello'"
        }
    }
}"#;
        let doc = hudlc::parser::parse(content).unwrap();
        let root = hudlc::transformer::transform(&doc).unwrap();
        let signals = collect_signals(&root);
        assert_eq!(signals, vec!["count", "name"]);
    }

    #[test]
    fn test_tilde_block_ranges() {
        let content = "div {\n    ~ {\n        on:click \"x\"\n    }\n}";
        let ranges = find_tilde_block_ranges(content);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0], (1, 3));
    }

    #[test]
    fn test_attribute_completions_in_tilde_block() {
        let content = "el {\n    div {\n        ~ {\n            \n        }\n    }\n}";
        let completions = get_completions(content, Position { line: 3, character: 12 });
        assert!(!completions.is_empty());
        let labels: Vec<&str> = completions.iter().map(|c| c.label.as_str()).collect();
        assert!(labels.contains(&"on:click"));
        assert!(labels.contains(&"show"));
        assert!(labels.contains(&"let:"));
    }

    #[test]
    fn test_action_completions_after_at() {
        let content = "el {\n    div {\n        ~ {\n            on:click \"@\"\n        }\n    }\n}";
        // Simulate cursor right after @
        let completions = get_completions(content, Position { line: 3, character: 25 });
        let labels: Vec<&str> = completions.iter().map(|c| c.label.as_str()).collect();
        assert!(labels.contains(&"@get"));
        assert!(labels.contains(&"@post"));
    }
}
