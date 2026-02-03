# Project Plan: hudl & hudl-lsp

## Phase 1: Rust Compiler & WASM Core
**Goal:** Convert `.hu.kdl` templates into a `views.wasm` module using Rust.

- [x] **Task 1.1**: Set up Rust workspace and `hudlc` crate.
- [x] **Task 1.2**: Implement KDL v2 parsing using `kdl-rs`.
- [x] **Task 1.3**: Implement AST transformation (handling `&id`, `.class` logic) in Rust.
- [x] **Task 1.4**: Implement Rust Code Generator (converting AST to Rust functions).
- [x] **Task 1.5**: Implement WASM compilation pipeline (building the `.wasm`).
- [x] **Task 1.6**: Create Go runtime wrapper using `wazero` to load and execute the WASM.

## Phase 2: Language Server (Rust)
**Goal:** Provide a rich editing experience with `hudl-lsp`.

- [x] **Task 2.1**: Set up `hudl-lsp` crate using `tower-lsp`.
- [x] **Task 2.2**: Implement `textDocument/formatting`.
- [x] **Task 2.3**: Implement `textDocument/publishDiagnostics`.
- [x] **Task 2.4**: Implement `textDocument/semanticTokens`.
- [x] **Task 2.5**: Implement basic VS Code extension to launch `hudl-lsp`.

## Phase 3: Control Flow & Data Binding
**Goal:** Pass data from Go to WASM and handle logic.

- [x] **Task 3.1**: Implement serialization protocol (Go struct -> CBOR -> WASM memory).
- [x] **Task 3.2**: Implement `if / else` logic in Rust codegen.
- [x] **Task 3.3**: Implement `each` iteration.
- [ ] **Task 3.4**: Implement `switch` logic.
- [ ] **Task 3.5**: Support complex expressions in backticks (mapping to CBOR data).

## Phase 4: Type Safety & Diagnostics
**Goal:** The LSP understands the Go code within the KDL.

- [ ] **Task 4.1**: Implement `analysis` package to load user's Go workspace (`go/packages`).
- [ ] **Task 4.2**: Map KDL `param` definitions to Go types.
- [ ] **Task 4.3**: Implement expression validator.
- [ ] **Task 4.4**: Implement Switch Exhaustiveness Checker.

## Phase 5: Polish & Developer Experience
- [ ] **Task 5.1**: Create Tree-sitter grammar for HUDL.
- [ ] **Task 5.2**: Create a basic VS Code extension that launches the binary.
- [ ] **Task 5.3**: Add standard library/runtime helpers.