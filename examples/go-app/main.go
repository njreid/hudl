// Example Go web application using Hudl templates.
//
// Build the templates first (from project root):
//
//	cargo run --bin hudlc -- examples -o examples/go-app/views.wasm
//
// Then run this app:
//
//	cd examples/go-app && go run .
package main

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"log"
	"net/http"
	"os"

	"github.com/njreid/hudl/examples/go-app/mockdata"
	"github.com/njreid/hudl/pkg/hudl"
)

// App holds the application state including the Hudl runtime.
type App struct {
	views *hudl.Runtime
}

func main() {
	// Load the compiled WASM views
	wasmBytes, err := os.ReadFile("views.wasm")
	if err != nil {
		fmt.Println("Error: views.wasm not found.")
		fmt.Println("")
		fmt.Println("To build the templates, run from the project root:")
		fmt.Println("  cargo run --bin hudlc -- examples -o examples/go-app/views.wasm")
		fmt.Println("")
		fmt.Println("Then run this app again:")
		fmt.Println("  cd examples/go-app && go run .")
		os.Exit(1)
	}

	ctx := context.Background()
	runtime, err := hudl.NewRuntime(ctx, hudl.Options{WASMBytes: wasmBytes})
	if err != nil {
		log.Fatalf("Failed to initialize Hudl runtime: %v", err)
	}
	defer runtime.Close()

	app := &App{views: runtime}

	// Routes
	http.HandleFunc("/", app.handleHome)
	http.HandleFunc("/dashboard", app.handleDashboard)
	http.HandleFunc("/register", app.handleRegister)
	http.HandleFunc("/features", app.handleFeatures)

	addr := ":8080"
	log.Printf("Starting server at http://localhost%s", addr)
	log.Printf("Routes:")
	log.Printf("  GET /           - Home page with features")
	log.Printf("  GET /dashboard  - Admin dashboard")
	log.Printf("  GET /register   - Registration form")
	log.Printf("  GET /features   - Features marketing page")
	log.Fatal(http.ListenAndServe(addr, nil))
}

// handleHome renders the home page.
func (app *App) handleHome(w http.ResponseWriter, r *http.Request) {
	if r.URL.Path != "/" {
		http.NotFound(w, r)
		return
	}

	// Render the features section as content
	features := mockdata.GetFeatures()
	featuresHTML, err := app.views.Render("FeatureList", features)
	if err != nil {
		http.Error(w, fmt.Sprintf("Failed to render features: %v", err), 500)
		return
	}

	// Render the layout with the features as content
	layoutData := mockdata.GetLayoutData("Welcome to Hudl", featuresHTML, true)
	html, err := app.views.Render("AppLayout", layoutData)
	if err != nil {
		http.Error(w, fmt.Sprintf("Failed to render layout: %v", err), 500)
		return
	}

	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.Write([]byte(html))
}

// handleDashboard renders the admin dashboard.
func (app *App) handleDashboard(w http.ResponseWriter, r *http.Request) {
	dashData := mockdata.GetDashboardData()

	dashboardHTML, err := app.views.Render("Dashboard", dashData)
	if err != nil {
		http.Error(w, fmt.Sprintf("Failed to render dashboard: %v", err), 500)
		return
	}

	layoutData := mockdata.GetLayoutData("Dashboard - Hudl App", dashboardHTML, true)
	html, err := app.views.Render("AppLayout", layoutData)
	if err != nil {
		http.Error(w, fmt.Sprintf("Failed to render layout: %v", err), 500)
		return
	}

	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.Write([]byte(html))
}

// handleRegister renders the registration form.
func (app *App) handleRegister(w http.ResponseWriter, r *http.Request) {
	csrfToken := generateCSRFToken()

	formData := mockdata.GetEmptyForm(csrfToken)
	if r.Method == "POST" {
		formData = mockdata.GetFormWithErrors(csrfToken)
	}

	formHTML, err := app.views.Render("RegistrationForm", formData)
	if err != nil {
		http.Error(w, fmt.Sprintf("Failed to render form: %v", err), 500)
		return
	}

	layoutData := mockdata.GetLayoutData("Register - Hudl App", formHTML, false)
	html, err := app.views.Render("AppLayout", layoutData)
	if err != nil {
		http.Error(w, fmt.Sprintf("Failed to render layout: %v", err), 500)
		return
	}

	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.Write([]byte(html))
}

// handleFeatures renders the features marketing page.
func (app *App) handleFeatures(w http.ResponseWriter, r *http.Request) {
	features := mockdata.GetFeatures()

	featuresHTML, err := app.views.Render("FeatureList", features)
	if err != nil {
		http.Error(w, fmt.Sprintf("Failed to render features: %v", err), 500)
		return
	}

	layoutData := mockdata.GetLayoutData("Features - Hudl App", featuresHTML, false)
	html, err := app.views.Render("AppLayout", layoutData)
	if err != nil {
		http.Error(w, fmt.Sprintf("Failed to render layout: %v", err), 500)
		return
	}

	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.Write([]byte(html))
}

func generateCSRFToken() string {
	bytes := make([]byte, 32)
	rand.Read(bytes)
	return hex.EncodeToString(bytes)
}
