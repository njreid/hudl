//! Code generation using CEL for expression evaluation.
//!
//! This module generates Rust code that:
//! - Uses cel_interpreter for expression evaluation
//! - Decodes Protocol Buffer wire format input
//! - Evaluates CEL expressions at runtime
//! - Generates scoped CSS for component styles

use crate::ast::{Node, Root, SwitchCase, datastar_attr_to_html};
use crate::proto::{ProtoField, ProtoSchema, ProtoType};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// Generate a unique scope ID for a component based on its name
fn generate_scope_id(component_name: &str) -> String {
    let mut hasher = DefaultHasher::new();
    component_name.hash(&mut hasher);
    let hash = hasher.finish();
    format!("h{:x}", hash & 0xFFFFFF) // 6 hex chars for readability
}

/// Generate the WASM library code using CEL with proto input.
pub fn generate_wasm_lib_cel(
    views: Vec<(String, Root)>,
    schema: &ProtoSchema,
) -> Result<String, String> {
    let mut code = String::new();

    // Standard imports
    code.push_str("use std::mem;\n");
    code.push_str("use std::slice;\n");
    code.push_str("use std::sync::Arc;\n");
    code.push_str("use std::collections::HashMap;\n");
    code.push_str("use cel_interpreter::{Context, Program, Value as CelValue};\n");
    code.push_str("use cel_interpreter::objects::{Key, Map as CelMap};\n\n");

    // Embed the schema definitions
    // We need the struct definitions in the generated code to deserialize the schema
    code.push_str(SCHEMA_DEFINITIONS);

    // Serialize schema to JSON and embed as a const string
    let schema_json = serde_json::to_string(schema).map_err(|e| e.to_string())?;
    // Escape the JSON string for Rust string literal
    let escaped_json = schema_json.replace('\\', "\\\\").replace('"', "\\\"");
    
    code.push_str(&format!(
        "const SCHEMA_JSON: &str = \"{}\";\n\n",
        escaped_json
    ));

    code.push_str("lazy_static::lazy_static! {\n");
    code.push_str("    static ref SCHEMA: ProtoSchema = serde_json::from_str(SCHEMA_JSON).unwrap();\n");
    code.push_str("}\n\n");

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

    // Proto wire format decoder
    code.push_str(crate::proto::PROTO_DECODER_SRC);

    // Proto wire format encoder
    code.push_str(crate::proto::PROTO_ENCODER_SRC);

    // CEL evaluation helpers
    code.push_str(CEL_HELPERS);

    // Generate message decoders for all messages in schema
    for (name, msg) in &schema.messages {
        generate_message_decoder(&mut code, name, &msg.fields, schema)?;
    }

    // Build map of component name -> data type
    let mut component_types = HashMap::new();
    for (name, root) in &views {
        let dt = root.data_type.clone().unwrap_or_default();
        component_types.insert(name.clone(), dt);
    }

    // Generate view render functions
    for (name, root) in views {
        // Use the data_type from the root metadata
        let data_type = root.data_type.as_deref();
        generate_view_function(&mut code, &name, &root, schema, data_type, &component_types)?;
    }

    Ok(code)
}

/// Fallback: Generate without proto schema (for backward compatibility)
pub fn generate_wasm_lib_cel_simple(views: Vec<(String, Root)>) -> Result<String, String> {
    let schema = ProtoSchema::default();
    generate_wasm_lib_cel(views, &schema)
}

const CEL_HELPERS: &str = r#"
fn cel_eval(expr: &str, ctx: &Context) -> CelValue {
    match Program::compile(expr) {
        Ok(prog) => prog.execute(ctx).unwrap_or(CelValue::Null),
        Err(_) => CelValue::Null,
    }
}

