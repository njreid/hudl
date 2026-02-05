//! Proto block and metadata extraction for Hudl templates.
//!
//! Extracts:
//! - Proto definitions from `/**` comment blocks
//! - Component metadata from `// name:` and `// data:` comments
//!
//! Example:
//! ```hudl
//! /**
//! import "models/user.proto";
//!
//! message UserCardData {
//!     User user = 1;
//!     bool show_email = 2;
//! }
//! */
//!
//! // name: UserCard
//! // data: UserCardData
//!
//! el {
//!     div `user.name`
//! }
//! ```

use regex::Regex;

/// A proto import from a `/** import "..."; */` block
#[derive(Debug, Clone, PartialEq)]
pub struct ProtoImport {
    pub path: String,
    pub line: u32,
}

/// An inline proto definition (message, enum, etc.)
#[derive(Debug, Clone, PartialEq)]
pub struct ProtoDefinition {
    pub content: String,
    pub start_line: u32,
    pub end_line: u32,
}

/// A param definition from `// param:` comments
#[derive(Debug, Clone, PartialEq)]
pub struct ParamDef {
    pub name: String,
    pub type_path: String,
    pub package: Option<String>,
}

/// An import from `// import:` comments
#[derive(Debug, Clone, PartialEq)]
pub struct ImportDef {
    pub alias: String,
    pub path: String,
}

/// Metadata extracted from a Hudl template
#[derive(Debug, Clone, Default)]
pub struct ViewMetadata {
    /// Component name from `// name:` comment
    pub name: Option<String>,
    /// Data type from `// data:` comment
    pub data_type: Option<String>,
    /// Proto imports from `/** import "..."; */`
    pub proto_imports: Vec<ProtoImport>,
    /// Inline proto definitions
    pub proto_definitions: Vec<ProtoDefinition>,
    /// Params from `// param:` comments
    pub params: Vec<ParamDef>,
    /// Imports from `// import:` comments
    pub imports: Vec<ImportDef>,
}

/// Extract metadata from a Hudl template's content
pub fn extract_metadata(content: &str) -> ViewMetadata {
    let mut metadata = ViewMetadata::default();

    // Extract name and data type from comments
    let name_re = Regex::new(r"//\s*name:\s*(\w+)").unwrap();
    let data_re = Regex::new(r"//\s*data:\s*([\w.]+)").unwrap();
    let param_re = Regex::new(r"//\s*param:\s*(\w+)\s+([\w./]+)").unwrap();
    let import_re = Regex::new(r"//\s*import:\s*(\w+)\s+(\S+)").unwrap();

    for line in content.lines() {
        if let Some(caps) = name_re.captures(line) {
            metadata.name = Some(caps[1].to_string());
        }
        if let Some(caps) = data_re.captures(line) {
            metadata.data_type = Some(caps[1].to_string());
        }
        if let Some(caps) = param_re.captures(line) {
            let type_path = caps[2].to_string();
            let package = type_path.rfind('.').map(|i| type_path[..i].to_string());
            metadata.params.push(ParamDef {
                name: caps[1].to_string(),
                type_path,
                package,
            });
        }
        if let Some(caps) = import_re.captures(line) {
            metadata.imports.push(ImportDef {
                alias: caps[1].to_string(),
                path: caps[2].to_string(),
            });
        }
    }

    // Extract proto blocks
    extract_proto_blocks(content, &mut metadata);

    metadata
}

/// Get the fully qualified type for a param definition
pub fn qualified_type(param: &ParamDef) -> String {
    param.type_path.clone()
}

