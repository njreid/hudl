//! Template interpreter for dev mode rendering.
//!
//! Instead of compiling templates to WASM, this module walks the AST
//! directly and renders HTML by evaluating CEL expressions at runtime.
//! This enables hot-reload during development without recompilation.

use crate::ast::{ControlFlow, Element, Node, Root, SwitchCase};
use crate::cel::{self, CompiledExpr, EvalContext};
use crate::proto::{ProtoField, ProtoSchema, ProtoType};
use cel_interpreter::Value as CelValue;
use cel_interpreter::objects::{Key, Map as CelMap};
use std::collections::HashMap;
use std::sync::Arc;

/// Errors that can occur during template interpretation.
#[derive(Debug)]
pub struct RenderError {
    pub message: String,
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Render a template AST with proto wire-format data.
///
/// # Arguments
/// * `root` - The parsed template AST
/// * `schema` - Proto schema for decoding the data
/// * `data_bytes` - Proto wire-format bytes (or empty for no data)
///
/// # Returns
/// Rendered HTML string, or an error.
pub fn render(root: &Root, schema: &ProtoSchema, data_bytes: &[u8]) -> Result<String, RenderError> {
    if let Some(data_type) = &root.data_type {
        // Decode proto wire format using the schema (handles empty data with defaults)
        let cel_value = decode_proto_message(data_bytes, data_type, schema)?;
        render_with_values(root, schema, cel_value)
    } else {
        render_with_values(root, schema, CelValue::Null)
    }
}

/// Render a template AST with pre-decoded CelValues (for textproto-based preview).
///
/// # Arguments
/// * `root` - The parsed template AST
/// * `schema` - Proto schema for enum constants
/// * `data` - Pre-decoded CelValue (typically a Map from textproto parsing)
///
/// # Returns
/// Rendered HTML string, or an error.
pub fn render_with_values(
    root: &Root,
    schema: &ProtoSchema,
    data: CelValue,
) -> Result<String, RenderError> {
    let mut ctx = EvalContext::new();

    // If data is a map, add each top-level field as a separate variable
    if let CelValue::Map(ref map) = data {
        for (key, value) in map.map.iter() {
            if let Key::String(name) = key {
                ctx.add_value(name, value.clone());
            }
        }
    }

    // Add enum constants to the context
    for (_, proto_enum) in &schema.enums {
        for ev in &proto_enum.values {
            ctx.add_string(&ev.name, &ev.name);
        }
    }

    // Render the AST
    let mut output = String::new();
    render_nodes(&root.nodes, &ctx, schema, &mut output)?;

    Ok(output)
}

/// Decode proto wire format bytes into a CelValue using schema info.
fn decode_proto_message(
    data: &[u8],
    message_name: &str,
    schema: &ProtoSchema,
) -> Result<CelValue, RenderError> {
    let message = schema.get_message(message_name).ok_or_else(|| RenderError {
        message: format!("Unknown message type: {}", message_name),
    })?;

    let mut reader = ProtoReader::new(data);
    let mut fields: HashMap<Key, CelValue> = HashMap::new();

    while reader.remaining() > 0 {
        let tag = reader.read_varint().ok_or_else(|| RenderError {
            message: "Failed to read proto tag".to_string(),
        })?;
        let field_number = (tag >> 3) as u32;
        let wire_type = (tag & 0x7) as u32;

        // Look up the field by number
        let field_def = message.fields.iter().find(|f| f.number == field_number);

        let value = decode_field_value(&mut reader, wire_type, field_def, schema)?;

        if let Some(field) = field_def {
            let key = Key::String(Arc::new(field.name.clone()));

            if field.repeated {
                // Accumulate repeated fields into a list
                if let Some(existing) = fields.get(&key) {
                    if let CelValue::List(list) = existing {
                        let mut new_list = list.as_ref().clone();
                        new_list.push(value);
                        fields.insert(key, CelValue::List(Arc::new(new_list)));
                    }
                } else {
                    fields.insert(key, CelValue::List(Arc::new(vec![value])));
                }
            } else {
                fields.insert(key, value);
            }
        } else {
            // Unknown field - skip
        }
    }

    // Ensure all fields have values (proto3 default semantics)
    for field in &message.fields {
        let key = Key::String(Arc::new(field.name.clone()));
        if !fields.contains_key(&key) {
            let default = if field.repeated {
                CelValue::List(Arc::new(Vec::new()))
            } else {
                default_value_for_type(&field.field_type, schema)
            };
            fields.insert(key, default);
        }
    }

    Ok(CelValue::Map(CelMap {
        map: Arc::new(fields),
    }))
}

/// Decode a single field value from the wire format.
fn decode_field_value(
    reader: &mut ProtoReader,
    wire_type: u32,
    field_def: Option<&ProtoField>,
    schema: &ProtoSchema,
) -> Result<CelValue, RenderError> {
    match wire_type {
        WIRE_VARINT => {
            let raw = reader.read_varint().ok_or_else(|| RenderError {
                message: "Failed to read varint".to_string(),
            })?;

            // Check if this is a bool or enum field
            if let Some(field) = field_def {
                match &field.field_type {
                    ProtoType::Bool => return Ok(CelValue::Bool(raw != 0)),
                    ProtoType::Enum(enum_name) => {
                        // Convert enum number to its name string
                        if let Some(proto_enum) = schema.get_enum(enum_name) {
                            for ev in &proto_enum.values {
                                if ev.number == raw as i32 {
                                    return Ok(CelValue::String(Arc::new(ev.name.clone())));
                                }
                            }
                        }
                        // Fallback: return as int
                        return Ok(CelValue::Int(raw as i64));
                    }
                    _ => {}
                }
            }

            // Default: treat as int
            // Handle zigzag decoding for sint32/sint64
            if let Some(field) = field_def {
                match &field.field_type {
                    ProtoType::Sint32 | ProtoType::Sint64 => {
                        let decoded = ((raw >> 1) as i64) ^ (-((raw & 1) as i64));
                        return Ok(CelValue::Int(decoded));
                    }
                    _ => {}
                }
            }

            Ok(CelValue::Int(raw as i64))
        }
        WIRE_FIXED64 => {
            let raw = reader.read_fixed64().ok_or_else(|| RenderError {
                message: "Failed to read fixed64".to_string(),
            })?;
            if let Some(field) = field_def {
                match &field.field_type {
                    ProtoType::Double => Ok(CelValue::Float(f64::from_bits(raw))),
                    ProtoType::Fixed64 | ProtoType::Sfixed64 => Ok(CelValue::Int(raw as i64)),
                    _ => Ok(CelValue::Float(f64::from_bits(raw))),
                }
            } else {
                Ok(CelValue::Float(f64::from_bits(raw)))
            }
        }
        WIRE_LENGTH_DELIMITED => {
            let bytes = reader.read_length_delimited().ok_or_else(|| RenderError {
                message: "Failed to read length-delimited field".to_string(),
            })?;

            if let Some(field) = field_def {
                match &field.field_type {
                    ProtoType::String => {
                        let s = std::str::from_utf8(bytes).unwrap_or("");
                        Ok(CelValue::String(Arc::new(s.to_string())))
                    }
                    ProtoType::Bytes => {
                        Ok(CelValue::Bytes(Arc::new(bytes.to_vec())))
                    }
                    ProtoType::Message(msg_name) => {
                        decode_proto_message(bytes, msg_name, schema)
                    }
                    _ => {
                        // Try as string
                        let s = std::str::from_utf8(bytes).unwrap_or("");
                        Ok(CelValue::String(Arc::new(s.to_string())))
                    }
                }
            } else {
                // No schema info - try as string
                let s = std::str::from_utf8(bytes).unwrap_or("");
                Ok(CelValue::String(Arc::new(s.to_string())))
            }
        }
        WIRE_FIXED32 => {
            let raw = reader.read_fixed32().ok_or_else(|| RenderError {
                message: "Failed to read fixed32".to_string(),
            })?;
            if let Some(field) = field_def {
                match &field.field_type {
                    ProtoType::Float => Ok(CelValue::Float(f32::from_bits(raw) as f64)),
                    ProtoType::Fixed32 | ProtoType::Sfixed32 => Ok(CelValue::Int(raw as i64)),
                    _ => Ok(CelValue::Float(f32::from_bits(raw) as f64)),
                }
            } else {
                Ok(CelValue::Float(f32::from_bits(raw) as f64))
            }
        }
        _ => Err(RenderError {
            message: format!("Unknown wire type: {}", wire_type),
        }),
    }
}

/// Get the default CelValue for a proto type (proto3 default semantics).
fn default_value_for_type(proto_type: &ProtoType, schema: &ProtoSchema) -> CelValue {
    match proto_type {
        ProtoType::Double | ProtoType::Float => CelValue::Float(0.0),
        ProtoType::Int32
        | ProtoType::Int64
        | ProtoType::Uint32
        | ProtoType::Uint64
        | ProtoType::Sint32
        | ProtoType::Sint64
        | ProtoType::Fixed32
        | ProtoType::Fixed64
        | ProtoType::Sfixed32
        | ProtoType::Sfixed64 => CelValue::Int(0),
        ProtoType::Bool => CelValue::Bool(false),
        ProtoType::String => CelValue::String(Arc::new(String::new())),
        ProtoType::Bytes => CelValue::Bytes(Arc::new(Vec::new())),
        ProtoType::Enum(enum_name) => {
            // Default enum value is the one with number 0
            if let Some(proto_enum) = schema.get_enum(enum_name) {
                for ev in &proto_enum.values {
                    if ev.number == 0 {
                        return CelValue::String(Arc::new(ev.name.clone()));
                    }
                }
            }
            CelValue::Int(0)
        }
        ProtoType::Message(_) => CelValue::Null,
        ProtoType::Map(_, _) => CelValue::Map(CelMap {
            map: Arc::new(HashMap::new()),
        }),
    }
}

/// Render a list of AST nodes into the output string.
fn render_nodes(
    nodes: &[Node],
    ctx: &EvalContext,
    schema: &ProtoSchema,
    output: &mut String,
) -> Result<(), RenderError> {
    for node in nodes {
        render_node(node, ctx, schema, output)?;
    }
    Ok(())
}

/// Render a single AST node.
fn render_node(
    node: &Node,
    ctx: &EvalContext,
    schema: &ProtoSchema,
    output: &mut String,
) -> Result<(), RenderError> {
    match node {
        Node::Element(el) => render_element(el, ctx, schema, output),
        Node::Text(text) => render_text(&text.content, ctx, output),
        Node::ControlFlow(cf) => render_control_flow(cf, ctx, schema, output),
    }
}

/// Render an HTML element.
fn render_element(
    el: &Element,
    ctx: &EvalContext,
    schema: &ProtoSchema,
    output: &mut String,
) -> Result<(), RenderError> {
    // Void elements (no closing tag)
    let void_elements = [
        "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
        "source", "track", "wbr",
    ];
    let is_void = void_elements.contains(&el.tag.as_str());

    // Opening tag
    output.push('<');
    output.push_str(&el.tag);

    // ID attribute
    if let Some(id) = &el.id {
        output.push_str(" id=\"");
        output.push_str(id);
        output.push('"');
    }

    // Class attribute
    if !el.classes.is_empty() {
        output.push_str(" class=\"");
        output.push_str(&el.classes.join(" "));
        output.push('"');
    }

    // Other attributes (may contain CEL expressions)
    for (key, value) in &el.attributes {
        // Check if value contains a CEL expression (backtick)
        if value.contains('`') {
            let rendered = render_interpolated_string(value, ctx)?;
            // Boolean attribute handling: if the expression evaluates to "false", skip
            if rendered == "false" {
                continue;
            }
            if rendered == "true" {
                // Boolean attribute: present with no value
                output.push(' ');
                output.push_str(key);
                continue;
            }
            output.push(' ');
            output.push_str(key);
            output.push_str("=\"");
            output.push_str(&cel::html_escape(&rendered));
            output.push('"');
        } else {
            output.push(' ');
            output.push_str(key);
            output.push_str("=\"");
            output.push_str(value);
            output.push('"');
        }
    }

    // Datastar attributes
    for attr in &el.datastar {
        let (html_attr, html_val) = crate::codegen::datastar_attr_to_html(attr);
        output.push(' ');
        output.push_str(&html_attr);
        if let Some(val) = html_val {
            output.push_str("=\"");
            output.push_str(&val.replace('"', "&quot;"));
            output.push('"');
        }
    }

    output.push('>');

    if !is_void {
        // Render children
        render_nodes(&el.children, ctx, schema, output)?;

        // Closing tag
        output.push_str("</");
        output.push_str(&el.tag);
        output.push('>');
    }

    Ok(())
}

/// Render text content, evaluating CEL interpolations.
fn render_text(content: &str, ctx: &EvalContext, output: &mut String) -> Result<(), RenderError> {
    let parts: Vec<&str> = content.split('`').collect();
    for (i, part) in parts.iter().enumerate() {
        if i % 2 == 0 {
            // Static text
            if !part.is_empty() {
                output.push_str(part);
            }
        } else {
            // CEL expression
            let result = evaluate_cel(part, ctx)?;

            // Check for raw() function
            if part.starts_with("raw(") && part.ends_with(')') {
                // raw() - no escaping
                output.push_str(&cel::cel_to_string(&result));
            } else {
                output.push_str(&cel::html_escape(&cel::cel_to_string(&result)));
            }
        }
    }
    Ok(())
}

/// Render control flow constructs.
fn render_control_flow(
    cf: &ControlFlow,
    ctx: &EvalContext,
    schema: &ProtoSchema,
    output: &mut String,
) -> Result<(), RenderError> {
    match cf {
        ControlFlow::If {
            condition,
            then_block,
            else_block,
        } => {
            let result = evaluate_cel(condition, ctx)?;
            if cel::is_truthy(&result) {
                render_nodes(then_block, ctx, schema, output)?;
            } else if let Some(else_nodes) = else_block {
                render_nodes(else_nodes, ctx, schema, output)?;
            }
        }
        ControlFlow::Each {
            binding,
            iterable,
            body,
        } => {
            let list_val = evaluate_cel(iterable, ctx)?;
            if let CelValue::List(items) = list_val {
                for (index, item) in items.iter().enumerate() {
                    let mut child_ctx = ctx.child();
                    child_ctx.add_value(binding, item.clone());
                    child_ctx.add_int("_index", index as i64);

                    // If the item is a map, also add its fields directly
                    // (some templates access fields directly on the binding)
                    render_nodes(body, &child_ctx, schema, output)?;
                }
            }
        }
        ControlFlow::Switch {
            expr,
            cases,
            default,
        } => {
            let switch_val = evaluate_cel(expr, ctx)?;
            let switch_str = cel::cel_to_string(&switch_val);

            let mut matched = false;
            for SwitchCase(pattern, children) in cases {
                // Try matching as string literal or enum value name
                if switch_str == *pattern {
                    render_nodes(children, ctx, schema, output)?;
                    matched = true;
                    break;
                }
            }

            if !matched {
                if let Some(default_nodes) = default {
                    render_nodes(default_nodes, ctx, schema, output)?;
                }
            }
        }
    }
    Ok(())
}

/// Evaluate a CEL expression string with the given context.
fn evaluate_cel(expr_str: &str, ctx: &EvalContext) -> Result<CelValue, RenderError> {
    let compiled = CompiledExpr::compile(expr_str).map_err(|e| RenderError {
        message: format!("CEL compile error in '{}': {}", expr_str, e),
    })?;
    compiled.evaluate(ctx).map_err(|e| RenderError {
        message: format!("CEL eval error in '{}': {}", expr_str, e),
    })
}

/// Render an interpolated string (mix of static text and `backtick` expressions).
fn render_interpolated_string(s: &str, ctx: &EvalContext) -> Result<String, RenderError> {
    let mut result = String::new();
    let parts: Vec<&str> = s.split('`').collect();
    for (i, part) in parts.iter().enumerate() {
        if i % 2 == 0 {
            result.push_str(part);
        } else {
            let val = evaluate_cel(part, ctx)?;
            result.push_str(&cel::cel_to_string(&val));
        }
    }
    Ok(result)
}

// --- Proto wire format reader ---

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
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
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

// Make datastar_attr_to_html accessible from codegen
// (it's used by the interpreter for datastar attributes)

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    fn parse_template(content: &str) -> (Root, ProtoSchema) {
        let schema = ProtoSchema::from_template(content, None).unwrap_or_default();
        let doc = parser::parse(content).unwrap();
        let root = crate::transformer::transform_with_metadata(&doc, content).unwrap();
        (root, schema)
    }

