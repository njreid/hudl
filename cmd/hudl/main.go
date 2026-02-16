package main

import (
	"bufio"
	"flag"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

const LayoutTemplate = `// name: AppLayout
// param: string title "Hudl Project"

el {
    html lang=en {
        head {
            meta charset=utf-8
            title ` + "`" + `title` + "`" + `
            _stylesheet "/style.css"
            _script "/datastar.js" type=module
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

// name: HomePage
// param: string title "Home"
// param: string description "Welcome to your new Hudl app!"

el {
    AppLayout title=` + "`" + `title` + "`" + ` {
        div {
            h2 ` + "`" + `title` + "`" + `
            p ` + "`" + `description` + "`" + `

            section {
                style {
                    margin-top "2rem"
                    padding "1rem"
                    background "#eee"
                    border-radius "8px"
                }
                h3 "Server-Sent Events Clock"
                // Datastar connection to /events
                div ~init="@get('/events')" {
                    span "Current Time: "
                    span#clock "Connecting..."
                }
            }
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
	"time"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
	"github.com/njreid/hudl/pkg/hudl"
	"MOD_NAME/views"
	"github.com/starfederation/datastar-go/datastar"
)

func main() {
	r := chi.NewRouter()
	r.Use(middleware.Logger)
	r.Use(middleware.Recoverer)

	// --- Hudl Runtime Initialization ---
	rt := hudl.MustNewRuntime(context.Background())
	defer rt.Close()

	// Initialize views wrapper
	v := views.NewViews(rt)

	// --- Static Asset Serving ---
	// Serve files from the ./public directory at the root path.
	// e.g., ./public/style.css is served at /style.css
	workDir, _ := os.Getwd()
	filesDir := http.Dir(filepath.Join(workDir, "public"))
	FileServer(r, "/", filesDir)

	// --- Routes ---
	r.Get("/", func(w http.ResponseWriter, r *http.Request) {
		// Render the top-level component using the generated wrapper
		html, err := v.HomePage("Home", "Welcome to your new Hudl app!")
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}

		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		w.Write([]byte(html))
	})

	// --- Datastar SSE Events ---
	r.Get("/events", func(w http.ResponseWriter, r *http.Request) {
		sse := datastar.NewSSE(w, r)
		ticker := time.NewTicker(time.Second)
		defer ticker.Stop()

		for {
			select {
			case <-r.Context().Done():
				return
			case <-ticker.C:
				currentTime := time.Now().Format("15:04:05")
				// Push element update to #clock
				sse.PatchElements(fmt.Sprintf("<span id=\"clock\">%s</span>", currentTime))
			}
		}
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
		fmt.Fprintf(os.Stderr, "  init [name] Initialize a new Hudl-enabled Go project\n")
		fmt.Fprintf(os.Stderr, "  dev       Run the project in development mode (hot-reload)\n")
		fmt.Fprintf(os.Stderr, "  build     Build the project (compile templates to WASM)\n")
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
		runInit(flag.Arg(1))
	case "dev":
		runDev()
	case "build":
		runBuild()
	case "generate":
		runGenerate()
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

func runBuild() {
	fmt.Println("Building Hudl templates...")

	// Check if views directory exists
	if _, err := os.Stat("views"); os.IsNotExist(err) {
		fmt.Println("Error: 'views' directory not found. Are you in the project root?")
		os.Exit(1)
	}

	cmd := exec.Command("hudlc", "views", "-o", "views.wasm")
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	if err := cmd.Run(); err != nil {
		fmt.Printf("Error: failed to run hudlc: %v\n", err)
		fmt.Println("Make sure hudlc is installed and in your PATH.")
		os.Exit(1)
	}
	fmt.Println("Success: views.wasm generated.")
}

func runGenerate() {
	fmt.Println("Generating Go wrappers...")

	// Check if views directory exists
	if _, err := os.Stat("views"); os.IsNotExist(err) {
		fmt.Println("Error: 'views' directory not found. Are you in the project root?")
		os.Exit(1)
	}

	// Assuming default options for now:
	// - views dir: views
	// - output: views/views.go
	// - package: views
	// - pb import: github.com/njreid/hudl/pkg/hudl/pb (need to make this configurable or detect from go.mod?)
	// Actually, for now let's assume the user has a pb package relative to the current module.
	
	// Try to detect module name
	modName := detectModuleName()
	pbImport := ""
	if modName != "" {
		// HACK: For the default scaffold, we know the pb is in the library
		// For user projects, they might define their own. 
		// We'll need a better way to configure this later.
		pbImport = "github.com/njreid/hudl/pkg/hudl/pb"
	}

	args := []string{"generate-go", "views",
		"-o", "views/views.go",
		"--package", "views",
		"--pb-package", "pb",
	}
	if pbImport != "" {
		args = append(args, "--pb-import", pbImport)
	}

	cmd := exec.Command("hudlc", args...)
	
	// If we have a pb import, use it. But for the generated scaffold, the pb is in `pkg/hudl/pb` inside the library?
	// No, the generated scaffold uses `github.com/njreid/hudl/pkg/hudl/pb` for `SimpleData`.
	// So we should pass that.
	// But `SimpleData` is defined in `pkg/hudl/pb`.
	// For user-defined protos, they might be elsewhere.
	
	// Let's pass what we know for the default scaffold.
	// Actually, let's just run it. If message types are simple names like `SimpleData`, generated code will use `*pb.SimpleData`.
	// We need `pb` to be imported.
	
	// HACK: For the default scaffold, we know the import.
	// For general usage, we might need a config file (hudl.json/toml) later.
	// For now, let's rely on manual flags if run directly, or sensible defaults.
	// Since we can't easily guess, let's omit the import flag and let the user fix the imports if needed?
	// Or try to guess.
	
	// Let's add a flag support to `hudl generate` later. For now, just run basic generation.
	
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	if err := cmd.Run(); err != nil {
		fmt.Printf("Error: failed to run hudlc: %v\n", err)
		os.Exit(1)
	}
}

func detectModuleName() string {
	if data, err := os.ReadFile("go.mod"); err == nil {
		for _, line := range strings.Split(string(data), "\n") {
			if strings.HasPrefix(line, "module ") {
				return strings.TrimSpace(strings.TrimPrefix(line, "module "))
			}
		}
	}
	return ""
}

func runDev() {
	// 0. Generate Go wrappers first
	runGenerate()

	fmt.Println("Starting Hudl development server...")

	// 1. Try to start LSP dev server in background if not already running
	if !isPortOpen("localhost:9999") {
		lspCmd := exec.Command("hudl-lsp", "--dev-server")
		// We don't pipe stdout to avoid clutter, but pipe stderr for errors
		lspCmd.Stderr = os.Stderr
		if err := lspCmd.Start(); err != nil {
			fmt.Printf("Note: could not start hudl-lsp automatically: %v\n", err)
			fmt.Println("If you already have hudl-lsp running in your editor, this is fine.")
		} else {
			// Small delay to let it start
			time.Sleep(500 * time.Millisecond)
			fmt.Println("  Started hudl-lsp dev-server (port 9999)")
			defer lspCmd.Process.Kill()
		}
	} else {
		fmt.Println("  hudl-lsp dev-server already running on port 9999")
	}

	// 2. Run Go app with HUDL_DEV=1
	fmt.Println("Starting Go application...")

	// Check if we should run main.go or .
	var goArgs []string
	if _, err := os.Stat("main.go"); err == nil {
		goArgs = []string{"run", "main.go"}
	} else {
		goArgs = []string{"run", "."}
	}

	goCmd := exec.Command("go", goArgs...)
	goCmd.Env = append(os.Environ(), "HUDL_DEV=1")
	goCmd.Stdout = os.Stdout
	goCmd.Stderr = os.Stderr

	if err := goCmd.Run(); err != nil {
		fmt.Printf("\nGo application exited: %v\n", err)
	}
}

func isPortOpen(addr string) bool {
	conn, err := net.DialTimeout("tcp", addr, 100*time.Millisecond)
	if err != nil {
		return false
	}
	conn.Close()
	return true
}

func runInit(name string) {
	if name == "" {
		reader := bufio.NewReader(os.Stdin)
		fmt.Print("Project name: ")
		name, _ = reader.ReadString('\n')
		name = strings.TrimSpace(name)
	}

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
		// Replace module name placeholder
		content = strings.ReplaceAll(content, "MOD_NAME", name)
		if err := os.WriteFile(filepath.Join(name, path), []byte(content), 0644); err != nil {
			fmt.Printf("Error writing %s: %v\n", path, err)
			os.Exit(1)
		}
	}

	// 5. Download datastar.js
	fmt.Println("Downloading datastar.js...")
	datastarURL := "https://cdn.jsdelivr.net/gh/starfederation/datastar@1.0.0-RC.7/bundles/datastar.js"
	if err := downloadFile(datastarURL, filepath.Join(name, "public/datastar.js")); err != nil {
		fmt.Printf("Warning: failed to download datastar.js: %v\n", err)
		fmt.Println("You may need to download it manually and place it in the public/ directory.")
	}

	// 6. Fetch dependencies
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
		"github.com/starfederation/datastar-go",
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

	// 6. Go mod tidy
	fmt.Println("Tidying go.mod...")
	cmdTidy := exec.Command("go", "mod", "tidy")
	cmdTidy.Dir = name
	if out, err := cmdTidy.CombinedOutput(); err != nil {
		fmt.Printf("Error running go mod tidy: %v\nOutput: %s\n", err, string(out))
	}

	fmt.Printf("\nSuccess! Project '%s' initialized.\n", name)
	fmt.Printf("To get started:\n\n")
	fmt.Printf("  cd %s\n", name)
	fmt.Printf("  hudl dev\n")
}

func downloadFile(url string, filepath string) error {
	const timeout = 10 * time.Second
	client := &http.Client{
		Timeout: timeout,
	}
	resp, err := client.Get(url)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("bad status: %s", resp.Status)
	}

	out, err := os.Create(filepath)
	if err != nil {
		return err
	}
	defer out.Close()

	_, err = io.Copy(out, resp.Body)
	return err
}