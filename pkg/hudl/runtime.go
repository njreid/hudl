package hudl

import (
	"context"
	"fmt"

	"github.com/fxamacker/cbor/v2"
	"github.com/tetratelabs/wazero"
	"github.com/tetratelabs/wazero/api"
	"github.com/tetratelabs/wazero/imports/wasi_snapshot_preview1"
)

type Runtime struct {
	rt     wazero.Runtime
	mod    api.Module
	ctx    context.Context
	malloc api.Function
	free   api.Function
}

func NewRuntime(ctx context.Context, wasmBytes []byte) (*Runtime, error) {
	r := wazero.NewRuntime(ctx)

	// Instantiate WASI
	wasi_snapshot_preview1.MustInstantiate(ctx, r)

	mod, err := r.Instantiate(ctx, wasmBytes)
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

func (r *Runtime) Close() error {
	return r.rt.Close(r.ctx)
}

func (r *Runtime) Render(viewName string, data any) (string, error) {
	renderFunc := r.mod.ExportedFunction(viewName)
	if renderFunc == nil {
		return "", fmt.Errorf("view function %s not found", viewName)
	}

	// 1. Serialize data to CBOR
	var params []byte
	if data != nil {
		var err error
		params, err = cbor.Marshal(data)
		if err != nil {
			return "", fmt.Errorf("failed to marshal data to CBOR: %w", err)
		}
	}

	// 2. Allocate memory for input params (CBOR)
	paramPtr := uint64(0)
	if len(params) > 0 {
		results, err := r.malloc.Call(r.ctx, uint64(len(params)))
		if err != nil {
			return "", fmt.Errorf("malloc failed: %w", err)
		}
		paramPtr = results[0]
		if !r.mod.Memory().Write(uint32(paramPtr), params) {
			return "", fmt.Errorf("failed to write params to memory")
		}
		defer r.free.Call(r.ctx, paramPtr, uint64(len(params)))
	}

	// 3. Call the view function
	results, err := renderFunc.Call(r.ctx, paramPtr, uint64(len(params)))
	if err != nil {
		return "", fmt.Errorf("render failed: %w", err)
	}

	packed := results[0]
	ptr := uint32(packed >> 32)
	size := uint32(packed)

	// 4. Read the result string from memory
	outBytes, ok := r.mod.Memory().Read(ptr, size)
	if !ok {
		return "", fmt.Errorf("failed to read result from memory at %d (size %d)", ptr, size)
	}

	// 5. Free the string memory in WASM
	defer r.free.Call(r.ctx, uint64(ptr), uint64(size))

	return string(outBytes), nil
}