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

## Phase 5: Protocol Buffers Integration (Completed)

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
- [x] Embed descriptors in WASM module for CEL type awareness
- [x] Resolve cross-file proto references

### 5.3 Component Metadata

- [x] Parse `// name:` and `// data:` comments
- [x] Map `// data:` type to proto message definition
- [x] Validate component data types exist in proto definitions

### 5.4 Go Runtime Updates

- [x] Update runtime to accept proto messages (wire format)
- [x] Remove CBOR serialization, use proto serialization
- [x] Generate Go proto bindings for view data types (`proto/views.proto` → `pkg/hudl/pb/`)

### 5.5 LSP Proto Integration

- [x] Use proto schema for enum exhaustiveness checking
- [x] Fall back to Go analyzer when proto info not available

---

## Phase 6: Type Safety & Diagnostics (Completed)

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
- [x] Report proto syntax errors in `/**` blocks
- [x] Report type mismatches in component invocations

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

- [x] Create `proto/views.proto` with view-specific messages
- [x] Define enums for status values (TransactionStatus)

### 7.2 Template Updates

- [x] Update `layout.hudl` with proto types and CEL
- [x] Update `dashboard.hudl` with proto types and CEL
- [x] Update `form.hudl` with proto types and CEL
- [x] Update `marketing.hudl` with proto types and CEL
- [x] Create component composition example (`dashboard_composed.hudl` + `stat_card.hudl`)

### 7.3 Go Application

- [x] Update `examples/go-app` to use proto messages
- [x] Generate Go proto bindings (`pkg/hudl/pb/views.pb.go`)
- [x] Demonstrate data transformation (mockdata → Proto → Template)

---

## Phase 8: Dev Mode Implementation (Completed)

**Goal:** Enable hot-reload development workflow where the LSP acts as a rendering sidecar.

### 8.1 Template Interpreter

- [x] Create `src/interpreter.rs` - walks AST and renders HTML directly using CEL
- [x] Implement proto wire-format → CEL value decoding with schema awareness
- [x] Handle control flow: if/else, each, switch/case
- [x] Support text interpolation with HTML escaping
- [x] Proto3 default value semantics for unpopulated fields

### 8.2 LSP Dev Server

- [x] Add HTTP server to LSP (`lsp/src/dev_server.rs`) using axum
- [x] Implement `POST /render` endpoint (proto wire bytes + component header → HTML)
- [x] Implement `GET /health` endpoint
- [x] Implement `GET /api/components` to list loaded components
- [x] File watching with `notify` crate (auto-reload on .hudl changes)
- [x] Template caching in memory for instant rendering
- [x] CLI: `hudl-lsp --dev-server --port PORT --watch DIR`
- [x] SSE-based live reload notifications
- [x] Injection of reload script into dev renders

### 8.3 Go Runtime Dev/Prod Switching

- [x] Implement `HUDL_DEV` and `HUDL_DEV_ADDR` environment variable support
- [x] In dev mode: HTTP POST to LSP for rendering
- [x] In prod mode: Use embedded WASM (existing behavior)
- [x] Identical API surface in both modes (`Render` and `RenderBytes`)
- [x] Send proto wire-format bytes directly (no re-serialization)
- [x] HTTP client with 5s timeout in Go runtime
- [x] Error propagation from LSP to Go caller

### 8.4 Developer Experience

- [x] LSP logs render requests in verbose mode
- [x] Show template compilation errors immediately on save
- [x] Support partial updates (single component re-render)
- [x] Document dev mode setup in README

### 8.5 Testing Dev Mode

- [x] Interpreter expanded tests (22 tests covering control flow, expressions, elements, proto edge cases, errors)
- [x] Dev server HTTP tests (11 tests: health, render, error paths, component listing, edit-render-error loop)
- [x] End-to-end edit-render-error loop test (render v1 → edit v2 → break → recover v3)
- [x] File watcher integration tests (async watcher → cache update cycle with temp dirs)
- [x] Go dev mode integration tests (subprocess dev server + Go client)
- [x] Benchmark: dev mode latency vs prod mode (Dev mode: ~0.14ms for static, ~0.5ms for dynamic)

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

