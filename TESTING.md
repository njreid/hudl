# Hudl Testing Guide

This document describes every layer of testing in the Hudl project: what exists
today and how to run it.

---

## Quick Reference

```bash
# All Rust unit + integration tests
cargo test

# Just unit tests (parser, proto, cel, formatter, interpreter, codegen)
cargo test --lib

# Just integration tests
cargo test --test compiler_spec --test expr_spec --test codegen_expr_spec

# LSP protocol tests (spawns hudl-lsp binary)
cargo test --manifest-path lsp/Cargo.toml --test lsp_protocol_test

# Dev server HTTP tests (in-process, no network)
cargo test --manifest-path lsp/Cargo.toml --test dev_server_http_test

# Go runtime tests (requires views.wasm)
go test ./pkg/hudl/...

# Build LSP (needed before LSP tests)
cargo build --manifest-path lsp/Cargo.toml
```

---

## 1. Rust Unit Tests (`cargo test --lib`)

In-source `#[cfg(test)]` modules. These are fast, isolated, and test individual
functions without I/O.

### `src/parser.rs` — 41 tests

Covers KDL syntax parsing: backtick expressions, selector shorthands (`div.foo#bar`),
attribute values, control flow nodes (`if`/`else`/`each`/`switch`), string literals,
escape sequences, proto block passthrough, and raw strings.

### `src/formatter.rs` — 12 tests

Pretty-printing and code formatting: indentation, backtick expression formatting,
control flow formatting, CSS selector preservation, proto block preservation,
comment handling.

### `src/proto.rs` — 9 tests

Proto schema extraction from `/** */` comment blocks: message parsing, enum parsing,
repeated fields, map fields, field path resolution, nested message references,
import statements.

### `src/cel.rs` — 8 tests

CEL expression compilation and evaluation: variable access, field access, function
calls (`size`), comparison operations, HTML escaping, truthy evaluation, ternary
operators.

### `src/interpreter.rs` — 25 tests

Dev mode template interpreter covering the full rendering surface. Uses a
`parse_template()` helper that runs the full parse-transform pipeline.

**Control flow:**

- `test_render_each_loop` — iterate over repeated field
- `test_render_each_with_index` — `<itemvar>_idx` variable available in loop
- `test_render_switch_enum` — switch on enum field, correct case rendered
- `test_render_switch_default` — default case when no match
- `test_render_nested_if` — if inside if

**Expressions:**

- `test_render_string_interpolation_multiple` — multiple backtick expressions in text
- `test_render_comparison_in_if` — `count > 0` in if condition
- `test_render_nested_field_access` — `user.name` nested access

**Element rendering:**

- `test_render_static_html` — basic static elements
- `test_render_with_data` — data-driven rendering
- `test_render_conditional` — conditional rendering with CEL
- `test_render_void_elements` — `<br>`, `<img>`, `<hr>` self-close correctly
- `test_render_css_classes` — class attribute from selector shorthand
- `test_render_id_attribute` — id from selector shorthand
- `test_render_dynamic_attributes` — attribute value is CEL expression
- `test_render_boolean_attributes` — `disabled=\`true\`` renders as just `disabled`

**Proto edge cases:**

- `test_render_empty_repeated_field` — empty array renders nothing in each
- `test_render_nested_message` — message field with sub-fields
- `test_render_enum_default` — enum defaults to first value (0)
- `test_render_missing_field_defaults` — field not in data uses proto3 default

**Error handling:**

- `test_render_unknown_variable_error` — CEL references undefined var
- `test_render_malformed_proto_error` — truncated wire-format bytes

**Datastar attributes:**

- `test_render_datastar_attrs` — tilde block attrs render as data-* HTML attributes
- `test_render_datastar_inline_tilde` — inline `~on:click` renders correctly
- `test_render_datastar_modifiers` — modifiers like `~prevent` and `~ifmissing` render

### `src/codegen_cel.rs` — 5 tests

Rust code generation for WASM compilation: basic codegen, CEL expression codegen,
proto schema-aware codegen, proto decoder generation, Datastar attribute generation.

---

## 2. Rust Integration Tests (`tests/`)

Separate test files that exercise multi-module pipelines.

### `tests/compiler_spec.rs` — 18 tests

Full compiler pipeline (parse -> transform -> codegen): element transformation,
shorthand selectors, attribute handling, style scoping, control flow with CEL,
boolean attributes, switch/case, each with index, multi-interpolation, nested
if/else, proto block extraction, component metadata.

### `tests/expr_spec.rs` — 13 tests

CEL expression parsing: literals, field access, comparison/arithmetic/logical
operators, precedence, function calls, method calls, list operations, enum values,
string concatenation. (1 test ignored: ternary.)

### `tests/codegen_expr_spec.rs` — 10 tests

CEL-specific codegen: comparison, field access, function calls, loop iteration,
switch/enum, boolean attributes, string interpolation, `raw()` function, nested
conditionals, component invocation.

### `tests/datastar_spec.rs` — 63 tests (4 `#[ignore]`)

AST-level tests verify tilde attributes are correctly parsed and stored in
`el.datastar` (sections 1-20). HTML rendering tests (section 21) verify the
interpreter produces correct `data-*` attributes. Only 4 tests remain ignored
(binding shorthand `~>` parser not yet implemented).

### `tests/datastar_missing_edge_cases_spec.rs` — 6 tests (1 `#[ignore]`)

