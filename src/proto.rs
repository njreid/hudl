//! Protocol Buffer definition parsing for Hudl templates.
//!
//! This module parses proto3 definitions embedded in `/**` comment blocks
//! and provides type information for validation and code generation.
//!
//! Example:
//! ```text
//! /**
//! message User {
//!     string name = 1;
//!     string email = 2;
//!     Address address = 3;
//! }
//!
//! message Address {
//!     string city = 1;
//!     string country = 2;
//! }
//! */
//! ```

use regex::Regex;
use std::collections::HashMap;

/// A parsed proto schema containing messages and enums.
#[derive(Debug, Clone, Default)]
pub struct ProtoSchema {
    pub messages: HashMap<String, ProtoMessage>,
    pub enums: HashMap<String, ProtoEnum>,
    pub imports: Vec<String>,
}

/// A proto message definition.
#[derive(Debug, Clone)]
pub struct ProtoMessage {
    pub name: String,
    pub fields: Vec<ProtoField>,
    pub nested_messages: Vec<ProtoMessage>,
    pub nested_enums: Vec<ProtoEnum>,
}

/// A proto field definition.
#[derive(Debug, Clone)]
pub struct ProtoField {
    pub name: String,
    pub field_type: ProtoType,
    pub number: u32,
    pub repeated: bool,
    pub optional: bool,
}

/// A proto type (scalar, message, enum, or map).
#[derive(Debug, Clone, PartialEq)]
pub enum ProtoType {
    // Scalar types
    Double,
    Float,
    Int32,
    Int64,
    Uint32,
    Uint64,
    Sint32,
    Sint64,
    Fixed32,
    Fixed64,
    Sfixed32,
    Sfixed64,
    Bool,
    String,
    Bytes,
    // Reference to a message or enum
    Message(String),
    Enum(String),
    // Map type
    Map(Box<ProtoType>, Box<ProtoType>),
}

/// A proto enum definition.
#[derive(Debug, Clone)]
pub struct ProtoEnum {
    pub name: String,
    pub values: Vec<ProtoEnumValue>,
}

/// A proto enum value.
#[derive(Debug, Clone)]
pub struct ProtoEnumValue {
    pub name: String,
    pub number: i32,
}

impl ProtoSchema {
    /// Parse proto definitions from a Hudl template content.
    ///
    /// Extracts all `/**` blocks and parses them as proto3 definitions.
    pub fn from_template(content: &str) -> Result<Self, String> {
        let mut schema = ProtoSchema::default();

        // Extract proto blocks from /** ... */ comments
        let proto_block_re = Regex::new(r"(?s)/\*\*\s*(.*?)\s*\*/").unwrap();

        for caps in proto_block_re.captures_iter(content) {
            let block_content = &caps[1];
            schema.parse_block(block_content)?;
        }

        Ok(schema)
    }

