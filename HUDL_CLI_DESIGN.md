# Hudl CLI Design

This document describes the design for the `hudl` CLI tool, a Go-based utility designed to manage the Hudl toolchain and scaffold new projects.

## Overview

The `hudl` CLI provides a unified entry point for developers. It handles the installation of the Rust-based components (`hudlc`, `hudl-lsp`) and provides a streamlined "Getting Started" experience via project scaffolding.

## 1. Installation and Distribution

The CLI will be a standard Go package installable via:

```bash
go install github.com/njreid/hudl/cmd/hudl@latest
```

### Binary Management Strategy

Since `hudlc` and `hudl-lsp` are written in Rust, the Go CLI will act as a package manager for these binaries:

1. **Release Artifacts**: Pre-compiled binaries for supported platforms (Linux, macOS, Windows; x86_64 and ARM64) will be hosted on GitHub Releases.
2. **Detection**: On first run or via a specific `install` command, the CLI will detect the user's Operating System and Architecture.
3. **Download**: The CLI will download the appropriate artifacts and place them in a standard location (e.g., `$GOPATH/bin` or `~/.hudl/bin`).
4. **Verification**: The CLI will verify checksums to ensure binary integrity.

## 2. CLI Commands

### `hudl install`

Downloads and installs the latest versions of `hudlc` and `hudl-lsp`.

### `hudl init`

Initializes a new Hudl-enabled Go web project in the current directory.

**Workflow:**

1. **Prompt**: Ask the user for the project name (e.g., "my-hudl-app").
2. **Go Module**: Run `go mod init <project-name>`.
3. **Dependencies**: Fetch `github.com/go-chi/chi/v5` and `github.com/njreid/hudl/pkg/hudl`.
4. **Structure**:
    *   `main.go`: The web server entry point.
    *   `views/`: Directory for `.hudl` templates.
    *   `views/layout.hudl`: A base layout component.
    *   `views/index.hudl`: A home page component using the layout.
    *   `public/`: Directory for static assets (CSS, JS, images).
    *   `Makefile` (optional): Commands for building WASM and running the server.

## 3. Project Scaffold Templates

### `views/layout.hudl`

```kdl
/**
message LayoutData {
    string title = 1;
    string content = 2; // HTML slot
}
*/
// name: AppLayout
// data: LayoutData

el {
    html lang=en {
        head {
            meta charset=utf-8
            title `title`
            _stylesheet "/style.css"
        }
        body {
            header { h1 "Hudl Project" }
            main "`raw(content)`"
            footer { p "Built with Hudl" }
        }
    }
}
```

### `views/index.hudl`

```kdl
/**
message IndexData {
    string message = 1;
}
*/
// name: HomePage
// data: IndexData

el {
    AppLayout title="Home" {
        div {
            h2 "Welcome!"
            p `message`
        }
    }
}
```

### `main.go`

The generated `main.go` will include:

*   Standard `chi` router setup.

*   Static file serving for the `./public` directory, mapped to the root `/` (e.g., `./public/style.css` is served at `/style.css`).

*   Hudl runtime initialization (automatically switching between Dev/Prod modes).

*   A handler for the root path that renders `HomePage`.

## 4. Environment Integration

The CLI will promote the use of:

* `HUDL_DEV=1`: For hot-reloading templates via the LSP sidecar.
* `views.wasm`: For production performance.

## 5. Implementation Details (Golang)

* **Prompting**: Use `fmt.Scanln` or a library like `survey` for interactive prompts.
* **Templates**: Use `embed` to package the scaffold files within the `hudl` binary.
* **Process Management**: The CLI can also provide a `hudl dev` command that wraps `go run main.go` and `hudl-lsp --dev-server` into a single process.
