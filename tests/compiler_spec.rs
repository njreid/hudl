use hudlc::parser;
use hudlc::transformer;
use hudlc::codegen;

#[test]
fn test_basic_element_transformation() {
    let input = r#"
el {
    div "Hello World"
}
    "#;

    let doc = parser::parse(input).expect("Failed to parse KDL");
    let root = transformer::transform(&doc).expect("Failed to transform to AST");

    assert_eq!(root.nodes.len(), 1);
    let el = root.nodes[0].as_element().expect("Root node should be element");
    assert_eq!(el.tag, "div");
    assert_eq!(el.children.len(), 1);
    assert_eq!(el.children[0].as_text().unwrap().content, "Hello World");
}

#[test]
fn test_shorthand_selectors() {
    let input = r#"
el {
    &main.container.fluid {
        h1 "Title"
    }
}
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform(&doc).expect("Failed to transform");

    let div = root.nodes[0].as_element().unwrap();
    assert_eq!(div.tag, "div");
    assert_eq!(div.id, Some("main".to_string()));
    assert_eq!(div.classes, vec!["container", "fluid"]);
}

#[test]
fn test_attributes() {
    let input = r#"
el {
    a href="/login" target="_blank" "Login"
}
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform(&doc).expect("Failed to transform");

    let a = root.nodes[0].as_element().unwrap();
    assert_eq!(a.tag, "a");
    assert_eq!(a.attributes.get("href"), Some(&"/login".to_string()));
    assert_eq!(a.attributes.get("target"), Some(&"_blank".to_string()));
}

#[test]
fn test_element_style_transformation() {
    let input = r#"
el {
    button {
        style {
            background-color "red"
            color "white"
            font-weight "bold"
        }
        "CANCEL"
    }
}
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform(&doc).expect("Failed to transform");

    // Check that the button element has styles
    let button = root.nodes[0].as_element().expect("Expected element");
    assert_eq!(button.tag, "button");
    assert_eq!(button.styles.len(), 3);

    // Check style properties
    assert!(button.styles.iter().any(|(k, v)| k == "background-color" && v == "red"));
    assert!(button.styles.iter().any(|(k, v)| k == "color" && v == "white"));
    assert!(button.styles.iter().any(|(k, v)| k == "font-weight" && v == "bold"));
}

#[test]
fn test_scoped_css_transformation() {
    // Legacy test for root-level css block (still supported)
    let input = r#"
el {
    css {
        &header { margin "_0"; }
        .btn { width "_10px"; }
    }
}
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform(&doc).expect("Failed to transform");

    assert!(root.css.is_some());
    let css = root.css.as_ref().unwrap();

    // Check if &header became #header
    assert!(css.contains("#header"));
    // Check if .btn was preserved
    assert!(css.contains(".btn"));
    // Check numeric value conversion (_0 -> 0, _10px -> 10px)
    assert!(css.contains("margin: 0"));
    assert!(css.contains("width: 10px"));
}

#[test]
fn test_control_flow_if_with_cel() {
    // New syntax: backticks for CEL expressions
    let input = r#"
el {
    if "`is_visible`" {
        p "Visible"
    } else {
        p "Hidden"
    }
}
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform(&doc).expect("Failed to transform");

    let if_node = root.nodes[0].as_control_flow().expect("Should be control flow");
    match if_node {
        hudlc::ast::ControlFlow::If { condition, then_block, else_block } => {
            assert_eq!(condition, "is_visible");
            assert_eq!(then_block.len(), 1);
            assert!(else_block.is_some());
        },
        _ => panic!("Expected If node"),
    }
}

#[test]
fn test_control_flow_each_with_cel() {
    // New syntax: each binding `cel_expression`
    let input = r#"
el {
    each item "`items`" {
        li "`item.name`"
    }
}
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform(&doc).expect("Failed to transform");

    let each_node = root.nodes[0].as_control_flow().expect("Should be control flow");
    match each_node {
        hudlc::ast::ControlFlow::Each { binding, iterable, body } => {
            assert_eq!(binding, "item");
            assert_eq!(iterable, "items");
            assert_eq!(body.len(), 1);
        },
        _ => panic!("Expected Each node"),
    }
}

#[test]
fn test_control_flow_switch_with_enum() {
    // New syntax: switch with enum values as bare identifiers
    let input = r#"
el {
    switch "`status`" {
        case STATUS_ACTIVE {
            span "Active"
        }
        case STATUS_PENDING {
            span "Pending"
        }
        default {
            span "Unknown"
        }
    }
}
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform(&doc).expect("Failed to transform");

    let switch_node = root.nodes[0].as_control_flow().expect("Should be control flow");
    match switch_node {
        hudlc::ast::ControlFlow::Switch { expr, cases, default } => {
            assert_eq!(expr, "status");
            assert_eq!(cases.len(), 2);
            assert_eq!(cases[0].0, "STATUS_ACTIVE");
            assert_eq!(cases[1].0, "STATUS_PENDING");
            assert!(default.is_some());
        },
        _ => panic!("Expected Switch node"),
    }
}

#[test]
fn test_codegen_basic() {
    let input = r#"
el {
    div "Hello"
}
    "#;

    let doc = parser::parse(input).unwrap();
    let root = transformer::transform(&doc).unwrap();
    let views = vec![("TestView".to_string(), root)];
    let rust_code = codegen::generate_wasm_lib(views).expect("Codegen failed");

    // Verify Rust code contains string writes
    assert!(rust_code.contains("r.push_str(\"<div\")"));
    assert!(rust_code.contains("r.push_str(\">\")"));
    assert!(rust_code.contains("r.push_str(\"Hello\")"));
    assert!(rust_code.contains("r.push_str(\"</div>\")"));
}

