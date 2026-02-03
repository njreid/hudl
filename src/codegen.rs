use crate::ast::{Root, Node, SwitchCase};
use crate::expr::{self, Expr, Literal, Op, UnaryOp};

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

    // Runtime Helpers for Expressions
    code.push_str("\nfn hudl_eq(a: &Value, b: &Value) -> bool { a == b }\n");
    code.push_str("fn hudl_truthy(v: &Value) -> bool {\n");
    code.push_str("    match v {\n");
    code.push_str("        Value::Bool(b) => *b,\n");
    code.push_str("        Value::Null => false,\n");
    code.push_str("        Value::Integer(i) => *i != 0,\n");
    code.push_str("        Value::Float(f) => *f != 0.0,\n");
    code.push_str("        Value::Text(s) => !s.is_empty(),\n");
    code.push_str("        Value::Array(a) => !a.is_empty(),\n");
    code.push_str("        _ => true,\n");
    code.push_str("    }\n");
    code.push_str("}\n");
    code.push_str("fn hudl_gt(a: &Value, b: &Value) -> bool {\n");
    code.push_str("    match (a, b) {\n");
    code.push_str("        (Value::Integer(i1), Value::Integer(i2)) => i1 > i2,\n");
    code.push_str("        (Value::Float(f1), Value::Float(f2)) => f1 > f2,\n");
    code.push_str("        _ => false,\n");
    code.push_str("    }\n");
    code.push_str("}\n");
    code.push_str("fn hudl_lt(a: &Value, b: &Value) -> bool {\n");
    code.push_str("    match (a, b) {\n");
    code.push_str("        (Value::Integer(i1), Value::Integer(i2)) => i1 < i2,\n");
    code.push_str("        (Value::Float(f1), Value::Float(f2)) => f1 < f2,\n");
    code.push_str("        _ => false,\n");
    code.push_str("    }\n");
    code.push_str("}\n");
    code.push_str("fn hudl_add(a: &Value, b: &Value) -> Value {\n");
    code.push_str("    match (a, b) {\n");
    code.push_str("        (Value::Integer(i1), Value::Integer(i2)) => Value::Integer(i1 + i2),\n");
    code.push_str("        (Value::Float(f1), Value::Float(f2)) => Value::Float(f1 + f2),\n");
    code.push_str("        (Value::Text(s1), Value::Text(s2)) => Value::Text(format!(\"{}{}\", s1, s2)),\n");
    code.push_str("        _ => Value::Null,\n");
    code.push_str("    }\n");
    code.push_str("}\n");
    code.push_str("fn hudl_sub(a: &Value, b: &Value) -> Value {\n");
    code.push_str("    match (a, b) {\n");
    code.push_str("        (Value::Integer(i1), Value::Integer(i2)) => Value::Integer(i1 - i2),\n");
    code.push_str("        (Value::Float(f1), Value::Float(f2)) => Value::Float(f1 - f2),\n");
    code.push_str("        _ => Value::Null,\n");
    code.push_str("    }\n");
    code.push_str("}\n");
    code.push_str("fn hudl_mul(a: &Value, b: &Value) -> Value {\n");
    code.push_str("    match (a, b) {\n");
    code.push_str("        (Value::Integer(i1), Value::Integer(i2)) => Value::Integer(i1 * i2),\n");
    code.push_str("        (Value::Float(f1), Value::Float(f2)) => Value::Float(f1 * f2),\n");
    code.push_str("        _ => Value::Null,\n");
    code.push_str("    }\n");
    code.push_str("}\n");
    code.push_str("fn hudl_div(a: &Value, b: &Value) -> Value {\n");
    code.push_str("    match (a, b) {\n");
    code.push_str("        (Value::Integer(i1), Value::Integer(i2)) if *i2 != 0 => Value::Integer(i1 / i2),\n");
    code.push_str("        (Value::Float(f1), Value::Float(f2)) if *f2 != 0.0 => Value::Float(f1 / f2),\n");
    code.push_str("        _ => Value::Null,\n");
    code.push_str("    }\n");
    code.push_str("}\n");
    code.push_str("fn hudl_neg(v: &Value) -> Value {\n");
    code.push_str("    match v {\n");
    code.push_str("        Value::Integer(i) => Value::Integer(-i),\n");
    code.push_str("        Value::Float(f) => Value::Float(-f),\n");
    code.push_str("        _ => Value::Null,\n");
    code.push_str("    }\n");
    code.push_str("}\n");
    code.push_str("fn hudl_len(v: &Value) -> Value {\n");
    code.push_str("    match v {\n");
    code.push_str("        Value::Array(a) => Value::Integer(a.len() as i128),\n");
    code.push_str("        Value::Text(s) => Value::Integer(s.len() as i128),\n");
    code.push_str("        _ => Value::Integer(0),\n");
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
        code.push_str("    pack(result_ptr, result_len)\n} \n");
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

