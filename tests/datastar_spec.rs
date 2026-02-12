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

/// Render a template and return the HTML string (for codegen-level tests)
fn render_html(input: &str) -> String {
    let doc = parser::parse(input).expect("Failed to parse");
    let root = transformer::transform_with_metadata(&doc, input).expect("Failed to transform");
    let schema = hudlc::proto::ProtoSchema::from_template(input, None).unwrap_or_default();
    hudlc::interpreter::render(&root, &schema, &[]).expect("Render failed")
}

// =============================================================================
// SECTION 1: Binding Shorthand (~>)
// =============================================================================

#[test]
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
    assert_datastar_attr(el, "bind", Some("username"), &[]);
}

#[test]
fn test_binding_shorthand_with_debounce() {
    let input = r#"
el {
    input~>searchQuery~debounce:300ms
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(el.tag, "input");
    assert_datastar_attr(el, "bind", Some("searchQuery"), &["debounce:300ms"]);
}

#[test]
fn test_binding_shorthand_with_throttle() {
    let input = r#"
el {
    input~>value~throttle:100ms
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_datastar_attr(el, "bind", Some("value"), &["throttle:100ms"]);
}

// =============================================================================
// SECTION 2: Inline Tilde Attributes
// =============================================================================

#[test]
fn test_inline_tilde_on_click() {
    let input = r#"
el {
    button ~on:click="doSomething()" Click
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(el.tag, "button");
    assert_datastar_attr(el, "on:click", Some("doSomething()"), &[]);
}

#[test]
fn test_inline_tilde_text() {
    let input = r#"
el {
    h1 ~text="Todo App"
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_eq!(el.tag, "h1");
    assert_datastar_attr(el, "text", Some("Todo App"), &[]);
}

#[test]
fn test_inline_tilde_with_modifiers() {
    let input = r#"
el {
    button ~on:click~once~prevent="submit()"
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_datastar_attr(el, "on:click", Some("submit()"), &["once", "prevent"]);
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
fn test_computed_with_operators() {
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

    assert_datastar_attr(el, "let:total", Some("$price * $quantity"), &[]);
}

#[test]
fn test_computed_with_function_call() {
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

    assert_datastar_attr(el, "let:upper", Some("$name.toUpperCase()"), &[]);
}

#[test]
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

    assert_datastar_attr(el, "let:fullName", Some("$firstName + ' ' + $lastName"), &[]);
}

// =============================================================================
// SECTION 6: Event Handlers (on:)
// =============================================================================

#[test]
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

    assert_datastar_attr(el, "on:click", Some("$count++"), &[]);
}

#[test]
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

    assert_datastar_attr(el, "on:click", Some("@get('/api/data')"), &[]);
}

#[test]
fn test_on_keydown_with_key_modifier() {
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

    // Key modifiers are part of the attr name: "on:keydown.enter"
    assert_datastar_attr(el, "on:keydown.enter", Some("submit()"), &[]);
}

#[test]
fn test_on_submit_prevent() {
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

    assert_datastar_attr(el, "on:submit", Some("@post('/login')"), &["prevent"]);
}

#[test]
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

    assert_datastar_attr(el, "on:click", Some("$isOpen = false"), &["outside"]);
}

#[test]
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

    assert_datastar_attr(el, "on:scroll", Some("updatePosition()"), &["throttle:50ms", "passive"]);
}

#[test]
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

    assert_datastar_attr(el, "on:resize", Some("handleResize()"), &["window"]);
}

// =============================================================================
// SECTION 7: Text Content (text)
// =============================================================================

#[test]
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

    assert_datastar_attr(el, "text", Some("$message"), &[]);
}

#[test]
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

    assert_datastar_attr(el, "text", Some("$greeting + ', ' + $name"), &[]);
}

// =============================================================================
// SECTION 8: Show/Hide (show)
// =============================================================================

