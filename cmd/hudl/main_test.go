package main

import (
	"bytes"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestCLI_Version(t *testing.T) {
	// Build the CLI binary
	tmpDir, err := os.MkdirTemp("", "hudl-cli-test")
	require.NoError(t, err)
	defer os.RemoveAll(tmpDir)

	binPath := filepath.Join(tmpDir, "hudl")
	cmd := exec.Command("go", "build", "-o", binPath, ".")
	// We need to run build in the cmd/hudl directory
	cmd.Dir = "." 
	err = cmd.Run()
	require.NoError(t, err)

	// Run 'hudl version'
	cmd = exec.Command(binPath, "version")
	var out bytes.Buffer
	cmd.Stdout = &out
	err = cmd.Run()
	require.NoError(t, err)

	assert.Contains(t, out.String(), "hudl version 0.1.0")
}

func TestCLI_Init(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping init test in short mode")
	}

	tmpDir, err := os.MkdirTemp("", "hudl-init-test")
	require.NoError(t, err)
	defer os.RemoveAll(tmpDir)

	binPath := filepath.Join(tmpDir, "hudl")
	cmd := exec.Command("go", "build", "-o", binPath, ".")
	err = cmd.Run()
	require.NoError(t, err)

	// Run 'hudl init'
	projectName := "test-app"
	cmd = exec.Command(binPath, "init")
	cmd.Dir = tmpDir
	
	// Provide project name via stdin
	cmd.Stdin = strings.NewReader(projectName + "\n")
	
	err = cmd.Run()
	require.NoError(t, err)

	// Verify structure
	projectPath := filepath.Join(tmpDir, projectName)
	assert.DirExists(t, projectPath)
	assert.FileExists(t, filepath.Join(projectPath, "go.mod"))
	assert.FileExists(t, filepath.Join(projectPath, "main.go"))
	assert.FileExists(t, filepath.Join(projectPath, "views/layout.hudl"))
	assert.FileExists(t, filepath.Join(projectPath, "views/index.hudl"))
	assert.FileExists(t, filepath.Join(projectPath, "public/style.css"))

	// Verify main.go content
	content, err := os.ReadFile(filepath.Join(projectPath, "main.go"))
	require.NoError(t, err)
	assert.Contains(t, string(content), "github.com/go-chi/chi/v5")
	assert.Contains(t, string(content), "github.com/njr/hudl/pkg/hudl")
}