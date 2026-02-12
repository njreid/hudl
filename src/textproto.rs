//! TextProto parser and skeleton generator.
//!
//! Parses textproto format (Protocol Buffer text format) into CelValues
//! and generates skeleton textproto for a given message type.

use crate::proto::{ProtoSchema, ProtoType};
use cel_interpreter::objects::{Key, Map as CelMap};
use cel_interpreter::Value as CelValue;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

/// Errors from textproto parsing or generation.
#[derive(Debug)]
pub struct TextProtoError {
    pub message: String,
    pub line: Option<usize>,
}

impl std::fmt::Display for TextProtoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(line) = self.line {
            write!(f, "line {}: {}", line, self.message)
        } else {
            write!(f, "{}", self.message)
        }
    }
}

/// Parse a textproto string into a CelValue using the schema for type resolution.
///
/// # Arguments
/// * `input` - The textproto string
/// * `message_name` - The root message type name
/// * `schema` - Proto schema for field type lookups
pub fn parse(
    input: &str,
    message_name: &str,
    schema: &ProtoSchema,
) -> Result<CelValue, TextProtoError> {
    let tokens = tokenize(input)?;
    let mut parser = Parser::new(&tokens, schema);
    parser.parse_message(message_name)
}

/// Generate a default textproto skeleton for a message type.
///
/// Produces a textproto string with default values for all fields.
pub fn generate_skeleton(
    message_name: &str,
    schema: &ProtoSchema,
) -> Result<String, TextProtoError> {
    let mut visited = HashSet::new();
    generate_skeleton_inner(message_name, schema, &mut visited, 0)
}

fn generate_skeleton_inner(
    message_name: &str,
    schema: &ProtoSchema,
    visited: &mut HashSet<String>,
    indent: usize,
) -> Result<String, TextProtoError> {
    let message = schema.get_message(message_name).ok_or_else(|| TextProtoError {
        message: format!("Unknown message type: {}", message_name),
        line: None,
    })?;

    if visited.contains(message_name) {
        // Cycle detected — emit a comment
        let prefix = "  ".repeat(indent);
        return Ok(format!("{}# (recursive: {})\n", prefix, message_name));
    }
    visited.insert(message_name.to_string());

    let mut output = String::new();
    let prefix = "  ".repeat(indent);

    for field in &message.fields {
        if field.repeated {
            // One example item for repeated fields
            output.push_str(&format!("{}# repeated\n", prefix));
            write_field_default(&mut output, &field.name, &field.field_type, schema, visited, indent)?;
        } else {
            write_field_default(&mut output, &field.name, &field.field_type, schema, visited, indent)?;
        }
    }

    visited.remove(message_name);
    Ok(output)
}

fn write_field_default(
    output: &mut String,
    field_name: &str,
    field_type: &ProtoType,
    schema: &ProtoSchema,
    visited: &mut HashSet<String>,
    indent: usize,
) -> Result<(), TextProtoError> {
    let prefix = "  ".repeat(indent);
    match field_type {
        ProtoType::String => {
            output.push_str(&format!("{}{}: \"\"\n", prefix, field_name));
        }
        ProtoType::Bool => {
            output.push_str(&format!("{}{}: false\n", prefix, field_name));
        }
        ProtoType::Double | ProtoType::Float => {
            output.push_str(&format!("{}{}: 0.0\n", prefix, field_name));
        }
        ProtoType::Int32
        | ProtoType::Int64
        | ProtoType::Uint32
        | ProtoType::Uint64
        | ProtoType::Sint32
        | ProtoType::Sint64
        | ProtoType::Fixed32
        | ProtoType::Fixed64
        | ProtoType::Sfixed32
        | ProtoType::Sfixed64 => {
            output.push_str(&format!("{}{}: 0\n", prefix, field_name));
        }
        ProtoType::Bytes => {
            output.push_str(&format!("{}{}: \"\"\n", prefix, field_name));
        }
        ProtoType::Enum(enum_name) => {
            let default = schema
                .get_enum(enum_name)
                .and_then(|e| e.values.first().map(|v| v.name.clone()))
                .unwrap_or_else(|| "UNKNOWN".to_string());
            output.push_str(&format!("{}{}: {}\n", prefix, field_name, default));
        }
        ProtoType::Message(msg_name) => {
            // Check if it's actually an enum (the proto parser may tag enums as Message)
            if let Some(proto_enum) = schema.get_enum(msg_name) {
                let default = proto_enum
                    .values
                    .first()
                    .map(|v| v.name.clone())
                    .unwrap_or_else(|| "UNKNOWN".to_string());
                output.push_str(&format!("{}{}: {}\n", prefix, field_name, default));
            } else {
                output.push_str(&format!("{}{} {{\n", prefix, field_name));
                let inner = generate_skeleton_inner(msg_name, schema, visited, indent + 1)?;
                output.push_str(&inner);
                output.push_str(&format!("{}}}\n", prefix));
            }
        }
        ProtoType::Map(_, _) => {
            output.push_str(&format!("{}# map field '{}' not supported in skeleton\n", prefix, field_name));
        }
    }
    Ok(())
}

