/// Datastar Integration Tests
///
/// These tests verify that Hudl's tilde (~) syntax correctly generates
/// Datastar-compliant HTML attributes. See DATASTAR_DESIGN.md for the
/// full specification.

use hudlc::parser;
use hudlc::transformer;
use hudlc::ast::DatastarAttr;

// =============================================================================
// Helper functions
// =============================================================================

fn parse_and_transform(input: &str) -> hudlc::ast::Root {
    let doc = parser::parse(input).expect("Failed to parse");
    transformer::transform(&doc).expect("Failed to transform")
}

fn get_first_element(root: &hudlc::ast::Root) -> &hudlc::ast::Element {
    root.nodes[0].as_element().expect("Expected element")
}

/// Find a datastar attribute by its name prefix (e.g., "on:click", ".active", "let:count")
fn find_datastar_attr<'a>(el: &'a hudlc::ast::Element, name: &str) -> Option<&'a DatastarAttr> {
    el.datastar.iter().find(|attr| attr.name == name)
}

/// Assert a datastar attribute exists with expected value and modifiers
fn assert_datastar_attr(el: &hudlc::ast::Element, name: &str, expected_value: Option<&str>, expected_mods: &[&str]) {
    let attr = find_datastar_attr(el, name)
        .unwrap_or_else(|| panic!("Expected datastar attr '{}' not found. Found: {:?}", name, el.datastar));

    assert_eq!(attr.value.as_deref(), expected_value,
        "Datastar attr '{}' value mismatch", name);

    let expected_mods: Vec<String> = expected_mods.iter().map(|s| s.to_string()).collect();
    assert_eq!(attr.modifiers, expected_mods,
        "Datastar attr '{}' modifiers mismatch", name);
}

