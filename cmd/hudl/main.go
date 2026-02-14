package main

import (
	"bufio"
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
)

const LayoutTemplate = `/**
message LayoutData {
    string title = 1;
}
*/
// name: AppLayout
// data: LayoutData

el {
    html lang=en {
        head {
            meta charset=utf-8
            title ` + "`" + `title` + "`" + `
            _stylesheet "/style.css"
        }
        body {
            header { h1 "Hudl Project" }
            main { #content }
            footer { p "Built with Hudl" }
        }
    }
}
`

const IndexTemplate = `import {
    "./layout"
}

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
            p ` + "`" + `message` + "`" + `
        }
    }
}
`

const MainGoTemplate = `package main

import (
	"context"
	"fmt"
	"log"
	"net/http"
	"os"
	"path/filepath"
	"strings"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
	"github.com/njreid/hudl/pkg/hudl"
	"github.com/njreid/hudl/pkg/hudl/pb"
)

func main() {
	r := chi.NewRouter()
	r.Use(middleware.Logger)
	r.Use(middleware.Recoverer)

	// --- Hudl Runtime Initialization ---
	// Automatically switches between Dev/Prod modes based on HUDL_DEV environment variable.
	// In Dev Mode: renders via HTTP to the LSP sidecar (hot-reload).
	// In Prod Mode: renders via embedded views.wasm (high performance).
	var wasmBytes []byte
	if os.Getenv("HUDL_DEV") == "" {
		var err error
		wasmBytes, err = os.ReadFile("views.wasm")
		if err != nil {
			log.Printf("Warning: views.wasm not found, prod mode will fail to render")
		}
	}

	rt, err := hudl.NewRuntime(context.Background(), hudl.Options{
		WASMBytes: wasmBytes,
	})
	if err != nil {
		log.Fatalf("Failed to initialize Hudl runtime: %v", err)
	}
	defer rt.Close()

	// --- Static Asset Serving ---
	// Serve files from the ./public directory at the root path.
	// e.g., ./public/style.css is served at /style.css
	workDir, _ := os.Getwd()
	filesDir := http.Dir(filepath.Join(workDir, "public"))
	FileServer(r, "/", filesDir)

	// --- Routes ---
	r.Get("/", func(w http.ResponseWriter, r *http.Request) {
		// 1. Prepare data for the page (using generated proto bindings)
		// For this scaffold, we use IndexData message.
		data := &pb.IndexData{
			Message: "Welcome to your new Hudl app!",
		}

		// 2. Render the top-level component
		html, err := rt.Render("HomePage", data)
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}

		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		w.Write([]byte(html))
	})

	port := ":8080"
	fmt.Printf("Server starting on http://localhost%s\n", port)
	log.Fatal(http.ListenAndServe(port, r))
}

// FileServer conveniently sets up a http.FileServer handler to serve
// static files from a http.FileSystem.
func FileServer(r chi.Router, path string, root http.FileSystem) {
	if strings.ContainsAny(path, "{}*") {
		panic("FileServer does not permit any URL parameters.")
	}

	if path != "/" && path[len(path)-1] != '/' {
		r.Get(path, http.RedirectHandler(path+"/", 301).ServeHTTP)
		path += "/"
	}
	path += "*"

	r.Get(path, func(w http.ResponseWriter, r *http.Request) {
		rctx := chi.RouteContext(r.Context())
		pathPrefix := strings.TrimSuffix(rctx.RoutePattern(), "/*")
		fs := http.StripPrefix(pathPrefix, http.FileServer(root))
		fs.ServeHTTP(w, r)
	})
}
`

const StylesTemplate = `body {
    font-family: system-ui, -apple-system, sans-serif;
    line-height: 1.5;
    max-width: 800px;
    margin: 0 auto;
    padding: 2rem;
    background: #f4f4f9;
}

header {
    border-bottom: 2px solid #ddd;
    margin-bottom: 2rem;
}

footer {
    margin-top: 4rem;
    color: #666;
    font-size: 0.8rem;
    border-top: 1px solid #eee;
    padding-top: 1rem;
}
`

