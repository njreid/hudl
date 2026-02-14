package hudl

import (
	"context"
	"fmt"
	"net"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestDevModeIntegration(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping integration test in short mode")
	}

	// 1. Create a temp directory with a .hudl file
	tmpDir, err := os.MkdirTemp("", "hudl-dev-test")
	require.NoError(t, err)
	defer os.RemoveAll(tmpDir)

	hudlFile := filepath.Join(tmpDir, "card.hudl")
	content := `// name: Card
el {
    div "v1 from dev mode"
}
`
	err = os.WriteFile(hudlFile, []byte(content), 0644)
	require.NoError(t, err)

	// 2. Find an available port
	ln, err := net.Listen("tcp", "localhost:0")
	require.NoError(t, err)
	port := ln.Addr().(*net.TCPAddr).Port
	ln.Close()

	addr := fmt.Sprintf("localhost:%d", port)

	// 3. Start hudl-lsp --dev-server in background
	// Find the binary - assuming it's in lsp/target/debug/hudl-lsp relative to project root
	// We are in pkg/hudl, so it's ../../lsp/target/debug/hudl-lsp
	lspPath, err := filepath.Abs("../../lsp/target/debug/hudl-lsp")
	require.NoError(t, err)

	cmd := exec.Command(lspPath, "--dev-server", "--port", fmt.Sprintf("%d", port), "--watch", tmpDir)
	// cmd.Stdout = os.Stdout
	// cmd.Stderr = os.Stderr
	err = cmd.Start()
	require.NoError(t, err)
	defer cmd.Process.Kill()

	// 4. Wait for server to be ready
	ready := false
	for i := 0; i < 50; i++ {
		conn, err := net.DialTimeout("tcp", addr, 100*time.Millisecond)
		if err == nil {
			conn.Close()
			ready = true
			break
		}
		time.Sleep(100 * time.Millisecond)
	}
	require.True(t, ready, "dev server failed to start in time")

	// 5. Use Go runtime in dev mode
	os.Setenv("HUDL_DEV", "true")
	os.Setenv("HUDL_DEV_ADDR", addr)
	defer os.Unsetenv("HUDL_DEV")
	defer os.Unsetenv("HUDL_DEV_ADDR")

	rt, err := NewRuntime(context.Background(), Options{})
	require.NoError(t, err)
	require.NotNil(t, rt)

	// 6. Render initial version
	html, err := rt.Render("Card", nil)
	require.NoError(t, err)
	assert.Contains(t, html, "v1 from dev mode")

	// 7. Update file and trigger hot-reload (simulated)
	// The dev server should detect the change via notify
	contentV2 := `// name: Card
el {
    div "v2 updated"
}
`
	err = os.WriteFile(hudlFile, []byte(contentV2), 0644)
	require.NoError(t, err)

	// Wait for reload
	time.Sleep(500 * time.Millisecond)

	html, err = rt.Render("Card", nil)
	require.NoError(t, err)
	assert.Contains(t, html, "v2 updated")
}
