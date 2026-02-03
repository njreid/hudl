use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use hudlc::{parser, transformer, codegen};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: hudlc <directory> [-o output.wasm]");
        return;
    }

    let dir_path = &args[1];
    let mut out_path = "views.wasm".to_string();

    if let Some(pos) = args.iter().position(|x| x == "-o") {
        if pos + 1 < args.len() {
            out_path = args[pos + 1].clone();
        }
    }

    if let Err(e) = run_build(dir_path, &out_path) {
        eprintln!("Build failed: {}", e);
        std::process::exit(1);
    }
}

fn run_build(dir: &str, output: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut views = Vec::new();

    // 1. Scan for .hu.kdl files
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("kdl") {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap();
            if stem.ends_with(".hu") {
                let view_name = stem.trim_end_matches(".hu");
                println!("Compiling {}...", path.display());
                
                let content = fs::read_to_string(&path)?;
                let doc = parser::parse(&content)?;
                let root = transformer::transform(&doc)?;
                
                // PascalCase the function name
                let func_name = view_name[..1].to_uppercase() + &view_name[1..];
                views.push((func_name, root));
            }
        }
    }

    if views.is_empty() {
        return Err("No .hu.kdl files found".into());
    }

    // 2. Generate the Rust library source
    let lib_source = codegen::generate_wasm_lib(views)?;
    let tmp_src = "hudl_generated_lib.rs";
    fs::write(tmp_src, lib_source)?;

    // 3. Compile to WASM using rustc
    println!("Building {}...", output);
    let status = Command::new("rustc")
        .args([
            "--target", "wasm32-unknown-unknown",
            "--crate-type", "cdylib",
            "-C", "opt-level=s",
            "-o", output,
            tmp_src,
        ])
        .status()?;

    if !status.success() {
        return Err("rustc failed".into());
    }

    // Cleanup
    let _ = fs::remove_file(tmp_src);

    println!("Success! Created {}", output);
    Ok(())
}