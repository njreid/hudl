//! Variable scope tracking for Hudl templates.
//!
//! This module handles:
//! - Root scope from `// data:` message type (exposes message fields as variables)
//! - Nested scopes from `each` nodes (introduces loop variables)
//! - CEL expression local variables (e.g., list comprehension variables)

use hudlc::proto::{ProtoSchema, ProtoType, ProtoMessage};
use std::collections::HashMap;

/// Information about a variable in scope
#[derive(Debug, Clone)]
pub struct VarInfo {
    /// The proto type of this variable
    pub proto_type: ProtoType,
    /// Whether this is a repeated field
    pub repeated: bool,
    /// Source of this variable (for error messages)
    pub source: VarSource,
}

/// Where a variable came from
#[derive(Debug, Clone)]
pub enum VarSource {
    /// From the root data message
    DataField,
    /// From an `each` loop variable
    EachLoop { line: u32 },
    /// From a CEL expression (list comprehension, etc.)
    CelLocal,
}

/// A scope containing variables and nested child scopes
#[derive(Debug, Clone)]
pub struct Scope {
    /// Variables in this scope
    pub vars: HashMap<String, VarInfo>,
    /// Parent scope (for lookup)
    parent: Option<Box<Scope>>,
}

impl Scope {
    /// Create a new empty scope
    pub fn new() -> Self {
        Scope {
            vars: HashMap::new(),
            parent: None,
        }
    }

    /// Create a child scope
    pub fn child(&self) -> Self {
        Scope {
            vars: HashMap::new(),
            parent: Some(Box::new(self.clone())),
        }
    }

    /// Add a variable to this scope
    pub fn add_var(&mut self, name: String, info: VarInfo) {
        self.vars.insert(name, info);
    }

    /// Look up a variable, checking parent scopes
    pub fn lookup(&self, name: &str) -> Option<&VarInfo> {
        self.vars.get(name).or_else(|| {
            self.parent.as_ref().and_then(|p| p.lookup(name))
        })
    }

    /// Check if a variable exists in any scope
    pub fn contains(&self, name: &str) -> bool {
        self.lookup(name).is_some()
    }

    /// Get all variable names in scope (including parent scopes)
    pub fn all_vars(&self) -> Vec<String> {
        let mut vars: Vec<String> = self.vars.keys().cloned().collect();
        if let Some(ref parent) = self.parent {
            for v in parent.all_vars() {
                if !vars.contains(&v) {
                    vars.push(v);
                }
            }
        }
        vars
    }
}

