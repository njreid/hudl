package main

import (
	"os"
)

func main() {
	content := `use crate::ast::{Root, Node};

pub fn generate_wasm_lib(views: Vec<(String, Root)>) -> Result<String, String> {
    let mut code = String::new();
    code.push_str("use std::mem;\n\n");
    code.push_str("#[no_mangle]\npub extern \"C\" fn hudl_malloc(size: usize) -> *mut u8 {\n");
    code.push_str("    let mut vec = Vec::with_capacity(size);\n");
    code.push_str("    let ptr = vec.as_mut_ptr();\n");
    code.push_str("    mem::forget(vec);\n");
    code.push_str("    ptr\n}\n\n");
    code.push_str("#[no_mangle]\npub extern \"C\" fn hudl_free(ptr: *mut u8, size: usize) {\n");
    code.push_str("    unsafe { let _ = Vec::from_raw_parts(ptr, size, size); }\n}
");
    code.push_str("fn pack_ptr_len(ptr: *const u8, len: usize) -> u64 {\n");
    code.push_str("    ((ptr as u64) << 32) | (len as u64)\n}\n");

    for (name, root) in views {
        code.push_str(&format!("\nfn render_{}(r: &mut String) {{ \n", name.to_lowercase()));
        for node in &root.nodes {
            let _ = generate_node(&mut code, node, 1);
        }
        code.push_str("}\n");

        code.push_str(&format!("\n#[no_mangle]\npub extern \"C\" fn {}(_ptr: *const u8, _len: usize) -> u64 {{ \n", name));
        code.push_str("    let mut out = String::new();\n");
        code.push_str(&format!("    render_{}(&mut out);\n", name.to_lowercase()));
        code.push_str("    let result_ptr = out.as_ptr();\n");
        code.push_str("    let result_len = out.len();\n");
        code.push_str("    mem::forget(out);\n");
        code.push_str("    pack_ptr_len(result_ptr, result_len)\n}\n");
    }
    Ok(code)
}

fn generate_node(code: &mut String, node: &Node, indent: usize) -> Result<(), String> {
    let pad = "    ".repeat(indent);
    match node {
        Node::Element(el) => {
            code.push_str(&format!("{}\"r.push_str(\
```