    /// Parse a single proto block.
    fn parse_block(&mut self, content: &str) -> Result<(), String> {
        // Parse imports
        let import_re = Regex::new(r#"import\s+"([^"]+)"\s*;"#).unwrap();
        for caps in import_re.captures_iter(content) {
            self.imports.push(caps[1].to_string());
        }

        // Parse messages
        self.parse_messages(content)?;

        // Parse enums
        self.parse_enums(content)?;

        Ok(())
    }

    /// Parse message definitions from proto content.
    fn parse_messages(&mut self, content: &str) -> Result<(), String> {
        // Simple regex-based parser for message definitions
        // For production, use protobuf-parse crate for full compliance
        let message_re = Regex::new(r"message\s+(\w+)\s*\{([^}]*)\}").unwrap();

        for caps in message_re.captures_iter(content) {
            let name = caps[1].to_string();
            let body = &caps[2];

            let fields = self.parse_fields(body)?;

            self.messages.insert(
                name.clone(),
                ProtoMessage {
                    name,
                    fields,
                    nested_messages: Vec::new(),
                    nested_enums: Vec::new(),
                },
            );
        }

        Ok(())
    }

    /// Parse field definitions from a message body.
    fn parse_fields(&self, body: &str) -> Result<Vec<ProtoField>, String> {
        let mut fields = Vec::new();

        // Match: [repeated] [optional] type name = number;
        let field_re = Regex::new(
            r"(?m)^\s*(repeated\s+|optional\s+)?([\w.]+)\s+(\w+)\s*=\s*(\d+)\s*;",
        )
        .unwrap();

        for caps in field_re.captures_iter(body) {
            let modifier = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("");
            let type_str = &caps[2];
            let name = caps[3].to_string();
            let number: u32 = caps[4].parse().unwrap_or(0);

            let field_type = Self::parse_type(type_str);
            let repeated = modifier == "repeated";
            let optional = modifier == "optional";

            fields.push(ProtoField {
                name,
                field_type,
                number,
                repeated,
                optional,
            });
        }

        // Also match map fields: map<key, value> name = number;
        let map_re = Regex::new(r"map\s*<\s*(\w+)\s*,\s*([\w.]+)\s*>\s+(\w+)\s*=\s*(\d+)\s*;")
            .unwrap();

        for caps in map_re.captures_iter(body) {
            let key_type = Self::parse_type(&caps[1]);
            let value_type = Self::parse_type(&caps[2]);
            let name = caps[3].to_string();
            let number: u32 = caps[4].parse().unwrap_or(0);

            fields.push(ProtoField {
                name,
                field_type: ProtoType::Map(Box::new(key_type), Box::new(value_type)),
                number,
                repeated: false,
                optional: false,
            });
        }

        Ok(fields)
    }

    /// Parse a type string into a ProtoType.
    fn parse_type(type_str: &str) -> ProtoType {
        match type_str {
            "double" => ProtoType::Double,
            "float" => ProtoType::Float,
            "int32" => ProtoType::Int32,
            "int64" => ProtoType::Int64,
            "uint32" => ProtoType::Uint32,
            "uint64" => ProtoType::Uint64,
            "sint32" => ProtoType::Sint32,
            "sint64" => ProtoType::Sint64,
            "fixed32" => ProtoType::Fixed32,
            "fixed64" => ProtoType::Fixed64,
            "sfixed32" => ProtoType::Sfixed32,
            "sfixed64" => ProtoType::Sfixed64,
            "bool" => ProtoType::Bool,
            "string" => ProtoType::String,
            "bytes" => ProtoType::Bytes,
            // Assume anything else is a message reference
            other => ProtoType::Message(other.to_string()),
        }
    }

    /// Parse enum definitions from proto content.
    fn parse_enums(&mut self, content: &str) -> Result<(), String> {
        let enum_re = Regex::new(r"enum\s+(\w+)\s*\{([^}]*)\}").unwrap();
        let value_re = Regex::new(r"(\w+)\s*=\s*(-?\d+)\s*;").unwrap();

        for caps in enum_re.captures_iter(content) {
            let name = caps[1].to_string();
            let body = &caps[2];

            let mut values = Vec::new();
            for vcaps in value_re.captures_iter(body) {
                values.push(ProtoEnumValue {
                    name: vcaps[1].to_string(),
                    number: vcaps[2].parse().unwrap_or(0),
                });
            }

            self.enums.insert(name.clone(), ProtoEnum { name, values });
        }

        Ok(())
    }

    /// Get a message by name.
    pub fn get_message(&self, name: &str) -> Option<&ProtoMessage> {
        self.messages.get(name)
    }

    /// Get an enum by name.
    pub fn get_enum(&self, name: &str) -> Option<&ProtoEnum> {
        self.enums.get(name)
    }

    /// Resolve a field path on a message type.
    ///
    /// Returns the final field type, or an error if the path is invalid.
    pub fn resolve_field_path(&self, message_name: &str, path: &str) -> Result<&ProtoType, String> {
        if path.is_empty() {
            return Err("Empty field path".to_string());
        }

        let parts: Vec<&str> = path.split('.').collect();
        let mut current_message = self
            .get_message(message_name)
            .ok_or_else(|| format!("Unknown message: {}", message_name))?;

        for (i, part) in parts.iter().enumerate() {
            let field = current_message
                .fields
                .iter()
                .find(|f| f.name == *part)
                .ok_or_else(|| {
                    format!(
                        "Field '{}' not found on message '{}'",
                        part, current_message.name
                    )
                })?;

            if i == parts.len() - 1 {
                // Last part - return the field type
                return Ok(&field.field_type);
            }

            // Navigate to nested message
            match &field.field_type {
                ProtoType::Message(msg_name) => {
                    current_message = self.get_message(msg_name).ok_or_else(|| {
                        format!("Unknown message type: {}", msg_name)
                    })?;
                }
                _ => {
                    return Err(format!(
                        "Cannot access field '{}' on non-message type {:?}",
                        parts[i + 1],
                        field.field_type
                    ));
                }
            }
        }

        unreachable!()
    }

    /// Get all enum values for a given enum name.
    pub fn get_enum_values(&self, name: &str) -> Option<Vec<String>> {
        self.enums
            .get(name)
            .map(|e| e.values.iter().map(|v| v.name.clone()).collect())
    }
}

impl ProtoType {
    /// Check if this type is a scalar type.
    pub fn is_scalar(&self) -> bool {
        matches!(
            self,
            ProtoType::Double
                | ProtoType::Float
                | ProtoType::Int32
                | ProtoType::Int64
                | ProtoType::Uint32
                | ProtoType::Uint64
                | ProtoType::Sint32
                | ProtoType::Sint64
                | ProtoType::Fixed32
                | ProtoType::Fixed64
                | ProtoType::Sfixed32
                | ProtoType::Sfixed64
                | ProtoType::Bool
                | ProtoType::String
                | ProtoType::Bytes
        )
    }