    #[test]
    fn test_render_static_html() {
        let content = r#"
// name: Simple
el {
    div.container {
        h1 "Hello World"
    }
}
"#;
        let (root, schema) = parse_template(content);
        let html = render(&root, &schema, &[]).unwrap();
        assert!(html.contains("<div class=\"container\">"));
        assert!(html.contains("<h1>Hello World</h1>"));
    }

    #[test]
    fn test_render_with_data() {
        let content = r#"
/**
message SimpleData {
    string title = 1;
}
*/
// name: Simple
// data: SimpleData
el {
    h1 `title`
}
"#;
        let (root, schema) = parse_template(content);

        // Manually construct proto wire format for: { title: "Hi" }
        // Field 1, wire type 2 (length-delimited): tag = (1 << 3) | 2 = 10
        // Length = 2, value = "Hi"
        let data: Vec<u8> = vec![10, 2, b'H', b'i'];
        let html = render(&root, &schema, &data).unwrap();
        assert!(html.contains("Hi"));
    }

    #[test]
    fn test_render_conditional() {
        let content = r#"
/**
message Data {
    bool show = 1;
}
*/
// name: Cond
// data: Data
el {
    if `show` {
        span "Visible"
    }
    else {
        span "Hidden"
    }
}
"#;
        let (root, schema) = parse_template(content);

        // show = true: field 1, varint, tag = 8, value = 1
        let data_true: Vec<u8> = vec![8, 1];
        let html = render(&root, &schema, &data_true).unwrap();
        assert!(html.contains("Visible"));
        assert!(!html.contains("Hidden"));

        // show = false (default, empty data)
        let html = render(&root, &schema, &[]).unwrap();
        assert!(html.contains("Hidden"));
        assert!(!html.contains("Visible"));
    }