/// Extract proto blocks from `/**` comments
fn extract_proto_blocks(content: &str, metadata: &mut ViewMetadata) {
    let proto_block_re = Regex::new(r"(?s)/\*\*\s*(.*?)\s*\*/").unwrap();
    let import_re = Regex::new(r#"import\s+"([^"]+)"\s*;"#).unwrap();

    for caps in proto_block_re.captures_iter(content) {
        let block_content = &caps[1];
        let block_start = content[..caps.get(0).unwrap().start()]
            .lines()
            .count() as u32;

        // Check for imports
        for import_caps in import_re.captures_iter(block_content) {
            metadata.proto_imports.push(ProtoImport {
                path: import_caps[1].to_string(),
                line: block_start,
            });
        }

        // Check for message/enum definitions
        if block_content.contains("message ") || block_content.contains("enum ") {
            let end_line = block_start
                + block_content.lines().count() as u32;
            metadata.proto_definitions.push(ProtoDefinition {
                content: block_content.to_string(),
                start_line: block_start,
                end_line,
            });
        }
    }
}

/// Get the fully qualified type name for the data type
pub fn resolve_data_type(metadata: &ViewMetadata) -> Option<String> {
    metadata.data_type.clone()
}

/// Check if a message type is defined inline
pub fn is_inline_type(metadata: &ViewMetadata, type_name: &str) -> bool {
    for def in &metadata.proto_definitions {
        if def.content.contains(&format!("message {}", type_name))
            || def.content.contains(&format!("enum {}", type_name))
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_name_and_data() {
        let content = r#"
// name: Dashboard
// data: DashboardData

el {
    div `revenue_formatted`
}
"#;
        let metadata = extract_metadata(content);
        assert_eq!(metadata.name, Some("Dashboard".to_string()));
        assert_eq!(metadata.data_type, Some("DashboardData".to_string()));
    }

    #[test]
    fn test_extract_proto_import() {
        let content = r#"
/** import "models/user.proto"; */

// name: UserCard
// data: User

el {
    div `name`
}
"#;
        let metadata = extract_metadata(content);
        assert_eq!(metadata.proto_imports.len(), 1);
        assert_eq!(metadata.proto_imports[0].path, "models/user.proto");
    }

    #[test]
    fn test_extract_multiple_imports() {
        let content = r#"
/**
import "models/user.proto";
import "models/order.proto";
*/

// name: OrderList
// data: OrderListData

el { }
"#;
        let metadata = extract_metadata(content);
        assert_eq!(metadata.proto_imports.len(), 2);
        assert_eq!(metadata.proto_imports[0].path, "models/user.proto");
        assert_eq!(metadata.proto_imports[1].path, "models/order.proto");
    }

    #[test]
    fn test_extract_inline_message() {
        let content = r#"
/**
message DashboardData {
    string revenue_formatted = 1;
    int32 active_users = 2;
}
*/

// name: Dashboard
// data: DashboardData

el {
    div `revenue_formatted`
}
"#;
        let metadata = extract_metadata(content);
        assert_eq!(metadata.proto_definitions.len(), 1);
        assert!(metadata.proto_definitions[0].content.contains("message DashboardData"));
        assert!(is_inline_type(&metadata, "DashboardData"));
    }

    #[test]
    fn test_extract_inline_enum() {
        let content = r#"
/**
enum Status {
    STATUS_UNKNOWN = 0;
    STATUS_ACTIVE = 1;
    STATUS_PENDING = 2;
}

message Transaction {
    string id = 1;
    Status status = 2;
}
*/

// name: TransactionRow
// data: Transaction

el {
    switch `status` {
        case STATUS_ACTIVE { span "Active" }
        default { span "Unknown" }
    }
}
"#;
        let metadata = extract_metadata(content);
        assert_eq!(metadata.proto_definitions.len(), 1);
        assert!(metadata.proto_definitions[0].content.contains("enum Status"));
        assert!(metadata.proto_definitions[0].content.contains("message Transaction"));
        assert!(is_inline_type(&metadata, "Status"));
        assert!(is_inline_type(&metadata, "Transaction"));
    }

    #[test]
    fn test_no_data_type() {
        let content = r#"
// name: Footer

el {
    footer {
        p "Built with Hudl"
    }
}
"#;
        let metadata = extract_metadata(content);
        assert_eq!(metadata.name, Some("Footer".to_string()));
        assert_eq!(metadata.data_type, None);
    }

    #[test]
    fn test_fully_qualified_data_type() {
        let content = r#"
/** import "myapp/models.proto"; */

// name: UserProfile
// data: myapp.models.User

el {
    div `name`
}
"#;
        let metadata = extract_metadata(content);
        assert_eq!(metadata.data_type, Some("myapp.models.User".to_string()));
    }
}
