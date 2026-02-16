package main

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
		// For this scaffold, we use SimpleData message.
		data := &pb.SimpleData{
			Title:       "Home",
			Description: "Welcome to your new Hudl app!",
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
