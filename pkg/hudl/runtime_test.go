package hudl

import (
	"context"
	"os"
	"testing"
)

func TestRuntime_Render(t *testing.T) {
	// Skip if views.wasm doesn't exist (e.g. CI environments without rustc)
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

	// Call 'Simple' view (generated from examples/simple.hu.kdl)
	// Output should contain "Hello from WASM"
	output, err := rt.Render("Simple", nil)
	if err != nil {
		t.Fatalf("Render failed: %v", err)
	}

	expected := "Hello from WASM"
	if !contains(output, expected) {
		t.Errorf("Expected output to contain %q, got %q", expected, output)
	}
}

func contains(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || find(s, substr))
}

func find(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
