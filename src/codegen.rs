use crate::ast::{Root, Node, ControlFlow};

pub fn generate(root: &Root) -> Result<String, String> {
    let mut code = String::new();
    
    // For WASM, we'll eventually wrap this in an exported function.
    // For now, let's generate a helper function.
    code.push_str("pub fn render(r: &mut String) {\n");
    
    for node in &root.nodes {
        generate_node(&mut code, node, 1)?;
    }
    
    code.push_str("}\n");
    Ok(code)
}

fn generate_node(code: &mut String, node: &Node, indent: usize) -> Result<(), String> {
    let pad = "    ".repeat(indent);
    
    match node {
        Node::Element(el) => {
            // Start tag
            code.push_str(&format!("{pad}r.push_str(\"<{}>\");\n", el.tag));
            
            if let Some(id) = &el.id {
                code.push_str(&format!("{pad}r.push_str(\" id=\\\"{}\\\"\");\n", id));
            }
            
            if !el.classes.is_empty() {
                code.push_str(&format!("{pad}r.push_str(\" class=\\\"{}\\\"\");\n", el.classes.join(" ")));
            }
            
            // Attributes
            for (k, v) in &el.attributes {
                code.push_str(&format!("{pad}r.push_str(\" {}=\\\"{}\\\"\");\n", k, v));
            }
            
            code.push_str(&format!("{pad}r.push_str(\">\");\n"));
            
            // Children
            for child in &el.children {
                generate_node(code, child, indent + 1)?;
            }
            
            // End tag
            code.push_str(&format!("{pad}r.push_str(\"</{}>\");\n", el.tag));
        }
        Node::Text(t) => {
            // Escape quotes
            let escaped = t.content.replace("\"", "\\\"");
            code.push_str(&format!("{pad}r.push_str(\"{}\");\n", escaped));
        }
        Node::ControlFlow(cf) => {
            match cf {
                ControlFlow::If { condition, then_block, else_block } => {
                    code.push_str(&format!("{pad}if {} {{\n", condition));
                    for n in then_block {
                        generate_node(code, n, indent + 1)?;
                    }
                    if let Some(eb) = else_block {
                        code.push_str(&format!("{pad}}} else {{\n"));
                        for n in eb {
                            generate_node(code, n, indent + 1)?;
                        }
                    }
                    code.push_str(&format!("{pad}}}
"));
                }
                _ => return Err("Control flow not implemented in codegen".to_string()),
            }
        }
    }
    Ok(())
}