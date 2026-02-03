use kdl::{KdlDocument, KdlNode};
use crate::ast::{ControlFlow, Root, Node, Element, Text};
use std::collections::HashMap;

pub fn transform(doc: &KdlDocument) -> Result<Root, String> {
    let mut nodes = Vec::new();
    let mut css = None;

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
    Ok(Root { nodes, css })
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
            
            for entry in rule.entries() {
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
                let args: Vec<String> = node.entries().iter()
                    .filter_map(|e| if e.name().is_none() { e.value().as_string().map(|s| s.to_string()) } else { None })
                    .collect();

                let (index_var, variable) = match args.len() {
                    2 => (Some(args[0].clone()), args[1].clone()),
                    1 => (None, args[0].clone()),
                    _ => return Err("each expects 1 or 2 arguments: [index] item".to_string()),
                };

                let iterable = node.entries().iter()
                    .find(|e| e.name().map(|n| n.value() == "of").unwrap_or(false))
                    .and_then(|e| e.value().as_string())
                    .ok_or("each missing 'of' property")?
                    .trim_matches('`')
                    .to_string();

                let children = if let Some(children) = node.children() {
                    transform_block(children.nodes())?
                } else {
                    Vec::new()
                };

                result.push(Node::ControlFlow(ControlFlow::Each {
                    variable,
                    index_var,
                    iterable,
                    children,
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