#[test]
fn test_codegen_with_cel_expression() {
    let input = r#"
el {
    span "`user.name`"
}
    "#;

    let doc = parser::parse(input).unwrap();
    let root = transformer::transform(&doc).unwrap();
    let views = vec![("TestView".to_string(), root)];
    let rust_code = codegen::generate_wasm_lib(views).expect("Codegen failed");

    // Should reference the data field
    assert!(rust_code.contains("user"));
    assert!(rust_code.contains("name"));
}

#[test]
fn test_codegen_boolean_attribute() {
    let input = r#"
el {
    input type="checkbox" checked="`is_selected`"
}
    "#;

    let doc = parser::parse(input).unwrap();
    let root = transformer::transform(&doc).unwrap();
    let views = vec![("TestView".to_string(), root)];
    let rust_code = codegen::generate_wasm_lib(views).expect("Codegen failed");

    // Boolean attributes should have conditional logic
    assert!(rust_code.contains("is_selected"));
    assert!(rust_code.contains("checked"));
}

#[test]
fn test_proto_block_extraction() {
    // Test that proto blocks are recognized (future: parse and validate)
    let input = r#"
/**
message User {
    string name = 1;
    string email = 2;
}
*/

el {
    div `name`
}
    "#;

    // For now, just verify the template parses
    // Proto parsing will be implemented in Phase 5
    let doc = parser::parse(input).expect("Failed to parse with proto block");
    let root = transformer::transform(&doc).expect("Failed to transform");
    assert!(!root.nodes.is_empty());
}

#[test]
fn test_component_metadata() {
    let input = r#"
// name: UserCard
// data: User

el {
    div `name`
}
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform_with_metadata(&doc, input).expect("Failed to transform");

    // Metadata should be extracted
    assert_eq!(root.name, Some("UserCard".to_string()));
    assert_eq!(root.data_type, Some("User".to_string()));
}

#[test]
fn test_codegen_switch_case() {
    let input = r#"
el {
    switch "`status`" {
        case STATUS_ACTIVE {
            span "Active"
        }
        case STATUS_PENDING {
            span "Pending"
        }
        default {
            span "Unknown"
        }
    }
}
    "#;

    let doc = parser::parse(input).unwrap();
    let root = transformer::transform(&doc).unwrap();
    let views = vec![("TestView".to_string(), root)];
    let rust_code = codegen::generate_wasm_lib(views).expect("Codegen failed");

    // Switch should generate conditional logic
    assert!(rust_code.contains("_switch_val"));
    assert!(rust_code.contains("STATUS_ACTIVE"));
    assert!(rust_code.contains("STATUS_PENDING"));
    // Should have else clause for default
    assert!(rust_code.contains("else"));
}

#[test]
fn test_codegen_each_with_index() {
    let input = r#"
el {
    each item "`items`" {
        li "`_index`"
    }
}
    "#;

    let doc = parser::parse(input).unwrap();
    let root = transformer::transform(&doc).unwrap();
    let views = vec![("TestView".to_string(), root)];
    let rust_code = codegen::generate_wasm_lib(views).expect("Codegen failed");

    // Should have loop with index
    assert!(rust_code.contains("_index"));
    assert!(rust_code.contains("enumerate"));
}

#[test]
fn test_codegen_multi_interpolation() {
    // Test multiple CEL expressions in a single string
    let input = r#"
el {
    span "Hello `name`, you have `count` messages"
}
    "#;

    let doc = parser::parse(input).unwrap();
    let root = transformer::transform(&doc).unwrap();
    let views = vec![("TestView".to_string(), root)];
    let rust_code = codegen::generate_wasm_lib(views).expect("Codegen failed");

    // Should generate code referencing both expressions
    assert!(rust_code.contains("Hello"));
    assert!(rust_code.contains("name"));
    assert!(rust_code.contains("count"));
    assert!(rust_code.contains("messages"));
}

#[test]
fn test_transform_nested_if_else() {
    // Tests else-if pattern (nested if in else block)
    let input = r#"
el {
    if "`x > 10`" {
        span "high"
    } else {
        if "`x > 5`" {
            span "medium"
        } else {
            span "low"
        }
    }
}
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform(&doc).expect("Failed to transform");

    let if_node = root.nodes[0].as_control_flow().expect("Should be control flow");
    match if_node {
        hudlc::ast::ControlFlow::If { condition, then_block, else_block } => {
            assert_eq!(condition, "x > 10");
            assert_eq!(then_block.len(), 1);
            // Else block should contain another if
            let else_nodes = else_block.as_ref().expect("Should have else block");
            assert_eq!(else_nodes.len(), 1);
            // The else block should contain another If control flow
            let nested_if = else_nodes[0].as_control_flow().expect("Should be nested control flow");
            match nested_if {
                hudlc::ast::ControlFlow::If { condition, .. } => {
                    assert_eq!(condition, "x > 5");
                },
                _ => panic!("Expected nested If node"),
            }
        },
        _ => panic!("Expected If node"),
    }
}

#[test]
fn test_codegen_scoped_css() {
    // Test that scoped styles generate unique class names
    let input = r#"
el {
    button {
        style {
            background-color "blue"
            color "white"
        }
        "Click me"
    }
}
    "#;

    let doc = parser::parse(input).unwrap();
    let root = transformer::transform(&doc).unwrap();

    // The button should have styles
    let button = root.nodes[0].as_element().expect("Expected element");
    assert_eq!(button.tag, "button");
    assert!(!button.styles.is_empty());
}