#[test]
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

    assert_datastar_attr(el, "show", Some("$isVisible"), &[]);
}

#[test]
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

    assert_datastar_attr(el, "show", Some("$count > 0"), &[]);
}

// =============================================================================
// SECTION 9: Dynamic HTML Attributes
// =============================================================================

#[test]
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

    assert_datastar_attr(el, "disabled", Some("$isLoading"), &[]);
}

#[test]
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

    assert_datastar_attr(el, "href", Some("'/user/' + $userId"), &[]);
}

#[test]
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

    assert_datastar_attr(el, "target", Some("$openInNew ? '_blank' : '_self'"), &[]);
}

// =============================================================================
// SECTION 10: Persist
// =============================================================================

#[test]
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

    assert_datastar_attr(el, "persist", None, &[]);
}

#[test]
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

    assert_datastar_attr(el, "persist", Some("theme,lang"), &[]);
}

#[test]
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

    assert_datastar_attr(el, "persist", Some("userPrefs"), &["session"]);
}

// =============================================================================
// SECTION 11: Element References (ref)
// =============================================================================

#[test]
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

    assert_datastar_attr(el, "ref", Some("emailInput"), &[]);
}

// =============================================================================
// SECTION 12: Intersection Observer (on:intersect)
// =============================================================================

#[test]
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

    assert_datastar_attr(el, "on:intersect", Some("$visible = true"), &[]);
}

#[test]
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

    assert_datastar_attr(el, "on:intersect", Some("@get('/lazy-content')"), &["once"]);
}

#[test]
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

    assert_datastar_attr(el, "on:intersect", Some("$visible = true"), &["half"]);
}

// =============================================================================
// SECTION 13: Teleport
// =============================================================================

#[test]
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

    assert_datastar_attr(el, "teleport", Some("#modal-container"), &[]);
}

#[test]
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

    assert_datastar_attr(el, "teleport", Some("#target"), &["prepend"]);
}

// =============================================================================
// SECTION 14: Scroll Into View
// =============================================================================

#[test]
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

    assert_datastar_attr(el, "scrollIntoView", None, &[]);
}

#[test]
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

    assert_datastar_attr(el, "scrollIntoView", None, &["smooth"]);
}

#[test]
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

    assert_datastar_attr(el, "scrollIntoView", None, &["smooth", "vcenter"]);
}

// =============================================================================
// SECTION 15: Action Error Handling (on:fetch)
// =============================================================================

#[test]
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

    assert_datastar_attr(el, "on:fetch", Some("evt.detail.type == 'error' && handleError(evt)"), &[]);
}

// =============================================================================
// SECTION 16: Custom Events (preserve colon)
// =============================================================================

#[test]
fn test_custom_event() {
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

    assert_datastar_attr(el, "on:myCustomEvent", Some("handleCustom()"), &[]);
}

// =============================================================================
// SECTION 17: Component with Tilde Block
// =============================================================================

#[test]
fn test_component_with_inline_tilde() {
    let input = r#"
el {
    Button ~on:click="handleSubmit()" Submit
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);

    assert_datastar_attr(el, "on:click", Some("handleSubmit()"), &[]);
}

#[test]
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

    assert_datastar_attr(el, "on:click", Some("handleSubmit()"), &[]);
    assert_datastar_attr(el, ".loading", Some("$isLoading"), &[]);
}

// =============================================================================
// SECTION 18: Multiple Tilde Blocks (Combined)
// =============================================================================

#[test]
fn test_multiple_tilde_blocks_combined() {
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

    assert_datastar_attr(el, "on:click", Some("handleClick()"), &[]);
    assert_datastar_attr(el, "show", Some("$isVisible"), &[]);
}

// =============================================================================
// SECTION 19: Static Attributes Outside Tilde (Non-Datastar data-*)
// =============================================================================

