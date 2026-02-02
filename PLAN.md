# Project Plan: hudl & hudl-lsp

## Phase 1: Rust Compiler & WASM Core

**Goal:** Convert `.hu.kdl` templates into a `views.wasm` module using Rust.

- [ ] **Task 1.1**: Set up Rust workspace and `hudlc` crate.
- [ ] **Task 1.2**: Implement KDL v2 parsing using `kdl-rs`.
- [ ] **Task 1.3**: Implement AST transformation (handling `&id`, `.class` logic) in Rust.
- [ ] **Task 1.4**: Implement Rust Code Generator (converting AST to Rust functions).
- [ ] **Task 1.5**: Implement WASM compilation pipeline (building the `.wasm`).
- [ ] **Task 1.6**: Create Go runtime wrapper using `wazero` to load and execute the WASM.

## Phase 2: Language Server (Rust)

**Goal:** Provide a rich editing experience with `hudl-lsp`.

- [ ] **Task 2.1**: Set up `hudl-lsp` crate using `tower-lsp`.
- [ ] **Task 2.2**: Implement `textDocument/formatting` (using `kdl` crate's formatter or custom logic).
- [ ] **Task 2.3**: Implement `textDocument/publishDiagnostics` (reporting parse errors).
- [ ] **Task 2.4**: Implement `textDocument/semanticTokens` (highlighting keywords like `switch`, `each`).
- [ ] **Task 2.5**: Implement basic VS Code extension to launch `hudl-lsp`.

## Phase 3: Control Flow & Data Binding

**Goal:** Pass data from Go to WASM and handle logic.

- [ ] **Task 3.1**: Implement serialization protocol (Go struct -> JSON/CBOR -> WASM memory).
- [ ] **Task 3.2**: Implement `if / else` logic in Rust codegen.
- [ ] **Task 3.3**: Implement `each` iteration (handling arrays/lists from serialized data).
- [ ] **Task 3.4**: Implement `switch` logic.

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

- [ ] **Task 5.1**: Create Tree-sitter grammar for HUDL (handling the specific node types).
- [ ] **Task 5.2**: Create a basic VS Code extension that launches the binary.
- [ ] **Task 5.3**: Add standard library/runtime helpers (e.g., standard Iterator implementations).
