use hudlc::parser;
use hudlc::transformer;
use hudlc::codegen_cel;
use hudlc::proto::ProtoSchema;

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
    div#main.container {
        span.text-bold "Selectors"
    }
}
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform(&doc).expect("Failed to transform");

    let el = root.nodes[0].as_element().unwrap();
    assert_eq!(el.tag, "div");
    assert_eq!(el.id, Some("main".to_string()));
    assert_eq!(el.classes, vec!["container".to_string()]);

    let span = el.children[0].as_element().unwrap();
    assert_eq!(span.tag, "span");
    assert_eq!(span.classes, vec!["text-bold".to_string()]);
}

#[test]
fn test_attributes() {
    let input = r#"
el {
    input type=text name=username placeholder="Enter username"
}
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform(&doc).expect("Failed to transform");

    let el = root.nodes[0].as_element().unwrap();
    assert_eq!(el.attributes.get("type").unwrap(), "text");
    assert_eq!(el.attributes.get("name").unwrap(), "username");
    assert_eq!(el.attributes.get("placeholder").unwrap(), "Enter username");
}

#[test]
fn test_element_style_transformation() {
    let input = r#"
el {
    div {
        style {
            color "red"
            margin-top 10px
        }
        "Styled div"
    }
}
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform(&doc).expect("Failed to transform");

    let el = root.nodes[0].as_element().unwrap();
    assert_eq!(el.styles.len(), 2);
    assert_eq!(el.styles[0], ("color".to_string(), "red".to_string()));
    assert_eq!(el.styles[1], ("margin-top".to_string(), "10px".to_string()));
}

#[test]
fn test_scoped_css_transformation() {
    let input = r#"
el {
    css {
        .card { background "white"; }
        #header { border-bottom "1px solid black"; }
    }
}
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform(&doc).expect("Failed to transform");

    let css = root.css.expect("CSS should be extracted");
    assert!(css.contains(".card { background: white; }"));
    assert!(css.contains("#header { border-bottom: 1px solid black; }"));
}

#[test]
fn test_control_flow_if_with_cel() {
    let input = r#"
el {
    if `user.is_admin` {
        div "Admin Panel"
    }
}
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform(&doc).expect("Failed to transform");

    let cf = root.nodes[0].as_control_flow().expect("Should be control flow");
    if let hudlc::ast::ControlFlow::If { condition, .. } = cf {
        assert_eq!(condition, "user.is_admin");
    } else {
        panic!("Expected If node");
    }
}

#[test]
fn test_control_flow_each_with_cel() {
    let input = r#"
el {
    each item `users` {
        li `item.name`
    }
}
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform(&doc).expect("Failed to transform");

    let cf = root.nodes[0].as_control_flow().expect("Should be control flow");
    if let hudlc::ast::ControlFlow::Each { binding, iterable, .. } = cf {
        assert_eq!(binding, "item");
        assert_eq!(iterable, "users");
    } else {
        panic!("Expected Each node");
    }
}

#[test]
fn test_transform_nested_if_else() {
    let input = r#"
el {
    if `a` {
        "then"
    } else {
        if `b` {
            "else if"
        } else {
            "else"
        }
    }
}
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform(&doc).expect("Failed to transform");

    assert_eq!(root.nodes.len(), 1);
    let cf = root.nodes[0].as_control_flow().unwrap();
    if let hudlc::ast::ControlFlow::If { else_block, .. } = cf {
        let else_nodes = else_block.as_ref().unwrap();
        assert_eq!(else_nodes.len(), 1);
        assert!(else_nodes[0].as_control_flow().is_some());
    } else {
        panic!("Expected If node");
    }
}

#[test]
fn test_component_metadata() {
    let input = r#"
// name: UserCard
// data: UserProfile

el { div "User" }
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform_with_metadata(&doc, input).expect("Failed to transform");

    assert_eq!(root.name, Some("UserCard".to_string()));
    assert_eq!(root.data_type, Some("UserProfile".to_string()));
}

#[test]
fn test_codegen_basic() {
    let input = r#"
el {
    div "Hello"
}
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform(&doc).expect("Failed to transform");
    let views = vec![("TestView".to_string(), root)];
    let schema = ProtoSchema::default();
    let rust_code = codegen_cel::generate_wasm_lib_cel(views, &schema).expect("Codegen failed");

    assert!(rust_code.contains("fn render_testview"));
    assert!(rust_code.contains(".push_str(\"<div\")"));
    assert!(rust_code.contains("Hello"));
}

#[test]
fn test_codegen_with_cel_expression() {
    let input = r#"
el {
    div `user.name`
}
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform(&doc).expect("Failed to transform");
    let views = vec![("TestView".to_string(), root)];
    let schema = ProtoSchema::default();
    let rust_code = codegen_cel::generate_wasm_lib_cel(views, &schema).expect("Codegen failed");

    assert!(rust_code.contains("cel_eval_safe(\"user.name\""));
}