// --- Tokenizer ---

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String),
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Colon,
    LBrace,
    RBrace,
    // line number for error reporting
    Newline(usize),
}

fn tokenize(input: &str) -> Result<Vec<Token>, TextProtoError> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    let mut line = 1;

    while let Some(&ch) = chars.peek() {
        match ch {
            '#' => {
                // Comment — skip to end of line
                while let Some(&c) = chars.peek() {
                    if c == '\n' {
                        break;
                    }
                    chars.next();
                }
            }
            '\n' => {
                tokens.push(Token::Newline(line));
                line += 1;
                chars.next();
            }
            ' ' | '\t' | '\r' => {
                chars.next();
            }
            ':' => {
                tokens.push(Token::Colon);
                chars.next();
            }
            '{' => {
                tokens.push(Token::LBrace);
                chars.next();
            }
            '}' => {
                tokens.push(Token::RBrace);
                chars.next();
            }
            '"' => {
                tokens.push(Token::String(read_string(&mut chars, line)?));
            }
            '\'' => {
                tokens.push(Token::String(read_single_quoted_string(&mut chars, line)?));
            }
            '-' | '0'..='9' => {
                let tok = read_number(&mut chars, line)?;
                tokens.push(tok);
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let ident = read_ident(&mut chars);
                match ident.as_str() {
                    "true" => tokens.push(Token::Bool(true)),
                    "false" => tokens.push(Token::Bool(false)),
                    _ => tokens.push(Token::Ident(ident)),
                }
            }
            _ => {
                return Err(TextProtoError {
                    message: format!("Unexpected character: '{}'", ch),
                    line: Some(line),
                });
            }
        }
    }

    Ok(tokens)
}

fn read_string(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    line: usize,
) -> Result<String, TextProtoError> {
    chars.next(); // consume opening quote
    let mut s = String::new();
    loop {
        match chars.next() {
            Some('"') => return Ok(s),
            Some('\\') => match chars.next() {
                Some('\\') => s.push('\\'),
                Some('"') => s.push('"'),
                Some('n') => s.push('\n'),
                Some('t') => s.push('\t'),
                Some('r') => s.push('\r'),
                Some(c) => {
                    s.push('\\');
                    s.push(c);
                }
                None => {
                    return Err(TextProtoError {
                        message: "Unterminated escape in string".to_string(),
                        line: Some(line),
                    });
                }
            },
            Some(c) => s.push(c),
            None => {
                return Err(TextProtoError {
                    message: "Unterminated string".to_string(),
                    line: Some(line),
                });
            }
        }
    }
}

