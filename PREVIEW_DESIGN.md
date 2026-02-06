# Hudl Component Preview Design

## Overview

This document outlines the design for a "Component Preview" feature integrated into the Hudl LSP. The goal is to provide a live, visual development environment where users can select any Hudl component, provide it with mock data (in Protobuf Text Format), and see the rendered result instantly.

## Architecture

### 1. LSP as Web Server

The `hudl-lsp` binary will host a lightweight HTTP server (likely extending the existing dev server logic).

* **Serving:** The server will serve a static Single Page Application (SPA) for the preview UI.
* **API:**
  * `GET /api/components`: Lists all available components in the workspace, grouped by file.
  * `GET /api/proto-schema/{component}`: Returns the expected Protobuf schema for a specific component (to help the frontend editor or for validation).
  * `POST /api/render-preview`: Accepts Component Name + Proto Text Data, returns Rendered HTML.
  * `WS /ws`: WebSocket connection for "hot reload" notifications when `.hudl` source files are saved.

### 2. Frontend (The Preview UI)

A simple **Svelte 5** app embedded in the LSP binary (or served from `dist/` during dev). Styling is provided by **Pico.css** for a clean, semantic, and zero-config UI.

**Layout (Pico.css Semantic Structure):**

* **`<header>` (Nav Bar):**
  * `<nav>` containing the component selection dropdown (using a `<select>` or searchable implementation).
  * Grouped by source filename (e.g., `dashboard.hudl > UserCard`).
* **`<main class="container-fluid">` (Split Pane):**
  * Using a `<grid>` or custom flex layout for the two-panel view.
  * **Left Panel (Editor):**
    *   Monaco Editor instance.
    *   Language: Protobuf Text Format (`.textproto`).
  * **Right Panel (Preview):**
    *   `<iframe>` to isolate styles.
    *   Source: A wrapper HTML page that renders the HTML returned by the LSP.
* **`<footer>`:**
  * Status indicators (e.g., "Last rendered: 12:00:01", "Syntax OK").

## Detailed Functionality

### 1. Component Selection & Mock Data Generation

* When the LSP starts, it scans all `.hudl` files to build a registry of components.
* When a user selects a component in the UI:
  * The UI requests the "default mock data" from the LSP.
  * **LSP Logic:** The LSP analyzes the component's input Proto message. It recursively walks the fields to generate a valid textproto skeleton.
    *   Scalars: Default values (0, "", false).
    *   Repeated: Empty list `[]` or a list with one example item.
    *   Messages: Nested blocks `{ ... }`.
  * **Nested Components:** If Component A calls Component B, the data for Component B must be present in Component A's input (if A passes data to B). The mock generation should account for this deep dependency if the data structures align.

### 2. Editor & Validation

* **Monaco Editor:** configured for Protobuf Text Format.
* **Client-Side Validation:** If possible, compile a lightweight textproto parser to WASM for immediate syntax checking in the browser.
* **Server-Side Validation:** On change (debounced), send text to LSP.
  * If syntax error: Return error line/col. Editor underlines in red.
  * If valid: LSP parses textproto -> Proto Message -> Renders Component -> Returns HTML.
* **State Retention:** The preview iframe **must not** update if the editor contains syntax errors. It retains the last successful render.

### 3. Live Reloading

* **Data Changes:** As the user types in the editor (and syntax is valid), the right panel updates.
* **Code Changes:**
    1.  User saves `user_card.hudl`.
    2.  LSP detects file change via file watcher.
    3.  LSP recompiles/re-parses the affected component.
    4.  LSP sends "Update" signal via WebSocket to the frontend.
    5.  Frontend re-submits the *current* mock data from the editor to `POST /api/render-preview`.
    6.  Preview updates with new code logic + existing mock data.

## Open Questions

### 1. Persistence

* **Problem:** If I spend 10 minutes crafting perfect mock data for `UserCard`, I don't want to lose it when I restart the LSP or switch components.
* **Proposal:**
  * **(File System):** Save to `*.preview.txtpb` files alongside the `.hudl` files. This allows checking mock data into git. Auxiliary preview files for a component can be added with componentname_auxname.preview.txtpb, and these are shown as sub options under the component in the header drop-down. If a preview file isn't found on disk for a component when it's selected for preview by the user, it should be created.

### 2. Static Assets

* **Problem:** The component might rely on CSS/images (e.g., `<link href="/styles.css">`).
* **Solution:** The LSP needs a generic "public" directory setting or middleware. The iframe will try to load `/styles.css`. The LSP dev server must serve these files from the project root or a configured `public/` folder.

### 3. Dependencies & "Magic" Data

* **Problem:** What if a component uses a global variable or data not passed via `// data:` (though Hudl tries to be explicit)?
* **Answer:** Hudl design enforces explicit data passing. Only data in the `// data:` proto is available.

### 4. Actions & Events

* **Problem:** `on:click="@post(...)"`.
* **Solution:** The preview iframe should likely have a dummy JS runtime that logs these actions to the console instead of actually executing network requests, or provides a UI log of "Action Triggered: POST /api/..."

### 5. Signal Debug

* Datastar signals present within the components, and their current values should appear in a right-hand sidebar. which can be closed.

### 6. Signal Debug Bridge

* **Problem:** Datastar signals exist in the browser context inside the iframe. The parent Svelte app needs to "see" them.
* **Proposal:** Inject a small script into the preview iframe that listens for Datastar signal changes and `postMessage`s them to the parent UI.

### 7. LSP Entry Point

* **Problem:** How does the user trigger the preview from the editor?
* **Proposal:** Use a VS Code CodeLens (e.g., "Preview Component") directly above the `// name: ...` metadata line in `.hudl` files.

## Implementation Steps

1. **LSP Server Upgrade:** Add HTTP endpoints for component listing and schema reflection.
2. **Mock Generator:** Implement the "Proto Message -> TextProto Skeleton" generator in Rust.
3. **Frontend Scaffold:** Create the Svelte 5 app with Monaco and Pico.css.
4. **Integration:** Connect LSP serving logic to the frontend.
