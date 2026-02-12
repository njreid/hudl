//! Legacy code generation (non-CEL) using Protocol Buffer wire format.
//!
//! This module generates simpler Rust code that doesn't use CEL.
//! For new code, prefer `codegen_cel` which provides full CEL expression support.

use crate::ast::{Node, Root, SwitchCase, DatastarAttr};
use crate::expr::{self, Expr, Literal, Op, UnaryOp};

pub fn generate_wasm_lib(views: Vec<(String, Root)>) -> Result<String, String> {
    let mut code = String::new();
    code.push_str("use std::mem;\n");
    code.push_str("use std::slice;\n");
    code.push_str("use std::collections::HashMap;\n\n");

    code.push_str("#[no_mangle]\npub extern \"C\" fn hudl_malloc(s: usize) -> *mut u8 { let mut v = Vec::with_capacity(s); let p = v.as_mut_ptr(); mem::forget(v); p }\n");
    code.push_str("#[no_mangle]\npub extern \"C\" fn hudl_free(p: *mut u8, s: usize) { unsafe { let _ = Vec::from_raw_parts(p, s, s); } }\n");
    code.push_str("fn pack(p: *const u8, l: usize) -> u64 { ((p as u64) << 32) | (l as u64) }\n\n");

    // Proto wire format decoder and Value type
    code.push_str(PROTO_VALUE_AND_DECODER);

    // Runtime Helpers for Expressions
    code.push_str(RUNTIME_HELPERS);

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
        code.push_str("        decode_proto_to_value(slice)\n");
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

const PROTO_VALUE_AND_DECODER: &str = r#"
/// Simple value type for template rendering
#[derive(Clone, Debug, PartialEq)]
enum Value {
    Null,
    Bool(bool),
    Integer(i128),
    Float(f64),
    Text(String),
    Array(Vec<Value>),
    Map(HashMap<String, Value>),
}

// Proto wire format types
const WIRE_VARINT: u32 = 0;
const WIRE_FIXED64: u32 = 1;
const WIRE_LENGTH_DELIMITED: u32 = 2;
const WIRE_FIXED32: u32 = 5;

struct ProtoReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ProtoReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    fn read_varint(&mut self) -> Option<u64> {
        let mut result: u64 = 0;
        let mut shift = 0;
        loop {
            if self.pos >= self.data.len() {
                return None;
            }
            let byte = self.data[self.pos];
            self.pos += 1;
            result |= ((byte & 0x7f) as u64) << shift;
            if byte & 0x80 == 0 {
                return Some(result);
            }
            shift += 7;
            if shift >= 64 {
                return None;
            }
        }
    }

    fn read_fixed32(&mut self) -> Option<u32> {
        if self.pos + 4 > self.data.len() {
            return None;
        }
        let bytes = &self.data[self.pos..self.pos + 4];
        self.pos += 4;
        Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_fixed64(&mut self) -> Option<u64> {
        if self.pos + 8 > self.data.len() {
            return None;
        }
        let bytes = &self.data[self.pos..self.pos + 8];
        self.pos += 8;
        Some(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
            bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_length_delimited(&mut self) -> Option<&'a [u8]> {
        let len = self.read_varint()? as usize;
        if self.pos + len > self.data.len() {
            return None;
        }
        let result = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Some(result)
    }
}

/// Decode proto wire format to Value
/// Note: Without schema, we decode to a map keyed by field number as string
fn decode_proto_to_value(data: &[u8]) -> Value {
    let mut reader = ProtoReader::new(data);
    let mut fields: HashMap<String, Value> = HashMap::new();

    while reader.remaining() > 0 {
        let tag = match reader.read_varint() {
            Some(t) => t,
            None => break,
        };
        let field_number = (tag >> 3) as u32;
        let wire_type = (tag & 0x7) as u32;

        let value = match wire_type {
            WIRE_VARINT => reader.read_varint().map(|n| Value::Integer(n as i128)),
            WIRE_FIXED64 => reader.read_fixed64().map(|n| Value::Float(f64::from_bits(n))),
            WIRE_LENGTH_DELIMITED => {
                reader.read_length_delimited().map(|b| {
                    // Try to decode as UTF-8 string, fallback to nested message
                    if let Ok(s) = std::str::from_utf8(b) {
                        Value::Text(s.to_string())
                    } else {
                        // Could be a nested message - try to decode recursively
                        decode_proto_to_value(b)
                    }
                })
            }
            WIRE_FIXED32 => reader.read_fixed32().map(|n| Value::Float(f32::from_bits(n) as f64)),
            _ => None,
        };

        if let Some(v) = value {
            let key = field_number.to_string();
            // Handle repeated fields
            if let Some(existing) = fields.remove(&key) {
                match existing {
                    Value::Array(mut arr) => {
                        arr.push(v);
                        fields.insert(key, Value::Array(arr));
                    }
                    other => {
                        fields.insert(key, Value::Array(vec![other, v]));
                    }
                }
            } else {
                fields.insert(key, v);
            }
        }
    }

    Value::Map(fields)
}

fn get_value<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    match v {
        Value::Map(m) => m.get(key),
        _ => None,
    }
}

"#;

const RUNTIME_HELPERS: &str = r#"
fn hudl_eq(a: &Value, b: &Value) -> bool { a == b }
fn hudl_truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Null => false,
        Value::Integer(i) => *i != 0,
        Value::Float(f) => *f != 0.0,
        Value::Text(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        _ => true,
    }
}
fn hudl_gt(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Integer(i1), Value::Integer(i2)) => i1 > i2,
        (Value::Float(f1), Value::Float(f2)) => f1 > f2,
        _ => false,
    }
}
fn hudl_lt(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Integer(i1), Value::Integer(i2)) => i1 < i2,
        (Value::Float(f1), Value::Float(f2)) => f1 < f2,
        _ => false,
    }
}
fn hudl_add(a: &Value, b: &Value) -> Value {
    match (a, b) {
        (Value::Integer(i1), Value::Integer(i2)) => Value::Integer(i1 + i2),
        (Value::Float(f1), Value::Float(f2)) => Value::Float(f1 + f2),
        (Value::Text(s1), Value::Text(s2)) => Value::Text(format!("{}{}", s1, s2)),
        _ => Value::Null,
    }
}
fn hudl_sub(a: &Value, b: &Value) -> Value {
    match (a, b) {
        (Value::Integer(i1), Value::Integer(i2)) => Value::Integer(i1 - i2),
        (Value::Float(f1), Value::Float(f2)) => Value::Float(f1 - f2),
        _ => Value::Null,
    }
}
fn hudl_mul(a: &Value, b: &Value) -> Value {
    match (a, b) {
        (Value::Integer(i1), Value::Integer(i2)) => Value::Integer(i1 * i2),
        (Value::Float(f1), Value::Float(f2)) => Value::Float(f1 * f2),
        _ => Value::Null,
    }
}
fn hudl_div(a: &Value, b: &Value) -> Value {
    match (a, b) {
        (Value::Integer(i1), Value::Integer(i2)) if *i2 != 0 => Value::Integer(i1 / i2),
        (Value::Float(f1), Value::Float(f2)) if *f2 != 0.0 => Value::Float(f1 / f2),
        _ => Value::Null,
    }
}
fn hudl_neg(v: &Value) -> Value {
    match v {
        Value::Integer(i) => Value::Integer(-i),
        Value::Float(f) => Value::Float(-f),
        _ => Value::Null,
    }
}
fn hudl_len(v: &Value) -> Value {
    match v {
        Value::Array(a) => Value::Integer(a.len() as i128),
        Value::Text(s) => Value::Integer(s.len() as i128),
        _ => Value::Integer(0),
    }
}

