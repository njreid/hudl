//! Code generation using CEL for expression evaluation.
//!
//! This module generates Rust code that:
//! - Uses cel_interpreter for expression evaluation
//! - Converts CBOR input to CEL values
//! - Evaluates CEL expressions at runtime

use crate::ast::{Root, Node, SwitchCase};

/// Collect all CEL expressions from the AST for pre-compilation.
fn collect_expressions(nodes: &[Node], exprs: &mut Vec<String>) {
    for node in nodes {
        match node {
            Node::Element(el) => {
                // Check attributes for CEL expressions
                for (_key, value) in &el.attributes {
                    if value.contains('`') {
                        extract_backtick_exprs(value, exprs);
                    }
                }
                collect_expressions(&el.children, exprs);
            }
            Node::Text(t) => {
                extract_backtick_exprs(&t.content, exprs);
            }
            Node::ControlFlow(cf) => match cf {
                crate::ast::ControlFlow::If { condition, then_block, else_block } => {
                    exprs.push(condition.clone());
                    collect_expressions(then_block, exprs);
                    if let Some(eb) = else_block {
                        collect_expressions(eb, exprs);
                    }
                }
                crate::ast::ControlFlow::Each { iterable, body, .. } => {
                    exprs.push(iterable.clone());
                    collect_expressions(body, exprs);
                }
                crate::ast::ControlFlow::Switch { expr, cases, default } => {
                    exprs.push(expr.clone());
                    for SwitchCase(_, children) in cases {
                        collect_expressions(children, exprs);
                    }
                    if let Some(def) = default {
                        collect_expressions(def, exprs);
                    }
                }
            },
        }
    }
}

/// Extract CEL expressions from a string with backticks.
fn extract_backtick_exprs(s: &str, exprs: &mut Vec<String>) {
    let parts: Vec<&str> = s.split('`').collect();
    for (i, part) in parts.iter().enumerate() {
        if i % 2 == 1 && !part.is_empty() {
            exprs.push(part.to_string());
        }
    }
}

/// Generate the WASM library code using CEL.
pub fn generate_wasm_lib_cel(views: Vec<(String, Root)>) -> Result<String, String> {
    let mut code = String::new();

    // Standard imports
    code.push_str("use std::mem;\n");
    code.push_str("use std::slice;\n");
    code.push_str("use std::sync::Arc;\n");
    code.push_str("use std::collections::HashMap;\n");
    code.push_str("use serde_cbor::Value as CborValue;\n");
    code.push_str("use cel_interpreter::{Context, Program, Value as CelValue};\n");
    code.push_str("use cel_interpreter::objects::{Key, Map as CelMap};\n\n");

    // Memory management for WASM
    code.push_str("#[no_mangle]\npub extern \"C\" fn hudl_malloc(s: usize) -> *mut u8 {\n");
    code.push_str("    let mut v = Vec::with_capacity(s);\n");
    code.push_str("    let p = v.as_mut_ptr();\n");
    code.push_str("    mem::forget(v);\n");
    code.push_str("    p\n");
    code.push_str("}\n\n");

    code.push_str("#[no_mangle]\npub extern \"C\" fn hudl_free(p: *mut u8, s: usize) {\n");
    code.push_str("    unsafe { let _ = Vec::from_raw_parts(p, s, s); }\n");
    code.push_str("}\n\n");

    code.push_str("fn pack(p: *const u8, l: usize) -> u64 {\n");
    code.push_str("    ((p as u64) << 32) | (l as u64)\n");
    code.push_str("}\n\n");

    // CBOR to CEL conversion
    code.push_str(CBOR_TO_CEL_HELPER);

    // CEL evaluation helpers
    code.push_str(CEL_HELPERS);

    // Generate view render functions
    for (name, root) in views {
        generate_view_function(&mut code, &name, &root)?;
    }

    Ok(code)
}

