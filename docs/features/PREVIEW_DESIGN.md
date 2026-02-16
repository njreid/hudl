# Hudl Component Preview Design (Simplified)

## Overview

This document outlines the simplified design for live development in Hudl. Instead of a separate complex preview UI, Hudl focuses on high-productivity "Dev Mode" where rendered pages automatically refresh when source files change.

## Architecture

### 1. LSP Dev Server

The `hudl-lsp` binary acts as a rendering sidecar during development.

*   **Endpoint:** `POST /render` accepts proto wire bytes and renders the requested component.
*   **SSE Endpoint:** `GET /events` provides a Server-Sent Events (SSE) stream for file system change notifications.
*   **File Watching:** Uses the `notify` crate to watch the project directory for `.hudl` and `.proto` changes.

### 2. Automatic Live-Reload

When in "Dev Mode" (`HUDL_DEV=1`), the LSP dev server automatically injects a small live-reload script into all rendered HTML responses.

#### Reload Script Logic

```html
<script>
  (function() {
    const ev = new EventSource('http://localhost:9999/__hudl/live_reload');
    ev.onmessage = (e) => {
      try {
        const data = JSON.parse(e.data);
        if (data.type === 'reload') {
          console.log('Hudl: File change detected, reloading...');
          location.reload();
        }
      } catch(err) {}
    };
  })();
</script>
```

The script:
1.  Connects to the `/events` SSE stream.
2.  Listens for `reload` events sent by the LSP when a file is saved.
3.  Triggers a browser `location.reload()` to show the updated template immediately.

### 3. Component Slots (#content)

To support layout composition, Hudl uses a special `#content` token.

*   **Layout Component:** Defines where child content should be placed.
    ```hudl
    // layout.hudl
    el {
        body {
            header { h1 "My App" }
            main { #content }
        }
    }
    ```
*   **Page Component:** Invokes the layout and provides children.
    ```hudl
    // index.hudl
    import { "./layout" }
    el {
        AppLayout {
            p "This text will be inserted into the #content slot."
        }
    }
    ```

## Implementation Status

*   [x] SSE `/events` endpoint in LSP.
*   [x] File watcher integration.
*   [x] `RELOAD_SCRIPT` injection in `render_handler`.
*   [x] `#content` slot support in interpreter and codegen.
*   [x] `import` syntax support for cross-file component resolution.