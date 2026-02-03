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
fn test_scoped_css_transformation() {
    let input = r#" 
el {
        css {
            &header { margin _0; }
            .btn { width _10px; }
        }
    } 
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform(&doc).expect("Failed to transform");

    // We expect the CSS block to be extracted or processed
    // For now, let's assume it becomes a special node or metadata
    assert!(root.css.is_some());
    let css = root.css.as_ref().unwrap();
    
    // Check if &header became #header
    assert!(css.contains("#header"));
    // Check if .btn was preserved (with scoping suffix, handled in codegen mostly, but transform prepares it)
    assert!(css.contains(".btn"));
    // Check numeric value conversion (_0 -> 0, _10px -> 10px)
    assert!(css.contains("margin: 0"));
    assert!(css.contains("width: 10px"));
}

#[test]
fn test_control_flow_if() {
    let input = r#" 
el {
        if "`show`" {
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
            assert_eq!(condition, "show");
            assert_eq!(then_block.len(), 1);
            assert!(else_block.is_some());
        },
        _ => panic!("Expected If node"),
    }
}

#[test]
fn test_codegen_basic() {
    let input = r#" 
el {
        div "Hello"
    } 
    "#;
    
    // End-to-end simulation
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
