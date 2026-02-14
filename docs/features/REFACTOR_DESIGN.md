# Refactor & Simplification Design

This document outlines the planned and completed refactorings for the Hudl project to simplify the codebase and ensure consistency across the compiler, LSP, and runtime.

## Ranked Opportunities

### 1. Unify CEL Implementation (High Priority)
*   **Problem**: Divergent expression implementations. `src/interpreter.rs` (Dev Mode) used `cel-interpreter`, while the LSP and the old `src/codegen.rs` used a custom, partial parser in `src/expr.rs`.
*   **Goal**: Migrate all components to use `cel-interpreter` via `src/cel.rs`.
*   **Status**: In Progress / Mostly Complete. `src/expr.rs` and `src/codegen.rs` have been removed. LSP has been migrated to `cel-interpreter`.

### 2. Shared Protocol Buffer Decoder (Medium-High Priority)
*   **Problem**: Duplicate Protobuf wire-format decoders in `src/interpreter.rs` and the templates within `src/codegen_cel.rs`.
*   **Goal**: Extract decoding logic into a shared module in `src/proto.rs`.
*   **Status**: In Progress. Shared decoder added to `src/proto.rs` and integrated into `interpreter.rs`.

### 3. Refactor `pre_parse` in `src/parser.rs` (Medium Priority)
*   **Problem**: A complex, 300+ line manual state machine performing a second layer of lexing/parsing.
*   **Goal**: Refactor into a more robust and readable implementation to better handle edge cases like backtick wrapping and keyword escaping.
*   **Status**: In Progress. Currently being refined to maintain compatibility with existing tests.

### 4. Consolidate Transformation Pipeline (Medium Priority)
*   **Problem**: Implicit coupling between `pre_parse` (keyword prefixing) and `transformer.rs` (prefix matching).
*   **Goal**: Make the pipeline from raw Hudl source to AST more explicit and formal.
*   **Status**: Planned.

### 5. Go Runtime API Modernization (Low-Medium Priority)
*   **Problem**: Heavy reliance on environment variables for configuration.
*   **Goal**: Introduce a proper `Options` struct for `NewRuntime` to allow programmatic configuration.
*   **Status**: Complete. `Options` struct and `NewRuntimeFromWASM` helper implemented.

## Centralized Utilities

*   **Datastar Logic**: `datastar_attr_to_html` and related helpers have been moved to `src/ast.rs` to serve as a central source of truth for both the interpreter and codegen.