    // --- Control flow tests ---

    #[test]
    fn test_render_each_loop() {
        let content = r#"
/**
message Data {
    repeated string items = 1;
}
*/
// name: List
// data: Data
el {
    each "item" `items` {
        li `item`
    }
}
"#;
        let (root, schema) = parse_template(content);

        // items = ["apple", "banana"]
        // field 1, wire type 2 (length-delimited): tag = 10
        let mut data: Vec<u8> = Vec::new();
        // "apple" (5 bytes)
        data.extend_from_slice(&[10, 5, b'a', b'p', b'p', b'l', b'e']);
        // "banana" (6 bytes)
        data.extend_from_slice(&[10, 6, b'b', b'a', b'n', b'a', b'n', b'a']);

        let html = render(&root, &schema, &data).unwrap();
        assert!(html.contains("<li>apple</li>"));
        assert!(html.contains("<li>banana</li>"));
    }

    #[test]
    fn test_render_each_with_index() {
        let content = r#"
/**
message Data {
    repeated string items = 1;
}
*/
// name: Indexed
// data: Data
el {
    each "item" `items` {
        span `_index`
    }
}
"#;
        let (root, schema) = parse_template(content);

        // items = ["a", "b"]
        let mut data: Vec<u8> = Vec::new();
        data.extend_from_slice(&[10, 1, b'a']);
        data.extend_from_slice(&[10, 1, b'b']);

        let html = render(&root, &schema, &data).unwrap();
        assert!(html.contains("<span>0</span>"));
        assert!(html.contains("<span>1</span>"));
    }