fn generate_expr_code(expr: &Expr, scope: &Vec<String>) -> String {
    match expr {
        Expr::Literal(lit) => match lit {
            Literal::String(s) => format!("Value::Text(\"{}\".to_string())", s),
            Literal::Int(i) => format!("Value::Integer({} as i128)", i),
            Literal::Float(f) => format!("Value::Float({})", f),
            Literal::Bool(b) => format!("Value::Bool({})", b),
            Literal::Null => "Value::Null".to_string(),
        },
        Expr::Variable(path) => {
             let res = resolve_variable(path, scope);
             format!("{}.cloned().unwrap_or(Value::Null)", res)
        },
        Expr::Binary(left, op, right) => {
             let l = generate_expr_code(left, scope);
             let r = generate_expr_code(right, scope);
             match op {
                 Op::Eq => format!("Value::Bool(hudl_eq(&{}, &{}))", l, r),
                 Op::Neq => format!("Value::Bool(!hudl_eq(&{}, &{}))", l, r),
                 Op::Gt => format!("Value::Bool(hudl_gt(&{}, &{}))", l, r),
                 Op::Lt => format!("Value::Bool(hudl_lt(&{}, &{}))", l, r),
                 Op::Gte => format!("Value::Bool(hudl_gt(&{}, &{}) || hudl_eq(&{}, &{}))", l, r, l, r),
                 Op::Lte => format!("Value::Bool(hudl_lt(&{}, &{}) || hudl_eq(&{}, &{}))", l, r, l, r),
                 Op::And => format!("Value::Bool(hudl_truthy(&{}) && hudl_truthy(&{}))", l, r),
                 Op::Or => format!("Value::Bool(hudl_truthy(&{}) || hudl_truthy(&{}))", l, r),
                 Op::Add => format!("hudl_add(&{}, &{})", l, r),
                 Op::Sub => format!("hudl_sub(&{}, &{})", l, r),
                 Op::Mul => format!("hudl_mul(&{}, &{})", l, r),
                 Op::Div => format!("hudl_div(&{}, &{})", l, r),
             }
        },
        Expr::Unary(op, expr) => {
             let e = generate_expr_code(expr, scope);
             match op {
                 UnaryOp::Not => format!("Value::Bool(!hudl_truthy(&{}))", e),
                 UnaryOp::Neg => format!("hudl_neg(&{})", e),
             }
        },
        Expr::Call(name, args) => {
             if name == "len" && args.len() == 1 {
                 let arg = generate_expr_code(&args[0], scope);
                 format!("hudl_len(&{})", arg)
             } else {
                 // Unknown function - return null (could be extended to support more built-ins)
                 "Value::Null".to_string()
             }
        },
        Expr::MethodCall(receiver, method, args) => {
            // Method calls like tx.CreatedAt.Format(time.RFC822) need special handling.
            // Since WASM can't call Go methods directly, method calls should be
            // pre-computed on the Go side and passed in the CBOR data.
            // For now, we try to resolve the receiver and look up a computed field.
            let recv = generate_expr_code(receiver, scope);
            // Try to access a pre-computed field named after the method call
            // e.g., tx.CreatedAt.Format_RFC822 for tx.CreatedAt.Format(time.RFC822)
            let computed_field = if args.is_empty() {
                method.clone()
            } else {
                // For method calls with args, the Go side should provide computed values
                format!("{}__computed", method)
            };
            format!("get_value(&{}, \"{}\").cloned().unwrap_or(Value::Null)", recv, computed_field)
        }
    }
}

