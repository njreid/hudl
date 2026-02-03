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

## Phase 4: CEL Integration (New)

**Goal:** Replace custom expression handling with CEL (Common Expression Language).

### 4.1 CEL Parser Integration
- [ ] Add `cel-rust` crate dependency
- [ ] Implement CEL expression extraction from backticks
- [ ] Parse and validate CEL syntax at compile time
- [ ] Handle CEL interpolation within strings (`"Hello `name`"`)

### 4.2 CEL Runtime in WASM
- [ ] Configure `cel-rust` for `wasm32-unknown-unknown` target
- [ ] Implement CEL evaluation context creation from input data
- [ ] Handle scoped variables in `each` loops (item binding, `_index`)
- [ ] Implement `raw()` custom function for unescaped HTML

### 4.3 CEL Code Generation
- [ ] Generate Rust code that evaluates CEL expressions at runtime
- [ ] Implement boolean attribute handling (present/absent based on CEL result)
- [ ] Handle switch/case with CEL expressions and implicit receiver
- [ ] Implement proper HTML escaping for CEL output

### 4.4 CEL Error Handling
- [ ] Implement runtime error capture
- [ ] Generate `ERROR` placeholder with tooltip on evaluation failure
- [ ] Ensure fail-soft behavior (continue rendering on error)

---

## Phase 5: Protocol Buffers Integration (New)

**Goal:** Use Protocol Buffers for type-safe data contracts.

### 5.1 Proto Definition Parsing
- [ ] Implement `/**` comment extraction for proto blocks
- [ ] Add proto3 parser (consider `prost-types` or `protobuf-parse`)
- [ ] Support `import` statements with relative path resolution
- [ ] Support inline message and enum definitions

### 5.2 Proto Descriptor Generation
- [ ] Build proto descriptors from parsed definitions
- [ ] Embed descriptors in WASM module for CEL type awareness
- [ ] Resolve cross-file proto references

### 5.3 Component Metadata
- [ ] Parse `// name:` and `// data:` comments
- [ ] Map `// data:` type to proto message definition
- [ ] Validate component data types exist in proto definitions

### 5.4 Go Runtime Updates
- [ ] Update runtime to accept proto messages (wire format)
- [ ] Remove CBOR serialization, use proto serialization
- [ ] Generate Go proto bindings for view data types

---

## Phase 6: Type Safety & Diagnostics (New)

**Goal:** Compile-time validation of CEL expressions against proto schemas.

### 6.1 CEL Type Checking
- [ ] Create CEL type environment from proto descriptors
- [ ] Validate all CEL expressions against available types
- [ ] Report unknown field/message errors as diagnostics
- [ ] Handle proto enums in CEL type checking

### 6.2 Component Composition Validation
- [ ] Track component signatures (name → expected data type)
- [ ] Validate component invocations match expected types
- [ ] Support components without data (`Footer` with no argument)

### 6.3 LSP Diagnostics Enhancement
- [ ] Report CEL syntax errors with precise locations
- [ ] Report proto syntax errors in `/**` blocks
- [ ] Report type mismatches in component invocations
- [ ] Report unknown proto imports

### 6.4 LSP Hover & Completion
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
```
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