/// Evaluate CEL expression with HTML escaping and error placeholder
fn cel_eval_safe(expr: &str, ctx: &Context) -> String {
    match Program::compile(expr) {
        Ok(prog) => match prog.execute(ctx) {
            Ok(val) => html_escape(&cel_to_string(&val)),
            Err(_) => format!("<span class=\"hudl-error\" title=\"Error evaluating: {}\">ERR</span>", html_escape(expr)),
        },
        Err(_) => format!("<span class=\"hudl-error\" title=\"Invalid expression: {}\">ERR</span>", html_escape(expr)),
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

fn proto_value_to_cel(v: &ProtoValue) -> CelValue {
    match v {
        ProtoValue::Varint(n) => CelValue::Int(*n as i64),
        ProtoValue::SignedVarint(n) => CelValue::Int(*n),
        ProtoValue::Fixed32(n) => CelValue::Int(*n as i64),
        ProtoValue::Fixed64(n) => CelValue::Int(*n as i64),
        ProtoValue::Float(f) => CelValue::Float(*f as f64),
        ProtoValue::Double(d) => CelValue::Float(*d),
        ProtoValue::String(s) => CelValue::String(Arc::new(s.clone())),
        ProtoValue::Bytes(b) => CelValue::Bytes(Arc::new(b.clone())),
        ProtoValue::Bool(b) => CelValue::Bool(*b),
        ProtoValue::Message(m) => {
            let cel_map: HashMap<Key, CelValue> = m
                .iter()
                .map(|(k, v)| (Key::Int(*k as i64), proto_value_to_cel(v)))
                .collect();
            CelValue::Map(CelMap { map: Arc::new(cel_map) })
        }
        ProtoValue::Repeated(list) => {
            CelValue::List(Arc::new(list.iter().map(proto_value_to_cel).collect()))
        }
    }
}

"#;

const SCHEMA_DEFINITIONS: &str = r#"
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProtoSchema {
    pub messages: HashMap<String, ProtoMessage>,
    pub enums: HashMap<String, ProtoEnum>,
    pub imports: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtoMessage {
    pub name: String,
    pub fields: Vec<ProtoField>,
    pub nested_messages: Vec<ProtoMessage>,
    pub nested_enums: Vec<ProtoEnum>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtoField {
    pub name: String,
    pub field_type: ProtoType,
    pub number: u32,
    pub repeated: bool,
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProtoType {
    Double, Float, Int32, Int64, Uint32, Uint64, Sint32, Sint64,
    Fixed32, Fixed64, Sfixed32, Sfixed64, Bool, String, Bytes,
    Message(String), Enum(String), Map(Box<ProtoType>, Box<ProtoType>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtoEnum {
    pub name: String,
    pub values: Vec<ProtoEnumValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtoEnumValue {
    pub name: String,
    pub number: i32,
}

impl ProtoSchema {
    pub fn get_message(&self, name: &str) -> Option<&ProtoMessage> {
        self.messages.get(name)
    }
}
"#;

fn generate_message_decoder(
    code: &mut String,
    name: &str,
    fields: &[ProtoField],
    schema: &ProtoSchema,
) -> Result<(), String> {
    let fn_name = format!("decode_{}", name.to_lowercase());

    code.push_str(&format!(
        "/// Decode {} proto message to CEL context\n",
        name
    ));
    code.push_str(&format!(
        "fn {}(data: &[u8], ctx: &mut Context) {{\n",
        fn_name
    ));
    code.push_str("    let fields = decode_proto_message(data);\n\n");

    for field in fields {
        generate_field_decoder(code, field, schema)?;
    }

    code.push_str("}\n\n");

    Ok(())
}

fn generate_field_decoder(
    code: &mut String,
    field: &ProtoField,
    schema: &ProtoSchema,
) -> Result<(), String> {
    let field_num = field.number;
    let field_name = &field.name;

    code.push_str(&format!("    // Field {}: {}\n", field_num, field_name));
    code.push_str(&format!("    if let Some(v) = fields.get(&{}) {{\n", field_num));

    if field.repeated {
        // Handle repeated field
        code.push_str("        let list = match v {\n");
        code.push_str("            ProtoValue::Repeated(items) => items.iter().map(|item| {\n");
        generate_value_conversion(code, &field.field_type, "item", schema, "                ")?;
        code.push_str("            }).collect(),\n");
        code.push_str("            single => vec![{\n");
        generate_value_conversion(code, &field.field_type, "single", schema, "                ")?;
        code.push_str("            }],\n");
        code.push_str("        };\n");
        code.push_str(&format!(
            "        let _ = ctx.add_variable(\"{}\", CelValue::List(Arc::new(list)));\n",
            field_name
        ));
    } else {
        // Handle singular field
        code.push_str("        let cel_val = {\n");
        generate_value_conversion(code, &field.field_type, "v", schema, "            ")?;
        code.push_str("        };\n");
        code.push_str(&format!(
            "        let _ = ctx.add_variable(\"{}\", cel_val);\n",
            field_name
        ));
    }

    code.push_str("    }\n\n");

    Ok(())
}

fn generate_value_conversion(
    code: &mut String,
    proto_type: &ProtoType,
    var_name: &str,
    schema: &ProtoSchema,
    indent: &str,
) -> Result<(), String> {
    match proto_type {
        ProtoType::String => {
            code.push_str(&format!("{}match {} {{\n", indent, var_name));
            code.push_str(&format!(
                "{}    ProtoValue::Bytes(b) => CelValue::String(Arc::new(String::from_utf8_lossy(b).to_string())),\n",
                indent
            ));
            code.push_str(&format!(
                "{}    ProtoValue::String(s) => CelValue::String(Arc::new(s.clone())),\n",
                indent
            ));
            code.push_str(&format!("{}    _ => CelValue::Null,\n", indent));
            code.push_str(&format!("{}}}\n", indent));
        }
        ProtoType::Int32 | ProtoType::Int64 | ProtoType::Uint32 | ProtoType::Uint64 => {
            code.push_str(&format!("{}match {} {{\n", indent, var_name));
            code.push_str(&format!(
                "{}    ProtoValue::Varint(n) => CelValue::Int(*n as i64),\n",
                indent
            ));
            code.push_str(&format!(
                "{}    ProtoValue::SignedVarint(n) => CelValue::Int(*n),\n",
                indent
            ));
            code.push_str(&format!("{}    _ => CelValue::Null,\n", indent));
            code.push_str(&format!("{}}}\n", indent));
        }
        ProtoType::Sint32 | ProtoType::Sint64 => {
            code.push_str(&format!("{}match {} {{\n", indent, var_name));
            code.push_str(&format!(
                "{}    ProtoValue::Varint(n) => {{\n",
                indent
            ));
            code.push_str(&format!(
                "{}        // ZigZag decode\n",
                indent
            ));
            code.push_str(&format!(
                "{}        CelValue::Int(((*n >> 1) as i64) ^ -((*n & 1) as i64))\n",
                indent
            ));
            code.push_str(&format!("{}    }}\n", indent));
            code.push_str(&format!(
                "{}    ProtoValue::SignedVarint(n) => CelValue::Int(*n),\n",
                indent
            ));
            code.push_str(&format!("{}    _ => CelValue::Null,\n", indent));
            code.push_str(&format!("{}}}\n", indent));
        }
        ProtoType::Bool => {
            code.push_str(&format!("{}match {} {{\n", indent, var_name));
            code.push_str(&format!(
                "{}    ProtoValue::Varint(n) => CelValue::Bool(*n != 0),\n",
                indent
            ));
            code.push_str(&format!(
                "{}    ProtoValue::Bool(b) => CelValue::Bool(*b),\n",
                indent
            ));
            code.push_str(&format!("{}    _ => CelValue::Null,\n", indent));
            code.push_str(&format!("{}}}\n", indent));
        }
        ProtoType::Float => {
            code.push_str(&format!("{}match {} {{\n", indent, var_name));
            code.push_str(&format!(
                "{}    ProtoValue::Fixed32(n) => CelValue::Float(f32::from_bits(*n) as f64),\n",
                indent
            ));
            code.push_str(&format!(
                "{}    ProtoValue::Float(f) => CelValue::Float(*f as f64),\n",
                indent
            ));
            code.push_str(&format!("{}    _ => CelValue::Null,\n", indent));
            code.push_str(&format!("{}}}\n", indent));
        }
        ProtoType::Double => {
            code.push_str(&format!("{}match {} {{\n", indent, var_name));
            code.push_str(&format!(
                "{}    ProtoValue::Fixed64(n) => CelValue::Float(f64::from_bits(*n)),\n",
                indent
            ));
            code.push_str(&format!(
                "{}    ProtoValue::Double(d) => CelValue::Float(*d),\n",
                indent
            ));
            code.push_str(&format!("{}    _ => CelValue::Null,\n", indent));
            code.push_str(&format!("{}}}\n", indent));
        }
        ProtoType::Fixed32 | ProtoType::Sfixed32 => {
            code.push_str(&format!("{}match {} {{\n", indent, var_name));
            code.push_str(&format!(
                "{}    ProtoValue::Fixed32(n) => CelValue::Int(*n as i64),\n",
                indent
            ));
            code.push_str(&format!("{}    _ => CelValue::Null,\n", indent));
            code.push_str(&format!("{}}}\n", indent));
        }
        ProtoType::Fixed64 | ProtoType::Sfixed64 => {
            code.push_str(&format!("{}match {} {{\n", indent, var_name));
            code.push_str(&format!(
                "{}    ProtoValue::Fixed64(n) => CelValue::Int(*n as i64),\n",
                indent
            ));
            code.push_str(&format!("{}    _ => CelValue::Null,\n", indent));
            code.push_str(&format!("{}}}\n", indent));
        }
        ProtoType::Bytes => {
            code.push_str(&format!("{}match {} {{\n", indent, var_name));
            code.push_str(&format!(
                "{}    ProtoValue::Bytes(b) => CelValue::Bytes(Arc::new(b.clone())),\n",
                indent
            ));
            code.push_str(&format!("{}    _ => CelValue::Null,\n", indent));
            code.push_str(&format!("{}}}\n", indent));
        }
        ProtoType::Message(msg_name) => {
            // For nested messages, decode with field name mapping
            if let Some(msg) = schema.messages.get(msg_name) {
                code.push_str(&format!("{}match {} {{\n", indent, var_name));
                code.push_str(&format!(
                    "{}    ProtoValue::Bytes(b) => {{\n",
                    indent
                ));
                code.push_str(&format!(
                    "{}        let nested = decode_proto_message(b);\n",
                    indent
                ));
                code.push_str(&format!(
                    "{}        let mut cel_map: HashMap<Key, CelValue> = HashMap::new();\n",
                    indent
                ));

                // Generate field-by-field extraction with proper names
                for field in &msg.fields {
                    code.push_str(&format!(
                        "{}        if let Some(fv) = nested.get(&{}) {{\n",
                        indent, field.number
                    ));
                    code.push_str(&format!(
                        "{}            cel_map.insert(Key::String(Arc::new(\"{}\".to_string())), ",
                        indent, field.name
                    ));
                    // Convert the value based on field type
                    generate_inline_value_conversion(code, &field.field_type, "fv", schema)?;
                    code.push_str(");\n");
                    code.push_str(&format!("{}        }}\n", indent));
                }

                code.push_str(&format!(
                    "{}        CelValue::Map(CelMap {{ map: Arc::new(cel_map) }})\n",
                    indent
                ));
                code.push_str(&format!("{}    }}\n", indent));
                code.push_str(&format!("{}    _ => CelValue::Null,\n", indent));
                code.push_str(&format!("{}}}\n", indent));
            } else {
                // Unknown message type - pass through as bytes
                code.push_str(&format!(
                    "{}proto_value_to_cel({})\n",
                    indent, var_name
                ));
            }
        }
        ProtoType::Enum(_) => {
            // Enums are encoded as varints
            code.push_str(&format!("{}match {} {{\n", indent, var_name));
            code.push_str(&format!(
                "{}    ProtoValue::Varint(n) => CelValue::Int(*n as i64),\n",
                indent
            ));
            code.push_str(&format!("{}    _ => CelValue::Null,\n", indent));
            code.push_str(&format!("{}}}\n", indent));
        }
        ProtoType::Map(key_type, value_type) => {
            // Maps are encoded as repeated message with key=1, value=2
            code.push_str(&format!("{}match {} {{\n", indent, var_name));
            code.push_str(&format!(
                "{}    ProtoValue::Repeated(items) => {{\n",
                indent
            ));
            code.push_str(&format!(
                "{}        let mut map_entries: HashMap<Key, CelValue> = HashMap::new();\n",
                indent
            ));
            code.push_str(&format!(
                "{}        for item in items {{\n",
                indent
            ));
            code.push_str(&format!(
                "{}            if let ProtoValue::Bytes(b) = item {{\n",
                indent
            ));
            code.push_str(&format!(
                "{}                let entry = decode_proto_message(b);\n",
                indent
            ));
            code.push_str(&format!(
                "{}                if let (Some(k), Some(v)) = (entry.get(&1), entry.get(&2)) {{\n",
                indent
            ));
            // Handle key based on key_type
            match key_type.as_ref() {
                ProtoType::String => {
                    code.push_str(&format!(
                        "{}                    if let ProtoValue::Bytes(kb) = k {{\n",
                        indent
                    ));
                    code.push_str(&format!(
                        "{}                        let key_str = String::from_utf8_lossy(kb).to_string();\n",
                        indent
                    ));
                    code.push_str(&format!(
                        "{}                        map_entries.insert(Key::String(Arc::new(key_str)), proto_value_to_cel(v));\n",
                        indent
                    ));
                    code.push_str(&format!("{}                    }}\n", indent));
                }
                _ => {
                    code.push_str(&format!(
                        "{}                    if let ProtoValue::Varint(ki) = k {{\n",
                        indent
                    ));
                    code.push_str(&format!(
                        "{}                        map_entries.insert(Key::Int(*ki as i64), proto_value_to_cel(v));\n",
                        indent
                    ));
                    code.push_str(&format!("{}                    }}\n", indent));
                }
            }
            code.push_str(&format!("{}                }}\n", indent));
            code.push_str(&format!("{}            }}\n", indent));
            code.push_str(&format!("{}        }}\n", indent));
            code.push_str(&format!(
                "{}        CelValue::Map(CelMap {{ map: Arc::new(map_entries) }})\n",
                indent
            ));
            code.push_str(&format!("{}    }}\n", indent));
            code.push_str(&format!("{}    _ => CelValue::Null,\n", indent));
            code.push_str(&format!("{}}}\n", indent));
            let _ = value_type; // Silence unused warning
        }
    }

    Ok(())
}

/// Collect all scoped styles from a node tree
fn collect_scoped_styles(nodes: &[Node], scope_class: &str) -> Vec<String> {
    let mut css_rules = Vec::new();

    for node in nodes {
        if let Node::Element(el) = node {
            if !el.styles.is_empty() {
                // Generate CSS rule with class selector
                let props: Vec<String> = el.styles
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect();
                css_rules.push(format!(
                    ".{} {{ {} }}",
                    scope_class,
                    props.join("; ")
                ));
            }
            // Recurse into children
            css_rules.extend(collect_scoped_styles(&el.children, scope_class));
        } else if let Node::ControlFlow(cf) = node {
            match cf {
                crate::ast::ControlFlow::If { then_block, else_block, .. } => {
                    css_rules.extend(collect_scoped_styles(then_block, scope_class));
                    if let Some(else_nodes) = else_block {
                        css_rules.extend(collect_scoped_styles(else_nodes, scope_class));
                    }
                }
                crate::ast::ControlFlow::Each { body, .. } => {
                    css_rules.extend(collect_scoped_styles(body, scope_class));
                }
                crate::ast::ControlFlow::Switch { cases, default, .. } => {
                    for SwitchCase(_, children) in cases {
                        css_rules.extend(collect_scoped_styles(children, scope_class));
                    }
                    if let Some(def_nodes) = default {
                        css_rules.extend(collect_scoped_styles(def_nodes, scope_class));
                    }
                }
            }
        }
    }

    css_rules
}

fn generate_view_function(
    code: &mut String,
    name: &str,
    root: &Root,
    schema: &ProtoSchema,
    data_type: Option<&str>,
    component_types: &HashMap<String, String>,
) -> Result<(), String> {
    let fn_name = name.to_lowercase();
    let scope_class = format!("h-{}", generate_scope_id(name));

    // Collect all scoped styles from the component
    let css_rules = collect_scoped_styles(&root.nodes, &scope_class);

    // Internal render function - takes proto data and pre-rendered content slot
    code.push_str(&format!(
        "\nfn render_{}(r: &mut String, proto_data: &[u8], content_html: &str) {{\n",
        fn_name
    ));

    // Emit scoped <style> tag if there are any styles
    if !css_rules.is_empty() {
        let all_css = css_rules.join(" ");
        // Escape quotes for Rust string literal
        let escaped_css = all_css.replace('\\', "\\\\").replace('"', "\\\"");
        code.push_str(&format!(
            "    r.push_str(\"<style>{}</style>\");\n",
            escaped_css
        ));
    }

    // Always decode proto fields for use in loop contexts
    code.push_str("    let _proto_fields = decode_proto_message(proto_data);\n");
    code.push_str("    let mut ctx = Context::default();\n");

    // If we have a data type, use the typed decoder
    if let Some(dt) = data_type {
        if schema.messages.contains_key(dt) {
            code.push_str(&format!(
                "    decode_{}(proto_data, &mut ctx);\n",
                dt.to_lowercase()
            ));
        } else {
            // Fallback to generic decoding
            code.push_str("    for (k, v) in &_proto_fields {\n");
            code.push_str("        let _ = ctx.add_variable(&k.to_string(), proto_value_to_cel(v));\n");
            code.push_str("    }\n");
        }
    } else {
        // No data type - use generic decoding
        code.push_str("    for (k, v) in &_proto_fields {\n");
        code.push_str("        let _ = ctx.add_variable(&k.to_string(), proto_value_to_cel(v));\n");
        code.push_str("    }\n");
    }

    for node in &root.nodes {
        generate_node_cel_scoped(code, node, 1, "r", &scope_class, component_types)?;
    }

    code.push_str("}\n");

    // Exported WASM function
    code.push_str(&format!(
        "\n#[no_mangle]\npub extern \"C\" fn {}(ptr: *const u8, len: usize) -> u64 {{\n",
        name
    ));
    code.push_str("    let proto_data = if len > 0 {\n");
    code.push_str("        unsafe { slice::from_raw_parts(ptr, len) }\n");
    code.push_str("    } else {\n");
    code.push_str("        &[]\n");
    code.push_str("    };\n\n");

    code.push_str("    let mut out = String::new();\n");
    code.push_str(&format!("    render_{}(&mut out, proto_data, \"\");\n", fn_name));
    code.push_str("    let result_ptr = out.as_ptr();\n");
    code.push_str("    let result_len = out.len();\n");
    code.push_str("    mem::forget(out);\n");
    code.push_str("    pack(result_ptr, result_len)\n");
    code.push_str("}\n");

    Ok(())
}

#[allow(dead_code)]
fn generate_node_cel(code: &mut String, node: &Node, indent: usize) -> Result<(), String> {
    // Delegate to scoped version with empty scope (no scoping)
    let empty_map = HashMap::new();
    generate_node_cel_scoped(code, node, indent, "r", "", &empty_map)
}

fn generate_node_cel_scoped(
    code: &mut String, 
    node: &Node, 
    indent: usize, 
    out_var: &str,
    scope_class: &str, 
    component_types: &HashMap<String, String>
) -> Result<(), String> {
    let pad = "    ".repeat(indent);

    match node {
        Node::ContentSlot => {
            code.push_str(&pad);
            code.push_str(&format!("{}.push_str(content_html);\n", out_var));
        }
        Node::Element(el) => {
            // Check if this is a component invocation
            // It must be in the component_types map (known components in this module)
            let is_component = component_types.contains_key(&el.tag);

            if is_component {
                // Component invocation
                // Syntax: ComponentName `expr` { children }
                let arg_val = if !el.children.is_empty() {
                    if let Node::Text(t) = &el.children[0] {
                        if t.content.starts_with('`') && t.content.ends_with('`') {
                            Some(&t.content[1..t.content.len()-1])
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                code.push_str(&pad);
                code.push_str("{\n");
                code.push_str(&pad);
                
                // Pre-render children for the content slot
                code.push_str("    let mut invocation_content = String::new();\n");
                for child in &el.children {
                    // Skip the first child if it was used as the proto data argument
                    if let Some(_) = arg_val {
                        if let Node::Text(t) = child {
                            if t.content.starts_with('`') && t.content.ends_with('`') {
                                continue;
                            }
                        }
                    }
                    generate_node_cel_scoped(code, child, indent + 1, "invocation_content", scope_class, component_types)?;
                }

                code.push_str(&pad);
                if let Some(expr) = arg_val {
                    // Pass specific data to component
                    code.push_str(&format!(
                        "    let component_data = cel_eval(\"{}\", &ctx);\n",
                        escape_string(expr)
                    ));
                    
                    // Get the data type for this component to encode correctly
                    let target_type = component_types.get(&el.tag).map(|s| s.as_str()).unwrap_or("");
                    
                    code.push_str(&pad);
                    code.push_str(&format!(
                        "    let encoded_data = encode_cel_to_proto(&component_data, \"{}\");\n",
                        target_type
                    ));
                    code.push_str(&pad);
                    code.push_str(&format!(
                        "    render_{}({}, &encoded_data, &invocation_content);\n",
                        el.tag.to_lowercase(),
                        out_var
                    ));
                } else {
                    // Pass current data through
                    code.push_str(&pad);
                    code.push_str(&format!(
                        "    render_{}({}, proto_data, &invocation_content);\n",
                        el.tag.to_lowercase(),
                        out_var
                    ));
                }
                
                code.push_str(&pad);
                code.push_str("}\n");
                return Ok(());
            }

            // Opening tag
            code.push_str(&pad);
            code.push_str(&format!("{}.push_str(\"<{}\");\n", out_var, el.tag));

            // ID attribute
            if let Some(id) = &el.id {
                code.push_str(&pad);
                code.push_str(&format!("{}.push_str(\" id=\\\"{}\\\"\");\n", out_var, id));
            }

            // Class attribute - include scope class if element has styles
            let has_scope_class = !el.styles.is_empty() && !scope_class.is_empty();
            if !el.classes.is_empty() || has_scope_class {
                let mut all_classes = el.classes.clone();
                if has_scope_class {
                    all_classes.push(scope_class.to_string());
                }
                code.push_str(&pad);
                code.push_str(&format!(
                    "{}.push_str(\" class=\\\"{}\\\"\");\n",
                    out_var,
                    all_classes.join(" ")
                ));
            }

            // Other attributes (may contain CEL expressions)
            for (key, value) in &el.attributes {
                if value.contains('`') {
                    // Dynamic attribute with CEL
                    generate_dynamic_attr_with_ctx(code, key, value, &pad, "&ctx", out_var)?;
                } else {
                    // Static attribute
                    code.push_str(&pad);
                    code.push_str(&format!("{}.push_str(\" {}=\\\"{}\\\"\");\n", out_var, key, value));
                }
            }

            // Datastar attributes
            for attr in &el.datastar {
                let (html_attr, html_val) = datastar_attr_to_html(attr);
                code.push_str(&pad);
                code.push_str(&format!("{}.push_str(\" ", out_var));
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
            code.push_str(&format!("{}.push_str(\">\");\n", out_var));

            // Children
            for child in &el.children {
                generate_node_cel_scoped(code, child, indent + 1, out_var, scope_class, component_types)?;
            }

            // Closing tag
            code.push_str(&pad);
            code.push_str(&format!("{}.push_str(\"</{}>\");\n", out_var, el.tag));
        }

        Node::Text(t) => {
            generate_text_with_interpolation_ctx(code, &t.content, &pad, "&ctx", out_var)?;
        }

        Node::ControlFlow(cf) => match cf {
            crate::ast::ControlFlow::If {
                condition,
                then_block,
                else_block,
            } => {
                code.push_str(&pad);
                code.push_str(&format!(
                    "if cel_truthy(&cel_eval(\"{}\", &ctx)) {{\n",
                    escape_string(condition)
                ));

                for child in then_block {
                    generate_node_cel_scoped(code, child, indent + 1, out_var, scope_class, component_types)?;
                }

                code.push_str(&pad);
                code.push_str("}");

                if let Some(else_nodes) = else_block {
                    code.push_str(" else {\n");
                    for child in else_nodes {
                        generate_node_cel_scoped(code, child, indent + 1, out_var, scope_class, component_types)?;
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
                code.push_str(&pad);
                code.push_str(&format!(
                    "if let CelValue::List(list) = cel_eval(\"{}\", &ctx) {{\n",
                    escape_string(iterable)
                ));
                code.push_str(&pad);
                code.push_str("    for (_idx, _item) in list.iter().enumerate() {\n");
                code.push_str(&pad);
                code.push_str("        // Create fresh context for loop iteration (Context doesn't impl Clone)\n");
                code.push_str(&pad);
                code.push_str("        let mut loop_ctx = Context::default();\n");
                code.push_str(&pad);
                code.push_str("        for (k, v) in &_proto_fields {\n");
                code.push_str(&pad);
                code.push_str("            let _ = loop_ctx.add_variable(&k.to_string(), proto_value_to_cel(v));\n");
                code.push_str(&pad);
                code.push_str("        }\n");
                code.push_str(&pad);
                code.push_str(&format!(
                    "        let _ = loop_ctx.add_variable(\"{}\", _item.clone());\n",
                    binding
                ));
                code.push_str(&pad);
                code.push_str(
                    "        let _ = loop_ctx.add_variable(\"_index\", CelValue::Int(_idx as i64));\n",
                );

                for child in body {
                    generate_node_cel_with_ctx_scoped(code, child, indent + 2, "&loop_ctx", out_var, scope_class, component_types)?;
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
                code.push_str(&pad);
                code.push_str("{\n");
                code.push_str(&pad);
                code.push_str(&format!(
                    "    let _switch_val = cel_eval(\"{}\", &ctx);\n",
                    escape_string(expr)
                ));

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
                    code.push_str(&format!(
                        "cel_to_string(&_switch_val) == \"{}\" {{\n",
                        pattern
                    ));

                    for child in children {
                        generate_node_cel_scoped(code, child, indent + 2, out_var, scope_class, component_types)?;
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
                        generate_node_cel_scoped(code, child, indent + 2, out_var, scope_class, component_types)?;
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
#[allow(dead_code)]
fn generate_node_cel_with_ctx(
    code: &mut String,
    node: &Node,
    indent: usize,
    ctx_var: &str,
) -> Result<(), String> {
    let empty_map = HashMap::new();
    generate_node_cel_with_ctx_scoped(code, node, indent, ctx_var, "r", "", &empty_map)
}

/// Generate node with custom context and scope class for scoped styles.
fn generate_node_cel_with_ctx_scoped(
    code: &mut String,
    node: &Node,
    indent: usize,
    ctx_var: &str,
    out_var: &str,
    scope_class: &str,
    component_types: &HashMap<String, String>,
) -> Result<(), String> {
    let pad = "    ".repeat(indent);

    match node {
        Node::ContentSlot => {
            code.push_str(&pad);
            code.push_str(&format!("{}.push_str(content_html);\n", out_var));
        }
        Node::Element(el) => {
            // Check if this is a component invocation
            let is_component = component_types.contains_key(&el.tag);

            if is_component {
                // Component invocation
                // Syntax: ComponentName `expr`
                let arg_val = if !el.children.is_empty() {
                    if let Node::Text(t) = &el.children[0] {
                        if t.content.starts_with('`') && t.content.ends_with('`') {
                            Some(&t.content[1..t.content.len()-1])
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                code.push_str(&pad);
                code.push_str("{\n");
                code.push_str(&pad);
                
                // Pre-render children for the content slot
                code.push_str("    let mut invocation_content = String::new();\n");
                for child in &el.children {
                    if let Some(_) = arg_val {
                        if let Node::Text(t) = child {
                            if t.content.starts_with('`') && t.content.ends_with('`') {
                                continue;
                            }
                        }
                    }
                    generate_node_cel_with_ctx_scoped(code, child, indent + 1, ctx_var, "invocation_content", scope_class, component_types)?;
                }

                code.push_str(&pad);
                if let Some(expr) = arg_val {
                    // Pass specific data to component
                    code.push_str(&format!(
                        "    let component_data = cel_eval(\"{}\", {});\n",
                        escape_string(expr),
                        ctx_var
                    ));
                    
                    // Get the data type for this component to encode correctly
                    let target_type = component_types.get(&el.tag).map(|s| s.as_str()).unwrap_or("");

                    code.push_str(&pad);
                    code.push_str(&format!(
                        "    let encoded_data = encode_cel_to_proto(&component_data, \"{}\");\n",
                        target_type
                    ));
                    code.push_str(&pad);
                    code.push_str(&format!(
                        "    render_{}({}, &encoded_data, &invocation_content);\n",
                        el.tag.to_lowercase(),
                        out_var
                    ));
                } else {
                    // Pass current data through
                    code.push_str(&pad);
                    code.push_str(&format!(
                        "    render_{}({}, proto_data, &invocation_content);\n",
                        el.tag.to_lowercase(),
                        out_var
                    ));
                }

                code.push_str(&pad);
                code.push_str("}\n");
                return Ok(());
            }

            code.push_str(&pad);
            code.push_str(&format!("{}.push_str(\"<{}\");\n", out_var, el.tag));

            if let Some(id) = &el.id {
                code.push_str(&pad);
                code.push_str(&format!("{}.push_str(\" id=\\\"{}\\\"\");\n", out_var, id));
            }

            // Class attribute - include scope class if element has styles
            let has_scope_class = !el.styles.is_empty() && !scope_class.is_empty();
            if !el.classes.is_empty() || has_scope_class {
                let mut all_classes = el.classes.clone();
                if has_scope_class {
                    all_classes.push(scope_class.to_string());
                }
                code.push_str(&pad);
                code.push_str(&format!(
                    "{}.push_str(\" class=\\\"{}\\\"\");\n",
                    out_var,
                    all_classes.join(" ")
                ));
            }

            for (key, value) in &el.attributes {
                if value.contains('`') {
                    generate_dynamic_attr_with_ctx(code, key, value, &pad, ctx_var, out_var)?;
                } else {
                    code.push_str(&pad);
                    code.push_str(&format!("{}.push_str(\" {}=\\\"{}\\\"\");\n", out_var, key, value));
                }
            }

            // Datastar attributes
            for attr in &el.datastar {
                let (html_attr, html_val) = datastar_attr_to_html(attr);
                code.push_str(&pad);
                code.push_str(&format!("{}.push_str(\" ", out_var));
                code.push_str(&html_attr);
                if let Some(val) = html_val {
                    code.push_str("=\\\"");
                    code.push_str(&val.replace('"', "&quot;"));
                    code.push_str("\\\"");
                }
                code.push_str("\");\n");
            }

            code.push_str(&pad);
            code.push_str(&format!("{}.push_str(\">\");\n", out_var));

            for child in &el.children {
                generate_node_cel_with_ctx_scoped(code, child, indent + 1, ctx_var, out_var, scope_class, component_types)?;
            }

            code.push_str(&pad);
            code.push_str(&format!("{}.push_str(\"</{}>\");\n", out_var, el.tag));
        }

        Node::Text(t) => {
            generate_text_with_interpolation_ctx(code, &t.content, &pad, ctx_var, out_var)?;
        }

        Node::ControlFlow(cf) => match cf {
            crate::ast::ControlFlow::If {
                condition,
                then_block,
                else_block,
            } => {
                code.push_str(&pad);
                code.push_str(&format!(
                    "if cel_truthy(&cel_eval(\"{}\", {})) {{\n",
                    escape_string(condition),
                    ctx_var
                ));

                for child in then_block {
                    generate_node_cel_with_ctx_scoped(code, child, indent + 1, ctx_var, out_var, scope_class, component_types)?;
                }

                code.push_str(&pad);
                code.push_str("}");

                if let Some(else_nodes) = else_block {
                    code.push_str(" else {\n");
                    for child in else_nodes {
                        generate_node_cel_with_ctx_scoped(code, child, indent + 1, ctx_var, out_var, scope_class, component_types)?;
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
                code.push_str(&pad);
                code.push_str(&format!(
                    "if let CelValue::List(list) = cel_eval(\"{}\", {}) {{\n",
                    escape_string(iterable),
                    ctx_var
                ));
                code.push_str(&pad);
                code.push_str("    for (_idx, _item) in list.iter().enumerate() {\n");
                code.push_str(&pad);
                code.push_str("        // Create fresh context for nested loop (Context doesn't impl Clone)\n");
                code.push_str(&pad);
                code.push_str("        let mut inner_ctx = Context::default();\n");
                code.push_str(&pad);
                code.push_str("        for (k, v) in &_proto_fields {\n");
                code.push_str(&pad);
                code.push_str("            let _ = inner_ctx.add_variable(&k.to_string(), proto_value_to_cel(v));\n");
                code.push_str(&pad);
                code.push_str("        }\n");
                code.push_str(&pad);
                code.push_str(&format!(
                    "        let _ = inner_ctx.add_variable(\"{}\", _item.clone());\n",
                    binding
                ));
                code.push_str(&pad);
                code.push_str(
                    "        let _ = inner_ctx.add_variable(\"_index\", CelValue::Int(_idx as i64));\n",
                );

                for child in body {
                    generate_node_cel_with_ctx_scoped(code, child, indent + 2, "&inner_ctx", out_var, scope_class, component_types)?;
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
                code.push_str(&pad);
                code.push_str("{\n");
                code.push_str(&pad);
                code.push_str(&format!(
                    "    let _switch_val = cel_eval(\"{}\", {});\n",
                    escape_string(expr),
                    ctx_var
                ));

                let mut first = true;
                for SwitchCase(pattern, children) in cases {
                    code.push_str(&pad);
                    if first {
                        code.push_str("    if ");
                        first = false;
                    } else {
                        code.push_str("    else if ");
                    }

                    code.push_str(&format!(
                        "cel_to_string(&_switch_val) == \"{}\" {{\n",
                        pattern
                    ));

                    for child in children {
                        generate_node_cel_with_ctx_scoped(code, child, indent + 2, ctx_var, out_var, scope_class, component_types)?;
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
                        generate_node_cel_with_ctx_scoped(code, child, indent + 2, ctx_var, out_var, scope_class, component_types)?;
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

fn generate_dynamic_attr_with_ctx(
    code: &mut String,
    key: &str,
    value: &str,
    pad: &str,
    ctx_var: &str,
    out_var: &str,
) -> Result<(), String> {
    // For boolean attributes like checked=`is_checked`
    let is_boolean_attr = matches!(
        key,
        "checked"
            | "disabled"
            | "readonly"
            | "required"
            | "selected"
            | "autofocus"
            | "autoplay"
            | "controls"
            | "loop"
            | "muted"
            | "open"
            | "hidden"
    );

    if is_boolean_attr && value.starts_with('`') && value.ends_with('`') {
        // Pure CEL expression for boolean attribute
        let expr = &value[1..value.len() - 1];
        code.push_str(pad);
        code.push_str(&format!(
            "if cel_truthy(&cel_eval(\"{}\", {})) {{\n",
            escape_string(expr),
            ctx_var
        ));
        code.push_str(pad);
        code.push_str(&format!("    {}.push_str(\" {}\");\n", out_var, key));
        code.push_str(pad);
        code.push_str("}\n");
    } else {
        // Dynamic value
        code.push_str(pad);
        code.push_str(&format!("{}.push_str(\" {}=\\\"\");\n", out_var, key));
        generate_text_with_interpolation_ctx(code, value, pad, ctx_var, out_var)?;
        code.push_str(pad);
        code.push_str(&format!("{}.push_str(\"\\\"\");\n", out_var));
    }

    Ok(())
}

fn generate_text_with_interpolation_ctx(
    code: &mut String,
    content: &str,
    pad: &str,
    ctx_var: &str,
    out_var: &str,
) -> Result<(), String> {
    let parts: Vec<&str> = content.split('`').collect();

    for (i, part) in parts.iter().enumerate() {
        if i % 2 == 0 {
            // Static text
            if !part.is_empty() {
                code.push_str(pad);
                code.push_str(&format!("{}.push_str(\"{}\");\n", out_var, escape_string(part)));
            }
        } else {
            // CEL expression
            if !part.is_empty() {
                code.push_str(pad);

                // Check for raw() function - outputs unescaped HTML
                let trimmed = part.trim();
                if trimmed.starts_with("raw(") && trimmed.ends_with(')') {
                    // Extract inner expression from raw(...)
                    let inner = &trimmed[4..trimmed.len() - 1];
                    code.push_str(&format!(
                        "{}.push_str(&cel_to_string(&cel_eval(\"{}\", {})));\n",
                        out_var,
                        escape_string(inner),
                        ctx_var
                    ));
                } else {
                    // Normal expression - HTML escaped
                    code.push_str(&format!(
                        "{}.push_str(&cel_eval_safe(\"{}\", {}));\n",
                        out_var,
                        escape_string(part),
                        ctx_var
                    ));
                }
            }
        }
    }

    Ok(())
}

/// Generate inline value conversion expression (for nested message fields)
fn generate_inline_value_conversion(
    code: &mut String,
    proto_type: &ProtoType,
    var_name: &str,
    schema: &ProtoSchema,
) -> Result<(), String> {
    match proto_type {
        ProtoType::String => {
            code.push_str(&format!(
                "match {} {{ ProtoValue::Bytes(b) => CelValue::String(Arc::new(String::from_utf8_lossy(b).to_string())), _ => CelValue::Null }}",
                var_name
            ));
        }
        ProtoType::Int32 | ProtoType::Int64 | ProtoType::Uint32 | ProtoType::Uint64 => {
            code.push_str(&format!(
                "match {} {{ ProtoValue::Varint(n) => CelValue::Int(*n as i64), _ => CelValue::Null }}",
                var_name
            ));
        }
        ProtoType::Bool => {
            code.push_str(&format!(
                "match {} {{ ProtoValue::Varint(n) => CelValue::Bool(*n != 0), _ => CelValue::Null }}",
                var_name
            ));
        }
        ProtoType::Float => {
            code.push_str(&format!(
                "match {} {{ ProtoValue::Fixed32(n) => CelValue::Float(f32::from_bits(*n) as f64), _ => CelValue::Null }}",
                var_name
            ));
        }
        ProtoType::Double => {
            code.push_str(&format!(
                "match {} {{ ProtoValue::Fixed64(n) => CelValue::Float(f64::from_bits(*n)), _ => CelValue::Null }}",
                var_name
            ));
        }
        ProtoType::Enum(_) => {
            code.push_str(&format!(
                "match {} {{ ProtoValue::Varint(n) => CelValue::Int(*n as i64), _ => CelValue::Null }}",
                var_name
            ));
        }
        ProtoType::Message(msg_name) => {
            // For nested messages within nested messages, recurse
            if let Some(msg) = schema.messages.get(msg_name) {
                code.push_str(&format!("match {} {{ ProtoValue::Bytes(b) => {{ ", var_name));
                code.push_str("let nested = decode_proto_message(b); ");
                code.push_str("let mut cel_map: HashMap<Key, CelValue> = HashMap::new(); ");
                for field in &msg.fields {
                    code.push_str(&format!(
                        "if let Some(fv) = nested.get(&{}) {{ cel_map.insert(Key::String(Arc::new(\"{}\".to_string())), ",
                        field.number, field.name
                    ));
                    generate_inline_value_conversion(code, &field.field_type, "fv", schema)?;
                    code.push_str("); } ");
                }
                code.push_str("CelValue::Map(CelMap { map: Arc::new(cel_map) }) ");
                code.push_str("}, _ => CelValue::Null }");
            } else {
                code.push_str(&format!("proto_value_to_cel({})", var_name));
            }
        }
        _ => {
            // Fallback for other types
            code.push_str(&format!("proto_value_to_cel({})", var_name));
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
        let schema = ProtoSchema::default();
        let rust_code = generate_wasm_lib_cel(views, &schema).expect("Codegen failed");

        assert!(rust_code.contains("cel_interpreter"));
        assert!(rust_code.contains(".push_str(\"<div\")"));
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
        let schema = ProtoSchema::default();
        let rust_code = generate_wasm_lib_cel(views, &schema).expect("Codegen failed");

        assert!(rust_code.contains("cel_eval"));
        assert!(rust_code.contains("name"));
    }

    #[test]
    fn test_generate_with_proto_schema() {
        let template = r#"
/**
message SimpleData {
    string title = 1;
    string description = 2;
    repeated string features = 3;
}
*/

// data: SimpleData

el {
    div "`title`"
}
        "#;

        let schema = ProtoSchema::from_template(template, None).unwrap();
        let doc = parser::parse(template).unwrap();
        let root = transformer::transform_with_metadata(&doc, template).unwrap();
        let views = vec![("Simple".to_string(), root)];
        let rust_code = generate_wasm_lib_cel(views, &schema).expect("Codegen failed");

        // Should generate a typed decoder
        assert!(rust_code.contains("decode_simpledata"));
        // Should reference the decoder
        assert!(rust_code.contains("decode_simpledata(proto_data, &mut ctx)"));
    }

    #[test]
    fn test_proto_decoder_generation() {
        let template = r#"
/**
message User {
    string name = 1;
    int32 age = 2;
    bool active = 3;
}
*/
        "#;

        let schema = ProtoSchema::from_template(template, None).unwrap();
        let mut code = String::new();
        let user = schema.get_message("User").unwrap();
        generate_message_decoder(&mut code, "User", &user.fields, &schema).unwrap();

        assert!(code.contains("fn decode_user"));
        assert!(code.contains("// Field 1: name"));
        assert!(code.contains("// Field 2: age"));
        assert!(code.contains("// Field 3: active"));
    }

    #[test]
    fn test_generate_with_datastar_attrs() {
        let input = r#"
el {
    button ~on:click="$count++" {
        ~ {
            show $isVisible
            .active $isSelected
            let:count 0
        }
        Click me
    }
}
        "#;

        let doc = parser::parse(input).unwrap();
        let root = transformer::transform(&doc).unwrap();
        let views = vec![("TestView".to_string(), root)];
        let schema = ProtoSchema::default();
        let rust_code = generate_wasm_lib_cel(views, &schema).expect("Codegen failed");

        // Event handler: on:click → data-on-click
        assert!(rust_code.contains("data-on-click"), "Should contain data-on-click");
        assert!(rust_code.contains("$count++"), "Should contain click expression");

        // Show: show → data-show
        assert!(rust_code.contains("data-show"), "Should contain data-show");
        assert!(rust_code.contains("$isVisible"), "Should contain show expression");

        // Class toggle: .active → data-class-active
        assert!(rust_code.contains("data-class-active"), "Should contain data-class-active");

        // Signal: let:count → data-signals-count (static value)
        assert!(rust_code.contains("data-signals-count"), "Should contain data-signals-count");
    }
}