const CBOR_TO_CEL_HELPER: &str = r#"
fn cbor_to_cel(cbor: &CborValue) -> CelValue {
    match cbor {
        CborValue::Null => CelValue::Null,
        CborValue::Bool(b) => CelValue::Bool(*b),
        CborValue::Integer(i) => CelValue::Int(*i as i64),
        CborValue::Float(f) => CelValue::Float(*f),
        CborValue::Text(s) => CelValue::String(Arc::new(s.clone())),
        CborValue::Bytes(b) => {
            CelValue::List(Arc::new(b.iter().map(|byte| CelValue::Int(*byte as i64)).collect()))
        }
        CborValue::Array(arr) => {
            CelValue::List(Arc::new(arr.iter().map(cbor_to_cel).collect()))
        }
        CborValue::Map(map) => {
            let cel_map: HashMap<Key, CelValue> = map
                .iter()
                .filter_map(|(k, v)| {
                    if let CborValue::Text(key) = k {
                        Some((Key::String(Arc::new(key.clone())), cbor_to_cel(v)))
                    } else {
                        None
                    }
                })
                .collect();
            CelValue::Map(CelMap { map: Arc::new(cel_map) })
        }
        _ => CelValue::Null,
    }
}

"#;

const CEL_HELPERS: &str = r#"
fn cel_eval(expr: &str, ctx: &Context) -> CelValue {
    match Program::compile(expr) {
        Ok(prog) => prog.execute(ctx).unwrap_or(CelValue::Null),
        Err(_) => CelValue::Null,
    }
}

fn cel_truthy(v: &CelValue) -> bool {
    match v {
        CelValue::Null => false,
        CelValue::Bool(b) => *b,
        CelValue::Int(i) => *i != 0,
        CelValue::UInt(u) => *u != 0,
        CelValue::Float(f) => *f != 0.0,
        CelValue::String(s) => !s.is_empty(),
        CelValue::List(l) => !l.is_empty(),
        CelValue::Map(m) => !m.map.is_empty(),
        _ => true,
    }
}

fn cel_to_string(v: &CelValue) -> String {
    match v {
        CelValue::Null => String::new(),
        CelValue::Bool(b) => b.to_string(),
        CelValue::Int(i) => i.to_string(),
        CelValue::UInt(u) => u.to_string(),
        CelValue::Float(f) => f.to_string(),
        CelValue::String(s) => s.to_string(),
        CelValue::Bytes(b) => String::from_utf8_lossy(b).to_string(),
        CelValue::List(l) => format!("{:?}", l),
        CelValue::Map(m) => format!("{:?}", m),
        _ => format!("{:?}", v),
    }
}

fn html_escape(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&#x27;"),
            _ => result.push(c),
        }
    }
    result
}

fn add_cbor_to_context(ctx: &mut Context, name: &str, value: &CborValue) {
    let cel_val = cbor_to_cel(value);
    let _ = ctx.add_variable(name, cel_val);
}

"#;

fn generate_view_function(code: &mut String, name: &str, root: &Root) -> Result<(), String> {
    let fn_name = name.to_lowercase();

    // Internal render function
    code.push_str(&format!("\nfn render_{}(r: &mut String, ctx: &Context) {{\n", fn_name));

    for node in &root.nodes {
        generate_node_cel(code, node, 1)?;
    }

    code.push_str("}\n");

    // Exported WASM function
    code.push_str(&format!("\n#[no_mangle]\npub extern \"C\" fn {}(ptr: *const u8, len: usize) -> u64 {{\n", name));
    code.push_str("    let cbor_data: CborValue = if len > 0 {\n");
    code.push_str("        let slice = unsafe { slice::from_raw_parts(ptr, len) };\n");
    code.push_str("        serde_cbor::from_slice(slice).unwrap_or(CborValue::Null)\n");
    code.push_str("    } else {\n");
    code.push_str("        CborValue::Null\n");
    code.push_str("    };\n\n");

    // Build context from CBOR map
    code.push_str("    let mut ctx = Context::default();\n");
    code.push_str("    if let CborValue::Map(map) = &cbor_data {\n");
    code.push_str("        for (k, v) in map {\n");
    code.push_str("            if let CborValue::Text(key) = k {\n");
    code.push_str("                add_cbor_to_context(&mut ctx, key, v);\n");
    code.push_str("            }\n");
    code.push_str("        }\n");
    code.push_str("    }\n\n");

    code.push_str("    let mut out = String::new();\n");
    code.push_str(&format!("    render_{}(&mut out, &ctx);\n", fn_name));
    code.push_str("    let result_ptr = out.as_ptr();\n");
    code.push_str("    let result_len = out.len();\n");
    code.push_str("    mem::forget(out);\n");
    code.push_str("    pack(result_ptr, result_len)\n");
    code.push_str("}\n");

    Ok(())
}