fn read_single_quoted_string(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    line: usize,
) -> Result<String, TextProtoError> {
    chars.next(); // consume opening quote
    let mut s = String::new();
    loop {
        match chars.next() {
            Some('\'') => return Ok(s),
            Some('\\') => match chars.next() {
                Some('\\') => s.push('\\'),
                Some('\'') => s.push('\''),
                Some('n') => s.push('\n'),
                Some('t') => s.push('\t'),
                Some('r') => s.push('\r'),
                Some(c) => {
                    s.push('\\');
                    s.push(c);
                }
                None => {
                    return Err(TextProtoError {
                        message: "Unterminated escape in string".to_string(),
                        line: Some(line),
                    });
                }
            },
            Some(c) => s.push(c),
            None => {
                return Err(TextProtoError {
                    message: "Unterminated string".to_string(),
                    line: Some(line),
                });
            }
        }
    }
}

fn read_number(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    line: usize,
) -> Result<Token, TextProtoError> {
    let mut s = String::new();
    let mut is_float = false;

    if chars.peek() == Some(&'-') {
        s.push('-');
        chars.next();
    }

    while let Some(&c) = chars.peek() {
        match c {
            '0'..='9' => {
                s.push(c);
                chars.next();
            }
            '.' => {
                is_float = true;
                s.push(c);
                chars.next();
            }
            'e' | 'E' => {
                is_float = true;
                s.push(c);
                chars.next();
                // optional sign
                if let Some(&sign) = chars.peek() {
                    if sign == '+' || sign == '-' {
                        s.push(sign);
                        chars.next();
                    }
                }
            }
            _ => break,
        }
    }

    if is_float {
        s.parse::<f64>()
            .map(Token::Float)
            .map_err(|_| TextProtoError {
                message: format!("Invalid float: {}", s),
                line: Some(line),
            })
    } else {
        s.parse::<i64>()
            .map(Token::Int)
            .map_err(|_| TextProtoError {
                message: format!("Invalid integer: {}", s),
                line: Some(line),
            })
    }
}

fn read_ident(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut s = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_alphanumeric() || c == '_' {
            s.push(c);
            chars.next();
        } else {
            break;
        }
    }
    s
}