    #[test]
    fn test_render_switch_enum() {
        // Note: The proto parser maps enum type names to ProtoType::Message(...),
        // so enum values are decoded as integers. Switch cases must match the
        // integer string representation.
        let content = r#"
/**
enum Status {
    UNKNOWN = 0;
    ACTIVE = 1;
    INACTIVE = 2;
}
message Data {
    int32 status = 1;
}
*/
// name: StatusView
// data: Data
el {
    switch `status` {
        case "1" {
            span "Is Active"
        }
        case "2" {
            span "Is Inactive"
        }
    }
}
"#;
        let (root, schema) = parse_template(content);

        // status = 1 (ACTIVE): field 1, varint, tag = 8, value = 1
        let data: Vec<u8> = vec![8, 1];
        let html = render(&root, &schema, &data).unwrap();
        assert!(html.contains("Is Active"));
        assert!(!html.contains("Is Inactive"));
    }

    #[test]
    fn test_render_switch_default() {
        let content = r#"
/**
enum Status {
    UNKNOWN = 0;
    ACTIVE = 1;
}
message Data {
    Status status = 1;
}
*/
// name: StatusDef
// data: Data
el {
    switch `status` {
        case "ACTIVE" {
            span "Active"
        }
        default {
            span "Other"
        }
    }
}
"#;
        let (root, schema) = parse_template(content);

        // status = UNKNOWN (0, default)
        let html = render(&root, &schema, &[]).unwrap();
        assert!(html.contains("Other"));
        assert!(!html.contains("Active"));
    }

