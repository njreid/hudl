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

    assert_datastar_attr(el, "bind", Some("username"), &[]);
}

// 2. Event Modifiers: ~stop, ~capture
#[test]
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

    assert_datastar_attr(el, "on:click", Some("handleClick()"), &["stop", "capture"]);
}

// 3. Intersect Modifier: ~full
#[test]
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

    assert_datastar_attr(el, "on:intersect", Some("visible()"), &["full"]);
}

// 4. Teleport Modifier: ~append
#[test]
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

    assert_datastar_attr(el, "teleport", Some("#target"), &["append"]);
}

// 5. ScrollIntoView Modifiers: instant, hstart, hcenter, hend, vstart, vend
#[test]
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

    assert_datastar_attr(el, "scrollIntoView", None, &["instant", "hcenter", "vend"]);
}

// 6. Complex Modifiers (e.g. headers)
#[test]
fn test_complex_modifiers() {
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

    // The modifier "header.X-Custom:value" is stored as-is
    assert_datastar_attr(el, "on:click", Some("fetch()"), &["header.X-Custom:value"]);
}
