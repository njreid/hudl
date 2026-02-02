# Project Plan: hudl & hudl-lsp

## Phase 1: Core Compiler (CLI)
**Goal:** Convert a basic `.hu.kdl` file into a `.go` file via CLI.

- [ ] **Task 1.1**: Set up project structure and `go.mod`.
- [ ] **Task 1.2**: Implement KDL v2 parsing using `calico32/kdl-go`.
- [ ] **Task 1.3**: Implement the "HKDL AST" transformation (handling `&id`, `.class` logic).
- [ ] **Task 1.4**: Implement the Go Code Generator (basic elements & attributes).
- [ ] **Task 1.5**: Support `import` and `// param` parsing.
- [ ] **Task 1.6**: Create `hudlc` CLI tool (`hudlc generate ./views`).

## Phase 2: Basic LSP (Architecture & Formatting)
**Goal:** Editor integration that formats code and runs the compiler on save.

- [ ] **Task 2.1**: Initialize `tliron/glsp` server skeleton.
- [ ] **Task 2.2**: Implement `textDocument/didOpen` and `didSave`.
- [ ] **Task 2.3**: Hook up the Phase 1 compiler to `didSave` (generate `.go` file).
- [ ] **Task 2.4**: Implement `textDocument/formatting`.
    - [ ] Selector collapsing logic.
    - [ ] `_` link prefix expansion/contraction.
    - [ ] Indentation normalization.

## Phase 3: Control Flow & Expressions
**Goal:** Support complex logic (`if`, `each`, `switch`) in the compiler.

- [ ] **Task 3.1**: Implement `if / else` compilation.
- [ ] **Task 3.2**: Implement `each` compilation (Iterator interface support).
- [ ] **Task 3.3**: Implement `switch` compilation (Type switches).
- [ ] **Task 3.4**: Update LSP formatter to handle control flow indentation.

## Phase 4: Type Safety & Diagnostics
**Goal:** The LSP understands the Go code within the KDL.

- [ ] **Task 4.1**: Implement `analysis` package to load user's Go workspace (`go/packages`).
- [ ] **Task 4.2**: Map KDL `param` definitions to Go types.
- [ ] **Task 4.3**: Implement expression validator.
    - [ ] Parse backticked strings.
    - [ ] Verify fields/methods against Go types.
    - [ ] Publish diagnostics for invalid fields.
- [ ] **Task 4.4**: Implement Switch Exhaustiveness Checker.
    - [ ] Identify interface implementations.
    - [ ] Compare against `case` clauses.

## Phase 5: Polish & Developer Experience
**Goal:** Syntax highlighting support and VS Code extension.

- [ ] **Task 5.1**: Create Tree-sitter grammar for HKDL (handling the specific node types).
- [ ] **Task 5.2**: Create a basic VS Code extension that launches the binary.
- [ ] **Task 5.3**: Add standard library/runtime helpers (e.g., standard Iterator implementations).