    #[test]
    fn test_render_nested_if() {
        let content = r#"
/**
message Data {
    bool outer = 1;
    bool inner = 2;
}
*/
// name: Nested
// data: Data
el {
    if `outer` {
        if `inner` {
            span "both true"
        }
        else {
            span "outer only"
        }
    }
    else {
        span "neither"
    }
}
"#;
        let (root, schema) = parse_template(content);

        // outer=true, inner=true
        let data: Vec<u8> = vec![8, 1, 16, 1];
        let html = render(&root, &schema, &data).unwrap();
        assert!(html.contains("both true"));

        // outer=true, inner=false (default)
        let data: Vec<u8> = vec![8, 1];
        let html = render(&root, &schema, &data).unwrap();
        assert!(html.contains("outer only"));

        // outer=false (default)
        let html = render(&root, &schema, &[]).unwrap();
        assert!(html.contains("neither"));
    }

    // --- Expression tests ---

    #[test]
    fn test_render_string_interpolation_multiple() {
        let content = r#"
/**
message Data {
    string first = 1;
    string last = 2;
}
*/
// name: Greeting
// data: Data
el {
    span "Hello `first` `last`!"
}
"#;
        let (root, schema) = parse_template(content);

        // first="Jane", last="Doe"
        let mut data: Vec<u8> = Vec::new();
        // field 1: "Jane"
        data.extend_from_slice(&[10, 4, b'J', b'a', b'n', b'e']);
        // field 2: "Doe"
        data.extend_from_slice(&[18, 3, b'D', b'o', b'e']);

        let html = render(&root, &schema, &data).unwrap();
        assert!(html.contains("Hello Jane Doe!"));
    }