// --- Parser ---

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    schema: &'a ProtoSchema,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token], schema: &'a ProtoSchema) -> Self {
        Self {
            tokens,
            pos: 0,
            schema,
        }
    }

    fn peek(&self) -> Option<&Token> {
        let mut i = self.pos;
        while i < self.tokens.len() {
            if let Token::Newline(_) = &self.tokens[i] {
                i += 1;
                continue;
            }
            return Some(&self.tokens[i]);
        }
        None
    }

    fn next(&mut self) -> Option<&Token> {
        while self.pos < self.tokens.len() {
            let tok = &self.tokens[self.pos];
            self.pos += 1;
            if let Token::Newline(_) = tok {
                continue;
            }
            return Some(tok);
        }
        None
    }

    fn current_line(&self) -> usize {
        // Walk backwards to find the nearest newline token
        for i in (0..self.pos).rev() {
            if let Token::Newline(line) = &self.tokens[i] {
                return *line;
            }
        }
        1
    }

    fn err(&self, msg: impl Into<String>) -> TextProtoError {
        TextProtoError {
            message: msg.into(),
            line: Some(self.current_line()),
        }
    }

    fn parse_message(&mut self, message_name: &str) -> Result<CelValue, TextProtoError> {
        let message = self
            .schema
            .get_message(message_name)
            .ok_or_else(|| self.err(format!("Unknown message type: {}", message_name)))?
            .clone();

        let mut fields: HashMap<Key, CelValue> = HashMap::new();

        while let Some(tok) = self.peek() {
            match tok {
                Token::RBrace => break,
                Token::Ident(_) => {
                    self.parse_field(&message.fields, &mut fields)?;
                }
                _ => {
                    return Err(self.err(format!("Expected field name, got {:?}", tok)));
                }
            }
        }

        // Fill in defaults for missing fields (proto3 semantics)
        for field in &message.fields {
            let key = Key::String(Arc::new(field.name.clone()));
            if !fields.contains_key(&key) {
                let default = if field.repeated {
                    CelValue::List(Arc::new(Vec::new()))
                } else {
                    default_value_for_type(&field.field_type, self.schema)
                };
                fields.insert(key, default);
            }
        }

        Ok(CelValue::Map(CelMap {
            map: Arc::new(fields),
        }))
    }

    fn parse_field(
        &mut self,
        message_fields: &[crate::proto::ProtoField],
        fields: &mut HashMap<Key, CelValue>,
    ) -> Result<(), TextProtoError> {
        // Clone the token data before calling self.err to avoid borrow conflicts
        let field_name = if let Some(Token::Ident(_)) = self.peek() {
            match self.next() {
                Some(Token::Ident(name)) => name.clone(),
                _ => unreachable!(),
            }
        } else {
            let line = self.current_line();
            return Err(TextProtoError {
                message: "Expected field name".to_string(),
                line: Some(line),
            });
        };

        // Look up field in the message definition
        let field_def = message_fields
            .iter()
            .find(|f| f.name == field_name)
            .ok_or_else(|| self.err(format!("Unknown field: {}", field_name)))?
            .clone();

        // Parse the value
        let value = match self.peek() {
            Some(Token::LBrace) => {
                // Nested message: field { ... }
                self.next(); // consume {
                let msg_name = match &field_def.field_type {
                    ProtoType::Message(name) => name.clone(),
                    _ => {
                        return Err(
                            self.err(format!("Field '{}' is not a message type", field_name))
                        );
                    }
                };
                let val = self.parse_message(&msg_name)?;
                self.expect_rbrace()?;
                val
            }
            Some(Token::Colon) => {
                self.next(); // consume :
                // Check for `field: { ... }` syntax
                if let Some(Token::LBrace) = self.peek() {
                    self.next(); // consume {
                    let msg_name = match &field_def.field_type {
                        ProtoType::Message(name) => name.clone(),
                        _ => {
                            return Err(
                                self.err(format!("Field '{}' is not a message type", field_name))
                            );
                        }
                    };
                    let val = self.parse_message(&msg_name)?;
                    self.expect_rbrace()?;
                    val
                } else {
                    self.parse_scalar_value(&field_def.field_type)?
                }
            }
            _ => {
                let line = self.current_line();
                return Err(TextProtoError {
                    message: format!(
                        "Expected ':' or '{{' after field name '{}'",
                        field_name
                    ),
                    line: Some(line),
                });
            }
        };

        let key = Key::String(Arc::new(field_name));

        if field_def.repeated {
            // Accumulate into list
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

        Ok(())
    }

    fn expect_rbrace(&mut self) -> Result<(), TextProtoError> {
        if let Some(Token::RBrace) = self.peek() {
            self.next();
            Ok(())
        } else {
            let line = self.current_line();
            Err(TextProtoError {
                message: "Expected '}'".to_string(),
                line: Some(line),
            })
        }
    }

    fn parse_scalar_value(&mut self, field_type: &ProtoType) -> Result<CelValue, TextProtoError> {
        // Clone the next token to avoid borrow conflicts
        let tok = self.next().cloned();
        match tok {
            Some(Token::String(s)) => Ok(CelValue::String(Arc::new(s))),
            Some(Token::Int(n)) => match field_type {
                ProtoType::Double | ProtoType::Float => Ok(CelValue::Float(n as f64)),
                _ => Ok(CelValue::Int(n)),
            },
            Some(Token::Float(f)) => Ok(CelValue::Float(f)),
            Some(Token::Bool(b)) => Ok(CelValue::Bool(b)),
            Some(Token::Ident(name)) => {
                // Resolve as enum value
                match field_type {
                    ProtoType::Enum(enum_name) => {
                        if let Some(proto_enum) = self.schema.get_enum(enum_name) {
                            if proto_enum.values.iter().any(|v| v.name == name) {
                                Ok(CelValue::String(Arc::new(name)))
                            } else {
                                Err(self.err(format!(
                                    "Unknown enum value '{}' for enum '{}'",
                                    name, enum_name
                                )))
                            }
                        } else {
                            Err(self.err(format!("Unknown enum: {}", enum_name)))
                        }
                    }
                    ProtoType::Message(msg_name) => {
                        // The proto parser sometimes tags enums as Message
                        if let Some(proto_enum) = self.schema.get_enum(msg_name) {
                            if proto_enum.values.iter().any(|v| v.name == name) {
                                Ok(CelValue::String(Arc::new(name)))
                            } else {
                                Err(self.err(format!(
                                    "Unknown enum value '{}' for enum '{}'",
                                    name, msg_name
                                )))
                            }
                        } else {
                            Err(self.err(format!(
                                "Expected scalar value for field, got identifier '{}'",
                                name
                            )))
                        }
                    }
                    _ => Err(self.err(format!(
                        "Unexpected identifier '{}' for {:?} field",
                        name, field_type
                    ))),
                }
            }
            _ => {
                let line = self.current_line();
                Err(TextProtoError {
                    message: "Expected value".to_string(),
                    line: Some(line),
                })
            }
        }
    }
}