fn generate_node_cel(code: &mut String, node: &Node, indent: usize) -> Result<(), String> {
    let pad = "    ".repeat(indent);

    match node {
        Node::Element(el) => {
            // Opening tag
            code.push_str(&pad);
            code.push_str(&format!("r.push_str(\"<{}\");\n", el.tag));

            // ID attribute
            if let Some(id) = &el.id {
                code.push_str(&pad);
                code.push_str(&format!("r.push_str(\" id=\\\"{}\\\"\");\n", id));
            }

            // Class attribute
            if !el.classes.is_empty() {
                code.push_str(&pad);
                code.push_str(&format!("r.push_str(\" class=\\\"{}\\\"\");\n", el.classes.join(" ")));
            }

            // Other attributes (may contain CEL expressions)
            for (key, value) in &el.attributes {
                if value.contains('`') {
                    // Dynamic attribute with CEL
                    generate_dynamic_attr(code, key, value, &pad)?;
                } else {
                    // Static attribute
                    code.push_str(&pad);
                    code.push_str(&format!("r.push_str(\" {}=\\\"{}\\\"\");\n", key, value));
                }
            }

            // Close opening tag
            code.push_str(&pad);
            code.push_str("r.push_str(\">\");\n");

            // Children
            for child in &el.children {
                generate_node_cel(code, child, indent + 1)?;
            }

            // Closing tag
            code.push_str(&pad);
            code.push_str(&format!("r.push_str(\"</{}>\");\n", el.tag));
        }

        Node::Text(t) => {
            generate_text_with_interpolation(code, &t.content, &pad)?;
        }

        Node::ControlFlow(cf) => match cf {
            crate::ast::ControlFlow::If { condition, then_block, else_block } => {
                code.push_str(&pad);
                code.push_str(&format!("if cel_truthy(&cel_eval(\"{}\", ctx)) {{\n",
                    escape_string(condition)));

                for child in then_block {
                    generate_node_cel(code, child, indent + 1)?;
                }

                code.push_str(&pad);
                code.push_str("}");

                if let Some(else_nodes) = else_block {
                    code.push_str(" else {\n");
                    for child in else_nodes {
                        generate_node_cel(code, child, indent + 1)?;
                    }
                    code.push_str(&pad);
                    code.push_str("}");
                }
                code.push_str("\n");
            }

            crate::ast::ControlFlow::Each { binding, iterable, body } => {
                code.push_str(&pad);
                code.push_str(&format!("if let CelValue::List(list) = cel_eval(\"{}\", ctx) {{\n",
                    escape_string(iterable)));
                code.push_str(&pad);
                code.push_str("    for (_idx, _item) in list.iter().enumerate() {\n");
                code.push_str(&pad);
                code.push_str("        let mut loop_ctx = ctx.clone();\n");
                code.push_str(&pad);
                code.push_str(&format!("        let _ = loop_ctx.add_variable(\"{}\", _item.clone());\n", binding));
                code.push_str(&pad);
                code.push_str("        let _ = loop_ctx.add_variable(\"_index\", CelValue::Int(_idx as i64));\n");

                for child in body {
                    generate_node_cel_with_ctx(code, child, indent + 2, "&loop_ctx")?;
                }

                code.push_str(&pad);
                code.push_str("    }\n");
                code.push_str(&pad);
                code.push_str("}\n");
            }

            crate::ast::ControlFlow::Switch { expr, cases, default } => {
                code.push_str(&pad);
                code.push_str("{\n");
                code.push_str(&pad);
                code.push_str(&format!("    let _switch_val = cel_eval(\"{}\", ctx);\n",
                    escape_string(expr)));

                let mut first = true;
                for SwitchCase(pattern, children) in cases {
                    code.push_str(&pad);
                    if first {
                        code.push_str("    if ");
                        first = false;
                    } else {
                        code.push_str("    else if ");
                    }

                    // Compare switch value to pattern (as string for enum values)
                    code.push_str(&format!("cel_to_string(&_switch_val) == \"{}\" {{\n", pattern));

                    for child in children {
                        generate_node_cel(code, child, indent + 2)?;
                    }

                    code.push_str(&pad);
                    code.push_str("    }\n");
                }

                if let Some(def_nodes) = default {
                    code.push_str(&pad);
                    if first {
                        code.push_str("    {\n");
                    } else {
                        code.push_str("    else {\n");
                    }

                    for child in def_nodes {
                        generate_node_cel(code, child, indent + 2)?;
                    }

                    code.push_str(&pad);
                    code.push_str("    }\n");
                }

                code.push_str(&pad);
                code.push_str("}\n");
            }
        },
    }

    Ok(())
}

