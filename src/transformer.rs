use kdl::{KdlDocument, KdlNode};
use regex::Regex;
use crate::ast::{ControlFlow, SwitchCase, Root, Node, Element, Text};
use std::collections::HashMap;

pub fn transform(doc: &KdlDocument) -> Result<Root, String> {
    let mut nodes = Vec::new();
    let mut css = None;
    let name = None;
    let data_type = None;

    for node in doc.nodes() {
        if node.name().value() == "el" {
            if let Some(children) = node.children() {
                let mut view_nodes = Vec::new();
                for child in children.nodes() {
                    if child.name().value() == "css" {
                        css = Some(process_css(child)?);
                    } else {
                        view_nodes.push(child.clone());
                    }
                }
                nodes.append(&mut transform_block(&view_nodes)?);
            }
        }
    }
    Ok(Root { nodes, css, name, data_type })
}

/// Extract component metadata from raw content (before KDL parsing)
pub fn extract_metadata(content: &str) -> (Option<String>, Option<String>) {
    let name_re = Regex::new(r"//\s*name:\s*(\w+)").unwrap();
    let data_re = Regex::new(r"//\s*data:\s*([\w.]+)").unwrap();

    let name = name_re.captures(content).map(|c| c[1].to_string());
    let data_type = data_re.captures(content).map(|c| c[1].to_string());

    (name, data_type)
}

/// Transform with metadata extraction from raw content
pub fn transform_with_metadata(doc: &KdlDocument, raw_content: &str) -> Result<Root, String> {
    let mut root = transform(doc)?;
    let (name, data_type) = extract_metadata(raw_content);
    root.name = name;
    root.data_type = data_type;
    Ok(root)
}

fn process_css(node: &KdlNode) -> Result<String, String> {
    let mut css_output = String::new();
    if let Some(children) = node.children() {
        for rule in children.nodes() {
            let selector_raw = rule.name().value();
            // Convert &header -> #header
            let selector = if selector_raw.starts_with('&') {
                selector_raw.replace("&", "#")
            } else {
                selector_raw.to_string()
            };
            
            css_output.push_str(&selector);
            css_output.push_str(" { ");
            
            for _entry in rule.entries() {
                // Property name is usually the entry name?
                // Wait, KDL: `margin _0;` -> node `margin` with arg `_0`.
                // But here we are iterating *children* of `css`.
                // `&header { margin _0; }`
                // `rule` is `&header`.
                // `rule` children are the properties?
            }
            
            if let Some(props) = rule.children() {
                for prop in props.nodes() {
                    let prop_name = prop.name().value();
                    let val = prop.entries().get(0)
                        .and_then(|e| e.value().as_string())
                        .unwrap_or("");
                    
                    // Handle numeric values with _ prefix
                    let clean_val = if val.starts_with('_') {
                        &val[1..]
                    } else {
                        val
                    };
                    
                    css_output.push_str(&format!("{}: {}; ", prop_name, clean_val));
                }
            }
            
            css_output.push_str("}\n");
        }
    }
    Ok(css_output)
}

fn parse_selector(input: &str) -> (String, Option<String>, Vec<String>) {
    let mut tag = "div".to_string();
    let mut id = None;
    let mut classes = Vec::new();
    
    // Heuristic parsing
    let mut current_token = String::new();
    let mut mode = 't'; // t=tag, i=id, c=class
    
    // Check if it starts with shorthand
    let start_idx = if input.starts_with('&') {
        mode = 'i';
        1
    } else if input.starts_with('.') {
        mode = 'c';
        1
    } else {
        0
    };

    if start_idx == 1 {
        // Tag remains "div"
    } else {
        // We are parsing tag
    }

    for c in input[start_idx..].chars() {
        if c == '&' {
            // Commit current
            match mode {
                't' => tag = current_token,
                'i' => id = Some(current_token),
                'c' => classes.push(current_token),
                _ => {}
            }
            current_token = String::new();
            mode = 'i';
        } else if c == '.' {
            // Commit current
            match mode {
                't' => tag = current_token,
                'i' => id = Some(current_token),
                'c' => classes.push(current_token),
                _ => {}
            }
            current_token = String::new();
            mode = 'c';
        } else {
            current_token.push(c);
        }
    }
    // Commit last
    match mode {
        't' => tag = current_token,
        'i' => id = Some(current_token),
        'c' => classes.push(current_token),
        _ => {}
    }

    (tag, id, classes)
}

