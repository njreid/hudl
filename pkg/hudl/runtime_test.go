package hudl

import (
	"context"
	"os"
	"strings"
	"testing"
)

func TestRuntime_Render(t *testing.T) {
	// Skip if views.wasm doesn't exist
	wasmBytes, err := os.ReadFile("../../views.wasm")
	if err != nil {
		t.Skip("views.wasm not found, skipping runtime test")
	}

	ctx := context.Background()
	rt, err := NewRuntime(ctx, wasmBytes)
	if err != nil {
		t.Fatalf("Failed to create runtime: %v", err)
	}
	defer rt.Close()

	// Test Simple view with proto data
	// SimpleData message: { title, description, features[] }
	simpleData := map[string]any{
		"title":       "Hello from WASM",
		"description": "This is rendered via Hudl with CEL expressions.",
		"features":    []string{"Performance", "Security", "Portability"},
	}

	output, err := rt.Render("Simple", simpleData)
	if err != nil {
		t.Fatalf("Render failed: %v", err)
	}

	// Verify output contains expected content
	if !strings.Contains(output, "Hello from WASM") {
		t.Errorf("Expected output to contain 'Hello from WASM', got: %s", output)
	}
	if !strings.Contains(output, "<div") {
		t.Errorf("Expected output to contain '<div', got: %s", output)
	}
}

func TestRuntime_RenderWithNestedData(t *testing.T) {
	wasmBytes, err := os.ReadFile("../../views.wasm")
	if err != nil {
		t.Skip("views.wasm not found, skipping runtime test")
	}

	ctx := context.Background()
	rt, err := NewRuntime(ctx, wasmBytes)
	if err != nil {
		t.Fatalf("Failed to create runtime: %v", err)
	}
	defer rt.Close()

	// Test Dashboard view with complex nested data
	// DashboardData message with transactions
	dashboardData := map[string]any{
		"revenue_formatted": "$12,345.67",
		"active_users":      2847,
		"system_load":       73,
		"transactions": []map[string]any{
			{
				"id":               "TXN-001",
				"customer_name":    "Alice Johnson",
				"customer_email":   "alice@example.com",
				"amount_formatted": "$150.00",
				"status":           1, // STATUS_ACTIVE
			},
			{
				"id":               "TXN-002",
				"customer_name":    "Bob Smith",
				"customer_email":   "bob@example.com",
				"amount_formatted": "$75.00",
				"status":           2, // STATUS_PENDING
			},
		},
	}

	output, err := rt.Render("Dashboard", dashboardData)
	if err != nil {
		t.Fatalf("Dashboard render failed: %v", err)
	}

	// Verify key content
	if !strings.Contains(output, "$12,345.67") {
		t.Errorf("Expected revenue in output")
	}
	if !strings.Contains(output, "Alice Johnson") {
		t.Errorf("Expected customer name in output")
	}
}

func TestRuntime_RenderConditional(t *testing.T) {
	wasmBytes, err := os.ReadFile("../../views.wasm")
	if err != nil {
		t.Skip("views.wasm not found, skipping runtime test")
	}

	ctx := context.Background()
	rt, err := NewRuntime(ctx, wasmBytes)
	if err != nil {
		t.Fatalf("Failed to create runtime: %v", err)
	}
	defer rt.Close()

	// Test Layout with logged in user
	layoutDataLoggedIn := map[string]any{
		"title":        "Test Page",
		"user_name":    "John",
		"is_logged_in": true,
		"content":      "<p>Page content</p>",
	}

	output, err := rt.Render("AppLayout", layoutDataLoggedIn)
	if err != nil {
		t.Fatalf("Layout render failed: %v", err)
	}

	// Should show logged-in state
	if !strings.Contains(output, "Hello, John") {
		t.Errorf("Expected 'Hello, John' in output for logged in user")
	}
	if !strings.Contains(output, "Logout") {
		t.Errorf("Expected 'Logout' link in output for logged in user")
	}

	// Test with logged out user
	layoutDataLoggedOut := map[string]any{
		"title":        "Test Page",
		"user_name":    "",
		"is_logged_in": false,
		"content":      "<p>Page content</p>",
	}

	output, err = rt.Render("AppLayout", layoutDataLoggedOut)
	if err != nil {
		t.Fatalf("Layout render failed: %v", err)
	}

	// Should show logged-out state
	if !strings.Contains(output, "Login") {
		t.Errorf("Expected 'Login' link in output for logged out user")
	}
}