#[test]
fn test_codegen_boolean_attribute() {
    let input = r#"
el {
    button disabled=`!is_valid` "Submit"
}
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform(&doc).expect("Failed to transform");
    let views = vec![("TestView".to_string(), root)];
    let schema = ProtoSchema::default();
    let rust_code = codegen_cel::generate_wasm_lib_cel(views, &schema).expect("Codegen failed");

    assert!(rust_code.contains("if cel_truthy(&cel_eval(\"!is_valid\""));
    assert!(rust_code.contains(".push_str(\" disabled\")"));
}

#[test]
fn test_codegen_multi_interpolation() {
    let input = r#"
el {
    span "Hello `first` `last`!"
}
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform(&doc).expect("Failed to transform");
    let views = vec![("TestView".to_string(), root)];
    let schema = ProtoSchema::default();
    let rust_code = codegen_cel::generate_wasm_lib_cel(views, &schema).expect("Codegen failed");

    assert!(rust_code.contains("Hello "));
    assert!(rust_code.contains("cel_eval_safe(\"first\""));
    assert!(rust_code.contains(" "));
    assert!(rust_code.contains("cel_eval_safe(\"last\""));
    assert!(rust_code.contains("!"));
}

#[test]
fn test_codegen_scoped_css() {
    let input = r#"
el {
    div {
        style { color "red" }
        "Text"
    }
}
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform_with_metadata(&doc, input).expect("Failed to transform");
    let views = vec![("Card".to_string(), root)];
    let schema = ProtoSchema::default();
    let rust_code = codegen_cel::generate_wasm_lib_cel(views, &schema).expect("Codegen failed");

    // Should contain a generated scope class
    assert!(rust_code.contains(".h-"));
    assert!(rust_code.contains("{ color: red }"));
}

#[test]
fn test_proto_block_extraction() {
    let input = r#"
/**
message User {
    string name = 1;
}
*/
// name: MyComp
el { div "ok" }
    "#;

    let schema = ProtoSchema::from_template(input, None).expect("Failed to parse with proto block");
    assert!(schema.messages.contains_key("User"));
}

#[test]
fn test_codegen_switch_case() {
    let input = r#"
el {
    switch `status` {
        case "ACTIVE" { div "active" }
        default { div "other" }
    }
}
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform(&doc).expect("Failed to transform");
    let views = vec![("TestView".to_string(), root)];
    let schema = ProtoSchema::default();
    let rust_code = codegen_cel::generate_wasm_lib_cel(views, &schema).expect("Codegen failed");

    assert!(rust_code.contains("let _switch_val = cel_eval(\"status\""));
    assert!(rust_code.contains("if cel_to_string(&_switch_val) == \"ACTIVE\""));
}

#[test]
fn test_control_flow_switch_with_enum() {
    let input = r#"
el {
    switch `status` {
        case STATUS_ACTIVE { div "Active" }
    }
}
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform(&doc).expect("Failed to transform");

    let cf = root.nodes[0].as_control_flow().unwrap();
    if let hudlc::ast::ControlFlow::Switch { cases, .. } = cf {
        assert_eq!(cases[0].0, "STATUS_ACTIVE");
    } else {
        panic!("Expected Switch node");
    }
}

#[test]
fn test_codegen_each_with_index() {
    let input = r#"
el {
    each item `items` {
        span `item_idx`
    }
}
    "#;

    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform(&doc).expect("Failed to transform");
    let views = vec![("TestView".to_string(), root)];
    let schema = ProtoSchema::default();
    let rust_code = codegen_cel::generate_wasm_lib_cel(views, &schema).expect("Codegen failed");

        assert!(rust_code.contains("for (_idx, _item) in list.iter().enumerate()"));
        assert!(rust_code.contains("let _ = loop_ctx.add_variable(\"item_idx\""));
    }

    

    #[test]

    fn test_codegen_slots() {

        let layout_input = r#"

    // name: Layout

    el {

        main { #content }

    }

        "#;

        let page_input = r#"

    import { "./layout" }

    // name: Page

    el {

        Layout {

            div "Inner"

        }

    }

        "#;

    

        let doc_l = parser::parse(layout_input).unwrap();

        let root_l = transformer::transform(&doc_l).unwrap();

        

        let doc_p = parser::parse(page_input).unwrap();

        let root_p = transformer::transform(&doc_p).unwrap();

    

        let views = vec![

            ("Layout".to_string(), root_l),

            ("Page".to_string(), root_p)

        ];

        let schema = ProtoSchema::default();

        let rust_code = codegen_cel::generate_wasm_lib_cel(views, &schema).expect("Codegen failed");

    

        // Layout should use content_html parameter

        assert!(rust_code.contains("r.push_str(content_html)"));

        // Page should render children and pass to Layout

        assert!(rust_code.contains("let mut invocation_content = String::new()"));

        assert!(rust_code.contains("render_layout(r, proto_data, &invocation_content)"));

    }

    