"#;

fn resolve_variable(path: &str, scope: &[String]) -> String {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() {
        return "None".to_string();
    }

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

fn generate_expr_code(expr: &Expr, scope: &[String]) -> String {
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
        }
        Expr::Binary(left, op, right) => {
            let l = generate_expr_code(left, scope);
            let r = generate_expr_code(right, scope);
            match op {
                Op::Eq => format!("Value::Bool(hudl_eq(&{}, &{}))", l, r),
                Op::Neq => format!("Value::Bool(!hudl_eq(&{}, &{}))", l, r),
                Op::Gt => format!("Value::Bool(hudl_gt(&{}, &{}))", l, r),
                Op::Lt => format!("Value::Bool(hudl_lt(&{}, &{}))", l, r),
                Op::Gte => {
                    format!(
                        "Value::Bool(hudl_gt(&{}, &{}) || hudl_eq(&{}, &{}))",
                        l, r, l, r
                    )
                }
                Op::Lte => {
                    format!(
                        "Value::Bool(hudl_lt(&{}, &{}) || hudl_eq(&{}, &{}))",
                        l, r, l, r
                    )
                }
                Op::And => format!("Value::Bool(hudl_truthy(&{}) && hudl_truthy(&{}))", l, r),
                Op::Or => format!("Value::Bool(hudl_truthy(&{}) || hudl_truthy(&{}))", l, r),
                Op::Add => format!("hudl_add(&{}, &{})", l, r),
                Op::Sub => format!("hudl_sub(&{}, &{})", l, r),
                Op::Mul => format!("hudl_mul(&{}, &{})", l, r),
                Op::Div => format!("hudl_div(&{}, &{})", l, r),
            }
        }
        Expr::Unary(op, expr) => {
            let e = generate_expr_code(expr, scope);
            match op {
                UnaryOp::Not => format!("Value::Bool(!hudl_truthy(&{}))", e),
                UnaryOp::Neg => format!("hudl_neg(&{})", e),
            }
        }
        Expr::Call(name, args) => {
            if name == "len" && args.len() == 1 {
                let arg = generate_expr_code(&args[0], scope);
                format!("hudl_len(&{})", arg)
            } else {
                // Unknown function - return null
                "Value::Null".to_string()
            }
        }
        Expr::MethodCall(receiver, method, args) => {
            let recv = generate_expr_code(receiver, scope);
            let computed_field = if args.is_empty() {
                method.clone()
            } else {
                format!("{}__computed", method)
            };
            format!(
                "get_value(&{}, \"{}\").cloned().unwrap_or(Value::Null)",
                recv, computed_field
            )
        }
    }
}

