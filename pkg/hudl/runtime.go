package hudl

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"time"

	"github.com/tetratelabs/wazero"
	"github.com/tetratelabs/wazero/api"
	"github.com/tetratelabs/wazero/imports/wasi_snapshot_preview1"
	"google.golang.org/protobuf/proto"
)

// Options configures the Hudl runtime.
type Options struct {
	// DevMode enables rendering via the LSP dev server instead of WASM.
	// If false, it will still check the HUDL_DEV environment variable.
	DevMode bool
	// DevServerAddr is the address of the LSP dev server (default: localhost:9999).
	// If empty, it will check the HUDL_DEV_ADDR environment variable.
	DevServerAddr string
	// WASMBytes is the compiled WASM module data (required in prod mode).
	WASMBytes []byte
	// HttpClient is used for dev mode requests (optional).
	HttpClient *http.Client
}

// Runtime renders Hudl templates.
type Runtime struct {
	// WASM runtime (prod mode)
	rt     wazero.Runtime
	mod    api.Module
	ctx    context.Context
	malloc api.Function
	free   api.Function

	// Dev mode
	devMode bool
	devAddr string
	client  *http.Client
}

// NewRuntime creates a new Hudl runtime with the given options.
func NewRuntime(ctx context.Context, opts Options) (*Runtime, error) {
	devMode := opts.DevMode
	if !devMode {
		if v := os.Getenv("HUDL_DEV"); v == "1" || v == "true" {
			devMode = true
		}
	}

	devAddr := opts.DevServerAddr
	if devAddr == "" {
		devAddr = os.Getenv("HUDL_DEV_ADDR")
		if devAddr == "" {
			devAddr = "localhost:9999"
		}
	}

	if devMode {
		client := opts.HttpClient
		if client == nil {
			client = &http.Client{
				Timeout: 5 * time.Second,
			}
		}
		return &Runtime{
			ctx:     ctx,
			devMode: true,
			devAddr: devAddr,
			client:  client,
		}, nil
	}

	// Prod mode: initialize WASM
	if opts.WASMBytes == nil {
		return nil, fmt.Errorf("wasmBytes required in prod mode (set HUDL_DEV=1 for dev mode)")
	}

	r := wazero.NewRuntime(ctx)
	wasi_snapshot_preview1.MustInstantiate(ctx, r)

	mod, err := r.Instantiate(ctx, opts.WASMBytes)
	if err != nil {
		r.Close(ctx)
		return nil, fmt.Errorf("failed to instantiate module: %w", err)
	}

	malloc := mod.ExportedFunction("hudl_malloc")
	free := mod.ExportedFunction("hudl_free")

	if malloc == nil || free == nil {
		r.Close(ctx)
		return nil, fmt.Errorf("missing required exports: hudl_malloc or hudl_free")
	}

	return &Runtime{
		rt:     r,
		mod:    mod,
		ctx:    ctx,
		malloc: malloc,
		free:   free,
	}, nil
}

// NewRuntimeFromWASM is a helper to create a prod-mode runtime from WASM bytes.
func NewRuntimeFromWASM(ctx context.Context, wasmBytes []byte) (*Runtime, error) {
	return NewRuntime(ctx, Options{WASMBytes: wasmBytes})
}

func (r *Runtime) Close() error {
	if r.rt != nil {
		return r.rt.Close(r.ctx)
	}
	return nil
}

// Render renders a view with the given proto message data.
func (r *Runtime) Render(viewName string, data proto.Message) (string, error) {
	var params []byte
	if data != nil {
		var err error
		params, err = proto.Marshal(data)
		if err != nil {
			return "", fmt.Errorf("failed to marshal data to proto: %w", err)
		}
	}

	if r.devMode {
		return r.renderDev(viewName, params)
	}
	return r.renderWASM(viewName, params)
}

// RenderBytes renders a view with raw proto wire format bytes.
func (r *Runtime) RenderBytes(viewName string, protoBytes []byte) (string, error) {
	if r.devMode {
		return r.renderDev(viewName, protoBytes)
	}
	return r.renderWASM(viewName, protoBytes)
}

func (r *Runtime) renderDev(viewName string, protoBytes []byte) (string, error) {
	url := fmt.Sprintf("http://%s/render", r.devAddr)

	req, err := http.NewRequestWithContext(r.ctx, "POST", url, bytes.NewReader(protoBytes))
	if err != nil {
		return "", fmt.Errorf("dev mode: failed to create request: %w", err)
	}
	req.Header.Set("X-Hudl-Component", viewName)
	req.Header.Set("Content-Type", "application/x-protobuf")

	resp, err := r.client.Do(req)
	if err != nil {
		return "", fmt.Errorf("dev mode: request to LSP failed (is hudl-lsp --dev-server running?): %w", err)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return "", fmt.Errorf("dev mode: failed to read response: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		var errResp struct {
			Error string `json:"error"`
		}
		if json.Unmarshal(body, &errResp) == nil && errResp.Error != "" {
			return "", fmt.Errorf("dev mode: render error: %s", errResp.Error)
		}
		return "", fmt.Errorf("dev mode: render failed with status %d: %s", resp.StatusCode, string(body))
	}

	return string(body), nil
}

func (r *Runtime) renderWASM(viewName string, protoBytes []byte) (string, error) {
	renderFunc := r.mod.ExportedFunction(viewName)
	if renderFunc == nil {
		return "", fmt.Errorf("view function %s not found", viewName)
	}

	paramPtr := uint64(0)
	if len(protoBytes) > 0 {
		results, err := r.malloc.Call(r.ctx, uint64(len(protoBytes)))
		if err != nil {
			return "", fmt.Errorf("malloc failed: %w", err)
		}
		paramPtr = results[0]
		if !r.mod.Memory().Write(uint32(paramPtr), protoBytes) {
			return "", fmt.Errorf("failed to write params to memory")
		}
		defer r.free.Call(r.ctx, paramPtr, uint64(len(protoBytes)))
	}

	results, err := renderFunc.Call(r.ctx, paramPtr, uint64(len(protoBytes)))
	if err != nil {
		return "", fmt.Errorf("render failed: %w", err)
	}

	packed := results[0]
	ptr := uint32(packed >> 32)
	size := uint32(packed)

	outBytes, ok := r.mod.Memory().Read(ptr, size)
	if !ok {
		return "", fmt.Errorf("failed to read result from memory at %d (size %d)", ptr, size)
	}

	defer r.free.Call(r.ctx, uint64(ptr), uint64(size))

	return string(outBytes), nil
}