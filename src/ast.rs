use std::collections::HashMap;

#[derive(Debug, PartialEq)]
pub struct Root {
    pub nodes: Vec<Node>,
    pub css: Option<String>,
    pub name: Option<String>,      // Component name from // name: comment
    pub data_type: Option<String>, // Data type from // data: comment
    pub imports: Vec<String>,      // Files imported via 'import { ... }'
}

#[derive(Debug, PartialEq)]
pub enum Node {
    Element(Element),
    Text(Text),
    ControlFlow(ControlFlow),
}

#[derive(Debug, PartialEq)]
pub struct Element {
    pub tag: String,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub attributes: HashMap<String, String>,
    pub children: Vec<Node>,
    /// Scoped styles for this element: Vec<(property, value)>
    pub styles: Vec<(String, String)>,
    /// Datastar reactive attributes from tilde blocks/inline
    /// Key is the Hudl attribute name (e.g., "on:click", ".active", "let:count")
    /// Value is (expression, modifiers) where modifiers is a list like ["once", "prevent"]
    pub datastar: Vec<DatastarAttr>,
}

/// A Datastar reactive attribute
#[derive(Debug, PartialEq, Clone)]
pub struct DatastarAttr {
    /// The attribute type/name (e.g., "on:click", "show", "let:count", ".active")
    pub name: String,
    /// The expression value (e.g., "$count++", "$isVisible")
    pub value: Option<String>,
    /// Modifiers (e.g., ["once", "prevent"], ["debounce:300ms"])
    pub modifiers: Vec<String>,
}

#[derive(Debug, PartialEq)]
pub struct Text {
    pub content: String,
}

#[derive(Debug, PartialEq)]
pub enum ControlFlow {
    If {
        condition: String,
        then_block: Vec<Node>,
        else_block: Option<Vec<Node>>,
    },
    Each {
        binding: String,   // Loop variable name (e.g., "item")
        iterable: String,  // CEL expression for the collection
        body: Vec<Node>,
    },
    Switch {
        expr: String,
        cases: Vec<SwitchCase>,
        default: Option<Vec<Node>>,
    },
}

/// Switch case: (pattern, children)
/// Pattern is either an enum value like "STATUS_ACTIVE" or a CEL expression
#[derive(Debug, PartialEq)]
pub struct SwitchCase(pub String, pub Vec<Node>);

// Helpers for tests
impl Node {
    pub fn as_element(&self) -> Option<&Element> {
        match self {
            Node::Element(e) => Some(e),
            _ => None,
        }
    }
    pub fn as_text(&self) -> Option<&Text> {
        match self {
            Node::Text(t) => Some(t),
            _ => None,
        }
    }
    pub fn as_control_flow(&self) -> Option<&ControlFlow> {
        match self {
            Node::ControlFlow(c) => Some(c),
            _ => None,
        }
    }
}
