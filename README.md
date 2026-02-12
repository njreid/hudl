# Hudl: The WASM-Native KDL Templating Language

**Hudl** is a type-safe templating language that compiles KDL (v2) document structures into high-performance **WebAssembly (WASM)** modules. It leverages the robustness of the Rust KDL ecosystem for parsing and compilation, while remaining easy to embed in Go applications using **wazero**.

## Architecture

*   **Compiler (`hudlc`)**: Written in Rust. Compiles `.hudl` templates into a single `.wasm` binary.
*   **Runtime**: Go application loads the `.wasm` file using `wazero`.
*   **Views**: Each template file becomes an exported function in the WASM module.

## Features

*   **HTML Mapping**: KDL nodes map directly to HTML tags.
*   **WASM Powered**: Templates are compiled to portable, secure WebAssembly instructions.
*   **KDL v2**: Fully compliant KDL v2 support via `kdl-rs`.
*   **Go Integration**: Zero-cgo embedding via `wazero`.
*   **Type Safety**: Strict parameter typing and exhaustiveness checks at compile time.

---

## Language Reference

### 1. Basic Structure

A generic HTML element is defined by its tag name. Attributes are KDL properties. Inner text is the last positional argument.

```kdl
// Generic form
a href="/" "Go Home"

// Compiles to:
// <a href="/">Go Home</a>
```

### 2. Shorthands (Pug/Jade Style)

CSS selectors can be used directly as node names. If no tag name is provided, `div` is assumed.

```kdl
// Explicit tag with selectors
h1&main-title.text-center "Welcome"

// Implicit div
&container.flex-row {
    .sidebar "Sidebar Content"
}

// Compiles to:
// <h1 id="main-title" class="text-center">Welcome</h1>
// <div id="container" class="flex-row">
//     <div class="sidebar">Sidebar Content</div>
// </div>
```

### 3. Special Link Nodes (`_`)

The `_` prefix creates `<link>` tags efficiently.

```kdl
// Stylesheet shorthand
_stylesheet "/css/main.css"

// Preload shorthand
_preload "/fonts/inter.woff2" as=font

// Generic link (rel comes after underscore)
_icon "/favicon.ico"
```

### 4. Components & Parameters

Top-level nodes (excluding `import`) define Go functions. Metadata is provided via structured comments.

```kdl
import {
    "github.com/myapp/models"
}

// name: UserBadge
// param: user models.User
el {
    .badge {
        span "`user.Name`"
        if "`user.IsAdmin`" {
            .icon.star
        }
    }
}
```

### 5. Scoped CSS

You can define styles scoped to a component using a `css` block. The compiler generates unique class names to prevent conflicts. 

Within a `css` block:
*   Nodes starting with `&` followed by alphanumeric characters (e.g., `&main`) are converted to CSS IDs (e.g., `#main`).
*   Nodes starting with `&` followed by punctuation (e.g., `&:hover`, `&::after`) are treated as standard CSS parent selectors.

```kdl
el {
    css {
        .card {
            background-color "white"
        }
        // Becomes #header
        &header {
            border-bottom "1px solid black"
        }
        // Standard CSS nesting/pseudo-class
        .card:hover {
            background-color "#f0f0f0"
        }
    }

    &header { h1 "My App" }
    .card { p "Content" }
}
```

### 6. Property & Node Values

*   **Unquoted Strings**: Standard strings do not need quotes.
*   **Numbers**: Prefix numbers with `_` to make them valid KDL identifiers.
    *   `width _6px` compiles to `width: 6px`.
    *   `_0%` (node name) compiles to `0%` (e.g., in keyframes).

### 7. Control Flow

#### If / Else

Standard conditional logic using Go expressions in backticks (wrapped in quotes if they contain spaces).

```kdl
if "`len(items) == 0`" {
    p "No items found."
} else {
    p "Found items."
}
```

#### Each (Iterators)

Iterates over any Go type implementing an `Iterator` interface. If two positional arguments are provided, the first is the index/key and the second is the value.

```kdl
// param: navItems models.NavIterator
// syntax: each [idx] item of=<expression>
each i item of="`navItems`" {
    li {
        span "Item #`i`: "
        a href="`item.URL`" "`item.Label`"
    }
}
```

#### Switch (Type & Value)

Provides exhaustiveness checking for Go interfaces

```kdl
// param: notification models.Notification
switch "`notification`" {
    // Type destructuring: 'v' is automatically typed as models.Email
    case models.Email {
        .icon-email
        span "`v.Subject`"
    }
    case models.SMS {
        .icon-sms
        span "`v.PhoneNumber`"
    }
    default {
        span "Unknown notification"
    }
}
```

---

## The LSP

The `hudl` ecosystem relies on `hudl-lsp`.

### Formatting

On save, the LSP normalizes your code:

* Expands `div id=foo` to `&foo`.
* Aligns `case` statements.
* Enforces indentation.

### Compilation

Running `hudlc` compiles your `.hudl` files into a single optimized `views.wasm` file. This binary contains all your templates as exported functions, ready to be called from your host application.

### Diagnostics

* **Type Checking**: Verifies that fields accessed in backticks (e.g., `user.Name`) exist on the Go struct.
* **Exhaustiveness**: Warns if a `switch` on an interface misses a specific implementation.

## Development Mode

Hudl provides a high-productivity "Dev Mode" that avoids the need for WASM recompilation during template development.

### 1. Start the LSP Dev Server

The LSP can act as a rendering sidecar. Run it from your project root:

```bash
hudl-lsp --dev-server --port 9999 --watch ./views
```

Optional: add `--verbose` or `-v` for detailed request logging.

### 2. Configure the Go Runtime

In your Go application, set the following environment variables (or use `hudl.Options`):

```bash
export HUDL_DEV=1
export HUDL_DEV_ADDR=localhost:9999
```

When `HUDL_DEV` is enabled, the Go runtime will send render requests to the LSP over HTTP instead of executing the WASM binary. 

### 3. Live Reload

The LSP dev server automatically injects a small live-reload script into rendered pages when in dev mode. This script uses Server-Sent Events (SSE) to listen for file changes and refreshes the browser automatically when a `.hudl` file is saved.