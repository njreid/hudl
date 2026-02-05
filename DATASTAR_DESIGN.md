# Datastar Integration Design for Hudl

This document describes how Datastar's reactive attributes and actions map to Hudl's `~` (tilde) syntax extension.

## Overview

Datastar is a hypermedia framework that uses `data-*` attributes to add reactivity to HTML elements. Hudl extends its syntax with the `~` prefix to generate these attributes in a more ergonomic way.

## Core Syntax Patterns

### 1. Binding Shorthand on Elements

```hudl
input~>signalName

// Generates HTML:
// <input data-bind="signalName">
```

The `~>` operator on an element creates a two-way binding (`data-bind`). It must be attached directly to the element name.

### 2. Inline Tilde Attributes

```hudl
button ~on:click="doSomething()" Click

// Generates HTML:
// <button data-on-click="doSomething()">Click</button>
```

### 3. Tilde Block (Multiple Reactive Attributes)

The tilde block is a **child node** of the element it applies to. It must be inside the element's braces:

```hudl
div {
    ~ {
        on:click "handleClick()"
        show $isVisible
        .active $isActive
    }
    span Content
}

// Generates HTML:
// <div data-on-click="handleClick()"
//      data-show="$isVisible"
//      data-class-active="$isActive">
//     <span>Content</span>
// </div>
```

**Formatting rules:**
- Multiple tilde blocks within the same parent are combined into one
- The tilde block is always positioned as the first child after formatting
- Quotes are optional for values without whitespace or special characters

### Syntax Shorthands

| Long form | Shorthand | Notes |
|-----------|-----------|-------|
| `class:active $expr` | `.active $expr` | Dynamic class uses `.` prefix |
| `~bind:signal` | `element~>signal` | Binding uses `~>` on element |

## Attribute Output Naming

Hudl uses colons exclusively in its syntax (e.g., `on:click`, `.active`). The output HTML uses Datastar's naming conventions:

| Hudl | Datastar HTML | Notes |
|-------------|-------------|-------|
| `on:click` | `data-on-click` | Browser events use dash |
| `on:myEvent` | `data-on:my-event` | Custom events use colon |
| `on:intersect` | `data-on-intersect` | Special Datastar subscriptions use dash |
| `on:fetch` | `data-on:datastar-fetch` | Datastar-specific events |
| `.active` | `data-class-active` | Classes use dash |
| `disabled` | `data-attr-disabled` | HTML attributes use dash |

The compiler handles these conversions automatically. User-defined custom events (non-standard DOM events) preserve the colon separator.

## Signal Name Handling

Signal names in Hudl are preserved exactly as written. This is important because Datastar expressions reference signals by name:

```hudl
div {
    ~ {
        let:mySignal 1
        on:click "$mySignal++"
    }
}
// The signal is referenced as $mySignal in expressions
```

Datastar performs its own transformations for HTML attribute names, but within expressions, signal names must match exactly. Hudl does not perform camelCase-to-kebab-case conversion on signal names to avoid mismatches.

## Attribute Mappings

### data-signals (Signal Initialization)

Defines reactive signals. Use `let:` syntax with a **static value** (no operators or function calls).

| Hudl | Datastar HTML | Notes |
|------|---------------|-------|
| `let:foo 1` | `data-signals-foo="1"` | Literal number |
| `let:foo hello` | `data-signals-foo="'hello'"` | Literal string (quotes optional) |
| `let:active true` | `data-signals-active="true"` | Literal boolean |

**Modifiers:**

- `~ifmissing` - Only set if signal doesn't exist

```hudl
div {
    ~ {
        let:count~ifmissing 0
    }
}
// → data-signals-count__ifmissing="0"
```

### data-computed (Derived Signals)

Computed values are expressions that contain **operators or function calls**. `let:` with any operation becomes a computed signal.

| Hudl | Datastar HTML | Notes |
|------|---------------|-------|
| `let:fullName "$firstName + ' ' + $lastName"` | `data-computed-fullName="$firstName + ' ' + $lastName"` | Has operators |
| `let:doubled "$value * 2"` | `data-computed-doubled="$value * 2"` | Has operators |
| `let:upper "$name.toUpperCase()"` | `data-computed-upper="$name.toUpperCase()"` | Has function call |

**Detection rules:**

- No operators, no function calls → `data-signals` (static)
- Any operators (`+`, `-`, `*`, `/`, `==`, `&&`, etc.) → `data-computed`
- Any function calls (`foo()`, `$x.method()`) → `data-computed`

```hudl
div {
    ~ {
        let:basePrice 100                      // static → data-signals
        let:quantity 1                         // static → data-signals
        let:total "$basePrice * $quantity"     // has operator → data-computed
        let:formatted "$total.toFixed(2)"      // has function → data-computed
    }
}
```

