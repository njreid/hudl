use kdl::{KdlDocument, KdlNode};
use regex::Regex;
use crate::ast::{ControlFlow, SwitchCase, Root, Node, Element, Text, DatastarAttr};
use std::collections::HashMap;

pub fn transform(doc: &KdlDocument) -> Result<Root, String> {
    let mut nodes = Vec::new();
    let mut css = None;
    let mut imports = Vec::new();
    let name = None;
    let data_type = None;

    for node in doc.nodes() {
        match node.name().value() {
            "__hudl_import" => {
                if let Some(children) = node.children() {
                    for import_node in children.nodes() {
                        imports.push(import_node.name().value().to_string());
                    }
                }
            }
            "__hudl_el" => {
                if let Some(children) = node.children() {
                    let mut view_nodes = Vec::new();
                    for child in children.nodes() {
                        if child.name().value() == "__hudl_css" {
                            css = Some(process_css(child)?);
                        } else {
                            view_nodes.push(child.clone());
                        }
                    }
                    nodes.append(&mut transform_block(&view_nodes)?);
                }
            }
            _ => {}
        }
    }
    Ok(Root { nodes, css, name, data_type, imports })
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

/// Process a style block inside an element
/// Returns Vec<(property, value)> for the element's styles
fn process_element_style(node: &KdlNode) -> Result<Vec<(String, String)>, String> {
    let mut styles = Vec::new();

    if let Some(children) = node.children() {
        for prop in children.nodes() {
            let prop_name = prop.name().value();

            // Get the value - can be a string argument or identifier
            let val = prop.entries().get(0)
                .map(|e| {
                    if let Some(s) = e.value().as_string() {
                        s.to_string()
                    } else if let Some(b) = e.value().as_bool() {
                        b.to_string()
                    } else if let Some(i) = e.value().as_integer() {
                        i.to_string()
                    } else if let Some(f) = e.value().as_float() {
                        f.to_string()
                    } else {
                        String::new()
                    }
                })
                .unwrap_or_default();

            // Handle numeric values with _ prefix (for KDL compatibility)
            let clean_val = if val.starts_with('_') {
                val[1..].to_string()
            } else {
                val
            };

            if !prop_name.is_empty() && !clean_val.is_empty() {
                styles.push((prop_name.to_string(), clean_val));
            }
        }
    }

    Ok(styles)
}

fn process_css(node: &KdlNode) -> Result<String, String> {
    let mut css_output = String::new();
    if let Some(children) = node.children() {
        for rule in children.nodes() {
            let selector = rule.name().value();

            css_output.push_str(selector);
            css_output.push_str(" { ");

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

fn parse_selector(input: &str) -> (String, Option<String>, Vec<String>, Vec<DatastarAttr>) {
    let mut tag = "div".to_string();
    let mut id = None;
    let mut classes = Vec::new();
    let mut datastar = Vec::new();
    
    // Check for binding shorthand: element~>signal
    let (input_cleaned, _remaining) = if let Some(idx) = input.find("~>") {
        let tag_part = &input[..idx];
        let bind_part = &input[idx + 2..];
        
        // Split bind part into signal name and modifiers
        let (signal, modifiers) = parse_attr_name_and_modifiers(bind_part);
        datastar.push(DatastarAttr {
            name: "bind".to_string(),
            value: Some(signal),
            modifiers,
        });
        
        (tag_part, String::new())
    } else {
        (input, String::new())
    };

    // Heuristic parsing
    let mut current_token = String::new();
    let mut mode = 't'; // t=tag, i=id, c=class
    
    // Check if it starts with shorthand
    let start_idx = if input_cleaned.starts_with('#') {
        mode = 'i';
        1
    } else if input_cleaned.starts_with('.') {
        mode = 'c';
        1
    } else {
        0
    };

    for c in input_cleaned[start_idx..].chars() {
        if c == '#' {
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

    (tag, id, classes, datastar)
}

fn transform_block(nodes: &[KdlNode]) -> Result<Vec<Node>, String> {
    let mut result = Vec::new();
    let mut iter = nodes.iter().peekable();

    while let Some(node) = iter.next() {
        let name = node.name().value();
        match name {
            "__hudl_if" => {
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
                    if next_node.name().value() == "__hudl_else" {
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
            "__hudl_each" => {
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
            "__hudl_switch" => {
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
                            "__hudl_case" => {
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
                            "__hudl_default" => {
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
            "__hudl_else" => {
                return Err("Unexpected 'else' without matching 'if'".to_string());
            }
            "__hudl_content" => {
                result.push(Node::ContentSlot);
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

    let (tag, mut id, mut classes, mut datastar) = parse_selector(name);

    let mut attributes = HashMap::new();
    let mut children = Vec::new();
    let mut styles = Vec::new();

    // 1. Process entries (Properties and Arguments)
    for entry in node.entries() {
        if let Some(prop_name) = entry.name() {
            let key = prop_name.value();
            let val = entry.value().as_string().unwrap_or_default().to_string();

            // Check for inline tilde attributes: ~on:click="expr"
            if key.starts_with('~') {
                let attr = parse_inline_tilde_attr(&key[1..], &val);
                datastar.push(attr);
            } else {
                match key {
                    "id" => id = Some(val),
                    "class" => classes.extend(val.split_whitespace().map(|s| s.to_string())),
                    _ => { attributes.insert(key.to_string(), val); }
                }
            }
        } else {
            // Positional argument -> Text content
            if let Some(v) = entry.value().as_string() {
                children.push(Node::Text(Text { content: v.to_string() }));
            }
        }
    }

    // 2. Process children, extracting style blocks and tilde blocks
    if let Some(child_block) = node.children() {
        let mut non_special_nodes = Vec::new();
        for child in child_block.nodes() {
            match child.name().value() {
                "style" => {
                    // Extract styles from this block
                    styles.append(&mut process_element_style(child)?);
                }
                "~" => {
                    // Tilde block - extract datastar attributes
                    datastar.append(&mut process_tilde_block(child)?);
                }
                _ => {
                    non_special_nodes.push(child.clone());
                }
            }
        }
        children.append(&mut transform_block(&non_special_nodes)?);
    }

    Ok(Node::Element(Element {
        tag,
        id,
        classes,
        attributes,
        children,
        styles,
        datastar,
    }))
}

/// Parse an inline tilde attribute like "on:click~once~prevent" with value "expr"
fn parse_inline_tilde_attr(name_with_mods: &str, value: &str) -> DatastarAttr {
    let (name, modifiers) = parse_attr_name_and_modifiers(name_with_mods);
    DatastarAttr {
        name,
        value: if value.is_empty() { None } else { Some(value.to_string()) },
        modifiers,
    }
}

/// Parse attribute name and modifiers from "on:click~once~prevent"
/// Returns (name, vec!["once", "prevent"])
fn parse_attr_name_and_modifiers(input: &str) -> (String, Vec<String>) {
    let parts: Vec<&str> = input.split('~').collect();
    let name = parts[0].to_string();
    let modifiers = parts[1..].iter().map(|s| s.to_string()).collect();
    (name, modifiers)
}

/// Process a tilde block: ~ { on:click "expr"; show $visible; .active $cond }
fn process_tilde_block(node: &KdlNode) -> Result<Vec<DatastarAttr>, String> {
    let mut attrs = Vec::new();

    if let Some(children) = node.children() {
        for child in children.nodes() {
            let name_raw = child.name().value();
            let (name, modifiers) = parse_attr_name_and_modifiers(name_raw);

            // Get the value (first positional argument) - handle all types
            let value = child.entries().iter()
                .filter(|e| e.name().is_none())
                .next()
                .map(|e| {
                    let v = e.value();
                    if let Some(s) = v.as_string() {
                        s.to_string()
                    } else if let Some(i) = v.as_integer() {
                        i.to_string()
                    } else if let Some(f) = v.as_float() {
                        f.to_string()
                    } else if let Some(b) = v.as_bool() {
                        b.to_string()
                    } else {
                        // Null or unknown
                        "null".to_string()
                    }
                });

            attrs.push(DatastarAttr {
                name,
                value,
                modifiers,
            });
        }
    }

    Ok(attrs)
}
