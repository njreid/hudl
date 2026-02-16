use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use hudlc::{parser, transformer, codegen_cel, codegen_go, proto::ProtoSchema};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        return;
    }

    match args[1].as_str() {
        "generate-go" => {
            if args.len() < 3 {
                println!("Usage: hudlc generate-go <directory> [--package <name>] [--pb-import <path>] [--pb-package <name>] [-o <output.go>]");
                std::process::exit(1);
            }
            let dir_path = &args[2];
            let mut out_path = "views.go".to_string();
            let mut package_name = "views".to_string();
            let mut pb_import = "".to_string();
            let mut pb_package = "pb".to_string();

            let mut i = 3;
            while i < args.len() {
                match args[i].as_str() {
                    "-o" => {
                        if i + 1 < args.len() { out_path = args[i+1].clone(); i += 1; }
                    }
                    "--package" => {
                        if i + 1 < args.len() { package_name = args[i+1].clone(); i += 1; }
                    }
                    "--pb-import" => {
                        if i + 1 < args.len() { pb_import = args[i+1].clone(); i += 1; }
                    }
                    "--pb-package" => {
                        if i + 1 < args.len() { pb_package = args[i+1].clone(); i += 1; }
                    }
                    _ => {}
                }
                i += 1;
            }

            if let Err(e) = run_generate_go(dir_path, &out_path, package_name, pb_import, pb_package) {
                eprintln!("Generate failed: {}", e);
                std::process::exit(1);
            }
        }
        _ => {
            // Default: build WASM
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
    }
}

fn print_usage() {
    println!("Usage:");
    println!("  hudlc <directory> [-o output.wasm]   Compile to WASM");
    println!("  hudlc generate-go <directory> ...    Generate Go wrapper");
}

fn run_generate_go(dir: &str, output: &str, pkg: String, pb_imp: String, pb_pkg: String) -> Result<(), Box<dyn std::error::Error>> {
    let mut views = Vec::new();
    let mut view_params = Vec::new();

    // Scan for .hudl files
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        
        let is_hudl = path.extension().and_then(|s| s.to_str()) == Some("hudl");
        if is_hudl {
            let content = fs::read_to_string(&path)?;
            let doc = parser::parse(&content).map_err(|e| format!("Parse error: {}", e))?;
            let root = transformer::transform_with_metadata(&doc, &content)?;
            
            let name = root.name.clone().unwrap_or_else(|| {
                path.file_stem().unwrap().to_string_lossy().to_string()
            });
            
            views.push(root);
            view_params.push((name, views.last().unwrap().params.clone()));
        }
    }

    let opts = codegen_go::GoOptions {
        package_name: pkg,
        pb_import_path: pb_imp,
        pb_package_name: pb_pkg,
    };

    let code = codegen_go::generate_go_wrapper(view_params, opts);
    fs::write(output, code)?;
    println!("Generated {}", output);
    Ok(())
}

fn run_build(dir: &str, output: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut views = Vec::new();
    let mut combined_schema = ProtoSchema::default();

    // 1. Scan for .hudl files
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        // Support both .hudl and legacy .hu.kdl extensions
        let is_hudl = path.extension().and_then(|s| s.to_str()) == Some("hudl");
        let is_legacy = path.to_string_lossy().ends_with(".hu.kdl");

        if is_hudl || is_legacy {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap();
            let view_name = if is_legacy {
                stem.trim_end_matches(".hu")
            } else {
                stem
            };

            println!("Compiling {}...", path.display());

            let content = fs::read_to_string(&path)?;

            // Parse proto schema from template (if present)
            if let Ok(schema) = ProtoSchema::from_template(&content, path.parent()) {
                // Merge into combined schema
                for (name, msg) in schema.messages {
                    combined_schema.messages.insert(name, msg);
                }
                for (name, e) in schema.enums {
                    combined_schema.enums.insert(name, e);
                }
                combined_schema.imports.extend(schema.imports);
            }

            let doc = parser::parse(&content).map_err(|e| format!("Parse error in {}: {}", path.display(), e))?;
            let root = transformer::transform_with_metadata(&doc, &content)?;

            // Use component name from metadata if available, otherwise derive from filename
            let func_name = root.name.clone().unwrap_or_else(|| {
                // Convert snake_case to PascalCase
                view_name
                    .split('_')
                    .map(|s| {
                        let mut c = s.chars();
                        match c.next() {
                            None => String::new(),
                            Some(f) => f.to_uppercase().chain(c).collect(),
                        }
                    })
                    .collect()
            });

            views.push((func_name, root));
        }
    }

    if views.is_empty() {
        return Err("No .hudl files found".into());
    }

    println!("Found {} view(s)", views.len());

    // 2. Setup temporary Cargo project
    let build_dir = Path::new("hudl_build");
    if build_dir.exists() {
        fs::remove_dir_all(build_dir)?;
    }
    fs::create_dir_all(build_dir.join("src"))?;

    // Cargo.toml with CEL dependency (no CBOR - uses proto wire format)
    let cargo_toml = r#"[package]
name = "hudl_views"
version = "0.1.0"
edition = "2021"

[dependencies]
cel-interpreter = { package = "cel", version = "0.12.0", default-features = false }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
lazy_static = "1.4"

[lib]
crate-type = ["cdylib"]
path = "src/lib.rs"

[profile.release]
opt-level = "s"
lto = true
"#;
    fs::write(build_dir.join("Cargo.toml"), cargo_toml)?;

    // 3. Generate the Rust library source using CEL codegen
    let lib_source = codegen_cel::generate_wasm_lib_cel(views, &combined_schema)?;
    fs::write(build_dir.join("src/lib.rs"), &lib_source)?;

    // 4. Build WASM using cargo
    println!("Building WASM...");
    let status = Command::new("cargo")
        .env("RUSTFLAGS", "-C link-args=-zstack-size=2097152")
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

    // Get file size
    let metadata = fs::metadata(output)?;
    let size_kb = metadata.len() as f64 / 1024.0;

    // Cleanup
    if std::env::var("HUDL_DEBUG").is_err() {
        let _ = fs::remove_dir_all(build_dir);
    }

    println!("Success! Created {} ({:.1} KB)", output, size_kb);
    Ok(())
}