### data-bind (Two-Way Binding)

Binds an input element's value to a signal.

| Hudl | Datastar HTML | Notes |
|------|---------------|-------|
| `input~>signalName` | `data-bind="signalName"` | Shorthand (preferred) |
| `~bind:signalName` | `data-bind="signalName"` | Explicit form |

**Modifiers:**

- `~debounce` - Debounce updates (default 100ms)
- `~throttle` - Throttle updates (default 100ms)

```hudl
input~>searchQuery~debounce:300ms
// → <input data-bind__debounce.300ms="searchQuery">
```

### data-on (Event Handlers)

Handles DOM events with expressions.

| Hudl | Datastar HTML | Notes |
|------|---------------|-------|
| `on:click "$count++"` | `data-on-click="$count++"` | Expression |
| `on:click "@get('/api')"` | `data-on-click="@get('/api')"` | Action call |
| `on:keydown.enter "submit()"` | `data-on-keydown.enter="submit()"` | Key filter |

**Event Modifiers:**

| Modifier | Hudl Syntax | Effect |
|----------|-------------|--------|
| `__once` | `on:click~once` | Fire only once |
| `__passive` | `on:scroll~passive` | Passive listener |
| `__capture` | `on:click~capture` | Capture phase |
| `__debounce` | `on:input~debounce:200ms` | Debounce handler |
| `__throttle` | `on:scroll~throttle:100ms` | Throttle handler |
| `__window` | `on:resize~window` | Listen on window |
| `__outside` | `on:click~outside` | Click outside element |
| `__prevent` | `on:submit~prevent` | preventDefault() |
| `__stop` | `on:click~stop` | stopPropagation() |

**Key Modifiers (for keyboard events):**
`.enter`, `.escape`, `.space`, `.tab`, `.delete`, `.backspace`, `.up`, `.down`, `.left`, `.right`, `.shift`, `.ctrl`, `.alt`, `.meta`, `.cmd`

```hudl
div {
    ~ {
        on:click~once~prevent "@post('/submit')"
        on:keydown.enter~window "handleEnter()"
        on:scroll~throttle:50ms~passive "updatePosition()"
    }
}
```

### data-text (Text Content)

Sets element's text content reactively.

| Hudl | Datastar HTML | Notes |
|------|---------------|-------|
| `text $message` | `data-text="$message"` | Simple reference |
| `text "$greeting + ', ' + $name"` | `data-text="$greeting + ', ' + $name"` | Expression (quotes needed) |

```hudl
span {
    ~ {
        text "$greeting + ', ' + $name"
    }
}
```

### data-show (Conditional Display)

Shows/hides element based on expression.

| Hudl | Datastar HTML | Notes |
|------|---------------|-------|
| `show $isVisible` | `data-show="$isVisible"` | Simple reference |
| `show "$count > 0"` | `data-show="$count > 0"` | Expression |

### data-class (Dynamic Classes)

Adds/removes CSS classes based on signals. Use the `.` prefix shorthand:

| Hudl | Datastar HTML | Notes |
|------|---------------|-------|
| `.active $isActive` | `data-class-active="$isActive"` | Shorthand |
| `class:active $isActive` | `data-class-active="$isActive"` | Long form |

```hudl
button {
    ~ {
        .active $isSelected
        .disabled $isLoading
        .pulse $hasUpdates
    }
    Submit
}
```

### Dynamic HTML Attributes

Any attribute name in a tilde block that isn't a reserved Datastar keyword (like `text`, `show`, `on`, `let`, `.class`, `ref`, `persist`, `teleport`, `scrollIntoView`) is treated as a reactive HTML attribute.

| Hudl | Datastar HTML | Notes |
|------|---------------|-------|
| `disabled $isLoading` | `data-attr-disabled="$isLoading"` | In tilde block |
| `href $linkUrl` | `data-attr-href="$linkUrl"` | Dynamic href |

```hudl
a {
    ~ {
        href "'/user/' + $userId"
        target "$openInNew ? '_blank' : '_self'"
    }
    "View Profile"
}

button {
    ~ {
        disabled "$isLoading || !$isValid"
    }
    Submit
}
```

### data-persist (Signal Persistence)

Persists signals to localStorage/sessionStorage.

| Hudl | Datastar HTML | Notes |
|------|---------------|-------|
| `persist` | `data-persist` | Persist all signals |
| `persist "theme,lang"` | `data-persist="theme,lang"` | Specific signals |

**Modifiers:**

- `~session` - Use sessionStorage instead of localStorage