fn transform_block(nodes: &[KdlNode]) -> Result<Vec<Node>, String> {
    let mut result = Vec::new();
    let mut iter = nodes.iter().peekable();

    while let Some(node) = iter.next() {
        let name = node.name().value();
        match name {
            "if" => {
                let condition = node.entries().get(0)
                    .and_then(|e| e.value().as_string())
                    .ok_or("if node missing condition")?
                    .to_string();
                
                let clean_cond = condition.trim_matches('`').to_string();

                let then_block = if let Some(children) = node.children() {
                    transform_block(children.nodes())?
                } else {
                    Vec::new()
                };

                let mut else_block = None;
                let mut has_else = false;
                if let Some(next_node) = iter.peek() {
                    if next_node.name().value() == "else" {
                        has_else = true;
                    }
                }

                if has_else {
                    let next_node = iter.next().unwrap(); // consume else
                    if let Some(children) = next_node.children() {
                        else_block = Some(transform_block(children.nodes())?);
                    }
                }

                result.push(Node::ControlFlow(ControlFlow::If {
                    condition: clean_cond,
                    then_block,
                    else_block,
                }));
            }
            "each" => {
                // New syntax: each binding `iterable` { ... }
                // Two positional arguments: binding name and CEL expression
                let args: Vec<String> = node.entries().iter()
                    .filter_map(|e| if e.name().is_none() { e.value().as_string().map(|s| s.to_string()) } else { None })
                    .collect();

                if args.len() != 2 {
                    return Err("each expects 2 arguments: binding `iterable`".to_string());
                }

                let binding = args[0].clone();
                let iterable = args[1].trim_matches('`').to_string();

                let body = if let Some(children) = node.children() {
                    transform_block(children.nodes())?
                } else {
                    Vec::new()
                };

                result.push(Node::ControlFlow(ControlFlow::Each {
                    binding,
                    iterable,
                    body,
                }));
            }
            "switch" => {
                let expr = node.entries().get(0)
                    .and_then(|e| e.value().as_string())
                    .ok_or("switch node missing expression")?
                    .trim_matches('`')
                    .to_string();

                let mut cases = Vec::new();
                let mut default = None;

                if let Some(children) = node.children() {
                    for child in children.nodes() {
                        match child.name().value() {
                            "case" => {
                                // Pattern can be a bare identifier (enum value) or string
                                let pattern = child.entries().get(0)
                                    .and_then(|e| e.value().as_string())
                                    .ok_or("case missing pattern")?
                                    .to_string();

                                let case_children = if let Some(block) = child.children() {
                                    transform_block(block.nodes())?
                                } else {
                                    Vec::new()
                                };

                                cases.push(SwitchCase(pattern, case_children));
                            }
                            "default" => {
                                let def_children = if let Some(block) = child.children() {
                                    transform_block(block.nodes())?
                                } else {
                                    Vec::new()
                                };
                                default = Some(def_children);
                            }
                            _ => {}
                        }
                    }
                }

                result.push(Node::ControlFlow(ControlFlow::Switch {
                    expr,
                    cases,
                    default,
                }));
            }
            "else" => {
                return Err("Unexpected 'else' without matching 'if'".to_string());
            }
            _ => {
                result.push(transform_node(node)?);
            }
        }
    }
    Ok(result)
}

fn transform_node(node: &KdlNode) -> Result<Node, String> {
    let name = node.name().value();
    
    let (tag, mut id, mut classes) = parse_selector(name);
    
    let mut attributes = HashMap::new();
    let mut children = Vec::new();

    // 1. Process entries (Properties and Arguments)
    for entry in node.entries() {
        if let Some(prop_name) = entry.name() {
            let key = prop_name.value();
            let val = entry.value().as_string().unwrap_or_default().to_string();
            
            match key {
                "id" => id = Some(val),
                "class" => classes.extend(val.split_whitespace().map(|s| s.to_string())),
                _ => { attributes.insert(key.to_string(), val); }
            }
        } else {
            // Positional argument -> Text content
            if let Some(v) = entry.value().as_string() {
                children.push(Node::Text(Text { content: v.to_string() }));
            }
        }
    }

    // 2. Process children
    if let Some(child_block) = node.children() {
        children.append(&mut transform_block(child_block.nodes())?);
    }

    Ok(Node::Element(Element {
        tag,
        id,
        classes,
        attributes,
        children,
    }))
}