func main() {
	flag.Usage = func() {
		fmt.Fprintf(os.Stderr, "Usage: hudl <command> [options]\n\n")
		fmt.Fprintf(os.Stderr, "Commands:\n")
		fmt.Fprintf(os.Stderr, "  install   Download and install hudlc and hudl-lsp binaries\n")
		fmt.Fprintf(os.Stderr, "  init      Initialize a new Hudl-enabled Go project\n")
		fmt.Fprintf(os.Stderr, "  version   Show version information\n")
		fmt.Fprintf(os.Stderr, "\nOptions:\n")
		flag.PrintDefaults()
	}

	flag.Parse()

	if flag.NArg() < 1 {
		flag.Usage()
		os.Exit(1)
	}

	command := flag.Arg(0)

	switch command {
	case "install":
		runInstall()
	case "init":
		runInit()
	case "version":
		fmt.Println("hudl version 0.1.0")
	default:
		fmt.Printf("Unknown command: %s\n", command)
		flag.Usage()
		os.Exit(1)
	}
}

func runInstall() {
	fmt.Println("Installing Hudl toolchain...")
	// TODO: Implement binary management
	fmt.Println("Pre-compiled binaries are not yet available on GitHub Releases.")
	fmt.Println("Please build from source for now:")
	fmt.Println("  make build")
}

func runInit() {
	reader := bufio.NewReader(os.Stdin)
	fmt.Print("Project name: ")
	name, _ := reader.ReadString('\n')
	name = strings.TrimSpace(name)

	if name == "" {
		fmt.Println("Error: project name is required")
		os.Exit(1)
	}

	fmt.Printf("Initializing project '%s'...\n", name)

	// 1. Create directory
	if err := os.Mkdir(name, 0755); err != nil {
		fmt.Printf("Error creating directory: %v\n", err)
		os.Exit(1)
	}

	// 2. Go mod init
	cmd := exec.Command("go", "mod", "init", name)
	cmd.Dir = name
	if err := cmd.Run(); err != nil {
		fmt.Printf("Error running go mod init: %v\n", err)
		os.Exit(1)
	}

	// 3. Create structure
	os.Mkdir(filepath.Join(name, "views"), 0755)
	os.Mkdir(filepath.Join(name, "public"), 0755)

	// 4. Write files
	files := map[string]string{
		"views/layout.hudl": LayoutTemplate,
		"views/index.hudl":  IndexTemplate,
		"public/style.css":  StylesTemplate,
		"main.go":           MainGoTemplate,
	}

	for path, content := range files {
		if err := os.WriteFile(filepath.Join(name, path), []byte(content), 0644); err != nil {
			fmt.Printf("Error writing %s: %v\n", path, err)
			os.Exit(1)
		}
	}

	// 5. Fetch dependencies
	fmt.Println("Fetching dependencies...")
	
	// Determine if we should use a local replace for development
	// If we are inside the hudl repo, we want to point to it locally
	absPath, _ := filepath.Abs(".")
	if strings.Contains(absPath, "github.com/njreid/hudl") || strings.Contains(absPath, "code/hudl") {
		fmt.Println("  Detecting local development environment, adding replace directive...")
		// Try to find the root of the repo
		repoRoot := absPath
		for i := 0; i < 5; i++ {
			if _, err := os.Stat(filepath.Join(repoRoot, "go.mod")); err == nil {
				// Verify it's OUR go.mod
				if content, _ := os.ReadFile(filepath.Join(repoRoot, "go.mod")); strings.Contains(string(content), "module github.com/njreid/hudl") {
					cmd := exec.Command("go", "mod", "edit", "-replace", "github.com/njreid/hudl="+repoRoot)
					cmd.Dir = name
					cmd.Run()
					break
				}
			}
			repoRoot = filepath.Dir(repoRoot)
		}
	}

	deps := []string{
		"github.com/go-chi/chi/v5",
		"github.com/njreid/hudl",
	}
	for _, dep := range deps {
		fmt.Printf("  get %s...\n", dep)
		cmd := exec.Command("go", "get", dep)
		cmd.Dir = name
		// Redirect output to avoid cluttering but show errors
		if out, err := cmd.CombinedOutput(); err != nil {
			fmt.Printf("Error fetching dependency %s: %v\nOutput: %s\n", dep, err, string(out))
		}
	}

	fmt.Printf("\nSuccess! Project '%s' initialized.\n", name)
	fmt.Printf("To get started:\n\n")
	fmt.Printf("  cd %s\n", name)
	fmt.Printf("  # For development with hot-reload:\n")
	fmt.Printf("  HUDL_DEV=1 go run main.go\n")
}