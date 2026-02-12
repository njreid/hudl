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
use serde::{Serialize, Deserialize};

/// A parsed proto schema containing messages and enums.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProtoSchema {
    pub messages: HashMap<String, ProtoMessage>,
    pub enums: HashMap<String, ProtoEnum>,
    pub imports: Vec<String>,
}

/// A proto message definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtoMessage {
    pub name: String,
    pub fields: Vec<ProtoField>,
    pub nested_messages: Vec<ProtoMessage>,
    pub nested_enums: Vec<ProtoEnum>,
}

/// A proto field definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtoField {
    pub name: String,
    pub field_type: ProtoType,
    pub number: u32,
    pub repeated: bool,
    pub optional: bool,
}

/// A proto type (scalar, message, enum, or map).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtoEnum {
    pub name: String,
    pub values: Vec<ProtoEnumValue>,
}

/// A proto enum value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtoEnumValue {
    pub name: String,
    pub number: i32,
}

/// A proto error with location information.
#[derive(Debug, Clone)]
pub struct ProtoError {
    pub message: String,
    pub line: u32,
}

use std::path::{Path, PathBuf};

impl ProtoSchema {
    /// Parse proto definitions from a Hudl template content.
    ///
    /// Extracts all `/**` blocks and parses them as proto3 definitions.
    /// If `base_path` is provided, it will also attempt to load and parse imported files.
    pub fn from_template(content: &str, base_path: Option<&Path>) -> Result<Self, Vec<ProtoError>> {
        let mut schema = ProtoSchema::default();
        let mut errors = Vec::new();

        // Extract proto blocks from /** ... */ comments
        let proto_block_re = Regex::new(r"(?s)/\*\*\s*(.*?)\s*\*/").unwrap();

        for caps in proto_block_re.captures_iter(content) {
            let block_match = caps.get(1).unwrap();
            let block_content = block_match.as_str();
            let block_offset = block_match.start();

            // Calculate start line of the block content
            let start_line = content[..block_offset].lines().count() as u32;

            if let Err(e) = schema.parse_block(block_content) {
                errors.push(ProtoError {
                    message: e,
                    line: start_line,
                });
            }
        }

        // Load imports if base_path is provided
        if let Some(base) = base_path {
            if let Err(mut import_errors) = schema.load_imports(base) {
                errors.append(&mut import_errors);
            }
        }

        if errors.is_empty() {
            Ok(schema)
        } else {
            Err(errors)
        }
    }

    /// Load and parse all imported proto files recursively.
    pub fn load_imports(&mut self, base_path: &Path) -> Result<(), Vec<ProtoError>> {
        let mut errors = Vec::new();
        let imports_to_load = self.imports.clone();
        
        for import_path in imports_to_load {
            let full_path = if Path::new(&import_path).is_absolute() {
                PathBuf::from(&import_path)
            } else {
                base_path.join(&import_path)
            };

            if !full_path.exists() {
                // If .proto doesn't exist, try .hudl (some imports might refer to Hudl files with inline protos)
                let hudl_path = full_path.with_extension("hudl");
                if hudl_path.exists() {
                    self.load_from_file(&hudl_path, &mut errors);
                } else {
                    errors.push(ProtoError {
                        message: format!("Import not found: {}", import_path),
                        line: 0,
                    });
                }
                continue;
            }

            self.load_from_file(&full_path, &mut errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn load_from_file(&mut self, path: &Path, errors: &mut Vec<ProtoError>) {
        if let Ok(content) = std::fs::read_to_string(path) {
            let is_hudl = path.extension().and_then(|s| s.to_str()) == Some("hudl");
            
            if is_hudl {
                // For Hudl files, extract the schema and merge
                match Self::from_template(&content, path.parent()) {
                    Ok(other_schema) => self.merge(other_schema),
                    Err(mut e) => errors.append(&mut e),
                }
            } else {
                // For .proto files, parse directly
                if let Err(e) = self.parse_block(&content) {
                    errors.push(ProtoError {
                        message: format!("Error in {}: {}", path.display(), e),
                        line: 0,
                    });
                }
            }
        }
    }

    /// Merge another schema into this one.
    pub fn merge(&mut self, other: ProtoSchema) {
        for (name, msg) in other.messages {
            self.messages.insert(name, msg);
        }
        for (name, en) in other.enums {
            self.enums.insert(name, en);
        }
        for import in other.imports {
            if !self.imports.contains(&import) {
                self.imports.push(import);
            }
        }
    }

    /// Parse a single proto block.
    fn parse_block(&mut self, content: &str) -> Result<(), String> {
        // Simple line-by-line validation to detect obvious syntax errors
        for (i, line) in content.lines().enumerate() {
            let mut trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("syntax") || trimmed == "{" || trimmed == "}" {
                continue;
            }

            // Remove comments from the line for validation
            if let Some(comment_pos) = trimmed.find("//") {
                trimmed = trimmed[..comment_pos].trim();
            }
            if trimmed.is_empty() {
                continue;
            }

            // Check if line matches a known pattern
            let is_import = trimmed.starts_with("import ") && trimmed.ends_with(";");
            let is_message_start = trimmed.starts_with("message ") && trimmed.contains("{");
            let is_enum_start = trimmed.starts_with("enum ") && trimmed.contains("{");
            let is_field = (trimmed.contains("=") || (trimmed.contains("<") && trimmed.contains(">"))) && trimmed.ends_with(";");

            if !is_import && !is_message_start && !is_enum_start && !is_field {
                return Err(format!("Syntax error on line {}: \"{}\"", i + 1, trimmed));
            }
        }

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
        let schema = ProtoSchema::from_template(content, None).unwrap();
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
        let schema = ProtoSchema::from_template(content, None).unwrap();
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
        let schema = ProtoSchema::from_template(content, None).unwrap();
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
        let schema = ProtoSchema::from_template(content, None).unwrap();
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
        let schema = ProtoSchema::from_template(content, None).unwrap();
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
        let schema = ProtoSchema::from_template(content, None).unwrap();

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
        let schema = ProtoSchema::from_template(content, None).unwrap();
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
        let schema = ProtoSchema::from_template(content, None).unwrap();
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
        let schema = ProtoSchema::from_template(content, None).unwrap();
        assert!(schema.imports.contains(&"base.proto".to_string()));
        assert!(schema.messages.contains_key("DashboardData"));
        assert!(schema.messages.contains_key("Metric"));
    }
}