impl Default for Scope {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the root scope from a proto schema and data type name
pub fn build_root_scope(schema: &ProtoSchema, data_type: Option<&str>) -> Scope {
    let mut scope = Scope::new();

    if let Some(type_name) = data_type {
        // Add the root variable _data
        scope.add_var(
            "_data".to_string(),
            VarInfo {
                proto_type: ProtoType::Message(type_name.to_string()),
                repeated: false,
                source: VarSource::DataField,
            },
        );

        if let Some(message) = schema.get_message(type_name) {
            add_message_fields_to_scope(&mut scope, message, VarSource::DataField);
        }
    }

    scope
}

/// Add all fields of a message to a scope
fn add_message_fields_to_scope(scope: &mut Scope, message: &ProtoMessage, source: VarSource) {
    for field in &message.fields {
        scope.add_var(
            field.name.clone(),
            VarInfo {
                proto_type: field.field_type.clone(),
                repeated: field.repeated,
                source: source.clone(),
            },
        );
    }
}

/// Extract scopes from the AST, tracking `each` loop variables
/// Returns a map of line number -> scope at that line
pub fn build_scopes_from_content(
    content: &str,
    schema: &ProtoSchema,
    data_type: Option<&str>,
) -> HashMap<u32, Scope> {
    let root_scope = build_root_scope(schema, data_type);
    let mut line_scopes: HashMap<u32, Scope> = HashMap::new();

    // Parse the KDL document
    let doc = match hudlc::parser::parse(content) {
        Ok(doc) => doc,
        Err(_) => return line_scopes,
    };

    // Process nodes recursively to find each loops
    fn process_node(
        node: &kdl::KdlNode,
        current_scope: &Scope,
        line_scopes: &mut HashMap<u32, Scope>,
        schema: &ProtoSchema,
        content: &str,
    ) {
        let node_name = node.name().value();

        // Calculate line number from node span
        let line = node.span().offset();
        let line_num = content[..line].lines().count() as u32;

        // Check if this is an `each` node
        if node_name == "each" {
            let entries: Vec<_> = node.entries().into_iter().collect();
            if entries.len() >= 2 {
                // First entry is the loop variable name
                // Second entry is the collection expression
                let var_name: Option<String> = entries[0].value().as_string()
                    .map(|s| s.trim_matches('`').to_string());
                let collection_expr: Option<String> = entries[1].value().as_string()
                    .map(|s| s.trim_matches('`').to_string());

                if let (Some(var_name), Some(collection_expr)) = (var_name, collection_expr) {
                    // Create a child scope with the loop variable
                    let mut child_scope = current_scope.child();

                    // Try to determine the type of the loop variable
                    // by looking up the collection and getting its element type
                    let var_type = infer_loop_var_type(current_scope, &collection_expr, schema);

                    child_scope.add_var(
                        var_name,
                        VarInfo {
                            proto_type: var_type,
                            repeated: false,
                            source: VarSource::EachLoop { line: line_num },
                        },
                    );

                    // Process children with the new scope
                    if let Some(children) = node.children() {
                        for child in children.nodes() {
                            process_node(child, &child_scope, line_scopes, schema, content);
                        }
                    }

                    // Store the scope for lines within this node
                    line_scopes.insert(line_num, child_scope);
                    return;
                }
            }
        }

        // Store current scope for this line
        line_scopes.insert(line_num, current_scope.clone());

        // Process children with the current scope
        if let Some(children) = node.children() {
            for child in children.nodes() {
                process_node(child, current_scope, line_scopes, schema, content);
            }
        }
    }

    // Process all top-level nodes
    for node in doc.nodes() {
        process_node(node, &root_scope, &mut line_scopes, schema, content);
    }

    // Store root scope for line 0
    line_scopes.insert(0, root_scope);

    line_scopes
}

/// Try to infer the type of a loop variable from the collection expression
fn infer_loop_var_type(scope: &Scope, collection_expr: &str, schema: &ProtoSchema) -> ProtoType {
    // Simple case: collection_expr is just a variable name like "items"
    // or a field path like "user.orders"

    let parts: Vec<&str> = collection_expr.split('.').collect();
    if parts.is_empty() {
        return ProtoType::String; // Fallback
    }

    // Look up the root variable
    let root = parts[0];
    if let Some(var_info) = scope.lookup(root) {
        if parts.len() == 1 {
            // Direct variable reference
            if var_info.repeated {
                // It's a repeated field, return the element type
                return var_info.proto_type.clone();
            }
        } else {
            // Field path - need to traverse
            match &var_info.proto_type {
                ProtoType::Message(msg_name) => {
                    let field_path = parts[1..].join(".");
                    if let Ok(field_type) = schema.resolve_field_path(msg_name, &field_path) {
                        return field_type.clone();
                    }
                }
                _ => {}
            }
        }
    }

    // Fallback to string type
    ProtoType::String
}

/// Get the scope for a specific line number
pub fn get_scope_for_line(line_scopes: &HashMap<u32, Scope>, line: u32) -> Scope {
    // Find the nearest scope at or before this line
    let mut best_line = 0;
    for &scope_line in line_scopes.keys() {
        if scope_line <= line && scope_line > best_line {
            best_line = scope_line;
        }
    }

    line_scopes.get(&best_line).cloned().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_scope() {
        let content = r#"/**
message TestData {
    string title = 1;
    int32 count = 2;
}
*/
// data: TestData
el {
    h1 `title`
}
"#;
        let schema = ProtoSchema::from_template(content, None).unwrap();
        let scope = build_root_scope(&schema, Some("TestData"));

        assert!(scope.contains("title"));
        assert!(scope.contains("count"));
        assert!(!scope.contains("unknown"));
    }

    #[test]
    fn test_each_scope() {
        let content = r#"/**
message ListData {
    repeated string items = 1;
}
*/
// data: ListData
el {
    each item `items` {
        li `item`
    }
}
"#;
        let schema = ProtoSchema::from_template(content, None).unwrap();
        let line_scopes = build_scopes_from_content(content, &schema, Some("ListData"));

        // Root scope should have 'items'
        let root_scope = get_scope_for_line(&line_scopes, 0);
        assert!(root_scope.contains("items"));

        // Inside each, we should have both 'items' and 'item'
        // The each is around line 8-10
        let each_scope = get_scope_for_line(&line_scopes, 9);
        assert!(each_scope.contains("items"));
        assert!(each_scope.contains("item"));
    }

    #[test]
    fn test_if_else_scope() {
        // if/else don't introduce new variables, but should preserve parent scope
        let content = r#"/**
message UserData {
    string name = 1;
    bool is_admin = 2;
}
*/
// data: UserData
el {
    if `is_admin` {
        span "Admin: " `name`
    }
    else {
        span "User: " `name`
    }
}
"#;
        let schema = ProtoSchema::from_template(content, None).unwrap();
        let scope = build_root_scope(&schema, Some("UserData"));

        // Both name and is_admin should be in scope
        assert!(scope.contains("name"));
        assert!(scope.contains("is_admin"));
    }

    #[test]
    fn test_switch_case_scope() {
        // switch/case don't introduce new variables, but should preserve parent scope
        let content = r#"/**
enum Status {
    STATUS_UNKNOWN = 0;
    STATUS_ACTIVE = 1;
    STATUS_PENDING = 2;
}

message OrderData {
    string id = 1;
    Status status = 2;
}
*/
// data: OrderData
el {
    switch `status` {
        case STATUS_ACTIVE {
            span.active `id`
        }
        case STATUS_PENDING {
            span.pending `id`
        }
        default {
            span.unknown `id`
        }
    }
}
"#;
        let schema = ProtoSchema::from_template(content, None).unwrap();
        let scope = build_root_scope(&schema, Some("OrderData"));

        // Both id and status should be in scope
        assert!(scope.contains("id"));
        assert!(scope.contains("status"));
    }

    #[test]
    fn test_nested_message_fields() {
        let content = r#"/**
message Address {
    string city = 1;
    string country = 2;
}

message User {
    string name = 1;
    Address address = 2;
}

message PageData {
    User user = 1;
}
*/
// data: PageData
el {
    h1 `user.name`
    p `user.address.city`
}
"#;
        let schema = ProtoSchema::from_template(content, None).unwrap();
        let scope = build_root_scope(&schema, Some("PageData"));

        // user should be in scope
        assert!(scope.contains("user"));

        // Verify we can resolve nested field paths
        assert!(schema.resolve_field_path("PageData", "user").is_ok());
        assert!(schema.resolve_field_path("User", "name").is_ok());
        assert!(schema.resolve_field_path("User", "address").is_ok());
        assert!(schema.resolve_field_path("Address", "city").is_ok());
    }

    #[test]
    fn test_each_with_nested_message() {
        let content = r#"/**
message Item {
    string name = 1;
    int32 price = 2;
}

message CartData {
    repeated Item items = 1;
    int32 total = 2;
}
*/
// data: CartData
el {
    div `total`
    each item `items` {
        div `item.name`
        span `item.price`
    }
}
"#;
        let schema = ProtoSchema::from_template(content, None).unwrap();
        let line_scopes = build_scopes_from_content(content, &schema, Some("CartData"));

        // Root scope should have 'items' and 'total'
        let root_scope = get_scope_for_line(&line_scopes, 0);
        assert!(root_scope.contains("items"));
        assert!(root_scope.contains("total"));

        // Inside each, should have 'item' in addition to root vars
        let each_scope = get_scope_for_line(&line_scopes, 15);
        assert!(each_scope.contains("item"));
        assert!(each_scope.contains("items")); // Still accessible
        assert!(each_scope.contains("total")); // Still accessible
    }

    #[test]
    fn test_nested_each_loops() {
        let content = r#"/**
message Category {
    string name = 1;
    repeated string items = 2;
}

message CatalogData {
    repeated Category categories = 1;
}
*/
// data: CatalogData
el {
    each cat `categories` {
        h2 `cat.name`
        each item `cat.items` {
            li `item`
        }
    }
}
"#;
        let schema = ProtoSchema::from_template(content, None).unwrap();
        let line_scopes = build_scopes_from_content(content, &schema, Some("CatalogData"));

        // Root scope should have 'categories'
        let root_scope = get_scope_for_line(&line_scopes, 0);
        assert!(root_scope.contains("categories"));

        // First each level should have 'cat'
        let outer_each = get_scope_for_line(&line_scopes, 13);
        assert!(outer_each.contains("cat"));
        assert!(outer_each.contains("categories"));

        // Inner each should have both 'cat' and 'item'
        let inner_each = get_scope_for_line(&line_scopes, 16);
        assert!(inner_each.contains("item"));
        assert!(inner_each.contains("cat"));
        assert!(inner_each.contains("categories"));
    }

    #[test]
    fn test_repeated_scalar_types() {
        let content = r#"/**
message TagsData {
    repeated string tags = 1;
    repeated int32 counts = 2;
    repeated bool flags = 3;
}
*/
// data: TagsData
el {
    each tag `tags` {
        span `tag`
    }
    each count `counts` {
        span `count`
    }
}
"#;
        let schema = ProtoSchema::from_template(content, None).unwrap();
        let scope = build_root_scope(&schema, Some("TagsData"));

        assert!(scope.contains("tags"));
        assert!(scope.contains("counts"));
        assert!(scope.contains("flags"));

        // Verify the types are repeated
        let tags_var = scope.lookup("tags").unwrap();
        assert!(tags_var.repeated);
    }

    #[test]
    fn test_no_data_type() {
        // When no data type is specified, scope should be empty
        let content = r#"/**
message SomeMessage {
    string field = 1;
}
*/
// name: StaticComponent
el {
    div "Static content"
}
"#;
        let schema = ProtoSchema::from_template(content, None).unwrap();
        let scope = build_root_scope(&schema, None);

        // No variables should be in scope
        assert!(!scope.contains("field"));
        assert!(scope.all_vars().is_empty());
    }

    #[test]
    fn test_scope_inheritance() {
        // Test that child scopes properly inherit from parent
        let mut parent = Scope::new();
        parent.add_var("parent_var".to_string(), VarInfo {
            proto_type: ProtoType::String,
            repeated: false,
            source: VarSource::DataField,
        });

        let mut child = parent.child();
        child.add_var("child_var".to_string(), VarInfo {
            proto_type: ProtoType::Int32,
            repeated: false,
            source: VarSource::EachLoop { line: 10 },
        });

        // Child should see both vars
        assert!(child.contains("parent_var"));
        assert!(child.contains("child_var"));

        // Parent should only see parent_var
        assert!(parent.contains("parent_var"));
        assert!(!parent.contains("child_var"));
    }

    #[test]
    fn test_all_vars_listing() {
        let content = r#"/**
message FormData {
    string username = 1;
    string email = 2;
    string password = 3;
}
*/
// data: FormData
el {
    input `username`
}
"#;
        let schema = ProtoSchema::from_template(content, None).unwrap();
        let scope = build_root_scope(&schema, Some("FormData"));

        let vars = scope.all_vars();
        assert!(vars.contains(&"username".to_string()));
        assert!(vars.contains(&"email".to_string()));
        assert!(vars.contains(&"password".to_string()));
        assert!(vars.contains(&"_data".to_string()));
        assert_eq!(vars.len(), 4);
    }

    #[test]
    fn test_cel_macro_temp_variable() {
        // CEL macros like filter, map, all, exists introduce temp variables
        // Example: items.filter(x, x.active) - 'x' is a temp variable
        // This test verifies the scope structure supports this pattern

        let mut scope = Scope::new();
        scope.add_var("items".to_string(), VarInfo {
            proto_type: ProtoType::String,
            repeated: true,
            source: VarSource::DataField,
        });

        // Simulate entering a CEL macro scope
        let mut macro_scope = scope.child();
        macro_scope.add_var("x".to_string(), VarInfo {
            proto_type: ProtoType::String,
            repeated: false,
            source: VarSource::CelLocal,
        });

        // Inside the macro, both 'items' and 'x' should be accessible
        assert!(macro_scope.contains("items"));
        assert!(macro_scope.contains("x"));

        // Outside the macro, only 'items' should be accessible
        assert!(scope.contains("items"));
        assert!(!scope.contains("x"));
    }

    #[test]
    fn test_complex_expression_scopes() {
        // Test a complex template with multiple scope levels
        let content = r#"/**
message User {
    string name = 1;
    bool active = 2;
}

message Team {
    string team_name = 1;
    repeated User members = 2;
}

message PageData {
    repeated Team teams = 1;
    string title = 2;
}
*/
// data: PageData
el {
    h1 `title`
    each team `teams` {
        h2 `team.team_name`
        each member `team.members` {
            div {
                span `member.name`
                if `member.active` {
                    span.badge "Active"
                }
            }
        }
    }
}
"#;
        let schema = ProtoSchema::from_template(content, None).unwrap();
        let line_scopes = build_scopes_from_content(content, &schema, Some("PageData"));

        // Root level: title, teams
        let root = get_scope_for_line(&line_scopes, 0);
        assert!(root.contains("title"));
        assert!(root.contains("teams"));
        assert!(!root.contains("team"));
        assert!(!root.contains("member"));

        // After first each: title, teams, team
        let team_scope = get_scope_for_line(&line_scopes, 20);
        assert!(team_scope.contains("title"));
        assert!(team_scope.contains("teams"));
        assert!(team_scope.contains("team"));

        // After nested each: title, teams, team, member
        let member_scope = get_scope_for_line(&line_scopes, 23);
        assert!(member_scope.contains("title"));
        assert!(member_scope.contains("teams"));
        assert!(member_scope.contains("team"));
        assert!(member_scope.contains("member"));
    }
}