/// Get the default CelValue for a proto type (proto3 semantics).
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
            if let Some(proto_enum) = schema.get_enum(enum_name) {
                for ev in &proto_enum.values {
                    if ev.number == 0 {
                        return CelValue::String(Arc::new(ev.name.clone()));
                    }
                }
            }
            CelValue::Int(0)
        }
        ProtoType::Message(name) => {
            // The proto parser sometimes tags enums as Message
            if let Some(proto_enum) = schema.get_enum(name) {
                for ev in &proto_enum.values {
                    if ev.number == 0 {
                        return CelValue::String(Arc::new(ev.name.clone()));
                    }
                }
                CelValue::Int(0)
            } else {
                CelValue::Null
            }
        }
        ProtoType::Map(_, _) => CelValue::Map(CelMap {
            map: Arc::new(HashMap::new()),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{ProtoField, ProtoMessage, ProtoSchema, ProtoType, ProtoEnum, ProtoEnumValue};

    fn make_schema() -> ProtoSchema {
        let mut schema = ProtoSchema::default();
        schema.messages.insert(
            "User".to_string(),
            ProtoMessage {
                name: "User".to_string(),
                fields: vec![
                    ProtoField {
                        name: "name".to_string(),
                        field_type: ProtoType::String,
                        number: 1,
                        repeated: false,
                        optional: false,
                    },
                    ProtoField {
                        name: "age".to_string(),
                        field_type: ProtoType::Int32,
                        number: 2,
                        repeated: false,
                        optional: false,
                    },
                    ProtoField {
                        name: "active".to_string(),
                        field_type: ProtoType::Bool,
                        number: 3,
                        repeated: false,
                        optional: false,
                    },
                    ProtoField {
                        name: "score".to_string(),
                        field_type: ProtoType::Double,
                        number: 4,
                        repeated: false,
                        optional: false,
                    },
                    ProtoField {
                        name: "tags".to_string(),
                        field_type: ProtoType::String,
                        number: 5,
                        repeated: true,
                        optional: false,
                    },
                    ProtoField {
                        name: "address".to_string(),
                        field_type: ProtoType::Message("Address".to_string()),
                        number: 6,
                        repeated: false,
                        optional: false,
                    },
                    ProtoField {
                        name: "role".to_string(),
                        field_type: ProtoType::Enum("Role".to_string()),
                        number: 7,
                        repeated: false,
                        optional: false,
                    },
                ],
                nested_messages: Vec::new(),
                nested_enums: Vec::new(),
            },
        );
        schema.messages.insert(
            "Address".to_string(),
            ProtoMessage {
                name: "Address".to_string(),
                fields: vec![
                    ProtoField {
                        name: "city".to_string(),
                        field_type: ProtoType::String,
                        number: 1,
                        repeated: false,
                        optional: false,
                    },
                    ProtoField {
                        name: "zip".to_string(),
                        field_type: ProtoType::String,
                        number: 2,
                        repeated: false,
                        optional: false,
                    },
                ],
                nested_messages: Vec::new(),
                nested_enums: Vec::new(),
            },
        );
        schema.enums.insert(
            "Role".to_string(),
            ProtoEnum {
                name: "Role".to_string(),
                values: vec![
                    ProtoEnumValue { name: "ROLE_UNKNOWN".to_string(), number: 0 },
                    ProtoEnumValue { name: "ROLE_ADMIN".to_string(), number: 1 },
                    ProtoEnumValue { name: "ROLE_USER".to_string(), number: 2 },
                ],
            },
        );
        schema
    }

    #[test]
    fn test_parse_scalars() {
        let schema = make_schema();
        let input = r#"
name: "Alice"
age: 30
active: true
score: 9.5
"#;
        let val = parse(input, "User", &schema).unwrap();
        if let CelValue::Map(map) = val {
            let m = &map.map;
            assert_eq!(
                m.get(&Key::String(Arc::new("name".to_string()))),
                Some(&CelValue::String(Arc::new("Alice".to_string())))
            );
            assert_eq!(
                m.get(&Key::String(Arc::new("age".to_string()))),
                Some(&CelValue::Int(30))
            );
            assert_eq!(
                m.get(&Key::String(Arc::new("active".to_string()))),
                Some(&CelValue::Bool(true))
            );
            assert_eq!(
                m.get(&Key::String(Arc::new("score".to_string()))),
                Some(&CelValue::Float(9.5))
            );
        } else {
            panic!("Expected map");
        }
    }

    #[test]
    fn test_parse_nested_message() {
        let schema = make_schema();
        let input = r#"
name: "Bob"
address {
  city: "NYC"
  zip: "10001"
}
"#;
        let val = parse(input, "User", &schema).unwrap();
        if let CelValue::Map(map) = &val {
            let addr = map.map.get(&Key::String(Arc::new("address".to_string()))).unwrap();
            if let CelValue::Map(addr_map) = addr {
                assert_eq!(
                    addr_map.map.get(&Key::String(Arc::new("city".to_string()))),
                    Some(&CelValue::String(Arc::new("NYC".to_string())))
                );
            } else {
                panic!("Expected address to be a map");
            }
        } else {
            panic!("Expected map");
        }
    }

    #[test]
    fn test_parse_nested_with_colon() {
        let schema = make_schema();
        let input = r#"
name: "Bob"
address: {
  city: "SF"
  zip: "94102"
}
"#;
        let val = parse(input, "User", &schema).unwrap();
        if let CelValue::Map(map) = &val {
            let addr = map.map.get(&Key::String(Arc::new("address".to_string()))).unwrap();
            if let CelValue::Map(addr_map) = addr {
                assert_eq!(
                    addr_map.map.get(&Key::String(Arc::new("city".to_string()))),
                    Some(&CelValue::String(Arc::new("SF".to_string())))
                );
            } else {
                panic!("Expected address to be a map");
            }
        } else {
            panic!("Expected map");
        }
    }

    #[test]
    fn test_parse_repeated() {
        let schema = make_schema();
        let input = r#"
name: "Carol"
tags: "rust"
tags: "wasm"
tags: "proto"
"#;
        let val = parse(input, "User", &schema).unwrap();
        if let CelValue::Map(map) = &val {
            let tags = map.map.get(&Key::String(Arc::new("tags".to_string()))).unwrap();
            if let CelValue::List(list) = tags {
                assert_eq!(list.len(), 3);
                assert_eq!(list[0], CelValue::String(Arc::new("rust".to_string())));
                assert_eq!(list[1], CelValue::String(Arc::new("wasm".to_string())));
                assert_eq!(list[2], CelValue::String(Arc::new("proto".to_string())));
            } else {
                panic!("Expected list");
            }
        } else {
            panic!("Expected map");
        }
    }

    #[test]
    fn test_parse_enum() {
        let schema = make_schema();
        let input = r#"
name: "Dave"
role: ROLE_ADMIN
"#;
        let val = parse(input, "User", &schema).unwrap();
        if let CelValue::Map(map) = &val {
            assert_eq!(
                map.map.get(&Key::String(Arc::new("role".to_string()))),
                Some(&CelValue::String(Arc::new("ROLE_ADMIN".to_string())))
            );
        } else {
            panic!("Expected map");
        }
    }

    #[test]
    fn test_parse_comments() {
        let schema = make_schema();
        let input = r#"
# This is a comment
name: "Eve"  # inline comment
# Another comment
age: 25
"#;
        let val = parse(input, "User", &schema).unwrap();
        if let CelValue::Map(map) = &val {
            assert_eq!(
                map.map.get(&Key::String(Arc::new("name".to_string()))),
                Some(&CelValue::String(Arc::new("Eve".to_string())))
            );
            assert_eq!(
                map.map.get(&Key::String(Arc::new("age".to_string()))),
                Some(&CelValue::Int(25))
            );
        } else {
            panic!("Expected map");
        }
    }

    #[test]
    fn test_parse_string_escaping() {
        let schema = make_schema();
        let input = r#"
name: "hello \"world\"\nbye"
"#;
        let val = parse(input, "User", &schema).unwrap();
        if let CelValue::Map(map) = &val {
            assert_eq!(
                map.map.get(&Key::String(Arc::new("name".to_string()))),
                Some(&CelValue::String(Arc::new("hello \"world\"\nbye".to_string())))
            );
        } else {
            panic!("Expected map");
        }
    }

    #[test]
    fn test_parse_defaults_for_missing() {
        let schema = make_schema();
        let input = r#"
name: "Frank"
"#;
        let val = parse(input, "User", &schema).unwrap();
        if let CelValue::Map(map) = &val {
            // age should default to 0
            assert_eq!(
                map.map.get(&Key::String(Arc::new("age".to_string()))),
                Some(&CelValue::Int(0))
            );
            // active should default to false
            assert_eq!(
                map.map.get(&Key::String(Arc::new("active".to_string()))),
                Some(&CelValue::Bool(false))
            );
            // tags should default to empty list
            if let Some(CelValue::List(list)) = map.map.get(&Key::String(Arc::new("tags".to_string()))) {
                assert!(list.is_empty());
            } else {
                panic!("Expected empty list for tags");
            }
        } else {
            panic!("Expected map");
        }
    }

    #[test]
    fn test_parse_error_unknown_field() {
        let schema = make_schema();
        let input = r#"
name: "Grace"
nonexistent: 42
"#;
        let result = parse(input, "User", &schema);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("Unknown field"));
    }

    #[test]
    fn test_parse_error_bad_syntax() {
        let schema = make_schema();
        let input = "name: \"unterminated";
        let result = parse(input, "User", &schema);
        assert!(result.is_err());
    }

    #[test]
    fn test_skeleton_generation() {
        let schema = make_schema();
        let skeleton = generate_skeleton("User", &schema).unwrap();
        assert!(skeleton.contains("name: \"\""));
        assert!(skeleton.contains("age: 0"));
        assert!(skeleton.contains("active: false"));
        assert!(skeleton.contains("score: 0.0"));
        assert!(skeleton.contains("address {"));
        assert!(skeleton.contains("city: \"\""));
        assert!(skeleton.contains("role: ROLE_UNKNOWN"));
    }

    #[test]
    fn test_skeleton_unknown_message() {
        let schema = make_schema();
        let result = generate_skeleton("Nonexistent", &schema);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty_input() {
        let schema = make_schema();
        let val = parse("", "User", &schema).unwrap();
        // Should get all defaults
        if let CelValue::Map(map) = &val {
            assert_eq!(
                map.map.get(&Key::String(Arc::new("name".to_string()))),
                Some(&CelValue::String(Arc::new(String::new())))
            );
        } else {
            panic!("Expected map");
        }
    }
}
