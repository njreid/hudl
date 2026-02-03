use kdl::{KdlDocument, KdlNode};
use crate::ast::{Root, Node, Element, Text};
use std::collections::HashMap;

pub fn transform(doc: &KdlDocument) -> Result<Root, String> {
    let mut nodes = Vec::new();
    let css = None;

    for node in doc.nodes() {
        match node.name().value() {
            "el" => {
                if let Some(children) = node.children() {
                    nodes.append(&mut transform_block(children.nodes())?);
                }
            }
            _ => {}
        }
    }
    Ok(Root { nodes, css })
}

fn transform_block(nodes: &[KdlNode]) -> Result<Vec<Node>, String> {
    let mut result = Vec::new();
    for node in nodes {
        result.push(transform_node(node)?);
    }
    Ok(result)
}

fn transform_node(node: &KdlNode) -> Result<Node, String> {
    let name = node.name().value();
    
    let mut tag = name.to_string();
    let mut id = None;
    let mut classes = Vec::new();
    let mut attributes = HashMap::new();
    let mut children = Vec::new();

    // 1. Process entries (Properties and Arguments)
    for entry in node.entries() {
        if let Some(prop_name) = entry.name() {
            let key = prop_name.value();
            let val = entry.value().as_string().unwrap_or_default().to_string();
            
            match key {
                "id" => id = Some(val),
                "class" => classes = val.split_whitespace().map(|s| s.to_string()).collect(),
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