```hudl
div {
    ~ {
        persist~session userPrefs
    }
}
```

### data-ref (Element References)

Creates a reference to the element accessible as a signal.

| Hudl | Datastar HTML | Notes |
|------|---------------|-------|
| `ref myInput` | `data-ref="myInput"` | In tilde block |

```hudl
input {
    ~ {
        ref emailInput
    }
}
// Access via $emailInput in expressions
```

### data-intersects (Intersection Observer)

Triggers when element enters/exits viewport.

| Hudl | Datastar HTML | Notes |
|------|---------------|-------|
| `on:intersect "$visible = true"` | `data-on-intersect="$visible = true"` | In tilde block |

**Modifiers:**

- `~once` - Only trigger once
- `~half` - 50% visibility threshold
- `~full` - 100% visibility threshold

```hudl
div {
    ~ {
        on:intersect~once "@get('/lazy-content')"
    }
}
```

### data-teleport (DOM Teleportation)

Moves element to another location in DOM.

| Hudl | Datastar HTML | Notes |
|------|---------------|-------|
| `teleport "#modal-container"` | `data-teleport="#modal-container"` | CSS selector |
| `teleport~prepend "#target"` | `data-teleport__prepend="#target"` | Prepend mode |
| `teleport~append "#target"` | `data-teleport__append="#target"` | Append mode |

### data-scroll-into-view (Scroll Control)

Scrolls element into view.

| Hudl | Datastar HTML | Notes |
|------|---------------|-------|
| `scrollIntoView` | `data-scroll-into-view` | Default behavior |
| `scrollIntoView~smooth` | `data-scroll-into-view__smooth` | Smooth scroll |
| `scrollIntoView~instant` | `data-scroll-into-view__instant` | Instant scroll |

**Position modifiers:** `~hstart`, `~hcenter`, `~hend`, `~vstart`, `~vcenter`, `~vend`

```hudl
div {
    ~ {
        scrollIntoView~smooth~vcenter
    }
}
```

## Actions (@ Prefix)

Actions are special functions available in expressions, always prefixed with `@`. This matches Datastar's syntax exactly.

### HTTP Actions

```hudl
button {
    ~ {
        on:click "@get('/api/data')"
    }
    "Fetch Data"
}
// Other methods: @post, @put, @patch, @delete
```

**Action Modifiers for HTTP:**

| Modifier | Effect |
|----------|--------|
| `__header.X-Custom=value` | Custom header |
| `__question.key=value` | Query parameter |
| `__body.field=value` | Body field override |

### Signal Actions

| Action | Hudl Usage | Effect |
|--------|------------|--------|
| `@setAll(pattern, value)` | `"@setAll('form.*', '')"` | Set multiple signals |
| `@toggleAll(pattern)` | `"@toggleAll('checkbox.*')"` | Toggle multiple |
| `@fit(signal, arr)` | `"@fit($selected, items)"` | Clamp to array values |
| `@peek(signal)` | `"@peek($loading)"` | Read without subscription |

### DOM Actions

| Action | Hudl Usage | Effect |
|--------|------------|--------|
| `@clipboard(text)` | `"@clipboard($shareUrl)"` | Copy to clipboard |

### Action Error Handling

HTTP actions trigger `datastar-fetch` events during the request lifecycle. Handle these with `on:fetch`:

| Event Type | Description |
|------------|-------------|
| `started` | Fetch request started |
| `finished` | Fetch request finished |
| `error` | Fetch encountered an error |
| `retrying` | Fetch is retrying |
| `retries-failed` | All retries have failed |

```hudl
div {
    ~ {
        on:click "@get('/api/data')"
        on:fetch "evt.detail.type == 'error' && handleError(evt)"
    }
}
```

## Complete Example

```hudl
/**
message TodoItem {
    string id = 1;
    string text = 2;
    bool completed = 3;
}
message State {
    repeated TodoItem todos = 1;
    string newTodo = 2;
    string filter = 3;
}
*/
// data: State

div {
    ~ {
        let:newTodo ""
        let:filter all
        let:filteredTodos "$filter == 'all' ? $todos : $todos.filter(t => $filter == 'completed' ? t.completed : !t.completed)"
    }

    h1 ~text="Todo App"

    form ~on:submit~prevent="@post('/todos', {text: $newTodo}); $newTodo = ''" {
        input~>newTodo placeholder="Add todo..."
        button type=submit Add
    }

    div.filters {
        each filterOption `["all", "active", "completed"]` {
            button {
                ~ {
                    on:click "$filter = '`filterOption`'"
                    .active "$filter == '`filterOption`'"
                }
                span `filterOption`
            }
        }
    }

    ul {
        // TODO: Think about how signals in backend expressions work.
        // Signal values ARE sent back to the server, so `$filteredTodos`
        // might work here. Need to consider:
        // - Type validation: how do we ensure the signal is iterable?
        // - Schema alignment: does the signal type match TodoItem[]?
        each todo `$filteredTodos` {
            li {
                ~ {
                    .completed $todo.completed
                    on:click "$todo.completed = !$todo.completed"
                }
                span ~text=$todo.text
                button ~on:click~stop="@delete('/todos/' + $todo.id)" x
            }
        }
    }
}
```

