//! Proto block and metadata extraction for Hudl templates.
//!
//! Extracts:
//! - Proto definitions from `/**` comment blocks
//! - Component metadata from `// name:` and `// param:` comments
//!
//! Example:
//! ```hudl
//! /**
//! import "models/user.proto";
//!
//! message UserData {
//!     User user = 1;
//!     bool show_email = 2;
//! }
//! */
//!
//! // name: UserCard
//! // param: UserData data
//!
//! el {
//!     div `data.user.name`
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
    pub type_name: String,
    pub repeated: bool,
    pub default_value: Option<String>,
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

    // Extract name from comments
    let name_re = Regex::new(r"//\s*name:\s*(\w+)").unwrap();
    // param: [repeated] <type> <name> [default]
    let param_re = Regex::new(r#"//\s*param:\s*(repeated\s+)?([\w.]+)\s+(\w+)(?:\s+(.*))?"#).unwrap();
    let import_re = Regex::new(r"//\s*import:\s*(\w+)\s+(\S+)").unwrap();

    for line in content.lines() {
        if let Some(caps) = name_re.captures(line) {
            metadata.name = Some(caps[1].to_string());
        }
        if let Some(caps) = param_re.captures(line) {
            let repeated = caps.get(1).is_some();
            let type_name = caps[2].to_string();
            let name = caps[3].to_string();
            let default_value = caps.get(4).map(|m| {
                let s = m.as_str().trim();
                if s.starts_with('"') && s.ends_with('"') {
                    s[1..s.len()-1].to_string()
                } else {
                    s.to_string()
                }
            });
            
            metadata.params.push(ParamDef {
                name,
                type_name,
                repeated,
                default_value,
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
#[allow(dead_code)]
pub fn qualified_type(param: &ParamDef) -> String {
    param.type_name.clone()
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

/// Check if a message type is defined inline
#[allow(dead_code)]
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
    fn test_extract_name_and_param() {
        let content = r#"
// name: Dashboard
// param: DashboardData data

el {
    div `data.revenue_formatted`
}
"#;
        let metadata = extract_metadata(content);
        assert_eq!(metadata.name, Some("Dashboard".to_string()));
        assert_eq!(metadata.params.len(), 1);
        assert_eq!(metadata.params[0].name, "data");
        assert_eq!(metadata.params[0].type_name, "DashboardData");
    }

    #[test]
    fn test_extract_proto_import() {
        let content = r#"
/** import "models/user.proto"; */

// name: UserCard
// param: User user

el {
    div `user.name`
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
// param: OrderListData data

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
// param: DashboardData data

el {
    div `data.revenue_formatted`
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
// param: Transaction tx

el {
    switch `tx.status` {
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
    fn test_no_params() {
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
        assert_eq!(metadata.params.len(), 0);
    }

    #[test]
    fn test_fully_qualified_param_type() {
        let content = r#"
/** import "myapp/models.proto"; */

// name: UserProfile
// param: myapp.models.User user

el {
    div `user.name`
}
"#;
        let metadata = extract_metadata(content);
        assert_eq!(metadata.params[0].type_name, "myapp.models.User");
    }
}
