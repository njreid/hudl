use crate::ast::{Root, Node};

pub fn generate_wasm_lib(views: Vec<(String, Root)>) -> Result<String, String> {
    let mut code = String::new();
    code.push_str("use std::mem;\n");
    code.push_str("use std::slice;\n");
    code.push_str("use serde_cbor::Value;\n\n");

    code.push_str("#[no_mangle]\npub extern \"C\" fn hudl_malloc(s: usize) -> *mut u8 { let mut v = Vec::with_capacity(s); let p = v.as_mut_ptr(); mem::forget(v); p }\n");
    code.push_str("#[no_mangle]\npub extern \"C\" fn hudl_free(p: *mut u8, s: usize) { unsafe { let _ = Vec::from_raw_parts(p, s, s); } }\n");
    code.push_str("fn pack(p: *const u8, l: usize) -> u64 { ((p as u64) << 32) | (l as u64) }\n");

    code.push_str("\nfn get_value<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {\n");
    code.push_str("    match v {\n");
    code.push_str("        Value::Map(m) => m.get(&Value::Text(key.to_string())),\n");
    code.push_str("        _ => None,\n");
    code.push_str("    }\n");
    code.push_str("}\n");

    for (name, root) in views {
        code.push_str("\nfn render_");
        code.push_str(&name.to_lowercase());
        code.push_str("(r: &mut String, _data: &Value) {\n");
        let scope = Vec::new();
        for node in &root.nodes {
            let _ = generate_node(&mut code, node, 1, &scope);
        }
        code.push_str("}\n");

        code.push_str("\n#[no_mangle]\npub extern \"C\" fn ");
        code.push_str(&name);
        code.push_str("(ptr: *const u8, len: usize) -> u64 {\n");
        code.push_str("    let data: Value = if len > 0 {\n");
        code.push_str("        let slice = unsafe { slice::from_raw_parts(ptr, len) };\n");
        code.push_str("        serde_cbor::from_slice(slice).unwrap_or(Value::Null)\n");
        code.push_str("    } else {\n");
        code.push_str("        Value::Null\n");
        code.push_str("    };\n\n");
        code.push_str("    let mut out = String::new();\n");
        code.push_str("    render_");
        code.push_str(&name.to_lowercase());
        code.push_str("(&mut out, &data);\n");
        code.push_str("    let result_ptr = out.as_ptr();\n");
        code.push_str("    let result_len = out.len();\n");
        code.push_str("    mem::forget(out);\n");
        code.push_str("    pack(result_ptr, result_len)\n}
");
    }
    Ok(code)
}

fn resolve_variable(path: &str, scope: &Vec<String>) -> String {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() { return "None".to_string(); }

    let root = parts[0];
    let mut expr = if scope.contains(&root.to_string()) {
        format!("Some({})", root)
    } else {
        format!("get_value(_data, \"{}\")", root)
    };

    for part in &parts[1..] {
        expr = format!("{}.and_then(|v| get_value(v, \"{}\"))", expr, part);
    }
    expr
}

fn generate_node(code: &mut String, node: &Node, indent: usize, scope: &Vec<String>) -> Result<(), String> {
    let pad = "    ".repeat(indent);
    match node {
        Node::Element(el) => {
            code.push_str(&pad);
            code.push_str("r.push_str(\"<");
            code.push_str(&el.tag);
            code.push_str("\")");
            code.push_str(");\n");
            
            if let Some(id) = &el.id {
                code.push_str(&pad);
                code.push_str("r.push_str(\" id=\\\"\"");
                code.push_str(id);
                code.push_str("\\\"\")");
                code.push_str(");\n");
            }
            
            code.push_str(&pad);
            code.push_str("r.push_str(\">\")");
            code.push_str(");\n");
            
            for child in &el.children {
                let _ = generate_node(code, child, indent + 1, scope);
            }
            
            code.push_str(&pad);
            code.push_str("r.push_str(\"</");
            code.push_str(&el.tag);
            code.push_str(">\")");
            code.push_str(");\n");
        }
        Node::Text(t) => {
            // "Hello `name`!" -> ["Hello ", "name", "!"]
            // If backticks were stripped by parser, we might just have `Hello name!`.
            // But checking `README`, parser handles backticks.
            // My `pre_parse` NO LONGER removes backticks.
            // So content is "Hello `name`!".
            
            let parts: Vec<&str> = t.content.split('`').collect();
            for (i, part) in parts.iter().enumerate() {
                if i % 2 == 0 {
                    // Literal
                    if !part.is_empty() {
                        code.push_str(&pad);
                        code.push_str("r.push_str(\"");
                        code.push_str(&part.replace("\"", "\\\""));
                        code.push_str("\");\n");
                    }
                } else {
                    // Variable
                    let expr = resolve_variable(part, scope);
                    code.push_str(&pad);
                    code.push_str(&format!("if let Some(v) = {} {{ ", expr));
                    // Basic string conversion for Value
                    code.push_str("if let Value::Text(s) = v { r.push_str(s); } else { r.push_str(&format!(\"{:?}\", v)); }");
                    code.push_str(" }\n");
                }
            }
        }
        Node::ControlFlow(cf) => match cf {
            crate::ast::ControlFlow::If { condition, then_block, else_block } => {
                let expr = resolve_variable(condition, scope);
                let check = format!("{}.and_then(|v| if let Value::Bool(b) = v {{ Some(*b) }} else {{ None }}).unwrap_or(false)", expr);

                code.push_str(&pad);
                code.push_str(&format!("if {} {{\n", check));
                
                for child in then_block {
                    generate_node(code, child, indent + 1, scope)?;
                }
                
                code.push_str(&pad);
                code.push_str("}");
                
                if let Some(else_nodes) = else_block {
                    code.push_str(" else {\n");
                    for child in else_nodes {
                        generate_node(code, child, indent + 1, scope)?;
                    }
                    code.push_str(&pad);
                    code.push_str("}");
                }
                code.push_str("\n");
            }
            crate::ast::ControlFlow::Each { variable, index_var, iterable, children } => {
                let expr = resolve_variable(iterable, scope);
                
                code.push_str(&pad);
                code.push_str(&format!("if let Some(Value::Array(list)) = {} {{\n", expr));
                code.push_str(&pad);
                code.push_str("    for (i, item) in list.iter().enumerate() {\n");
                
                let mut new_scope = scope.clone();
                new_scope.push(variable.clone());
                if let Some(idx) = index_var {
                    new_scope.push(idx.clone());
                    // NOTE: `i` is usize. We need to make it accessible as Value?
                    // Or just let it be Rust variable?
                    // `resolve_variable` expects variables to be `&Value`.
                    // We might need to wrap `i` in `Value::Integer`.
                }
                
                // Shadow `item` and `i` (if needed) to be consistent with `resolve_variable` expectations?
                // `item` is `&Value`, so that's good.
                // `i` is `usize`.
                
                // Let's generate a shadowing binding for `i` to Value if declared.
                 if let Some(idx) = index_var {
                     code.push_str(&pad);
                     code.push_str(&format!("        let {}_val = Value::Integer(i as i128);\n", idx));
                     code.push_str(&pad);
                     code.push_str(&format!("        let {} = &{}_val;\n", idx, idx));
                 }
                 
                 // `item` is already `&Value` from iterator.
                 // But we need to make sure the name matches `variable`.
                 code.push_str(&pad);
                 code.push_str(&format!("        let {} = item;\n", variable));

                for child in children {
                    generate_node(code, child, indent + 2, &new_scope)?;
                }

                code.push_str(&pad);
                code.push_str("    }\n");
                code.push_str(&pad);
                code.push_str("}\n");
            }
            crate::ast::ControlFlow::Switch { expr, cases, default } => {
                let val_expr = resolve_variable(expr, scope);
                // Assign to a temp variable to avoid re-evaluation (though get_value is cheap)
                // We use a scope-based name to avoid collisions if nested?
                // Or just a block.
                code.push_str(&pad);
                code.push_str("{\n");
                code.push_str(&pad);
                code.push_str(&format!("    let _switch_val = {};\n", val_expr));
                
                let mut first = true;
                for case in cases {
                    code.push_str(&pad);
                    if first {
                        code.push_str("    if ");
                        first = false;
                    } else {
                        code.push_str("    else if ");
                    }
                    
                    // Pattern handling: Currently assuming pattern is a string literal "foo"
                    // We need to convert it to Value::Text("foo".to_string()) comparison.
                    // If the pattern itself is quoted in AST string: "\"admin\""
                    let pattern_clean = case.pattern.trim_matches('"');
                    code.push_str(&format!("_switch_val == Some(&Value::Text(\"{}\".to_string())) {{\n", pattern_clean));
                    
                    for child in &case.children {
                        generate_node(code, child, indent + 2, scope)?;
                    }
                    
                    code.push_str(&pad);
                    code.push_str("    }\n");
                }
                
                if let Some(def_nodes) = default {
                    code.push_str(&pad);
                    if first {
                        code.push_str("    if true {\n"); // Only default exists
                    } else {
                        code.push_str("    else {\n");
                    }
                    
                    for child in def_nodes {
                        generate_node(code, child, indent + 2, scope)?;
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
