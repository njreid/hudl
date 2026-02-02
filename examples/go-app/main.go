package main

import (
	"context"
	"fmt"
	"log"
	"os"

	"github.com/tetratelabs/wazero"
	"github.com/tetratelabs/wazero/imports/wasi_snapshot_preview1"
)

func main() {
	// 1. Initialize Wazero Runtime
	ctx := context.Background()
	r := wazero.NewRuntime(ctx)
	defer r.Close(ctx)

	// Enable WASI (needed for I/O if the WASM uses it)
	wasi_snapshot_preview1.MustInstantiate(ctx, r)

	// 2. Load the compiled views.wasm
	// (Assume 'views.wasm' exists in the current directory, compiled by hudlc)
	wasmBytes, err := os.ReadFile("../views.wasm")
	if err != nil {
		log.Fatalf("Failed to read views.wasm: %v", err)
	}

	mod, err := r.Instantiate(ctx, wasmBytes)
	if err != nil {
		log.Fatalf("Failed to instantiate module: %v", err)
	}

	// 3. Render a View
	// Each template is exported as a function, e.g., "Layout"
	// We need to pass parameters. The ABI for passing complex data (JSON)
	// typically involves writing to memory and passing pointer/length.
	// For this example, we assume a simplified ABI or just call a no-arg function
	// or use a helper from the 'hudl-go' runtime library (mocked here).

	fmt.Println("Rendering 'Layout'...")

	// Mock interaction: calling the exported function directly
	// Real implementation would use the Hudl Runtime wrapper to handle memory.
	renderFunc := mod.ExportedFunction("Layout")
	if renderFunc == nil {
		log.Fatal("Function 'Layout' not found in WASM")
	}

	// Call the function (simplified)
	results, err := renderFunc.Call(ctx)
	if err != nil {
		log.Fatalf("Call failed: %v", err)
	}

	fmt.Printf("Render result (ptr/len or status): %v\n", results)
}
