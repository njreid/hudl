# hudl: The Go-Native KDL Templating Language

**hudl** is a type-safe templating language that compiles KDL document structures (specifically **KDL v2**) into efficient, executable Go functions. It combines the clean, node-based syntax of KDL with the performance and type safety of Go.

Designed to be used with a dedicated Language Server Protocol (LSP) implementation, `hudl` provides a development experience similar to writing native code, complete with auto-formatting, type checking, and exhaustiveness analysis.

## Features

* **HTML Mapping**: KDL nodes map directly to HTML tags.
* **Pug-like Shorthand**: Use `&id` and `.class` selectors; `div` is implied if omitted.
* **Unquoted Strings**: Clean syntax with minimal noise.
* **Go Integration**: Directly import Go packages, use Go expressions in backticks, and defined typed parameters.
* **Control Flow**: Strict `if`, `each` (iterator-based), and `switch` (type-safe pattern matching).
* **Scoped CSS**: Define component-local styles that are automatically scoped with unique class names.
* **LSP Powered**: Auto-formatting, instant code generation, and diagnostics.

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

### Code Generation

The LSP runs in the background. Saving a `.hu.kdl` file generates a sibling `.hu.kdl.go` file containing the generated Go functions, ready to be called from your HTTP handlers and compiled into the main go binary.

### Diagnostics

* **Type Checking**: Verifies that fields accessed in backticks (e.g., `user.Name`) exist on the Go struct.
* **Exhaustiveness**: Warns if a `switch` on an interface misses a specific implementation.