    #[test]
    fn test_render_comparison_in_if() {
        let content = r#"
/**
message Data {
    int32 count = 1;
}
*/
// name: Counter
// data: Data
el {
    if `count > 0` {
        span "has items"
    }
    else {
        span "empty"
    }
}
"#;
        let (root, schema) = parse_template(content);

        // count = 5: field 1, varint, tag = 8, value = 5
        let data: Vec<u8> = vec![8, 5];
        let html = render(&root, &schema, &data).unwrap();
        assert!(html.contains("has items"));

        // count = 0 (default)
        let html = render(&root, &schema, &[]).unwrap();
        assert!(html.contains("empty"));
    }

    #[test]
    fn test_render_nested_field_access() {
        let content = r#"
/**
message Inner {
    string value = 1;
}
message Data {
    Inner inner = 1;
}
*/
// name: Deep
// data: Data
el {
    span `inner.value`
}
"#;
        let (root, schema) = parse_template(content);

        // inner = { value: "deep" }
        // Outer: field 1, wire type 2 (length-delimited), tag = 10
        // Inner message bytes: field 1, wire type 2, tag = 10, len = 4, "deep"
        let inner_bytes: Vec<u8> = vec![10, 4, b'd', b'e', b'e', b'p'];
        let mut data: Vec<u8> = Vec::new();
        data.push(10); // tag
        data.push(inner_bytes.len() as u8); // length
        data.extend_from_slice(&inner_bytes);

        let html = render(&root, &schema, &data).unwrap();
        assert!(html.contains("deep"));
    }

