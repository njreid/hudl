# Hudl + Datastar Documentation

This documentation describes how to use Datastar's hypermedia framework within Hudl templates. Hudl provides a first-class, ergonomic syntax for Datastar's reactive attributes using the `~` (tilde) operator.

## Core Concepts

Datastar brings reactivity to your HTML using `data-*` attributes. Hudl simplifies generating these attributes, handling naming conventions and expression formatting automatically.

### Syntax Patterns

There are three ways to apply Datastar attributes in Hudl:

#### 1. Binding Shorthand (`~>`)

Use `~>` directly on an element name for two-way data binding.

```hudl
input~>searchQuery
// Generates: <input data-bind="searchQuery">
```

#### 2. Inline Attributes (`~key:modifier=value`)

Apply single attributes directly to the element.

```hudl
button ~on:click="submit()" "Submit"
// Generates: <button data-on-click="submit()">Submit</button>
```

#### 3. Tilde Block (`~ { ... }`)

Group multiple reactive attributes in a dedicated block. This is the preferred style for complex elements.

```hudl
div {
    ~ {
        init "@get('/api/data')"
        on:click "handleClick()"
        show $isVisible
        .active $isActive
    }
    span "Content"
}
```

## Components & Imports

Hudl supports component-based architecture. You can define a component in one file and use it in another by importing it.

### Defining a Component

```hudl
// views/layout.hudl
// name: AppLayout
el {
    html {
        head { title "My App" }
        body {
            main { #content }
        }
    }
}
```

### Importing and Using Components

Use the `import` node at the top of your file to bring in components from other `.hudl` files.

```hudl
// views/index.hudl
import {
    "./layout"
}

// name: HomePage
el {
    AppLayout {
        h1 "Welcome"
    }
}
```

The `import` block takes a list of relative paths (without the `.hudl` extension).

## Attribute Reference

### Signals (`let`)

Define reactive state directly in your HTML.

```hudl
div {
    ~ {
        // Static values (data-signals)
        let:count 0
        let:label "Hello"

        // Computed values (data-computed)
        let:doubled "$count * 2"
        let:valid "$text.length > 0"

        // Modifiers
        let:init~ifmissing 100
    }
}
```

### Two-Way Binding (`bind` / `~>`)

Bind input values to signals.

```hudl
// Shorthand (Preferred)
input~>username

// With modifiers
input~>search~debounce:300ms

// Explicit form inside tilde block
input {
    ~ {
        bind username
    }
}
```

### Event Handling (`on`)

Listen to DOM events and execute expressions or actions.

```hudl
button {
    ~ {
        on:click "$count++"
        on:submit~prevent "@post('/save')"
        on:keydown.enter "submit()"
        on:click~outside "close()"
        on:scroll~throttle:100ms~passive "onScroll()"
    }
    "Action"
}
```

**Common Modifiers:**

- `~prevent`: `preventDefault()`
- `~stop`: `stopPropagation()`
- `~once`: Trigger only once
- `~window`: Attach listener to window
- `~outside`: Trigger when clicking outside element
- `~debounce:ms`: Debounce handler
- `~throttle:ms`: Throttle handler

### Visibility (`show`)

Conditionally display elements (toggles `display: none`).

```hudl
div ~show="$isVisible" "I am visible"
```

### Text Content (`text`)

Reactive text content.

```hudl
span ~text="$count + ' items'"
```

### Dynamic Classes (`class` / `.`)

Toggle CSS classes based on expressions.

```hudl
div {
    ~ {
        // Shorthand (Preferred)
        .active $isActive
        .text-red "$hasError"

        // Long form
        class:highlight $shouldHighlight
    }
}
```

### HTML Attributes

Bind any standard HTML attribute dynamically.

```hudl
button {
    ~ {
        disabled "$isLoading"
        href "'/user/' + $id"
        placeholder "$placeholderText"
    }
    "Click me"
}
```

### Persistence (`persist`)

Save signals to local storage or session storage.

```hudl
// Persist all signals to localStorage
div ~persist

// Persist specific signals
div ~persist="theme,lang"

// Persist to sessionStorage
div ~persist~session="token"
```

### Element Reference (`ref`)

Get a reference to the DOM element in your signals.

```hudl
input ~ref="emailInput"
// Access via $emailInput
```

### Intersection Observer (`on:intersect`)

Trigger events when an element enters the viewport.

```hudl
div {
    ~ {
        on:intersect "@get('/lazy-load')"
        on:intersect~once "markSeen()"
        on:intersect~half "halfVisible()"
    }
}
```

### Teleport

Move an element to another part of the DOM.

```hudl
div {
    ~ {
        teleport "#modal-root"
        teleport~prepend "#list"
        teleport~append "#logs"
    }
}
```

### Scroll Into View

Declarative scrolling behavior.

```hudl
div {
    ~ {
        scrollIntoView~smooth~vcenter
    }
}
```

## Actions

Actions are helper functions available in your expressions, prefixed with `@`.

### HTTP Actions

- `@get(url)`
- `@post(url, body)`
- `@put(url, body)`
- `@patch(url, body)`
- `@delete(url)`

**Example:**

```hudl
button ~on:click="@post('/api/todos', { text: $newTodo })" "Add"
```

### Helper Actions

- `@setAll('pattern', value)`
- `@toggleAll('pattern')`
- `@clipboard(text)`

## Naming Conventions

Hudl automatically maps your attributes to the correct Datastar HTML attributes:

| Hudl Syntax | Output Attribute |
|-------------|------------------|
| `on:click` | `data-on-click` |
| `on:custom` | `data-on:custom` |
| `.active` | `data-class-active` |
| `let:foo` | `data-signals-foo` |
| `init` | `data-init` |
| `show` | `data-show` |
| `text` | `data-text` |

Signal names (like `$count`) are preserved exactly as written to ensure compatibility with Datastar's expression engine.