/// Generate node with a custom context variable name (for loops).
fn generate_node_cel_with_ctx(code: &mut String, node: &Node, indent: usize, ctx_var: &str) -> Result<(), String> {
    let pad = "    ".repeat(indent);

    match node {
        Node::Element(el) => {
            code.push_str(&pad);
            code.push_str(&format!("r.push_str(\"<{}\");\n", el.tag));

            if let Some(id) = &el.id {
                code.push_str(&pad);
                code.push_str(&format!("r.push_str(\" id=\\\"{}\\\"\");\n", id));
            }

            if !el.classes.is_empty() {
                code.push_str(&pad);
                code.push_str(&format!("r.push_str(\" class=\\\"{}\\\"\");\n", el.classes.join(" ")));
            }

            for (key, value) in &el.attributes {
                if value.contains('`') {
                    generate_dynamic_attr_with_ctx(code, key, value, &pad, ctx_var)?;
                } else {
                    code.push_str(&pad);
                    code.push_str(&format!("r.push_str(\" {}=\\\"{}\\\"\");\n", key, value));
                }
            }

            code.push_str(&pad);
            code.push_str("r.push_str(\">\");\n");

            for child in &el.children {
                generate_node_cel_with_ctx(code, child, indent + 1, ctx_var)?;
            }

            code.push_str(&pad);
            code.push_str(&format!("r.push_str(\"</{}>\");\n", el.tag));
        }

        Node::Text(t) => {
            generate_text_with_interpolation_ctx(code, &t.content, &pad, ctx_var)?;
        }

        Node::ControlFlow(cf) => match cf {
            crate::ast::ControlFlow::If { condition, then_block, else_block } => {
                code.push_str(&pad);
                code.push_str(&format!("if cel_truthy(&cel_eval(\"{}\", {})) {{\n",
                    escape_string(condition), ctx_var));

                for child in then_block {
                    generate_node_cel_with_ctx(code, child, indent + 1, ctx_var)?;
                }

                code.push_str(&pad);
                code.push_str("}");

                if let Some(else_nodes) = else_block {
                    code.push_str(" else {\n");
                    for child in else_nodes {
                        generate_node_cel_with_ctx(code, child, indent + 1, ctx_var)?;
                    }
                    code.push_str(&pad);
                    code.push_str("}");
                }
                code.push_str("\n");
            }

            crate::ast::ControlFlow::Each { binding, iterable, body } => {
                code.push_str(&pad);
                code.push_str(&format!("if let CelValue::List(list) = cel_eval(\"{}\", {}) {{\n",
                    escape_string(iterable), ctx_var));
                code.push_str(&pad);
                code.push_str("    for (_idx, _item) in list.iter().enumerate() {\n");
                code.push_str(&pad);
                code.push_str(&format!("        let mut inner_ctx = {}.clone();\n", ctx_var));
                code.push_str(&pad);
                code.push_str(&format!("        let _ = inner_ctx.add_variable(\"{}\", _item.clone());\n", binding));
                code.push_str(&pad);
                code.push_str("        let _ = inner_ctx.add_variable(\"_index\", CelValue::Int(_idx as i64));\n");

                for child in body {
                    generate_node_cel_with_ctx(code, child, indent + 2, "&inner_ctx")?;
                }

                code.push_str(&pad);
                code.push_str("    }\n");
                code.push_str(&pad);
                code.push_str("}\n");
            }

            crate::ast::ControlFlow::Switch { expr, cases, default } => {
                code.push_str(&pad);
                code.push_str("{\n");
                code.push_str(&pad);
                code.push_str(&format!("    let _switch_val = cel_eval(\"{}\", {});\n",
                    escape_string(expr), ctx_var));

                let mut first = true;
                for SwitchCase(pattern, children) in cases {
                    code.push_str(&pad);
                    if first {
                        code.push_str("    if ");
                        first = false;
                    } else {
                        code.push_str("    else if ");
                    }

                    code.push_str(&format!("cel_to_string(&_switch_val) == \"{}\" {{\n", pattern));

                    for child in children {
                        generate_node_cel_with_ctx(code, child, indent + 2, ctx_var)?;
                    }

                    code.push_str(&pad);
                    code.push_str("    }\n");
                }

                if let Some(def_nodes) = default {
                    code.push_str(&pad);
                    if first {
                        code.push_str("    {\n");
                    } else {
                        code.push_str("    else {\n");
                    }

                    for child in def_nodes {
                        generate_node_cel_with_ctx(code, child, indent + 2, ctx_var)?;
                    }

                    code.push_str(&pad);
                    code.push_str("    }\n");
                }

                code.push_str(&pad);
                code.push_str("}\n");
            }
        },
    }

    Ok(())
}