#[test]
fn test_static_data_attributes() {
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

    // Regular data attributes remain in el.attributes
    assert_eq!(
        el.attributes.get("data-testid"),
        Some(&"submit-btn".to_string())
    );
    assert_eq!(
        el.attributes.get("data-track"),
        Some(&"cta-click".to_string())
    );
    // Datastar attribute from tilde block
    assert_datastar_attr(el, "on:click", Some("@post('/submit')"), &[]);
}

// =============================================================================
// SECTION 20: Complex Example - Multiple Features Combined
// =============================================================================

#[test]
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

    assert_eq!(form.tag, "form");
    assert_datastar_attr(form, "on:submit", Some("@post('/todos', {text: $newTodo}); $newTodo = ''"), &["prevent"]);
}

#[test]
fn test_complex_list_with_signals() {
    let input = r#"
el {
    div {
        ~ {
            let:filter all
            let:items "[]"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let div = get_first_element(&root);

    assert_datastar_attr(div, "let:filter", Some("all"), &[]);
    assert_datastar_attr(div, "let:items", Some("[]"), &[]);
}

// =============================================================================
// SECTION 21: HTML Rendering Tests (codegen-level verification)
// =============================================================================

#[test]
fn test_render_on_click_html() {
    let html = render_html(r#"
// name: Test
el {
    button {
        ~ {
            on:click "$count++"
        }
        Click
    }
}
    "#);
    assert!(html.contains("data-on-click=\"$count++\""), "HTML: {}", html);
}

#[test]
fn test_render_show_html() {
    let html = render_html(r#"
// name: Test
el {
    div {
        ~ {
            show $isVisible
        }
    }
}
    "#);
    assert!(html.contains("data-show=\"$isVisible\""), "HTML: {}", html);
}

#[test]
fn test_render_class_toggle_html() {
    let html = render_html(r#"
// name: Test
el {
    div {
        ~ {
            .active $isSelected
        }
    }
}
    "#);
    assert!(html.contains("data-class-active=\"$isSelected\""), "HTML: {}", html);
}

#[test]
fn test_render_signal_html() {
    let html = render_html(r#"
// name: Test
el {
    div {
        ~ {
            let:count 0
        }
    }
}
    "#);
    assert!(html.contains("data-signals-count=\"0\""), "HTML: {}", html);
}

#[test]
fn test_render_computed_html() {
    let html = render_html(r#"
// name: Test
el {
    div {
        ~ {
            let:total "$price * $quantity"
        }
    }
}
    "#);
    assert!(html.contains("data-computed-total=\"$price * $quantity\""), "HTML: {}", html);
}

#[test]
fn test_render_modifier_html() {
    let html = render_html(r#"
// name: Test
el {
    form {
        ~ {
            on:submit~prevent "@post('/login')"
        }
    }
}
    "#);
    assert!(html.contains("data-on-submit__prevent=\"@post('/login')\""), "HTML: {}", html);
}

#[test]
fn test_render_custom_event_html() {
    let html = render_html(r#"
// name: Test
el {
    div {
        ~ {
            on:myCustomEvent "handleCustom()"
        }
    }
}
    "#);
    // Custom events preserve colon and kebab-case
    assert!(html.contains("data-on:my-custom-event=\"handleCustom()\""), "HTML: {}", html);
}

#[test]
fn test_render_on_fetch_html() {
    let html = render_html(r#"
// name: Test
el {
    div {
        ~ {
            on:fetch "handleFetch()"
        }
    }
}
    "#);
    // on:fetch maps to data-on:datastar-fetch
    assert!(html.contains("data-on:datastar-fetch=\"handleFetch()\""), "HTML: {}", html);
}

#[test]
fn test_render_signal_string_quoting_html() {
    let html = render_html(r#"
// name: Test
el {
    div {
        ~ {
            let:name hello
        }
    }
}
    "#);
    // Bare string values get wrapped in quotes for Datastar
    assert!(html.contains("data-signals-name=\"'hello'\""), "HTML: {}", html);
}

#[test]
fn test_render_persist_html() {
    let html = render_html(r#"
// name: Test
el {
    div {
        ~ {
            persist
        }
    }
}
    "#);
    assert!(html.contains("data-persist"), "HTML: {}", html);
}

#[test]
fn test_render_teleport_html() {
    let html = render_html(r##"
// name: Test
el {
    div {
        ~ {
            teleport "#modal"
        }
    }
}
    "##);
    assert!(html.contains("data-teleport=\"#modal\""), "HTML: {}", html);
}

#[test]
fn test_render_ref_html() {
    let html = render_html(r#"
// name: Test
el {
    input {
        ~ {
            ref emailInput
        }
    }
}
    "#);
    assert!(html.contains("data-ref=\"emailInput\""), "HTML: {}", html);
}

#[test]
fn test_render_scroll_into_view_html() {
    let html = render_html(r#"
// name: Test
el {
    div {
        ~ {
            scrollIntoView~smooth
        }
    }
}
    "#);
    assert!(html.contains("data-scroll-into-view__smooth"), "HTML: {}", html);
}

// =============================================================================
// SECTION 23: Action Tests (HTTP, Signal, DOM actions)
// =============================================================================

#[test]
fn test_action_http_put() {
    let input = r#"
el {
    button {
        ~ {
            on:click "@put('/api/items/1', {name: $newName})"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);
    assert_datastar_attr(el, "on:click", Some("@put('/api/items/1', {name: $newName})"), &[]);
}

#[test]
fn test_action_http_patch() {
    let input = r#"
el {
    button {
        ~ {
            on:click "@patch('/api/items/1', {status: 'done'})"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);
    assert_datastar_attr(el, "on:click", Some("@patch('/api/items/1', {status: 'done'})"), &[]);
}

#[test]
fn test_action_http_delete() {
    let input = r#"
el {
    button {
        ~ {
            on:click "@delete('/api/items/' + $id)"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);
    assert_datastar_attr(el, "on:click", Some("@delete('/api/items/' + $id)"), &[]);
}

#[test]
fn test_action_signal_set_all() {
    let input = r#"
el {
    button {
        ~ {
            on:click "@setAll('form.*', '')"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);
    assert_datastar_attr(el, "on:click", Some("@setAll('form.*', '')"), &[]);
}

#[test]
fn test_action_signal_toggle_all() {
    let input = r#"
el {
    button {
        ~ {
            on:click "@toggleAll('checkbox.*')"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);
    assert_datastar_attr(el, "on:click", Some("@toggleAll('checkbox.*')"), &[]);
}

#[test]
fn test_action_dom_clipboard() {
    let input = r#"
el {
    button {
        ~ {
            on:click "@clipboard($shareUrl)"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);
    assert_datastar_attr(el, "on:click", Some("@clipboard($shareUrl)"), &[]);
}

#[test]
fn test_action_chaining() {
    let input = r#"
el {
    button {
        ~ {
            on:click "@post('/api/save'); @setAll('form.*', '')"
        }
    }
}
    "#;
    let root = parse_and_transform(input);
    let el = get_first_element(&root);
    assert_datastar_attr(el, "on:click", Some("@post('/api/save'); @setAll('form.*', '')"), &[]);
}

#[test]
fn test_render_action_html() {
    let html = render_html(r#"
// name: Test
el {
    button {
        ~ {
            on:click "@setAll('form.*', '')"
        }
    }
}
    "#);
    assert!(html.contains("data-on-click=\"@setAll('form.*', '')\""), "HTML: {}", html);
}

#[test]
fn test_render_action_with_modifiers_html() {
    let html = render_html(r#"
// name: Test
el {
    form {
        ~ {
            on:submit~prevent "@post('/api/save')"
        }
    }
}
    "#);
    assert!(html.contains("data-on-submit__prevent=\"@post('/api/save')\""), "HTML: {}", html);
}
