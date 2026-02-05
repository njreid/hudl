# Project Plan: hudl & hudl-lsp

## Architecture Overview

Hudl uses **Protocol Buffers** for type-safe data contracts and **CEL (Common Expression Language)** for template expressions. Templates compile to WASM modules that include the `cel-rust` interpreter.

---

## Phase 1: Rust Compiler & WASM Core (Completed)

**Goal:** Convert `.hudl` templates into a `views.wasm` module using Rust.

- [x] **Task 1.1**: Set up Rust workspace and `hudlc` crate.
- [x] **Task 1.2**: Implement KDL v2 parsing using `kdl-rs`.
- [x] **Task 1.3**: Implement AST transformation (handling `&id`, `.class` logic) in Rust.
- [x] **Task 1.4**: Implement Rust Code Generator (converting AST to Rust functions).
- [x] **Task 1.5**: Implement WASM compilation pipeline (building the `.wasm`).
- [x] **Task 1.6**: Create Go runtime wrapper using `wazero` to load and execute the WASM.

---

## Phase 2: Language Server (Completed)

**Goal:** Provide a rich editing experience with `hudl-lsp`.

- [x] **Task 2.1**: Set up `hudl-lsp` crate using `tower-lsp`.
- [x] **Task 2.2**: Implement `textDocument/formatting`.
- [x] **Task 2.3**: Implement `textDocument/publishDiagnostics`.
- [x] **Task 2.4**: Implement `textDocument/semanticTokens`.
- [x] **Task 2.5**: Implement basic VS Code extension to launch `hudl-lsp`.

---

## Phase 3: Control Flow (Completed)

**Goal:** Implement control flow constructs.

- [x] **Task 3.1**: Implement `if / else` logic in Rust codegen.
- [x] **Task 3.2**: Implement `each` iteration.
- [x] **Task 3.3**: Implement `switch` logic.

---

## Phase 4: CEL Integration (Completed)

**Goal:** Replace custom expression handling with CEL (Common Expression Language).

### 4.1 CEL Parser Integration

- [x] Add `cel-rust` crate dependency
- [x] Implement CEL expression extraction from backticks (pre-parser in `parser.rs`)
- [x] Parse and validate CEL syntax at compile time
- [x] Handle CEL interpolation within strings (`"Hello`name`"`)

### 4.2 CEL Runtime in WASM

- [x] Configure `cel-rust` for `wasm32-unknown-unknown` target
- [x] Implement CEL evaluation context creation from input data
- [x] Handle scoped variables in `each` loops (item binding, `_index`)
- [x] Implement `raw()` custom function for unescaped HTML

### 4.3 CEL Code Generation

- [x] Generate Rust code that evaluates CEL expressions at runtime (`codegen_cel.rs`)
- [x] Implement boolean attribute handling (present/absent based on CEL result)
- [x] Handle switch/case with CEL expressions and implicit receiver
- [x] Implement proper HTML escaping for CEL output

### 4.4 CEL Error Handling

- [x] Implement runtime error capture (returns Null on error)
- [x] Generate `ERROR` placeholder with tooltip on evaluation failure
- [x] Ensure fail-soft behavior (continue rendering on error)

---

## Phase 5: Protocol Buffers Integration (In Progress)

**Goal:** Use Protocol Buffers for type-safe data contracts.

### 5.1 Proto Definition Parsing

- [x] Implement `/**` comment extraction for proto blocks
- [x] Add proto3 parser (`protobuf-parse` crate added, custom parser in `src/proto.rs`)
- [x] Support `import` statements with relative path resolution
- [x] Support inline message and enum definitions
- [x] Support map fields (`map<K, V>`)

### 5.2 Proto Schema Features

- [x] Parse messages with scalar types (string, int32, bool, etc.)
- [x] Parse repeated fields
- [x] Parse enum definitions with values
- [x] Resolve field paths on message types
- [x] Get enum values for exhaustiveness checking
- [ ] Embed descriptors in WASM module for CEL type awareness
- [ ] Resolve cross-file proto references

### 5.3 Component Metadata

- [x] Parse `// name:` and `// data:` comments
- [x] Map `// data:` type to proto message definition
- [ ] Validate component data types exist in proto definitions

### 5.4 Go Runtime Updates

- [x] Update runtime to accept proto messages (wire format)
- [x] Remove CBOR serialization, use proto serialization
- [x] Generate Go proto bindings for view data types (`proto/views.proto` → `pkg/hudl/pb/`)

### 5.5 LSP Proto Integration

- [x] Use proto schema for enum exhaustiveness checking
- [x] Fall back to Go analyzer when proto info not available

---

## Phase 6: Type Safety & Diagnostics (In Progress)

**Goal:** Compile-time validation of CEL expressions against proto schemas.

### 6.1 Go Type Analyzer

- [x] Create `hudl-analyzer` Go process (`cmd/hudl-analyzer/main.go`)
- [x] Implement JSON-RPC communication with LSP
- [x] Implement `validateExpression` for field path validation
- [x] Implement `findImplementations` for interface types
- [x] Cache loaded packages for performance

### 6.2 LSP Analyzer Client