fn generate_node(
    code: &mut String,
    node: &Node,
    indent: usize,
    scope: &[String],
) -> Result<(), String> {
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

            // Datastar attributes
            for attr in &el.datastar {
                let (html_attr, html_val) = datastar_attr_to_html(attr);
                code.push_str(&pad);
                code.push_str("r.push_str(\" ");
                code.push_str(&html_attr);
                if let Some(val) = html_val {
                    code.push_str("=\\\"");
                    code.push_str(&val.replace('"', "&quot;"));
                    code.push_str("\\\"");
                }
                code.push_str("\");\n");
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
                        code.push_str(&part.replace('"', "\\\""));
                        code.push_str("\");\n");
                    }
                } else if let Ok(expr) = expr::parse(part) {
                    let val_code = generate_expr_code(&expr, scope);
                    code.push_str(&pad);
                    code.push_str("{\n");
                    code.push_str(&pad);
                    code.push_str(&format!("    let v = {};\n", val_code));
                    code.push_str(&pad);
                    code.push_str("    if let Value::Text(s) = v { r.push_str(&s); } else { r.push_str(&format!(\"{:?}\", v)); }\n");
                    code.push_str(&pad);
                    code.push_str("}\n");
                } else {
                    return Err(format!(
                        "Failed to parse expression in backticks: {}",
                        part
                    ));
                }
            }
        }
        Node::ControlFlow(cf) => match cf {
            crate::ast::ControlFlow::If {
                condition,
                then_block,
                else_block,
            } => {
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
            crate::ast::ControlFlow::Each {
                binding,
                iterable,
                body,
            } => {
                let expr_ast =
                    expr::parse(iterable).map_err(|e| format!("Parse error in each: {}", e))?;
                let val_code = generate_expr_code(&expr_ast, scope);

                code.push_str(&pad);
                code.push_str(&format!("if let Value::Array(list) = {} {{\n", val_code));
                code.push_str(&pad);
                code.push_str("    for (_index, _item) in list.iter().enumerate() {\n");

                let mut new_scope = scope.to_vec();
                new_scope.push(binding.clone());
                new_scope.push("_index".to_string());

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
            crate::ast::ControlFlow::Switch {
                expr,
                cases,
                default,
            } => {
                let expr_ast =
                    expr::parse(expr).map_err(|e| format!("Parse error in switch: {}", e))?;
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

                    code.push_str(&format!(
                        "_switch_val == Value::Text(\"{}\".to_string()) {{\n",
                        pattern
                    ));

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

/// Convert a Datastar attribute from Hudl format to HTML data-* format
/// Returns (html_attr_name, optional_value)
pub fn datastar_attr_to_html(attr: &DatastarAttr) -> (String, Option<String>) {
    let name = &attr.name;
    let modifiers = &attr.modifiers;

    // Build modifier suffix: ["once", "prevent"] -> "__once__prevent"
    // Modifier params use ":" in Hudl but "." in Datastar HTML (e.g., debounce:300ms -> __debounce.300ms)
    let mod_suffix = if modifiers.is_empty() {
        String::new()
    } else {
        let html_mods: Vec<String> = modifiers.iter().map(|m| m.replace(':', ".")).collect();
        format!("__{}", html_mods.join("__"))
    };

    // Handle different attribute types
    let html_name = if name.starts_with('.') {
        // .active -> data-class-active
        format!("data-class-{}{}", &name[1..], mod_suffix)
    } else if name.starts_with("class:") {
        // class:active -> data-class-active
        format!("data-class-{}{}", &name[6..], mod_suffix)
    } else if name.starts_with("on:") {
        // Event handlers
        let event = &name[3..];
        if event == "fetch" {
            // on:fetch -> data-on:datastar-fetch
            format!("data-on:datastar-fetch{}", mod_suffix)
        } else if is_standard_dom_event(event) {
            // Standard DOM events: on:click -> data-on-click
            format!("data-on-{}{}", event, mod_suffix)
        } else {
            // Custom events preserve colon: on:myEvent -> data-on:my-event
            format!("data-on:{}{}", to_kebab_case(event), mod_suffix)
        }
    } else if name.starts_with("let:") {
        // Signal or computed
        let signal_name = &name[4..];
        if is_computed_expression(attr.value.as_deref()) {
            format!("data-computed-{}{}", signal_name, mod_suffix)
        } else {
            format!("data-signals-{}{}", signal_name, mod_suffix)
        }
    } else if name == "show" {
        format!("data-show{}", mod_suffix)
    } else if name == "text" {
        format!("data-text{}", mod_suffix)
    } else if name == "ref" {
        format!("data-ref{}", mod_suffix)
    } else if name == "persist" {
        format!("data-persist{}", mod_suffix)
    } else if name == "teleport" {
        format!("data-teleport{}", mod_suffix)
    } else if name == "scrollIntoView" {
        format!("data-scroll-into-view{}", mod_suffix)
    } else if name == "bind" {
        format!("data-bind{}", mod_suffix)
    } else {
        // Unknown/HTML attributes: disabled -> data-attr-disabled
        format!("data-attr-{}{}", name, mod_suffix)
    };

    // Process the value
    let html_value = attr.value.as_ref().map(|v| {
        // For signals with string values, wrap in quotes for Datastar
        if name.starts_with("let:") && !is_computed_expression(Some(v)) {
            // Check if it's already a quoted string or a number/boolean
            if v.starts_with('"') || v.starts_with('\'') || v.parse::<f64>().is_ok()
               || v == "true" || v == "false" || v == "null" {
                v.clone()
            } else {
                // Bare identifier - wrap in single quotes for Datastar
                format!("'{}'", v)
            }
        } else {
            v.clone()
        }
    });

    (html_name, html_value)
}

/// Check if a value is a computed expression (has operators or function calls)
fn is_computed_expression(value: Option<&str>) -> bool {
    match value {
        None => false,
        Some(v) => {
            // Check for operators
            let has_operator = v.contains('+') || v.contains('-') || v.contains('*')
                || v.contains('/') || v.contains("==") || v.contains("!=")
                || v.contains("&&") || v.contains("||") || v.contains('>')
                || v.contains('<') || v.contains('?') || v.contains(':');
            // Check for function calls (parens not at start)
            let has_function = v.contains('(') && !v.starts_with('(');
            // Check for signal references
            let has_signal_ref = v.contains('$');

            has_operator || has_function || has_signal_ref
        }
    }
}

/// Check if an event name is a standard DOM event
fn is_standard_dom_event(event: &str) -> bool {
    // Handle key modifiers like "keydown.enter"
    let base_event = event.split('.').next().unwrap_or(event);

    matches!(base_event,
        "click" | "dblclick" | "mousedown" | "mouseup" | "mousemove" | "mouseenter" |
        "mouseleave" | "mouseover" | "mouseout" | "wheel" | "contextmenu" |
        "keydown" | "keyup" | "keypress" |
        "focus" | "blur" | "focusin" | "focusout" |
        "input" | "change" | "submit" | "reset" | "invalid" |
        "scroll" | "resize" |
        "load" | "unload" | "error" | "abort" |
        "drag" | "dragstart" | "dragend" | "dragenter" | "dragleave" | "dragover" | "drop" |
        "touchstart" | "touchend" | "touchmove" | "touchcancel" |
        "pointerdown" | "pointerup" | "pointermove" | "pointerenter" | "pointerleave" |
        "animationstart" | "animationend" | "animationiteration" |
        "transitionstart" | "transitionend" | "transitionrun" | "transitioncancel" |
        "copy" | "cut" | "paste" |
        "select" | "selectstart" |
        // Datastar special events that use dash
        "intersect"
    )
}

/// Convert camelCase to kebab-case
fn to_kebab_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                result.push('-');
            }
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }
    result
}
