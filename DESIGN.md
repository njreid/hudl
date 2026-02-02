# Hudl Architecture & Design

## System Overview

The `hudl` system consists of two primary components:
1.  **Compiler (`hudlc`)**: A Rust CLI that compiles KDL documents into a WASM binary.
2.  **Runtime (Go)**: A Go library that wraps `wazero` to load the WASM and execute view functions.

## Technology Stack

*   **Compiler Language**: Rust
*   **Target**: WebAssembly (WASM)
*   **KDL Parser**: `kdl-rs` (Official, fully compliant KDL v2)
*   **Runtime**: Go 1.22+ with `github.com/tetratelabs/wazero`
*   **LSP Framework**: `tower-lsp` (Rust)
*   **LSP Types**: `lsp-types`

---

## Compiler Design (Rust)

### 1. The "View" Model
Each `.hu.kdl` file corresponds to a public exported function in the WASM module.
*   **Input**: Serialized parameters (JSON/CBOR/Memory pointer).
*   **Output**: HTML string written to shared memory buffer.

### 2. Compilation Pipeline
1.  **Parse**: Use `kdl-rs` to parse `.hu.kdl` files into a Rust AST.
2.  **Transform**: Apply Hudl-specific logic (shorthands `&id`, `.class`, scoped CSS).
3.  **Codegen**: Generate Rust code representing the view logic (concatenating strings, loops).
4.  **WASM Build**: Compile the generated Rust code to `wasm32-unknown-unknown`.

### 3. Control Flow & Data
*   **Iterators/Logic**: Data is passed from Host (Go) to Guest (WASM).
*   **Expressions**: Expressions inside backticks must be evaluatable in the WASM context (or treated as string interpolations). Complex host types might need serialization.

---

## LSP Design (Rust)

The LSP server (`hudl-lsp`) is written in Rust using `tower-lsp`.

### 1. Capabilities
*   **Formatting**: Standard KDL formatting via `kdl-rs` (or custom formatter to handle Hudl specifics like `} else {` newlines).
*   **Diagnostics**: 
    *   Syntax errors from `kdl-rs`.
    *   Hudl-specific checks (e.g., valid control flow usage, unknown parameters).
*   **Semantic Highlighting**:
    *   Keywords: `switch`, `case`, `default`, `each`, `if`, `else`.
    *   Special Nodes: `&id`, `.class`.
    *   Properties: `_numeric`.

### 2. Integration
The LSP runs as a standalone binary. Editors (VS Code, Neovim) connect via stdio.

---

## Runtime Design (Go + Wazero)

1.  **Initialization**: Load `views.wasm` into `wazero` runtime.
2.  **Invocation**: calling `views.Render("Layout", params)` calls the WASM function.
3.  **Memory**: Use a shared linear memory buffer to pass large strings (HTML output) back to Go.


## LSP Design (hudl-lsp)

The LSP uses `tliron/glsp` to handle the protocol.

### 1. Initialization

* **Capabilities**: `textDocumentSync: Full`, `formatting`, `diagnostics`.
* **Workspace**: Loads the Go module context (`go.mod`) to resolve imports.

### 2. Formatting & Refactoring (`textDocument/formatting`)

The formatter is opinionated and aggressive.

* **Selector Collapse**: `tag class="foo"` → `tag.foo`.
* **Div Inference**: `div.foo` → `.foo`.
* **Link Rewriting**: `<link rel="stylesheet">` → `_stylesheet`.
* **Indentation**: Standard 4-space (or configured) KDL indentation.

### 3. Type Analysis & Diagnostics

The LSP maintains a "Virtual Go File" representation of the KDL logic.

1. **Extract Types**: Parse `// param: name Type` comments.
2. **Resolve Symbols**: Use `go/packages` to find `Type` in the user's project.
3. **Validate Expressions**:
   * Extract content within backticks `` `user.Name` ``.
   * Check if `Name` exists on `Type` using `go/types`.
4. **Exhaustiveness Check**:
   * If `switch` expression is an Interface.
   * Find all implementations of that interface in the workspace.
   * Ensure all implementations are covered by `case` or a `default` exists.

### 4. Code Generation (`textDocument/didSave`)

Upon saving, the LSP triggers the compiler to write the `_gen.go` file to disk. This ensures the Go compiler (and `gopls`) always has up-to-date function definitions for the views.

---

## Directory Structure

```text
/
├── cmd/
│   └── hudl-lsp/       # Main entry point
├── internal/
│   ├── ast/            # Internal HUDL object model
│   ├── parser/         # KDL -> AST
│   ├── generator/      # AST -> Go Code
│   ├── analysis/       # go/types integration
│   └── server/         # glsp handlers
└── pkg/
    └── hudl/           # Runtime helpers (Iterator interfaces etc)
```
