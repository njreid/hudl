# Hudl: The WASM-Native KDL Templating Language

**Hudl** is a type-safe templating language that compiles KDL (v2) document structures into high-performance **WebAssembly (WASM)** modules. It uses **Protocol Buffers** for data contracts and **CEL (Common Expression Language)** for expressions, and is designed to be easily embedded in Go applications using **wazero**.

## Architecture

* **Compiler (`hudlc`)**: Written in Rust. Compiles `.hudl` templates and proto definitions into a single `.wasm` binary.
* **Runtime**: Go application loads the `.wasm` file using `wazero`.
* **Views**: Each template file becomes an exported function in the WASM module.
* **Interpreter**: A Rust-based interpreter provides instant hot-reload during development.

## Features

* **HTML Mapping**: KDL nodes map directly to HTML tags.
* **WASM Powered**: Templates are compiled to portable, secure WebAssembly instructions.
* **KDL v2**: Fully compliant KDL v2 support via `kdl-rs`.
* **Type Safety**: Data contracts defined via Protocol Buffers (proto3).
* **Expressions**: High-performance evaluation via CEL (Common Expression Language).
* **Datastar Integration**: First-class support for the Datastar hypermedia framework.

---

## Language Reference

### 1. Basic Structure

A generic HTML element is defined by its tag name. Attributes are KDL properties. Inner text is a positional argument or interpolated string.

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
h1#main-title.text-center "Welcome"

// Implicit div with ID shorthand
#container.flex-row {
    .sidebar "Sidebar Content"
}

// Compiles to:
// <h1 id="main-title" class="text-center">Welcome</h1>
// <div id="container" class="flex-row">
//     <div class="sidebar">Sidebar Content</div>
// </div>
```

### 3. Special Link Nodes (`_`)

The `_` prefix creates `<link>` or `<script>` tags efficiently.

```kdl
// Stylesheet shorthand
_stylesheet "/style.css"

// Script shorthand
_script "/app.js"

// Generic link (rel comes after underscore)
_icon "/favicon.ico"
```

### 4. Components & Imports

Hudl supports reusable components. Top-level metadata is provided via structured comments. Use `import` to use components from other files and `#content` to define where nested children should be rendered.

```kdl
// layout.hudl
// name: AppLayout
el {
    html {
        head { title `title` }
        body {
            header { h1 "My App" }
            main { #content }
        }
    }
}

// index.hudl
import {
    "./layout"
}

// HomePage
// param: User user
el {
    AppLayout title="Home" {
        .welcome-card {
            h2 "Welcome back, `user.name`!"
        }
    }
}
```

### 5. Scoped CSS

You can define styles scoped to a component using a `css` block or inline `style` blocks.

```kdl
el {
    css {
        .card { background-color "white" }
        #header { border-bottom "1px solid black" }
    }

    button {
        style { color "red" }
        "Delete"
    }
}
```

### 6. Control Flow

#### If / Else

Conditional logic using CEL expressions in backticks.

```kdl
if `size(items) == 0` {
    p "No items found."
} else {
    p "Found `size(items)` items."
}
```

#### Each (Iterators)

Iterates over a collection. Inside the block, the binding name and `<itemvar>_idx` are available.

```kdl
// param: repeated NavItem nav_items
each item `nav_items` {
    li {
        span "Item #`item_idx`: "
        a href=`item.url` `item.label`
    }
}
```

#### Switch

Provides branching based on values or types.

```kdl
// param: Notification notification
switch `notification.type` {
    case "email" {
        .icon-email
        span `notification.subject`
    }
    case "sms" {
        .icon-sms
        span `notification.phone_number`
    }
    default {
        span "Unknown notification"
    }
}
```

---

## Datastar Integration

Hudl provides first-class syntax for [Datastar](https://data-star.dev) reactive attributes using the `~` prefix.

```kdl
div {
    ~ {
        let:count 0
        on:click "$count++"
        show "$count < 10"
    }

    // Binding shorthand
    input~>username placeholder="Enter name..."

    button ~on:click="@post('/api/save')" "Save"
}
```

---

## Syntax Highlighting

Hudl provides advanced syntax highlighting via Tree-sitter, with intentional color differentiation between backend and frontend logic:

*   **Backend (CEL)**: Anything contained inside backticks (`` `...` ``) is highlighted as backend code. These expressions are evaluated by the Go/WASM runtime.
*   **Frontend (Datastar)**: Properties starting with `~` and the special `~ { }` block are highlighted as frontend expressions. These run in the browser via the Datastar framework.

### Combined Expressions

Backend CEL expressions can be seamlessly embedded within frontend Datastar expressions. This is powerful for initializing frontend state with backend data:

```kdl
div {
    ~ {
        // Initialize frontend signal with backend data
        let:user_id `user.id`
        
        // Dynamic frontend action using backend-provided URL
        on:click "@get('`api_base`/users/`user.id`')"
    }
}
```

In the example above, `` `user.id` `` and `` `api_base` `` will be highlighted differently than the `let:` and `on:` Datastar attributes, making it easy to distinguish where each piece of logic executes.

---

## The LSP

The `hudl` ecosystem relies on `hudl-lsp` for a rich editing experience.

### Formatting

On save, the LSP normalizes your code:

* Expands attributes to shorthands where possible.
* Aligns blocks and enforces consistent indentation.
* Groups multiple tilde blocks.

### Diagnostics

* **Type Checking**: Verifies CEL expressions against your Protocol Buffer schemas.
* **Component Validation**: Ensures components are called with the correct data types.
* **Switch Exhaustiveness**: Warns if a switch on an enum misses a case.

---

## Getting Started

The easiest way to start a new Hudl project is using the **Hudl CLI**.

### 1. Install the Toolchain

Currently, Hudl requires building from source:

```bash
# Clone the repository
git clone https://github.com/njreid/hudl.git
cd hudl

# Build and install the CLI and compiler
make build
go install ./cmd/hudl
```

### 2. Initialize a New Project

```bash
hudl init my-app
cd my-app
```

This creates a standard scaffold with:
* `main.go`: A Go server using `chi` and `hudl` runtime.
* `views/`: Directory for your `.hudl` templates.
* `public/`: Static assets, including `datastar.js`.

---

## Development Mode

Hudl provides a high-productivity "Dev Mode" with automatic hot-reload.

### Using the CLI (Recommended)

Simply run:

```bash
hudl dev
```

This command:
1.  Generates Go wrappers for your views.
2.  Starts the LSP Dev Server in the background.
3.  Runs your Go application with `HUDL_DEV=1`.

### Manual Setup

If you prefer manual control:

1. **Start the LSP Dev Server**:
   ```bash
   hudl-lsp --dev-server --port 9999 --watch ./views
   ```

2. **Configure the Go Runtime**:
   Set the environment variables before running your app:
   ```bash
   export HUDL_DEV=1
   export HUDL_DEV_ADDR=localhost:9999
   go run .
   ```

The runtime will now use SSE-based hot-reload via the LSP, automatically refreshing your browser when you save any `.hudl` file.

---

## Production Build

For production, Hudl templates are compiled into a high-performance WebAssembly module.

```bash
hudl build
```

This generates `views.wasm`, which your Go application will load automatically when `HUDL_DEV` is not set.
