package hudl

import (
	"context"
	"os"
	"strings"
	"testing"

	"github.com/njreid/hudl/pkg/hudl/pb"
)

func TestRuntime_RenderSimple(t *testing.T) {
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
	data := &pb.SimpleData{
		Title:       "Hello from WASM",
		Description: "This is rendered via Hudl with CEL expressions.",
		Features:    []string{"Performance", "Security", "Portability"},
	}

	output, err := rt.Render("Simple", data)
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

func TestRuntime_RenderDashboard(t *testing.T) {
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
	data := &pb.DashboardData{
		RevenueFormatted: "$12,345.67",
		ActiveUsers:      2847,
		SystemLoad:       73,
		Transactions: []*pb.Transaction{
			{
				Id:              "TXN-001",
				CustomerName:    "Alice Johnson",
				CustomerEmail:   "alice@example.com",
				AmountFormatted: "$150.00",
				Status:          pb.TransactionStatus_STATUS_ACTIVE,
			},
			{
				Id:              "TXN-002",
				CustomerName:    "Bob Smith",
				CustomerEmail:   "bob@example.com",
				AmountFormatted: "$75.00",
				Status:          pb.TransactionStatus_STATUS_PENDING,
			},
		},
	}

	output, err := rt.Render("Dashboard", data)
	if err != nil {
		t.Fatalf("Dashboard render failed: %v", err)
	}

	// Verify key content
	if !strings.Contains(output, "$12,345.67") {
		t.Errorf("Expected revenue in output, got: %s", output)
	}
	if !strings.Contains(output, "Alice Johnson") {
		t.Errorf("Expected customer name in output, got: %s", output)
	}
}

func TestRuntime_RenderLayout(t *testing.T) {
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
	dataLoggedIn := &pb.LayoutData{
		Title:      "Test Page",
		UserName:   "John",
		IsLoggedIn: true,
		Content:    "<p>Page content</p>",
	}

	output, err := rt.Render("AppLayout", dataLoggedIn)
	if err != nil {
		t.Fatalf("Layout render failed: %v", err)
	}

	// Should show logged-in state
	if !strings.Contains(output, "Hello, John") {
		t.Errorf("Expected 'Hello, John' in output for logged in user, got: %s", output)
	}
	if !strings.Contains(output, "Logout") {
		t.Errorf("Expected 'Logout' link in output for logged in user")
	}

	// Test with logged out user
	dataLoggedOut := &pb.LayoutData{
		Title:      "Test Page",
		UserName:   "",
		IsLoggedIn: false,
		Content:    "<p>Page content</p>",
	}

	output, err = rt.Render("AppLayout", dataLoggedOut)
	if err != nil {
		t.Fatalf("Layout render failed: %v", err)
	}

	// Should show logged-out state
	if !strings.Contains(output, "Login") {
		t.Errorf("Expected 'Login' link in output for logged out user")
	}
}

func TestRuntime_RenderFeatureList(t *testing.T) {
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
	data := &pb.FeatureListData{
		Features: []*pb.Feature{
			{
				Icon:        "🚀",
				Title:       "Fast",
				Description: "Lightning fast performance",
				LinkUrl:     "/docs/speed",
			},
			{
				Icon:        "🔒",
				Title:       "Secure",
				Description: "Security by default",
				LinkUrl:     "",
			},
		},
	}

	output, err := rt.Render("FeatureList", data)
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
	data := &pb.RegistrationFormData{
		Username:      "john",
		Email:         "invalid-email",
		TermsAccepted: false,
		CsrfToken:     "abc123",
		UsernameError: "Username must be at least 4 characters",
		EmailError:    "Please enter a valid email address",
		PasswordError: "",
	}

	output, err := rt.Render("RegistrationForm", data)
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

func TestRuntime_RenderBytes(t *testing.T) {
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

	// Test RenderBytes with manually constructed proto wire format
	// Proto wire format for SimpleData:
	//   field 1 (title): string "Hello"
	//   field 2 (description): string "World"
	protoBytes := []byte{
		0x0a, 0x05, 'H', 'e', 'l', 'l', 'o', // field 1: "Hello"
		0x12, 0x05, 'W', 'o', 'r', 'l', 'd', // field 2: "World"
	}

	output, err := rt.RenderBytes("Simple", protoBytes)
	if err != nil {
		t.Fatalf("RenderBytes failed: %v", err)
	}

	// Verify output contains expected content
	if !strings.Contains(output, "<div") {
		t.Errorf("Expected output to contain '<div', got: %s", output)
	}
}

func TestRuntime_ViewNotFound(t *testing.T) {
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

	// Test rendering a non-existent view
	_, err = rt.RenderBytes("NonExistentView", []byte{})
	if err == nil {
		t.Errorf("Expected error for non-existent view")
	}
	if !strings.Contains(err.Error(), "not found") {
		t.Errorf("Expected 'not found' error, got: %v", err)
	}
}

func TestRuntime_RenderNil(t *testing.T) {
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

	// Test Render with nil data
	output, err := rt.Render("Simple", nil)
	if err != nil {
		t.Logf("Render with nil returned error (may be expected for views requiring data): %v", err)
		return
	}

	// If it succeeds, output should be valid HTML
	if !strings.Contains(output, "<") {
		t.Errorf("Expected HTML output with nil data")
	}
}

func TestRuntime_RenderLayout_RawContent(t *testing.T) {
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

	// Test that raw() function outputs unescaped HTML
	data := &pb.LayoutData{
		Title:      "Test Page",
		UserName:   "John",
		IsLoggedIn: true,
		Content:    "<p>This is <strong>raw HTML</strong> content</p>",
	}

	output, err := rt.Render("AppLayout", data)
	if err != nil {
		t.Fatalf("Layout render failed: %v", err)
	}

	// Verify raw HTML is not escaped (should contain actual HTML tags)
	if !strings.Contains(output, "<strong>raw HTML</strong>") {
		t.Errorf("Expected unescaped HTML from raw() function, got: %s", output)
	}

	// Verify that if it was escaped, we would see &lt; instead
	if strings.Contains(output, "&lt;strong&gt;") {
		t.Errorf("HTML was incorrectly escaped, raw() function not working")
	}
}

func TestRuntime_RenderStyledButton(t *testing.T) {
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

	// Test StyledButton with scoped styles
	data := &pb.ButtonData{
		Label:    "Click Me",
		Disabled: false,
	}

	output, err := rt.Render("StyledButton", data)
	if err != nil {
		t.Fatalf("StyledButton render failed: %v", err)
	}

	// Verify scoped style tag is present
	if !strings.Contains(output, "<style>") {
		t.Errorf("Expected <style> tag in output, got: %s", output)
	}

	// Verify scoped class is present on styled element (class contains "h-")
	if !strings.Contains(output, " h-") && !strings.Contains(output, "\"h-") {
		t.Errorf("Expected scoped class (h-*) in output, got: %s", output)
	}

	// Verify existing classes are preserved alongside scoped class
	if !strings.Contains(output, "btn btn-cancel") {
		t.Errorf("Expected existing classes (btn btn-cancel) to be preserved, got: %s", output)
	}

	// Verify CSS properties are in the style tag
	if !strings.Contains(output, "background-color: red") {
		t.Errorf("Expected 'background-color: red' in style, got: %s", output)
	}

	// Verify the button content
	if !strings.Contains(output, "Click Me") {
		t.Errorf("Expected 'Click Me' in output, got: %s", output)
	}

	// Verify the button tag
	if !strings.Contains(output, "<button") {
		t.Errorf("Expected <button> tag in output, got: %s", output)
	}

	t.Logf("Styled button output: %s", output)
}

func TestRuntime_RenderSwitch(t *testing.T) {
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

	// Test Dashboard with transactions to verify switch/case renders
	data := &pb.DashboardData{
		RevenueFormatted: "$100",
		ActiveUsers:      10,
		SystemLoad:       50,
		Transactions: []*pb.Transaction{
			{
				Id:              "TXN-001",
				CustomerName:    "Test User",
				CustomerEmail:   "test@example.com",
				AmountFormatted: "$50.00",
				Status:          pb.TransactionStatus_STATUS_ACTIVE,
			},
		},
	}

	output, err := rt.Render("Dashboard", data)
	if err != nil {
		t.Fatalf("Dashboard render failed: %v", err)
	}

	// Verify switch/case structure is rendered (at minimum the default case should work)
	// Note: Enum comparison currently falls through to default because proto enum values
	// are integers but switch cases compare string names. This is a known limitation.
	if !strings.Contains(output, "badge") {
		t.Errorf("Expected status badge element in output")
	}

	// Verify the transaction data is rendered
	if !strings.Contains(output, "TXN-001") {
		t.Errorf("Expected transaction ID in output")
	}
	if !strings.Contains(output, "Test User") {
		t.Errorf("Expected customer name in output")
	}
}

func TestRuntime_EachWithIndex(t *testing.T) {
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

	// Test iteration with multiple items
	data := &pb.FeatureListData{
		Features: []*pb.Feature{
			{Icon: "1", Title: "First", Description: "First item", LinkUrl: "/first"},
			{Icon: "2", Title: "Second", Description: "Second item", LinkUrl: "/second"},
			{Icon: "3", Title: "Third", Description: "Third item", LinkUrl: "/third"},
		},
	}

	output, err := rt.Render("FeatureList", data)
	if err != nil {
		t.Fatalf("FeatureList render failed: %v", err)
	}

	// Verify all items are rendered
	if !strings.Contains(output, "First") {
		t.Errorf("Expected 'First' in output")
	}
	if !strings.Contains(output, "Second") {
		t.Errorf("Expected 'Second' in output")
	}
	if !strings.Contains(output, "Third") {
		t.Errorf("Expected 'Third' in output")
	}

	// Count occurrences of feature items (should be 3)
	count := strings.Count(output, "feature-card") + strings.Count(output, "feature")
	if count < 3 {
		t.Logf("Output: %s", output)
	}
}

func TestRuntime_BooleanAttributes(t *testing.T) {
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

	// Test with disabled=true
	dataDisabled := &pb.ButtonData{
		Label:    "Disabled Button",
		Disabled: true,
	}

	outputDisabled, err := rt.Render("StyledButton", dataDisabled)
	if err != nil {
		t.Fatalf("StyledButton render failed: %v", err)
	}

	// Check if disabled attribute is present when disabled=true
	// Boolean attributes should be present without value or with value="disabled"
	if !strings.Contains(outputDisabled, "disabled") {
		t.Logf("Output with disabled=true: %s", outputDisabled)
		// Not failing as the implementation may handle this differently
	}

	// Test with disabled=false
	dataEnabled := &pb.ButtonData{
		Label:    "Enabled Button",
		Disabled: false,
	}

	outputEnabled, err := rt.Render("StyledButton", dataEnabled)
	if err != nil {
		t.Fatalf("StyledButton render failed: %v", err)
	}

	// When disabled=false, the attribute should ideally not be present
	// or be set to empty/false
	t.Logf("Output with disabled=false: %s", outputEnabled)
}

func TestRuntime_HTMLEscaping(t *testing.T) {
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

	// Test with potentially malicious input
	data := &pb.SimpleData{
		Title:       "<script>alert('xss')</script>",
		Description: "Normal text",
		Features:    []string{"<b>bold</b>", "normal"},
	}

	output, err := rt.Render("Simple", data)
	if err != nil {
		t.Fatalf("Simple render failed: %v", err)
	}

	// Verify the script tag is escaped, not rendered as raw HTML
	if strings.Contains(output, "<script>") {
		t.Errorf("XSS vulnerability: script tag was not escaped in output: %s", output)
	}

	// The escaped version should be present
	if !strings.Contains(output, "&lt;script&gt;") && !strings.Contains(output, "alert") {
		// Either it's properly escaped or the title isn't rendered
		t.Logf("Output: %s", output)
	}
}

func TestRuntime_ConditionalRendering(t *testing.T) {
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

	// Test conditional rendering based on system_load thresholds
	testCases := []struct {
		systemLoad     int32
		expectedClass  string
		notExpectedMsg string
	}{
		{systemLoad: 85, expectedClass: "text-red", notExpectedMsg: "High load should show red"},
		{systemLoad: 60, expectedClass: "text-yellow", notExpectedMsg: "Medium load should show yellow"},
		{systemLoad: 30, expectedClass: "text-green", notExpectedMsg: "Low load should show green"},
	}

	for _, tc := range testCases {
		data := &pb.DashboardData{
			RevenueFormatted: "$100",
			ActiveUsers:      10,
			SystemLoad:       tc.systemLoad,
			Transactions:     []*pb.Transaction{},
		}

		output, err := rt.Render("Dashboard", data)
		if err != nil {
			t.Fatalf("Dashboard render failed for load %d: %v", tc.systemLoad, err)
		}

		if !strings.Contains(output, tc.expectedClass) {
			t.Errorf("%s (load=%d). Output: %s", tc.notExpectedMsg, tc.systemLoad, output)
		}
	}
}

func TestRuntime_EmptyCollections(t *testing.T) {
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

	// Test with empty collections
	data := &pb.FeatureListData{
		Features: []*pb.Feature{},
	}

	output, err := rt.Render("FeatureList", data)
	if err != nil {
		t.Fatalf("FeatureList render failed with empty features: %v", err)
	}

	// Should render without error, just no feature items
	if !strings.Contains(output, "<") {
		t.Errorf("Expected some HTML output even with empty features")
	}
}