Datastar integration (tilde block syntax). Test suite written ahead of
implementation — ready to un-ignore as features land.

---

## 3. LSP Protocol Tests (`lsp/tests/lsp_protocol_test.rs`)

**18 tests.** These spawn the actual `hudl-lsp` binary and communicate via
stdin/stdout using the JSON-RPC 2.0 / LSP protocol. A custom `LspClient` struct
handles Content-Length framing, request/response correlation, and notification
parsing.

**Coverage:**

- Protocol handshake: initialize capabilities (sync, formatting, semantic tokens)
- Document lifecycle: `didOpen` (valid + syntax error), `didChange`
- Formatting: format document, formatting of invalid document
- Semantic tokens: token generation, keyword coverage
- Advanced features: CEL expressions, control flow, CSS selectors, proto blocks,
  inline styles, special link nodes (`_stylesheet`, `_import`), unknown documents
- Validation: switch exhaustiveness, default case handling
- Shutdown: clean shutdown sequence

---

## 4. Dev Server HTTP Tests (`lsp/tests/dev_server_http_test.rs`)

**11 tests.** In-process HTTP tests using `tower::ServiceExt` to make requests
against the axum router without binding a port. Tests `DevServerState` template
loading, caching, reloading, and all HTTP endpoints.

**Coverage:**

- `/health`: returns ok status, reflects loaded template count
- `/render` happy path: simple component rendering, timing header, empty proto data
  with defaults
- `/render` error paths: missing `X-Hudl-Component` header (400), unknown component
  (404), garbage proto bytes (400)
- `/api/components`: empty list, populated list
- **Edit-render-error loop**: full cycle test — render v1, edit to v2, verify
  update, break with invalid syntax (stale template preserved), fix to v3, verify
  recovery

---

## 5. Go Runtime Tests (`pkg/hudl/runtime_test.go`)

**16 tests.** Integration tests using pre-compiled `views.wasm`. Tests skip if
`views.wasm` is not found. These exercise the full production path: Go serializes
a proto message -> passes wire-format bytes to WASM -> reads back rendered HTML.

**Coverage:**

- Basic rendering with `SimpleData` proto
- Complex nested data (`DashboardData` with `Transaction` sub-messages)
- Conditional rendering (`LayoutData.is_logged_in`)
- Iteration (`FeatureListData.features`)
- Form error handling (`FormData.error_message`)
- Raw bytes input (`RenderBytes`)
- Error cases: view not found, nil data
- Security: HTML escaping / XSS prevention
- Advanced: `raw()` content, scoped CSS, switch/case, each with index,
  boolean attributes, empty collections

---

## 6. Dev Server Architecture

The dev server (`lsp/src/dev_server.rs`) is the core of the edit -> live re-render
loop:

```text
┌─────────────┐   file save    ┌──────────────┐    POST /render    ┌──────────────┐
│  Editor     │ ──────────────>│  Dev Server  │ <─────────────────│  Go Runtime  │
│  (.hudl)    │                │  (file watch │ ──────────────────>│  (dev mode)  │
└─────────────┘                │   + cache)   │    HTML or error   └──────────────┘
                               └──────────────┘
```

1. User edits a `.hudl` file and saves
2. `notify` file watcher detects the change
3. Dev server re-parses the file (proto schema + KDL + AST transform)
4. If parse/transform fails -> error logged, stale template stays cached
5. Go runtime sends `POST /render` with component name + proto bytes
6. Dev server calls `interpreter::render()` on the cached AST
7. Returns HTML (200) or error JSON (400/404) to Go runtime

---

## 7. Test Counts Summary

| Suite | Tests | Ignored |
|-------|-------|---------|
| Rust unit tests (`--lib`) | 100 | 0 |
| Rust integration tests (`tests/`) | 110 | 6 |
| LSP protocol tests | 18 | 0 |
| Dev server HTTP tests | 11 | 0 |
| Go runtime tests | 16 | 0 |
| **Total** | **255** | **6** |

---

## 8. Running Tests

### Prerequisites

```bash
# Rust toolchain
rustup update stable

# Go (for runtime tests)
# go 1.21+

# Build WASM (needed for Go runtime tests)
cargo build --target wasm32-wasip1 --release
cp target/wasm32-wasip1/release/hudlc.wasm views.wasm
```

### Individual test suites

```bash
# Parser tests only
cargo test --lib parser::

# Interpreter tests only
cargo test --lib interpreter::

# CEL tests only
cargo test --lib cel::

# Proto tests only
cargo test --lib proto::

# Formatter tests only
cargo test --lib formatter::

# Codegen tests only
cargo test --lib codegen_cel::

# All integration tests
cargo test --test compiler_spec --test expr_spec --test codegen_expr_spec

# LSP protocol tests
cargo test --manifest-path lsp/Cargo.toml --test lsp_protocol_test -- --test-threads=1

# Dev server HTTP tests
cargo test --manifest-path lsp/Cargo.toml --test dev_server_http_test

# Go runtime tests
go test -v ./pkg/hudl/...

# Datastar tests (currently ignored)
cargo test --test datastar_spec -- --ignored
```

### CI

All non-ignored Rust tests and Go tests should pass on every commit.
LSP protocol tests require the `hudl-lsp` binary to build, so they run
after `cargo build --manifest-path lsp/Cargo.toml`.
