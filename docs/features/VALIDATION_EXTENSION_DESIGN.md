# Validation & Completion Extension Plan

This document outlines the strategy for adding intelligent HTML completion and validation to the Hudl LSP, leveraging industry-standard data sources.

## Data Sources

Instead of scraping HTML specifications, we will use the machine-readable data maintained by the VS Code team and the Unified ecosystem.

### 1. Elements & Attributes
**Source:** [`vscode-custom-data`](https://github.com/microsoft/vscode-custom-data)
**File:** `web-data/data/browsers.html-data.json`
**Format:** JSON

**Contains:**
*   List of all valid HTML tags.
*   Markdown descriptions for hover documentation.
*   List of valid attributes for each tag.
*   Global attributes valid on all tags.
*   Attribute value sets (e.g., valid `type` values for `<input>`).

### 2. Containment Relationships (Content Models)
**Source:** [`wooorm/html-element-content-categories`](https://github.com/wooorm/html-element-content-categories) & [`wooorm/html-void-elements`](https://github.com/wooorm/html-void-elements)

**Logic:**
*   Map elements to categories (Flow, Phrasing, Heading, etc.).
*   Define rules based on parent expectations (e.g., "Parent requires Flow content").
*   **Void Elements:** Identify tags that strictly cannot have children (e.g., `<img>`, `<input>`).

## Implementation Roadmap

### Phase 1: Data Ingestion (Compile Time)
Do not parse JSON at runtime. Use a `build.rs` script to generate static Rust structures.

1.  **Fetch Data:** Script to download `browsers.html-data.json`.
2.  **Generate Code:**
    *   Use `serde` to parse JSON.
    *   Generate `phf` (Perfect Hash Function) maps for O(1) lookups.
    *   Target Structure:
        ```rust
        pub struct TagData {
            pub description: &'static str,
            pub attributes: &'static [&'static str],
            pub void: bool,
        }
        pub static HTML_TAGS: phf::Map<&'static str, TagData> = ...;
        ```

### Phase 2: Context Awareness
Integrate with the existing LSP parser (Tree-sitter/KDL).

1.  **Cursor Mapping:** Map `textDocument/completion` position to the specific node in the AST.
2.  **Context Detection:**
    *   **Node Name:** User typing `div` → Suggest tags.
    *   **Attribute Key:** User typing `div cla` → Suggest attributes (using `HTML_TAGS` lookup).
    *   **Attribute Value:** User typing `input type="` → Suggest values from the data set.

### Phase 3: Validation Logic
Trigger on `textDocument/didSave` or `textDocument/didChange`.

1.  **Unknown Tag:** Warning if `!HTML_TAGS.contains_key(tag)`.
2.  **Unknown Attribute:** Warning if attribute not in tag's allowed list (allow-list `data-*`, `aria-*`, and Hudl `~` attributes).
3.  **Void Content:** Error if a Void element has children.
4.  **Strict Containment (Future):** Implement full category-based content model checks.

## Dependencies & Effort
*   **Dependencies:** `phf`, `phf_codegen`, `serde_json`.
*   **Effort Estimate:** ~3-5 days for full implementation including basic completion and void element validation.