    /// Get the CEL type name for this proto type.
    pub fn cel_type(&self) -> &str {
        match self {
            ProtoType::Double | ProtoType::Float => "double",
            ProtoType::Int32
            | ProtoType::Int64
            | ProtoType::Uint32
            | ProtoType::Uint64
            | ProtoType::Sint32
            | ProtoType::Sint64
            | ProtoType::Fixed32
            | ProtoType::Fixed64
            | ProtoType::Sfixed32
            | ProtoType::Sfixed64 => "int",
            ProtoType::Bool => "bool",
            ProtoType::String => "string",
            ProtoType::Bytes => "bytes",
            ProtoType::Message(name) => name,
            ProtoType::Enum(name) => name,
            ProtoType::Map(_, _) => "map",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_message() {
        let content = r#"
/**
message User {
    string name = 1;
    string email = 2;
    int32 age = 3;
}
*/
"#;
        let schema = ProtoSchema::from_template(content).unwrap();
        assert!(schema.messages.contains_key("User"));

        let user = schema.get_message("User").unwrap();
        assert_eq!(user.fields.len(), 3);
        assert_eq!(user.fields[0].name, "name");
        assert_eq!(user.fields[0].field_type, ProtoType::String);
        assert_eq!(user.fields[2].field_type, ProtoType::Int32);
    }

    #[test]
    fn test_parse_nested_message_reference() {
        let content = r#"
/**
message User {
    string name = 1;
    Address address = 2;
}

message Address {
    string city = 1;
    string country = 2;
}
*/
"#;
        let schema = ProtoSchema::from_template(content).unwrap();
        assert!(schema.messages.contains_key("User"));
        assert!(schema.messages.contains_key("Address"));

        let user = schema.get_message("User").unwrap();
        assert_eq!(
            user.fields[1].field_type,
            ProtoType::Message("Address".to_string())
        );
    }

    #[test]
    fn test_parse_repeated_field() {
        let content = r#"
/**
message UserList {
    repeated User users = 1;
}
*/
"#;
        let schema = ProtoSchema::from_template(content).unwrap();
        let list = schema.get_message("UserList").unwrap();
        assert!(list.fields[0].repeated);
    }

    #[test]
    fn test_parse_enum() {
        let content = r#"
/**
enum Status {
    STATUS_UNKNOWN = 0;
    STATUS_ACTIVE = 1;
    STATUS_INACTIVE = 2;
}
*/
"#;
        let schema = ProtoSchema::from_template(content).unwrap();
        assert!(schema.enums.contains_key("Status"));

        let status = schema.get_enum("Status").unwrap();
        assert_eq!(status.values.len(), 3);
        assert_eq!(status.values[0].name, "STATUS_UNKNOWN");
        assert_eq!(status.values[1].name, "STATUS_ACTIVE");
    }

    #[test]
    fn test_parse_imports() {
        let content = r#"
/**
import "models/user.proto";
import "common/types.proto";

message Dashboard {
    string title = 1;
}
*/
"#;
        let schema = ProtoSchema::from_template(content).unwrap();
        assert_eq!(schema.imports.len(), 2);
        assert!(schema.imports.contains(&"models/user.proto".to_string()));
        assert!(schema.imports.contains(&"common/types.proto".to_string()));
    }

    #[test]
    fn test_resolve_field_path() {
        let content = r#"
/**
message User {
    string name = 1;
    Profile profile = 2;
}

message Profile {
    string bio = 1;
    Address address = 2;
}

message Address {
    string city = 1;
}
*/
"#;
        let schema = ProtoSchema::from_template(content).unwrap();

        // Direct field access
        let name_type = schema.resolve_field_path("User", "name").unwrap();
        assert_eq!(*name_type, ProtoType::String);

        // Nested field access
        let bio_type = schema.resolve_field_path("User", "profile.bio").unwrap();
        assert_eq!(*bio_type, ProtoType::String);

        // Deeply nested field access
        let city_type = schema
            .resolve_field_path("User", "profile.address.city")
            .unwrap();
        assert_eq!(*city_type, ProtoType::String);

        // Invalid field
        let err = schema.resolve_field_path("User", "invalid");
        assert!(err.is_err());
    }

    #[test]
    fn test_get_enum_values() {
        let content = r#"
/**
enum Role {
    ROLE_UNKNOWN = 0;
    ROLE_ADMIN = 1;
    ROLE_USER = 2;
    ROLE_GUEST = 3;
}
*/
"#;
        let schema = ProtoSchema::from_template(content).unwrap();
        let values = schema.get_enum_values("Role").unwrap();
        assert_eq!(values.len(), 4);
        assert!(values.contains(&"ROLE_ADMIN".to_string()));
    }

    #[test]
    fn test_parse_map_field() {
        let content = r#"
/**
message Config {
    map<string, string> settings = 1;
    map<string, User> users = 2;
}
*/
"#;
        let schema = ProtoSchema::from_template(content).unwrap();
        let config = schema.get_message("Config").unwrap();

        assert_eq!(config.fields.len(), 2);
        match &config.fields[0].field_type {
            ProtoType::Map(k, v) => {
                assert_eq!(**k, ProtoType::String);
                assert_eq!(**v, ProtoType::String);
            }
            _ => panic!("Expected map type"),
        }
    }

    #[test]
    fn test_multiple_proto_blocks() {
        let content = r#"
/** import "base.proto"; */

// name: Dashboard
// data: DashboardData

/**
message DashboardData {
    string title = 1;
    repeated Metric metrics = 2;
}

message Metric {
    string name = 1;
    double value = 2;
}
*/

el {
    div `title`
}
"#;
        let schema = ProtoSchema::from_template(content).unwrap();
        assert!(schema.imports.contains(&"base.proto".to_string()));
        assert!(schema.messages.contains_key("DashboardData"));
        assert!(schema.messages.contains_key("Metric"));
    }
}
