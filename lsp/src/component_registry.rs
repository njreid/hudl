//! Global component registry for the Hudl LSP.
//!
//! Scans the workspace for `.hudl` files and maintains a map of
//! component names to their expected data types and source files.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tower_lsp::lsp_types::Url;
use crate::param;

/// Information about a Hudl component
#[derive(Debug, Clone)]
pub struct ComponentInfo {
    pub name: String,
    pub data_type: Option<String>,
    pub uri: Url,
}

/// Registry of all components in the workspace
pub struct ComponentRegistry {
    pub components: HashMap<String, ComponentInfo>,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self {
            components: HashMap::new(),
        }
    }

    /// Scan a directory recursively for .hudl files
    pub fn scan_workspace(&mut self, root_path: &str) {
        let root = Path::new(root_path);
        if !root.exists() {
            return;
        }

        self.scan_dir(root);
    }

    fn scan_dir(&mut self, dir: &Path) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if path.is_dir() {
                    self.scan_dir(&path);
                } else if path.extension().and_then(|s| s.to_str()) == Some("hudl") {
                    self.process_file(&path);
                }
            }
        }
    }

    pub fn process_file(&mut self, path: &Path) {
        if let Ok(content) = fs::read_to_string(path) {
            let metadata = param::extract_metadata(&content);
            if let Some(name) = metadata.name {
                let uri = Url::from_file_path(path).unwrap();
                self.components.insert(name.clone(), ComponentInfo {
                    name,
                    data_type: metadata.data_type,
                    uri,
                });
            }
        }
    }

    pub fn get(&self, name: &str) -> Option<&ComponentInfo> {
        self.components.get(name)
    }
}
