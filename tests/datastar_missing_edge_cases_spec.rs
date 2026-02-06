/// Datastar Integration Tests - Missing Edge Cases
///
/// These tests cover edge cases described in DATASTAR_DESIGN.md and docs.md
/// that were missing from the main datastar_spec.rs.

use hudlc::parser;
use hudlc::transformer;
use hudlc::ast::DatastarAttr;

fn parse_and_transform(input: &str) -> hudlc::ast::Root {
    let doc = parser::parse(input).expect("Failed to parse");
    transformer::transform(&doc).expect("Failed to transform")
}

fn get_first_element(root: &hudlc::ast::Root) -> &hudlc::ast::Element {
    root.nodes[0].as_element().expect("Expected element")
}

fn find_datastar_attr<'a>(el: &'a hudlc::ast::Element, name: &str) -> Option<&'a DatastarAttr> {
    el.datastar.iter().find(|attr| attr.name == name)
}

fn assert_datastar_attr(el: &hudlc::ast::Element, name: &str, expected_value: Option<&str>, expected_mods: &[&str]) {
    let attr = find_datastar_attr(el, name)
        .unwrap_or_else(|| panic!("Expected datastar attr '{}' not found. Found: {:?}", name, el.datastar));

    assert_eq!(attr.value.as_deref(), expected_value,
        "Datastar attr '{}' value mismatch", name);

    let expected_mods: Vec<String> = expected_mods.iter().map(|s| s.to_string()).collect();
    assert_eq!(attr.modifiers, expected_mods,
        "Datastar attr '{}' modifiers mismatch", name);
}

// 1. Explicit bind inside tilde block
#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_explicit_bind_in_block() {
    let input = r#"
el {
    input {
        ~ {
            bind username
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-bind"),
        Some(&"username".to_string())
    );
}

// 2. Event Modifiers: ~stop, ~capture
#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_event_modifiers_stop_capture() {
    let input = r#"
el {
    button {
        ~ {
            on:click~stop~capture "handleClick()"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-on-click__stop__capture"),
        Some(&"handleClick()".to_string())
    );
}

// 3. Intersect Modifier: ~full
#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_intersect_full() {
    let input = r#"
el {
    div {
        ~ {
            on:intersect~full "visible()"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-on-intersect__full"),
        Some(&"visible()".to_string())
    );
}

// 4. Teleport Modifier: ~append
#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_teleport_append() {
    let input = r##"
el {
    div {
        ~ {
            teleport~append "#target"
        }
    }
}
    "##;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-teleport__append"),
        Some(&"#target".to_string())
    );
}

// 5. ScrollIntoView Modifiers: instant, hstart, hcenter, hend, vstart, vend
#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_scroll_into_view_modifiers() {
    let input = r#"
el {
    div {
        ~ {
            scrollIntoView~instant~hcenter~vend
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert!(el.attributes.contains_key("data-scroll-into-view__instant__hcenter__vend"));
}

// 6. Complex Modifiers (e.g. headers)
// Assuming ~header.X-Custom:value maps to __header.X-Custom.value or similar
#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_complex_modifiers() {
    // This tests the parser's ability to handle dotted modifiers AND value modifiers on the same attribute
    // on:click~header.X-Custom:value
    let input = r#"
el {
    button {
        ~ {
            on:click~header.X-Custom:value "fetch()"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    // Exact output format depends on implementation, but checking for the components
    let key = el.attributes.keys().find(|k| k.starts_with("data-on-click")).expect("Attribute not found");
    assert!(key.contains("header"));
    assert!(key.contains("X-Custom"));
    assert!(key.contains("value"));
}