- [x] Implement `AnalyzerClient` in Rust (`lsp/src/analyzer_client.rs`)
- [x] Spawn hudl-analyzer on LSP initialization
- [x] Send validation requests for expressions in backticks
- [x] Graceful degradation when analyzer unavailable

### 6.3 LSP Diagnostics Enhancement

- [x] Report KDL syntax errors with locations
- [x] Report unknown variable errors
- [x] Report invalid field access errors (via Go analyzer)
- [ ] Report proto syntax errors in `/**` blocks
- [ ] Report type mismatches in component invocations

### 6.4 Switch Exhaustiveness

- [x] Extract switch statements from templates (`lsp/src/exhaustiveness.rs`)
- [x] Check for missing cases against enum values
- [x] Warn when switch lacks default and is non-exhaustive

### 6.5 LSP Hover & Completion (TODO)

- [ ] Show proto field types on hover
- [ ] Show CEL expression result types on hover
- [ ] Autocomplete proto field names in CEL expressions
- [ ] Autocomplete component names

---

## Phase 7: Example Application (New)

**Goal:** Build a complete example demonstrating the CEL/Proto architecture.

### 7.1 Proto Definitions

- [ ] Create `proto/models.proto` with example messages
- [ ] Create `proto/views.proto` with view-specific messages
- [ ] Define enums for status values, roles, etc.

### 7.2 Template Updates

- [ ] Update `layout.hudl` with proto types and CEL
- [ ] Update `dashboard.hudl` with proto types and CEL
- [ ] Update `form.hudl` with proto types and CEL
- [ ] Update `marketing.hudl` with proto types and CEL
- [ ] Create component composition example

### 7.3 Go Application

- [ ] Update `examples/go-app` to use proto messages
- [ ] Generate Go proto bindings
- [ ] Demonstrate data transformation (DB → Proto → Template)

---

## Phase 8: Dev Mode Implementation (New)

**Goal:** Enable hot-reload development workflow where the LSP acts as a rendering sidecar.

### 8.1 LSP Dev Server

- [ ] Add HTTP server to LSP (configurable port, default 9999)
- [ ] Implement `/render` endpoint accepting JSON:

  ```json
  { "view": "Dashboard", "data": { ... } }
  ```

- [ ] Return rendered HTML or JSON error response
- [ ] Implement file watching for `.hudl` files (re-parse on change)
- [ ] Keep parsed templates in memory for instant rendering

### 8.2 Go Runtime Dev/Prod Switching

- [ ] Add `Options` struct with `DevMode` and `LspAddr` fields
- [ ] Implement `HUDL_DEV` and `HUDL_LSP_ADDR` environment variable support
- [ ] In dev mode: HTTP POST to LSP for rendering
- [ ] In prod mode: Use embedded WASM (existing behavior)
- [ ] Ensure identical API surface in both modes

### 8.3 Dev Mode Communication

- [ ] Send proto wire-format bytes directly (no re-serialization)
- [ ] HTTP client with connection pooling in Go runtime
- [ ] Timeout handling (5s default) with fallback behavior
- [ ] Error propagation from LSP to Go caller

### 8.4 Developer Experience

- [ ] LSP logs render requests in verbose mode
- [ ] Show template compilation errors immediately on save
- [ ] Support partial updates (single component re-render)
- [ ] Document dev mode setup in README

### 8.5 Testing Dev Mode

- [ ] Integration test: LSP dev server + Go client
- [ ] Test hot reload: modify template, verify new output
- [ ] Test error handling: invalid template, LSP unavailable
- [ ] Benchmark: dev mode latency vs prod mode

---

## Implementation Notes

### CEL in WASM

- `cel-rust` compiles to WASM but may need feature flags adjusted
- Proto descriptors must be embedded for CEL's type-aware evaluation
- Consider lazy compilation of CEL programs for performance

### Proto Parsing Options

1. **`protobuf-parse`**: Pure Rust proto parser
2. **`prost-build`**: Requires `protoc` binary
3. **Custom parser**: Simpler but more work

Recommend `protobuf-parse` for self-contained toolchain.

### Switch/Case Semantics

Cases with backticks use implicit receiver:

```kdl
switch `status` {
    case `matches('^ACTIVE')` { }  // Becomes: status.matches('^ACTIVE')
}
```

The switch value is bound as `_switch_value` in the CEL context for case evaluation.

### Each Loop Scoping

```kdl
each item `items` {
    // CEL context gains:
    // - item: current element
    // - _index: zero-based index
}
```

For maps, `item` is a map entry with `key` and `value` fields.

---

## Dependencies

### Rust (Cargo.toml)

```toml
cel-rust = "0.8"          # CEL interpreter
protobuf-parse = "3.0"    # Proto parser
prost = "0.12"            # Proto runtime
prost-types = "0.12"      # Well-known types
```

### Go (go.mod)

```text
google.golang.org/protobuf v1.32.0
github.com/tetratelabs/wazero v1.6.0
```

---

## Testing Strategy

### Unit Tests

- CEL expression parsing and validation
- Proto definition parsing
- Component type checking
- HTML generation with CEL interpolation

### Integration Tests

- Compile template with proto types
- Render with proto message input
- Error handling (invalid expressions, missing fields)

### End-to-End Tests

- Full Go application with proto messages
- Multiple components with composition
- All control flow constructs