    // --- Element rendering tests ---

    #[test]
    fn test_render_void_elements() {
        let content = r#"
// name: Void
el {
    br
    hr
    img src="test.png"
}
"#;
        let (root, schema) = parse_template(content);
        let html = render(&root, &schema, &[]).unwrap();
        assert!(html.contains("<br>"));
        assert!(!html.contains("</br>"));
        assert!(html.contains("<hr>"));
        assert!(!html.contains("</hr>"));
        assert!(html.contains("<img"));
        assert!(!html.contains("</img>"));
    }

    #[test]
    fn test_render_css_classes() {
        let content = r#"
// name: Classes
el {
    div.foo.bar "text"
}
"#;
        let (root, schema) = parse_template(content);
        let html = render(&root, &schema, &[]).unwrap();
        assert!(html.contains(r#"class="foo bar""#));
    }

    #[test]
    fn test_render_id_attribute() {
        let content = r#"
// name: WithId
el {
    div&main "text"
}
"#;
        let (root, schema) = parse_template(content);
        let html = render(&root, &schema, &[]).unwrap();
        assert!(html.contains(r#"id="main""#));
    }

    #[test]
    fn test_render_dynamic_attributes() {
        let content = r#"
/**
message Data {
    string url = 1;
}
*/
// name: Dynamic
// data: Data
el {
    a href="`url`" "click"
}
"#;
        let (root, schema) = parse_template(content);

        // url = "https://example.com"
        let url = b"https://example.com";
        let mut data: Vec<u8> = Vec::new();
        data.push(10); // tag (field 1, wire type 2)
        data.push(url.len() as u8);
        data.extend_from_slice(url);

        let html = render(&root, &schema, &data).unwrap();
        assert!(html.contains("href=\"https://example.com\""));
    }

    #[test]
    fn test_render_boolean_attributes() {
        let content = r#"
/**
message Data {
    bool is_disabled = 1;
    bool is_checked = 2;
}
*/
// name: BoolAttr
// data: Data
el {
    input disabled="`is_disabled`" checked="`is_checked`"
}
"#;
        let (root, schema) = parse_template(content);

        // is_disabled=true, is_checked=false
        let data: Vec<u8> = vec![8, 1]; // field 1 = true, field 2 = false (default)
        let html = render(&root, &schema, &data).unwrap();
        assert!(html.contains(" disabled"));
        assert!(!html.contains("checked"));
    }

    // --- Proto edge case tests ---

    #[test]
    fn test_render_empty_repeated_field() {
        let content = r#"
/**
message Data {
    repeated string items = 1;
}
*/
// name: Empty
// data: Data
el {
    each "item" `items` {
        li `item`
    }
}
"#;
        let (root, schema) = parse_template(content);

        // No data → empty repeated field
        let html = render(&root, &schema, &[]).unwrap();
        assert!(!html.contains("<li>"));
    }

    #[test]
    fn test_render_nested_message() {
        let content = r#"
/**
message Address {
    string city = 1;
    string state = 2;
}
message Data {
    string name = 1;
    Address address = 2;
}
*/
// name: Person
// data: Data
el {
    div `name`
    div `address.city`
}
"#;
        let (root, schema) = parse_template(content);

        // name = "Alice", address = { city: "NYC", state: "NY" }
        let mut data: Vec<u8> = Vec::new();
        // field 1 (name): tag=10, "Alice"
        data.extend_from_slice(&[10, 5, b'A', b'l', b'i', b'c', b'e']);
        // field 2 (address): tag=18, nested message
        let mut addr: Vec<u8> = Vec::new();
        addr.extend_from_slice(&[10, 3, b'N', b'Y', b'C']); // city="NYC"
        addr.extend_from_slice(&[18, 2, b'N', b'Y']); // state="NY"
        data.push(18); // tag
        data.push(addr.len() as u8);
        data.extend_from_slice(&addr);

        let html = render(&root, &schema, &data).unwrap();
        assert!(html.contains("Alice"));
        assert!(html.contains("NYC"));
    }

    #[test]
    fn test_render_enum_default() {
        // The proto parser treats enum type names as Message references,
        // so we use int32 here and verify the default is 0.
        let content = r#"
/**
message Data {
    int32 status = 1;
}
*/
// name: EnumDef
// data: Data
el {
    span `status`
}
"#;
        let (root, schema) = parse_template(content);

        // No data → int32 defaults to 0
        let html = render(&root, &schema, &[]).unwrap();
        assert!(html.contains("<span>0</span>"));
    }

    #[test]
    fn test_render_missing_field_defaults() {
        let content = r#"
/**
message Data {
    string name = 1;
    int32 count = 2;
    bool active = 3;
}
*/
// name: Defaults
// data: Data
el {
    span `name`
    span `count`
    span `active`
}
"#;
        let (root, schema) = parse_template(content);

        // No data → proto3 defaults
        let html = render(&root, &schema, &[]).unwrap();
        // string defaults to "", int defaults to 0, bool defaults to false
        assert!(html.contains("<span>0</span>"));
        assert!(html.contains("<span>false</span>"));
    }

    // --- Error handling tests ---

    #[test]
    fn test_render_unknown_variable_error() {
        let content = r#"
/**
message Data {
    string name = 1;
}
*/
// name: ErrVar
// data: Data
el {
    span `nonexistent`
}
"#;
        let (root, schema) = parse_template(content);
        let result = render(&root, &schema, &[]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("nonexistent"), "error: {}", err.message);
    }

    #[test]
    fn test_render_malformed_proto_error() {
        let content = r#"
/**
message Data {
    string name = 1;
}
*/
// name: BadProto
// data: Data
el {
    span `name`
}
"#;
        let (root, schema) = parse_template(content);

        // Garbage bytes: an incomplete varint (high bit set, no continuation)
        let bad_data: Vec<u8> = vec![0xFF, 0xFF, 0xFF, 0xFF];
        let result = render(&root, &schema, &bad_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_render_datastar_attrs() {
        let content = r#"
// name: Reactive
el {
    button {
        ~ {
            on:click "$count++"
            show $isVisible
            .active $isSelected
            let:count 0
        }
        Click
    }
}
"#;
        let (root, schema) = parse_template(content);
        let html = render(&root, &schema, &[]).unwrap();

        // Event handler
        assert!(html.contains("data-on-click=\"$count++\""), "HTML: {}", html);
        // Show
        assert!(html.contains("data-show=\"$isVisible\""), "HTML: {}", html);
        // Class toggle
        assert!(html.contains("data-class-active=\"$isSelected\""), "HTML: {}", html);
        // Signal (static value → data-signals)
        assert!(html.contains("data-signals-count=\"0\""), "HTML: {}", html);
    }

    #[test]
    fn test_render_datastar_inline_tilde() {
        let content = r#"
// name: Inline
el {
    button ~on:click="handleClick()" Click
}
"#;
        let (root, schema) = parse_template(content);
        let html = render(&root, &schema, &[]).unwrap();

        assert!(html.contains("data-on-click=\"handleClick()\""), "HTML: {}", html);
        assert!(html.contains("Click"), "HTML: {}", html);
    }

    #[test]
    fn test_render_datastar_modifiers() {
        let content = r#"
// name: Modifiers
el {
    form {
        ~ {
            on:submit~prevent "@post('/login')"
            let:count~ifmissing 0
        }
    }
}
"#;
        let (root, schema) = parse_template(content);
        let html = render(&root, &schema, &[]).unwrap();

        assert!(html.contains("data-on-submit__prevent"), "HTML: {}", html);
        assert!(html.contains("data-signals-count__ifmissing=\"0\""), "HTML: {}", html);
    }
}