fn generate_dynamic_attr(code: &mut String, key: &str, value: &str, pad: &str) -> Result<(), String> {
    generate_dynamic_attr_with_ctx(code, key, value, pad, "ctx")
}

fn generate_dynamic_attr_with_ctx(code: &mut String, key: &str, value: &str, pad: &str, ctx_var: &str) -> Result<(), String> {
    // For boolean attributes like checked=`is_checked`
    let is_boolean_attr = matches!(key, "checked" | "disabled" | "readonly" | "required" | "selected" | "autofocus" | "autoplay" | "controls" | "loop" | "muted" | "open" | "hidden");

    if is_boolean_attr && value.starts_with('`') && value.ends_with('`') {
        // Pure CEL expression for boolean attribute
        let expr = &value[1..value.len()-1];
        code.push_str(pad);
        code.push_str(&format!("if cel_truthy(&cel_eval(\"{}\", {})) {{\n", escape_string(expr), ctx_var));
        code.push_str(pad);
        code.push_str(&format!("    r.push_str(\" {}\");\n", key));
        code.push_str(pad);
        code.push_str("}\n");
    } else {
        // Dynamic value
        code.push_str(pad);
        code.push_str(&format!("r.push_str(\" {}=\\\"\");\n", key));
        generate_text_with_interpolation_ctx(code, value, pad, ctx_var)?;
        code.push_str(pad);
        code.push_str("r.push_str(\"\\\"\");\n");
    }

    Ok(())
}

fn generate_text_with_interpolation(code: &mut String, content: &str, pad: &str) -> Result<(), String> {
    generate_text_with_interpolation_ctx(code, content, pad, "ctx")
}

fn generate_text_with_interpolation_ctx(code: &mut String, content: &str, pad: &str, ctx_var: &str) -> Result<(), String> {
    let parts: Vec<&str> = content.split('`').collect();

    for (i, part) in parts.iter().enumerate() {
        if i % 2 == 0 {
            // Static text
            if !part.is_empty() {
                code.push_str(pad);
                code.push_str(&format!("r.push_str(\"{}\");\n", escape_string(part)));
            }
        } else {
            // CEL expression
            if !part.is_empty() {
                code.push_str(pad);
                code.push_str(&format!("r.push_str(&html_escape(&cel_to_string(&cel_eval(\"{}\", {}))));\n",
                    escape_string(part), ctx_var));
            }
        }
    }

    Ok(())
}

fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;
    use crate::transformer;

    #[test]
    fn test_generate_basic() {
        let input = r#"
el {
    div "Hello"
}
        "#;

        let doc = parser::parse(input).unwrap();
        let root = transformer::transform(&doc).unwrap();
        let views = vec![("TestView".to_string(), root)];
        let rust_code = generate_wasm_lib_cel(views).expect("Codegen failed");

        assert!(rust_code.contains("cel_interpreter"));
        assert!(rust_code.contains("r.push_str(\"<div\")"));
        assert!(rust_code.contains("Hello"));
    }

    #[test]
    fn test_generate_with_cel() {
        let input = r#"
el {
    span "`name`"
}
        "#;

        let doc = parser::parse(input).unwrap();
        let root = transformer::transform(&doc).unwrap();
        let views = vec![("TestView".to_string(), root)];
        let rust_code = generate_wasm_lib_cel(views).expect("Codegen failed");

        assert!(rust_code.contains("cel_eval"));
        assert!(rust_code.contains("name"));
    }
}
