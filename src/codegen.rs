use crate::ast::{Root, Node};

pub fn generate_wasm_lib(views: Vec<(String, Root)>) -> Result<String, String> {
    let mut code = String::new();
    code.push_str("use std::mem;\n");
    code.push_str("#[no_mangle]\npub extern \"C\" fn hudl_malloc(s: usize) -> *mut u8 { let mut v = Vec::with_capacity(s); let p = v.as_mut_ptr(); mem::forget(v); p }\n");
    code.push_str("#[no_mangle]\npub extern \"C\" fn hudl_free(p: *mut u8, s: usize) { unsafe { let _ = Vec::from_raw_parts(p, s, s); } }\n");
    code.push_str("fn pack(p: *const u8, l: usize) -> u64 { ((p as u64) << 32) | (l as u64) }\n");

    for (name, root) in views {
        code.push_str(&format!("\nfn render_{}(r: &mut String) {{ \n", name.to_lowercase()));
        for node in &root.nodes {
            let _ = generate_node(&mut code, node);
        }
        code.push_str("}\n");

        code.push_str(&format!("\n#[no_mangle]\npub extern \"C\" fn {}(_p: *const u8, _l: usize) -> u64 {{ \n", name));
        code.push_str("  let mut o = String::new();\n");
        code.push_str(&format!("  render_{}(&mut o);\n", name.to_lowercase()));
        code.push_str("  let p = o.as_ptr(); let l = o.len(); mem::forget(o); pack(p, l)\n");
        code.push_str("}\n");
    }
    Ok(code)
}

fn generate_node(code: &mut String, node: &Node) -> Result<(), String> {
    match node {
        Node::Element(el) => {
            code.push_str(&format!("  r.push_str(\"<{}\");\n", el.tag));
            code.push_str("  r.push_str(\">\");\n");
            for child in &el.children {
                let _ = generate_node(code, child);
            }
            code.push_str(&format!("  r.push_str(\"</{}>\");\n", el.tag));
        }
        Node::Text(t) => {
            code.push_str(&format!("  r.push_str(\"{}\");\n", t.content));
        }
        _ => {}
    }
    Ok(())
}