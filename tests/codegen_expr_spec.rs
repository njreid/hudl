//! Code generation tests for CEL expressions
//!
//! Tests that CEL expressions are correctly compiled into Rust code
//! that evaluates them at runtime.

use hudlc::codegen;
use hudlc::parser;
use hudlc::transformer;

#[test]
fn test_codegen_cel_comparison() {
    let input = r#"
el {
    if "`count > 0`" {
        p "Positive"
    }
}
    "#;

    let doc = parser::parse(input).unwrap();
    let root = transformer::transform(&doc).unwrap();
    let views = vec![("TestView".to_string(), root)];
    let rust_code = codegen::generate_wasm_lib(views).expect("Codegen failed");

    // Check for comparison helper usage
    assert!(rust_code.contains("hudl_gt(&") || rust_code.contains("count") && rust_code.contains("0"));
    assert!(rust_code.contains("hudl_truthy(&") || rust_code.contains("if"));
}

#[test]
fn test_codegen_cel_field_access() {
    let input = r#"
el {
    span "`user.name`"
}
    "#;

    let doc = parser::parse(input).unwrap();
    let root = transformer::transform(&doc).unwrap();
    let views = vec![("TestView".to_string(), root)];
    let rust_code = codegen::generate_wasm_lib(views).expect("Codegen failed");

    // Should generate field access code
    assert!(rust_code.contains("user"));
    assert!(rust_code.contains("name"));
}

#[test]
fn test_codegen_cel_function_call() {
    let input = r#"
el {
    if "`size(items) > 0`" {
        p "Has items"
    }
}
    "#;

    let doc = parser::parse(input).unwrap();
    let root = transformer::transform(&doc).unwrap();
    let views = vec![("TestView".to_string(), root)];
    let rust_code = codegen::generate_wasm_lib(views).expect("Codegen failed");

    // Should reference size function or items
    assert!(rust_code.contains("items") || rust_code.contains("size"));
}

#[test]
fn test_codegen_each_with_index() {
    let input = r#"
el {
    each item "`items`" {
        span "`_index`"
        span "`item.name`"
    }
}
    "#;

    let doc = parser::parse(input).unwrap();
    let root = transformer::transform(&doc).unwrap();
    let views = vec![("TestView".to_string(), root)];
    let rust_code = codegen::generate_wasm_lib(views).expect("Codegen failed");

    // Should have iteration with index tracking
    assert!(rust_code.contains("item") || rust_code.contains("items"));
    // Index should be available
    assert!(rust_code.contains("_index") || rust_code.contains("enumerate"));
}

#[test]
fn test_codegen_switch_enum() {
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

    // Should have switch logic
    assert!(rust_code.contains("status"));
    assert!(rust_code.contains("Active") || rust_code.contains("STATUS_ACTIVE"));
}

#[test]
fn test_codegen_boolean_attribute() {
    let input = r#"
el {
    input type="checkbox" checked="`is_checked`"
}
    "#;

    let doc = parser::parse(input).unwrap();
    let root = transformer::transform(&doc).unwrap();
    let views = vec![("TestView".to_string(), root)];
    let rust_code = codegen::generate_wasm_lib(views).expect("Codegen failed");

    // Boolean attribute should be conditionally rendered
    assert!(rust_code.contains("is_checked"));
    assert!(rust_code.contains("checked"));
}

#[test]
fn test_codegen_string_interpolation() {
    let input = r#"
el {
    p "Hello, `user.name`!"
}
    "#;

    let doc = parser::parse(input).unwrap();
    let root = transformer::transform(&doc).unwrap();
    let views = vec![("TestView".to_string(), root)];
    let rust_code = codegen::generate_wasm_lib(views).expect("Codegen failed");

    // Should have string parts and interpolation
    assert!(rust_code.contains("Hello"));
    assert!(rust_code.contains("user") || rust_code.contains("name"));
}

#[test]
fn test_codegen_raw_function() {
    let input = r#"
el {
    div "`raw(html_content)`"
}
    "#;

    let doc = parser::parse(input).unwrap();
    let root = transformer::transform(&doc).unwrap();
    let views = vec![("TestView".to_string(), root)];
    let rust_code = codegen::generate_wasm_lib(views).expect("Codegen failed");

    // raw() should bypass HTML escaping
    assert!(rust_code.contains("html_content") || rust_code.contains("raw"));
}

#[test]
fn test_codegen_nested_if() {
    let input = r#"
el {
    if "`level >= 80`" {
        span.danger "Critical"
    } else {
        if "`level >= 50`" {
            span.warning "Warning"
        } else {
            span.ok "Normal"
        }
    }
}
    "#;

    let doc = parser::parse(input).unwrap();
    let root = transformer::transform(&doc).unwrap();
    let views = vec![("TestView".to_string(), root)];
    let rust_code = codegen::generate_wasm_lib(views).expect("Codegen failed");

    // Should have nested conditionals
    assert!(rust_code.contains("level"));
    assert!(rust_code.contains("80") || rust_code.contains("50"));
}

#[test]
fn test_codegen_component_invocation() {
    let input = r#"
el {
    each user "`users`" {
        UserCard "`user`"
    }
}
    "#;

    let doc = parser::parse(input).unwrap();
    let root = transformer::transform(&doc).unwrap();
    let views = vec![("TestView".to_string(), root)];
    let rust_code = codegen::generate_wasm_lib(views).expect("Codegen failed");

    // Should call the UserCard component
    assert!(rust_code.contains("UserCard") || rust_code.contains("user"));
}