## Phase 9: Live Reload (Simplified)

**Goal:** Provide automatic browser refresh when templates change without a complex frontend.

- [x] Implement `/events` SSE endpoint in LSP dev server
- [x] Implement `RELOAD_SCRIPT` injection in `render_handler`
- [x] Verify browser reloads on `.hudl` file save
- [x] Remove obsolete Node.js dependencies and preview SPA

---

## Phase 10: Datastar Integration

**Goal:** Generate Datastar `data-*` reactive attributes from Hudl's `~` (tilde) syntax.

See `DATASTAR_DESIGN.md` for full syntax reference and attribute mappings.

### 10.1 Core Syntax (Complete)

- [x] Tilde block parsing as child node (`{ ~ { ... } }`) — `parser.rs`, `transformer.rs`
- [x] Inline tilde attribute parsing (`~on:click="value"`) — `parser.rs`
- [x] `DatastarAttr` AST type and transformer extraction — `transformer.rs`
- [x] `datastar_attr_to_html` codegen for legacy codegen — `codegen.rs`
- [x] Basic attribute generation in CEL codegen (`codegen_cel.rs`)
- [x] Signal/computed detection (`let:` static vs expression → `data-signals` vs `data-computed`) — `codegen.rs:is_computed_expression`
- [x] Modifier parsing and chaining (`~once`, `~debounce:300ms`) — `transformer.rs:parse_attr_name_and_modifiers`, `codegen.rs:datastar_attr_to_html`
- [x] Interpreter support for Datastar attributes in dev mode — `interpreter.rs:render_element`
- [x] Formatter: combine multiple tilde blocks, position as first child

### 10.2 Bindings

- [x] `~>` binding shorthand parsing (`input~>signalName`)
- [x] `~bind:` explicit form
- [x] Formatter normalization to shorthand
- [x] Binding modifiers (debounce, throttle)

### 10.3 Actions (Complete)

- [x] HTTP actions (`@get`, `@post`, `@put`, `@patch`, `@delete`)
- [x] Signal actions (`@setAll`, `@toggleAll`, `@fit`, `@peek`)
- [x] DOM actions (`@clipboard`)
- [x] Action modifier parsing

### 10.4 Advanced Features

- [x] Intersection observer (`on:intersect`)
- [x] Teleport (`teleport`)
- [x] Persist (`persist`, `persist~session`)
- [x] Scroll into view (`scrollIntoView~smooth`)
- [x] Element refs (`ref`)

### 10.5 Tooling Integration

- [x] LSP diagnostics for invalid tilde attributes
- [x] LSP support for signal name completion
- [x] LSP support for action completion
- [x] Syntax highlighting differentiation for tilde blocks

### 10.6 Testing

- [x] Un-ignore `tests/datastar_spec.rs` AST + rendering tests (all 63 passing)
- [x] Un-ignore `tests/datastar_missing_edge_cases_spec.rs` tests (all 6 passing)
- [x] Un-ignore remaining tests when binding shorthand (`~>`) is implemented

---

## Phase 11: Hudl CLI (In Progress)

**Goal:** Provide a `go install`-able CLI for binary management and project scaffolding.

### 11.1 CLI Core

- [x] Set up `cmd/hudl` package and command-line argument parsing.
- [x] Implement interactive prompting for project metadata.

### 11.2 Binary Management (`hudl install`)

- [ ] Implement OS/Arch detection.
- [ ] Implement download and extraction logic for Rust binaries (`hudlc`, `hudl-lsp`).
- [ ] Implement checksum verification for downloaded artifacts.

### 11.3 Project Scaffolding (`hudl init`)

- [x] Implement `go mod init` and dependency fetching.
- [x] Create `main.go` template with `chi` router and static asset serving.
- [x] Create `views/layout.hudl` and `views/index.hudl` templates.
- [x] Create `./public` directory for static assets.

### 11.4 Verification & Testing

- [x] Implement unit tests for CLI commands.
- [ ] Implement integration tests for project scaffolding.
- [ ] Ensure `go install` works as expected.