func TestRuntime_RenderIteration(t *testing.T) {
	wasmBytes, err := os.ReadFile("../../views.wasm")
	if err != nil {
		t.Skip("views.wasm not found, skipping runtime test")
	}

	ctx := context.Background()
	rt, err := NewRuntime(ctx, wasmBytes)
	if err != nil {
		t.Fatalf("Failed to create runtime: %v", err)
	}
	defer rt.Close()

	// Test FeatureList with features array
	featureListData := map[string]any{
		"features": []map[string]any{
			{
				"icon":        "🚀",
				"title":       "Fast",
				"description": "Lightning fast performance",
				"link_url":    "/docs/speed",
			},
			{
				"icon":        "🔒",
				"title":       "Secure",
				"description": "Security by default",
				"link_url":    "",
			},
		},
	}

	output, err := rt.Render("FeatureList", featureListData)
	if err != nil {
		t.Fatalf("FeatureList render failed: %v", err)
	}

	// Verify iteration worked
	if !strings.Contains(output, "Fast") {
		t.Errorf("Expected 'Fast' feature in output")
	}
	if !strings.Contains(output, "Secure") {
		t.Errorf("Expected 'Secure' feature in output")
	}
	// Check conditional link (only first feature has link_url)
	if !strings.Contains(output, "/docs/speed") {
		t.Errorf("Expected link URL in output for first feature")
	}
}

func TestRuntime_RenderForm(t *testing.T) {
	wasmBytes, err := os.ReadFile("../../views.wasm")
	if err != nil {
		t.Skip("views.wasm not found, skipping runtime test")
	}

	ctx := context.Background()
	rt, err := NewRuntime(ctx, wasmBytes)
	if err != nil {
		t.Fatalf("Failed to create runtime: %v", err)
	}
	defer rt.Close()

	// Test RegistrationForm with validation errors
	formData := map[string]any{
		"username":       "john",
		"email":          "invalid-email",
		"terms_accepted": false,
		"csrf_token":     "abc123",
		"username_error": "Username must be at least 4 characters",
		"email_error":    "Please enter a valid email address",
		"password_error": "",
	}

	output, err := rt.Render("RegistrationForm", formData)
	if err != nil {
		t.Fatalf("RegistrationForm render failed: %v", err)
	}

	// Verify form structure
	if !strings.Contains(output, "Create Account") {
		t.Errorf("Expected form title in output")
	}
	if !strings.Contains(output, "abc123") {
		t.Errorf("Expected CSRF token in output")
	}
	// Verify error messages are shown
	if !strings.Contains(output, "Username must be at least 4 characters") {
		t.Errorf("Expected username error in output")
	}
	if !strings.Contains(output, "Please enter a valid email address") {
		t.Errorf("Expected email error in output")
	}
}

func TestRuntime_NoData(t *testing.T) {
	wasmBytes, err := os.ReadFile("../../views.wasm")
	if err != nil {
		t.Skip("views.wasm not found, skipping runtime test")
	}

	ctx := context.Background()
	rt, err := NewRuntime(ctx, wasmBytes)
	if err != nil {
		t.Fatalf("Failed to create runtime: %v", err)
	}
	defer rt.Close()

	// Test rendering with nil data (for components that don't need data)
	output, err := rt.Render("Simple", nil)
	if err != nil {
		// This might fail if Simple requires data - that's expected
		t.Logf("Render with nil data returned error (expected if component requires data): %v", err)
		return
	}

	// If it succeeds, output should still be valid HTML
	if !strings.Contains(output, "<") {
		t.Errorf("Expected HTML output even with nil data")
	}
}