## Modifier Syntax Summary

Modifiers are appended with `~` and can be chained:

```text
~modifier              // Boolean modifier
~modifier:value        // Value modifier
~modifier.subkey       // Dotted modifier (for keys, headers)
~modifier:value~next   // Chained modifiers
```

Examples:

```hudl
on:click~once~prevent            // Multiple boolean modifiers
on:keydown.enter.shift           // Key modifiers use dots
on:input~debounce:300ms          // Timed modifier
```

Modifier order is generally not significant - they are combined into the final attribute regardless of order.

## Custom Data Attributes

Non-Datastar `data-*` attributes (for testing, analytics, third-party libraries, etc.) use regular attribute syntax outside the tilde block:

```hudl
button data-testid=submit-btn data-track=cta-click {
    ~ {
        on:click "@post('/submit')"
    }
    Submit
}
```

The tilde system is exclusively for Datastar reactivity. All other attributes use standard Hudl attribute syntax.

## Component Composability

Tilde attributes work with components. The tilde block is a child node that belongs to its parent element. When used with a component, tilde attributes apply to the component's **single root element**.

**Component definition:**
```hudl
// name: Button
el {
    button.btn { slot }
}
```

**Usage with inline tilde attributes:**
```hudl
Button ~on:click="handleSubmit()" Submit
```

**Usage with tilde block:**
```hudl
Button {
    ~ {
        on:click "handleSubmit()"
        .loading $isLoading
    }
    Submit
}
```

Both forms are equivalent. The `on:click` and `.loading` attributes are applied to the component's root `<button>` element.

**Constraint:** Components must have a single root element. This ensures tilde attributes have an unambiguous target.

## Out of Scope for v1

The following features are explicitly out of scope for the initial implementation:

### View Transitions

View Transitions API support (`data-view-transition`) is deferred to a future version. This includes both element-level transition names and page-level transitions.

### Server-Sent Events (SSE)

SSE is a transport mechanism for delivering HTML fragments that Datastar will morph into the DOM. This is handled entirely by Datastar's client-side runtime and the server's SSE implementation. Hudl's responsibility is only to generate valid HTML with Datastar attributes - the SSE transport layer is outside Hudl's scope.

### Proto Schema Validation for Signals

Signals defined via `let:` are frontend-only reactive state. They are separate from the backend data passed to view functions via proto messages. Type validation of signal values against proto schema definitions is not supported - these are different concerns (frontend reactivity vs backend data).

### Reactive Array Iteration

The interaction between Hudl's `each` loop and Datastar's reactive signals is not defined in v1. Server-rendered `each` loops iterate at render time, while Datastar reactivity happens at runtime in the browser.

## Implementation Phases

### Phase 1: Core Syntax

- [ ] Tilde block parsing as child node (`{ ~ { ... } }`)
- [ ] Inline tilde attribute parsing (`~on:click=value`, `~disabled=expr`)
- [ ] Basic attribute generation (on, show, text, class, HTML attrs)
- [ ] Signal/computed detection (static vs expression)
- [ ] Modifier parsing and chaining
- [ ] Formatter: combine multiple tilde blocks, position as first child

### Phase 2: Bindings

- [ ] `~>` binding shorthand parsing
- [ ] `~bind:` explicit form
- [ ] Formatter normalization to shorthand
- [ ] Binding modifiers (debounce, throttle)

### Phase 3: Actions

- [ ] HTTP actions (@get, @post, @put, @patch, @delete)
- [ ] Signal actions (@setAll, @toggleAll, @fit, @peek)
- [ ] DOM actions (@clipboard)
- [ ] Action modifier parsing

### Phase 4: Advanced Features

- [ ] Intersection observer (on:intersect)
- [ ] Teleport
- [ ] Persist
- [ ] Scroll into view
- [ ] Element refs

### Phase 5: Tooling Integration

- [ ] LSP support for signal name completion
- [ ] LSP support for action completion
- [ ] Diagnostics for invalid tilde attributes
- [ ] Tree-sitter grammar updates
- [ ] Syntax highlighting differentiation for tilde blocks
