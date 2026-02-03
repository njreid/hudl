use std::env;
use std::fs;
use std::path::{Path, PathBuf};
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
                
                let func_name = view_name[..1].to_uppercase() + &view_name[1..];
                views.push((func_name, root));
            }
        }
    }

    if views.is_empty() {
        return Err("No .hu.kdl files found".into());
    }

    // 2. Setup temporary Cargo project
    let build_dir = Path::new("hudl_build");
    if build_dir.exists() {
        fs::remove_dir_all(build_dir)?;
    }
    fs::create_dir_all(build_dir.join("src"))?;

    let cargo_toml = r#"[package]
name = "hudl_views"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_cbor = "0.11"

[lib]
crate-type = ["cdylib"]
path = "src/lib.rs"
"#;
    fs::write(build_dir.join("Cargo.toml"), cargo_toml)?;

    // 3. Generate the Rust library source
    let lib_source = codegen::generate_wasm_lib(views)?;
    fs::write(build_dir.join("src/lib.rs"), lib_source)?;

    // 4. Build WASM using cargo
    println!("Building WASM...");
    let status = Command::new("cargo")
        .args([
            "build",
            "--target", "wasm32-unknown-unknown",
            "--release",
            "--manifest-path", "hudl_build/Cargo.toml",
        ])
        .status()?;

    if !status.success() {
        return Err("cargo build failed".into());
    }

    // 5. Copy output
    let wasm_file = "hudl_build/target/wasm32-unknown-unknown/release/hudl_views.wasm";
    fs::copy(wasm_file, output)?;

    // Cleanup
    let _ = fs::remove_dir_all(build_dir);

    println!("Success! Created {}", output);
    Ok(())
}