// =============================================================================
// SECTION 1: Binding Shorthand (~>)
// =============================================================================

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_binding_shorthand_basic() {
    // input~>signalName → data-bind="signalName"
    let input = r#"
el {
    input~>username
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(el.tag, "input");
    assert_eq!(el.attributes.get("data-bind"), Some(&"username".to_string()));
}

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_binding_shorthand_with_debounce() {
    // input~>searchQuery~debounce:300ms → data-bind__debounce.300ms="searchQuery"
    let input = r#"
el {
    input~>searchQuery~debounce:300ms
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(el.tag, "input");
    assert_eq!(
        el.attributes.get("data-bind__debounce.300ms"),
        Some(&"searchQuery".to_string())
    );
}

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_binding_shorthand_with_throttle() {
    let input = r#"
el {
    input~>value~throttle:100ms
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-bind__throttle.100ms"),
        Some(&"value".to_string())
    );
}

// =============================================================================
// SECTION 2: Inline Tilde Attributes
// =============================================================================

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_inline_tilde_on_click() {
    // button ~on:click="doSomething()" → data-on-click="doSomething()"
    let input = r#"
el {
    button ~on:click="doSomething()" Click
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(el.tag, "button");
    assert_eq!(
        el.attributes.get("data-on-click"),
        Some(&"doSomething()".to_string())
    );
}

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_inline_tilde_text() {
    // h1 ~text="Todo App" → data-text="Todo App"
    let input = r#"
el {
    h1 ~text="Todo App"
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(el.tag, "h1");
    assert_eq!(el.attributes.get("data-text"), Some(&"Todo App".to_string()));
}

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_inline_tilde_with_modifiers() {
    // button ~on:click~once~prevent="submit()" → data-on-click__once__prevent="submit()"
    let input = r#"
el {
    button ~on:click~once~prevent="submit()"
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-on-click__once__prevent"),
        Some(&"submit()".to_string())
    );
}

// =============================================================================
// SECTION 3: Tilde Block (Child Node)
// =============================================================================

#[test]
fn test_tilde_block_basic() {
    let input = r#"
el {
    div {
        ~ {
            on:click "handleClick()"
            show $isVisible
        }
        span Content
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(el.tag, "div");
    assert_datastar_attr(el, "on:click", Some("handleClick()"), &[]);
    assert_datastar_attr(el, "show", Some("$isVisible"), &[]);

    // Verify child is preserved
    assert_eq!(el.children.len(), 1);
    let child = el.children[0].as_element().expect("Expected span element");
    assert_eq!(child.tag, "span");
}

#[test]
fn test_tilde_block_class_shorthand() {
    // .active $isActive → data-class-active="$isActive"
    let input = r#"
el {
    button {
        ~ {
            .active $isSelected
            .disabled $isLoading
        }
        Submit
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_datastar_attr(el, ".active", Some("$isSelected"), &[]);
    assert_datastar_attr(el, ".disabled", Some("$isLoading"), &[]);
}

#[test]
fn test_tilde_block_class_long_form() {
    // class:active $isActive → data-class-active="$isActive"
    let input = r#"
el {
    button {
        ~ {
            class:active $isActive
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_datastar_attr(el, "class:active", Some("$isActive"), &[]);
}

// =============================================================================
// SECTION 4: Signals (let: with static values)
// =============================================================================

#[test]
fn test_signal_number() {
    // let:count 0 → data-signals-count="0"
    let input = r#"
el {
    div {
        ~ {
            let:count 0
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_datastar_attr(el, "let:count", Some("0"), &[]);
}

#[test]
fn test_signal_string() {
    // let:name hello → stored as-is, codegen wraps in quotes
    let input = r#"
el {
    div {
        ~ {
            let:name hello
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_datastar_attr(el, "let:name", Some("hello"), &[]);
}

#[test]
fn test_signal_boolean() {
    // let:active "true" → data-signals-active="true"
    // Note: KDL requires booleans to be quoted when used as bare values
    let input = r#"
el {
    div {
        ~ {
            let:active "true"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_datastar_attr(el, "let:active", Some("true"), &[]);
}

#[test]
fn test_signal_with_ifmissing_modifier() {
    // let:count~ifmissing 0 → data-signals-count__ifmissing="0"
    let input = r#"
el {
    div {
        ~ {
            let:count~ifmissing 0
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_datastar_attr(el, "let:count", Some("0"), &["ifmissing"]);
}

// =============================================================================
// SECTION 5: Computed Values (let: with expressions)
// =============================================================================

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_computed_with_operators() {
    // let:total "$price * $quantity" → data-computed-total="$price * $quantity"
    let input = r#"
el {
    div {
        ~ {
            let:total "$price * $quantity"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-computed-total"),
        Some(&"$price * $quantity".to_string())
    );
}

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_computed_with_function_call() {
    // let:upper "$name.toUpperCase()" → data-computed-upper="$name.toUpperCase()"
    let input = r#"
el {
    div {
        ~ {
            let:upper "$name.toUpperCase()"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-computed-upper"),
        Some(&"$name.toUpperCase()".to_string())
    );
}

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_computed_with_string_concat() {
    let input = r#"
el {
    div {
        ~ {
            let:fullName "$firstName + ' ' + $lastName"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-computed-fullName"),
        Some(&"$firstName + ' ' + $lastName".to_string())
    );
}

// =============================================================================
// SECTION 6: Event Handlers (on:)
// =============================================================================

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_on_click_expression() {
    let input = r#"
el {
    button {
        ~ {
            on:click "$count++"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-on-click"),
        Some(&"$count++".to_string())
    );
}

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_on_click_with_action() {
    let input = r#"
el {
    button {
        ~ {
            on:click "@get('/api/data')"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-on-click"),
        Some(&"@get('/api/data')".to_string())
    );
}

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_on_keydown_with_key_modifier() {
    // on:keydown.enter "submit()" → data-on-keydown.enter="submit()"
    let input = r#"
el {
    input {
        ~ {
            on:keydown.enter "submit()"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-on-keydown.enter"),
        Some(&"submit()".to_string())
    );
}

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_on_submit_prevent() {
    // on:submit~prevent → data-on-submit__prevent
    let input = r#"
el {
    form {
        ~ {
            on:submit~prevent "@post('/login')"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-on-submit__prevent"),
        Some(&"@post('/login')".to_string())
    );
}

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_on_click_outside() {
    let input = r#"
el {
    div {
        ~ {
            on:click~outside "$isOpen = false"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-on-click__outside"),
        Some(&"$isOpen = false".to_string())
    );
}

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_on_scroll_throttle_passive() {
    let input = r#"
el {
    div {
        ~ {
            on:scroll~throttle:50ms~passive "updatePosition()"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-on-scroll__throttle.50ms__passive"),
        Some(&"updatePosition()".to_string())
    );
}

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_on_resize_window() {
    let input = r#"
el {
    div {
        ~ {
            on:resize~window "handleResize()"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-on-resize__window"),
        Some(&"handleResize()".to_string())
    );
}

// =============================================================================
// SECTION 7: Text Content (text)
// =============================================================================

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_text_simple() {
    let input = r#"
el {
    span {
        ~ {
            text $message
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-text"),
        Some(&"$message".to_string())
    );
}

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_text_expression() {
    let input = r#"
el {
    span {
        ~ {
            text "$greeting + ', ' + $name"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-text"),
        Some(&"$greeting + ', ' + $name".to_string())
    );
}

// =============================================================================
// SECTION 8: Show/Hide (show)
// =============================================================================

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_show_simple() {
    let input = r#"
el {
    div {
        ~ {
            show $isVisible
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-show"),
        Some(&"$isVisible".to_string())
    );
}

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_show_expression() {
    let input = r#"
el {
    div {
        ~ {
            show "$count > 0"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-show"),
        Some(&"$count > 0".to_string())
    );
}

// =============================================================================
// SECTION 9: Dynamic HTML Attributes
// =============================================================================

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_dynamic_disabled() {
    let input = r#"
el {
    button {
        ~ {
            disabled $isLoading
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-attr-disabled"),
        Some(&"$isLoading".to_string())
    );
}

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_dynamic_href() {
    let input = r#"
el {
    a {
        ~ {
            href "'/user/' + $userId"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-attr-href"),
        Some(&"'/user/' + $userId".to_string())
    );
}

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_dynamic_target() {
    let input = r#"
el {
    a {
        ~ {
            target "$openInNew ? '_blank' : '_self'"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-attr-target"),
        Some(&"$openInNew ? '_blank' : '_self'".to_string())
    );
}

// =============================================================================
// SECTION 10: Persist
// =============================================================================

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_persist_all() {
    let input = r#"
el {
    div {
        ~ {
            persist
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert!(el.attributes.contains_key("data-persist"));
}

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_persist_specific() {
    let input = r#"
el {
    div {
        ~ {
            persist "theme,lang"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-persist"),
        Some(&"theme,lang".to_string())
    );
}

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_persist_session() {
    let input = r#"
el {
    div {
        ~ {
            persist~session userPrefs
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-persist__session"),
        Some(&"userPrefs".to_string())
    );
}

// =============================================================================
// SECTION 11: Element References (ref)
// =============================================================================

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_ref() {
    let input = r#"
el {
    input {
        ~ {
            ref emailInput
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-ref"),
        Some(&"emailInput".to_string())
    );
}

// =============================================================================
// SECTION 12: Intersection Observer (on:intersect)
// =============================================================================

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_on_intersect() {
    let input = r#"
el {
    div {
        ~ {
            on:intersect "$visible = true"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-on-intersect"),
        Some(&"$visible = true".to_string())
    );
}

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_on_intersect_once() {
    let input = r#"
el {
    div {
        ~ {
            on:intersect~once "@get('/lazy-content')"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-on-intersect__once"),
        Some(&"@get('/lazy-content')".to_string())
    );
}

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_on_intersect_half() {
    let input = r#"
el {
    div {
        ~ {
            on:intersect~half "$visible = true"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-on-intersect__half"),
        Some(&"$visible = true".to_string())
    );
}

// =============================================================================
// SECTION 13: Teleport
// =============================================================================

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_teleport() {
    let input = r##"
el {
    div {
        ~ {
            teleport "#modal-container"
        }
    }
}
    "##;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-teleport"),
        Some(&"#modal-container".to_string())
    );
}

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_teleport_prepend() {
    let input = r##"
el {
    div {
        ~ {
            teleport~prepend "#target"
        }
    }
}
    "##;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-teleport__prepend"),
        Some(&"#target".to_string())
    );
}

// =============================================================================
// SECTION 14: Scroll Into View
// =============================================================================

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_scroll_into_view() {
    let input = r#"
el {
    div {
        ~ {
            scrollIntoView
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert!(el.attributes.contains_key("data-scroll-into-view"));
}

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_scroll_into_view_smooth() {
    let input = r#"
el {
    div {
        ~ {
            scrollIntoView~smooth
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert!(el.attributes.contains_key("data-scroll-into-view__smooth"));
}

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_scroll_into_view_smooth_vcenter() {
    let input = r#"
el {
    div {
        ~ {
            scrollIntoView~smooth~vcenter
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert!(el.attributes.contains_key("data-scroll-into-view__smooth__vcenter"));
}

// =============================================================================
// SECTION 15: Action Error Handling (on:fetch)
// =============================================================================

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_on_fetch_error_handling() {
    let input = r#"
el {
    div {
        ~ {
            on:fetch "evt.detail.type == 'error' && handleError(evt)"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    // on:fetch maps to data-on:datastar-fetch (special event)
    assert_eq!(
        el.attributes.get("data-on:datastar-fetch"),
        Some(&"evt.detail.type == 'error' && handleError(evt)".to_string())
    );
}

// =============================================================================
// SECTION 16: Custom Events (preserve colon)
// =============================================================================

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_custom_event() {
    // on:myCustomEvent → data-on:my-custom-event (custom events preserve colon)
    let input = r#"
el {
    div {
        ~ {
            on:myCustomEvent "handleCustom()"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-on:my-custom-event"),
        Some(&"handleCustom()".to_string())
    );
}

// =============================================================================
// SECTION 17: Component with Tilde Block
// =============================================================================

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_component_with_inline_tilde() {
    // When a component is invoked with inline tilde attributes,
    // they should apply to the component's root element
    let input = r#"
el {
    Button ~on:click="handleSubmit()" Submit
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    // The component invocation should have the tilde attribute recorded
    // (actual application to root happens at render time)
    assert_eq!(
        el.attributes.get("data-on-click"),
        Some(&"handleSubmit()".to_string())
    );
}

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_component_with_tilde_block() {
    let input = r#"
el {
    Button {
        ~ {
            on:click "handleSubmit()"
            .loading $isLoading
        }
        Submit
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(
        el.attributes.get("data-on-click"),
        Some(&"handleSubmit()".to_string())
    );
    assert_eq!(
        el.attributes.get("data-class-loading"),
        Some(&"$isLoading".to_string())
    );
}

// =============================================================================
// SECTION 18: Multiple Tilde Blocks (Combined)
// =============================================================================

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_multiple_tilde_blocks_combined() {
    // Multiple tilde blocks in same element should be combined
    let input = r#"
el {
    div {
        ~ {
            on:click "handleClick()"
        }
        span Content
        ~ {
            show $isVisible
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    // Both tilde blocks should be merged onto the div
    assert_eq!(
        el.attributes.get("data-on-click"),
        Some(&"handleClick()".to_string())
    );
    assert_eq!(
        el.attributes.get("data-show"),
        Some(&"$isVisible".to_string())
    );
}

// =============================================================================
// SECTION 19: Static Attributes Outside Tilde (Non-Datastar data-*)
// =============================================================================

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_static_data_attributes() {
    // Non-Datastar data-* attributes should remain as regular attributes
    let input = r#"
el {
    button data-testid="submit-btn" data-track="cta-click" {
        ~ {
            on:click "@post('/submit')"
        }
        Submit
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    // Regular data attributes
    assert_eq!(
        el.attributes.get("data-testid"),
        Some(&"submit-btn".to_string())
    );
    assert_eq!(
        el.attributes.get("data-track"),
        Some(&"cta-click".to_string())
    );
    // Datastar attribute from tilde block
    assert_eq!(
        el.attributes.get("data-on-click"),
        Some(&"@post('/submit')".to_string())
    );
}

// =============================================================================
// SECTION 20: Complex Example - Multiple Features Combined
// =============================================================================

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_complex_form_example() {
    let input = r#"
el {
    form ~on:submit~prevent="@post('/todos', {text: $newTodo}); $newTodo = ''" {
        input~>newTodo placeholder="Add todo..."
        button type=submit {
            ~ {
                .loading $isSubmitting
                disabled $isSubmitting
            }
            Add
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let form = get_first_element(&root);

    // Form should have submit handler with prevent
    assert_eq!(form.tag, "form");
    assert_eq!(
        form.attributes.get("data-on-submit__prevent"),
        Some(&"@post('/todos', {text: $newTodo}); $newTodo = ''".to_string())
    );

    // Input should have binding
    let input = form.children[0].as_element().expect("Expected input");
    assert_eq!(input.tag, "input");
    assert_eq!(
        input.attributes.get("data-bind"),
        Some(&"newTodo".to_string())
    );
    assert_eq!(
        input.attributes.get("placeholder"),
        Some(&"Add todo...".to_string())
    );

    // Button should have class and disabled bindings
    let button = form.children[1].as_element().expect("Expected button");
    assert_eq!(button.tag, "button");
    assert_eq!(
        button.attributes.get("data-class-loading"),
        Some(&"$isSubmitting".to_string())
    );
    assert_eq!(
        button.attributes.get("data-attr-disabled"),
        Some(&"$isSubmitting".to_string())
    );
}

#[test]
#[ignore = "Datastar support not yet implemented"]
fn test_complex_list_with_signals() {
    let input = r#"
el {
    div {
        ~ {
            let:filter all
            let:items "[]"
        }
        ul {
            each item `items` {
                li {
                    ~ {
                        .selected "$item.id == $selectedId"
                        on:click "$selectedId = $item.id"
                    }
                    span ~text=$item.name
                }
            }
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let div = get_first_element(&root);

    // Root div should have signals
    assert_eq!(
        div.attributes.get("data-signals-filter"),
        Some(&"'all'".to_string())
    );
    assert_eq!(
        div.attributes.get("data-signals-items"),
        Some(&"[]".to_string())
    );
}