fn generate_node(code: &mut String, node: &Node, indent: usize, scope: &Vec<String>) -> Result<(), String> {
    let pad = "    ".repeat(indent);
    match node {
        Node::Element(el) => {
            // Opening tag
            code.push_str(&pad);
            code.push_str("r.push_str(\"<");
            code.push_str(&el.tag);
            code.push_str("\");\n");

            // ID attribute
            if let Some(id) = &el.id {
                code.push_str(&pad);
                code.push_str("r.push_str(\" id=\\\"");
                code.push_str(id);
                code.push_str("\\\"\");\n");
            }

            // Class attribute
            if !el.classes.is_empty() {
                code.push_str(&pad);
                code.push_str("r.push_str(\" class=\\\"");
                code.push_str(&el.classes.join(" "));
                code.push_str("\\\"\");\n");
            }

            // Other attributes
            for (key, value) in &el.attributes {
                code.push_str(&pad);
                code.push_str("r.push_str(\" ");
                code.push_str(key);
                code.push_str("=\\\"");
                code.push_str(value);
                code.push_str("\\\"\");\n");
            }

            // Close opening tag
            code.push_str(&pad);
            code.push_str("r.push_str(\">\");\n");

            // Children
            for child in &el.children {
                generate_node(code, child, indent + 1, scope)?;
            }

            // Closing tag
            code.push_str(&pad);
            code.push_str("r.push_str(\"</");
            code.push_str(&el.tag);
            code.push_str(">\");\n");
        }
        Node::Text(t) => {
            let parts: Vec<&str> = t.content.split('`').collect();
            for (i, part) in parts.iter().enumerate() {
                if i % 2 == 0 {
                    if !part.is_empty() {
                        code.push_str(&pad);
                        code.push_str("r.push_str(\"");
                        code.push_str(&part.replace("\"", "\\\""));
                        code.push_str("\");\n");
                    }
                } else {
                    if let Ok(expr) = expr::parse(part) {
                         let val_code = generate_expr_code(&expr, scope);
                         code.push_str(&pad);
                         code.push_str("{\n");
                         code.push_str(&pad);
                         code.push_str(&format!("    let v = {};\n", val_code));
                         code.push_str(&pad);
                         code.push_str("    if let Value::Text(s) = v {{ r.push_str(&s); }} else {{ r.push_str(&format!(\"{:?}\", v)); }}\n");
                         code.push_str(&pad);
                         code.push_str("}\n");
                    } else {
                         return Err(format!("Failed to parse expression in backticks: {}", part));
                    }
                }
            }
        }
        Node::ControlFlow(cf) => match cf {
            crate::ast::ControlFlow::If { condition, then_block, else_block } => {
                let expr = expr::parse(condition).map_err(|e| format!("Parse error in if: {}", e))?;
                let val_code = generate_expr_code(&expr, scope);
                
                code.push_str(&pad);
                code.push_str(&format!("if hudl_truthy(&{}) {{\n", val_code));
                
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
            crate::ast::ControlFlow::Each { binding, iterable, body } => {
                let expr_ast = expr::parse(iterable).map_err(|e| format!("Parse error in each: {}", e))?;
                let val_code = generate_expr_code(&expr_ast, scope);

                code.push_str(&pad);
                code.push_str(&format!("if let Value::Array(list) = {} {{\n", val_code));
                code.push_str(&pad);
                code.push_str("    for (_index, _item) in list.iter().enumerate() {\n");

                let mut new_scope = scope.clone();
                new_scope.push(binding.clone());
                new_scope.push("_index".to_string());

                // Create _index as Value for CEL access
                code.push_str(&pad);
                code.push_str("        let _index_val = Value::Integer(_index as i128);\n");
                code.push_str(&pad);
                code.push_str("        let _index = &_index_val;\n");

                code.push_str(&pad);
                code.push_str(&format!("        let {} = _item;\n", binding));

                for child in body {
                    generate_node(code, child, indent + 2, &new_scope)?;
                }

                code.push_str(&pad);
                code.push_str("    }\n");
                code.push_str(&pad);
                code.push_str("}\n");
            }
            crate::ast::ControlFlow::Switch { expr, cases, default } => {
                let expr_ast = expr::parse(expr).map_err(|e| format!("Parse error in switch: {}", e))?;
                let val_code = generate_expr_code(&expr_ast, scope);

                code.push_str(&pad);
                code.push_str("{\n");
                code.push_str(&pad);
                code.push_str(&format!("    let _switch_val = {};\n", val_code));

                let mut first = true;
                for case in cases {
                    let SwitchCase(pattern, children) = case;
                    code.push_str(&pad);
                    if first {
                        code.push_str("    if ");
                        first = false;
                    } else {
                        code.push_str("    else if ");
                    }

                    // Pattern comparison - for enum values, compare as string
                    code.push_str(&format!("_switch_val == Value::Text(\"{}\".to_string()) {{\n", pattern));

                    for child in children {
                        generate_node(code, child, indent + 2, scope)?;
                    }

                    code.push_str(&pad);
                    code.push_str("    }\n");
                }

                if let Some(def_nodes) = default {
                    code.push_str(&pad);
                    if first {
                        code.push_str("    if true {\n");
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
