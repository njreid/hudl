# hudl Architecture & Design

## System Overview

The `hudl` system consists of two primary components:

1. **Compiler (`hudlc`)**: Parses KDL documents and generates Go source code (`io.Writer` streaming).
2. **Language Server (`hudl-lsp`)**: Provides editor integration, diagnostics, and orchestrates the compiler.

## Technology Stack



*   **Language**: Go (Golang) 1.22+

*   **LSP SDK**: `github.com/tliron/glsp`

*   **KDL Parser**: `github.com/calico32/kdl-go` (Supports KDL v2)

*   **Type Analysis**: `golang.org/x/tools/go/packages` & `go/types`

---

## Compiler Design

### 1. The "View" Model

Each top-level 'el' node within a `.hu.kdl` file corresponds to a Go package-level struct method.

* **Input**: `ctx context.Context`, `w io.Writer`, plus defined parameters.
* **Output**: Streamed bytes to `w`.

### 2. AST Transformation

The parsing pipeline:

1. **Raw KDL**: Parse bytes into generic KDL nodes.
2. **HKDL AST**: Convert generic nodes to Semantic Nodes (`Element`, `ControlFlow`, `Text`, `Import`).
   * *Normalization*: Convert `div id=x` to `Element{Tag: "div", ID: "x"}`.
   * *Resolution*: Map `_style` to `<link>`.
   * *CSS Scoping*: 
     * Inside `css` blocks, `&alpha` nodes are mapped to `#alpha` in final CSS output.
     * `&:pseudo` nodes are preserved as-is.
3. **Go Generation**: Walk the HKDL AST and emit Go code.

### 3. Control Flow Contracts

* **Iterators**: The `each` keyword expects the expression to satisfy a standard iteration pattern.
  * `each item of=expr` -> `for _, item := range expr`
  * `each i item of=expr` -> `for i, item := range expr`
  * For custom types, the compiler will generate calls to a standard `Iterator` interface:
    ```go
    type Iterator[T any] interface {
        Next() (T, bool)
    }
    ```

* **Switch**:
  * Generates a standard Go `switch v := expr.(type)`.
  * Variable `v` is injected into the scope of `case` blocks.

---

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
│   ├── ast/            # Internal HKDL object model
│   ├── parser/         # KDL -> AST
│   ├── generator/      # AST -> Go Code
│   ├── analysis/       # go/types integration
│   └── server/         # glsp handlers
└── pkg/
    └── hudl/           # Runtime helpers (Iterator interfaces etc)
```
