use std::collections::HashMap;

#[derive(Debug, PartialEq, Clone)]
pub struct Param {
    pub name: String,
    pub type_name: String,
    pub repeated: bool,
    pub default_value: Option<String>,
}

#[derive(Debug, PartialEq)]
pub struct Root {
    pub nodes: Vec<Node>,
    pub css: Option<String>,
    pub name: Option<String>,      // Component name from // name: comment
    pub params: Vec<Param>,        // Component parameters from // param: comments
    pub imports: Vec<String>,      // Files imported via 'import { ... }'
}

#[derive(Debug, PartialEq)]
pub enum Node {
    Element(Element),
    Text(Text),
    ControlFlow(ControlFlow),
    ContentSlot, // Special token #content
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

/// Convert a Datastar reactive attribute to HTML attribute name and value
pub fn datastar_attr_to_html(attr: &DatastarAttr) -> (String, Option<String>) {
    let mut html_name = String::from("data-");
    let mut html_value = attr.value.clone();

    // 1. Map attribute names based on design spec
    if attr.name.starts_with("let:") {
        let signal_name = &attr.name[4..];
        let val = attr.value.as_deref().unwrap_or("");
        let is_computed = is_computed_expression(val);

        if is_computed {
            html_name.push_str("computed-");
        } else {
            html_name.push_str("signals-");
            // Wrap static strings in single quotes for Datastar if they aren't already numbers/bools
            if !val.is_empty()
                && !val.chars().all(|c| c.is_ascii_digit())
                && val != "true"
                && val != "false"
                && !val.starts_with('\'')
            {
                html_value = Some(format!("'{}'", val));
            }
        }
        html_name.push_str(signal_name);
    } else if attr.name.starts_with('.') {
        html_name.push_str("class-");
        html_name.push_str(&attr.name[1..]);
    } else if attr.name == "on:fetch" {
        html_name.push_str("on:datastar-fetch");
    } else if attr.name.starts_with("on:") {
        let event_name = &attr.name[3..];
        let browser_events = [
            "click",
            "input",
            "change",
            "submit",
            "keydown",
            "keyup",
            "keypress",
            "mouseenter",
            "mouseleave",
            "mouseover",
            "mouseout",
            "mousedown",
            "mouseup",
            "scroll",
            "resize",
            "focus",
            "blur",
            "intersect",
        ];

        if browser_events.contains(&event_name) {
            html_name.push_str("on-");
            html_name.push_str(event_name);
        } else {
            // Custom events use colon and kebab-case
            html_name.push_str("on:");
            html_name.push_str(&to_kebab_case(event_name));
        }
    } else if attr.name == "scrollIntoView" {
        html_name.push_str("scroll-into-view");
    } else {
        // text, show, bind, persist, ref, teleport, and dynamic HTML attributes
        match attr.name.as_str() {
            "bind" | "text" | "show" | "persist" | "ref" | "teleport" | "init" => {
                html_name.push_str(&attr.name);
            }
            _ => {
                html_name.push_str("attr-");
                html_name.push_str(&attr.name);
            }
        }
    }

    // 2. Append modifiers
    for modifier in &attr.modifiers {
        html_name.push_str("__");
        html_name.push_str(&modifier.replace(':', "."));
    }

    (html_name, html_value)
}

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

fn is_computed_expression(expr: &str) -> bool {
    let operators = [
        "+", "-", "*", "/", "==", "!=", ">", "<", ">=", "<=", "&&", "||", "!", "?", ":",
    ];
    if operators.iter().any(|op| expr.contains(op)) {
        return true;
    }
    // contains function/method call
    if expr.contains('(') && expr.contains(')') {
        return true;
    }
    // contains signal reference (starting with $)
    if expr.contains('$') {
        return true;
    }
    false
}
