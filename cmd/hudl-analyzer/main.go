// hudl-analyzer provides type analysis for Hudl templates.
// It runs as a long-lived process communicating via JSON-RPC over stdin/stdout.
package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"go/types"
	"os"
	"strings"

	"golang.org/x/tools/go/packages"
)

// JSON-RPC request/response types
type Request struct {
	JSONRPC string          `json:"jsonrpc"`
	ID      int             `json:"id"`
	Method  string          `json:"method"`
	Params  json.RawMessage `json:"params"`
}

type Response struct {
	JSONRPC string      `json:"jsonrpc"`
	ID      int         `json:"id"`
	Result  interface{} `json:"result,omitempty"`
	Error   *RPCError   `json:"error,omitempty"`
}

type RPCError struct {
	Code    int    `json:"code"`
	Message string `json:"message"`
}

// Request params
type InitializeParams struct {
	WorkspaceRoot string `json:"workspaceRoot"`
}

type ValidateExprParams struct {
	RootType   string `json:"rootType"`   // e.g., "github.com/myapp/models.User"
	Expression string `json:"expression"` // e.g., "profile.Address.City"
}

type FindImplsParams struct {
	PackagePath   string `json:"packagePath"`
	InterfaceName string `json:"interfaceName"`
}

type GetTypeInfoParams struct {
	PackagePath string `json:"packagePath"`
	TypeName    string `json:"typeName"`
}

// Response results
type InitializeResult struct {
	Initialized bool `json:"initialized"`
}

type ValidateExprResult struct {
	Valid      bool   `json:"valid"`
	ResultType string `json:"resultType,omitempty"`
	Error      string `json:"error,omitempty"`
}

type FindImplsResult struct {
	Implementations []string `json:"implementations"`
}

type TypeInfoResult struct {
	Kind    string      `json:"kind"` // "struct", "interface", "alias", "primitive"
	Fields  []FieldInfo `json:"fields,omitempty"`
	Methods []MethodInfo `json:"methods,omitempty"`
}

type FieldInfo struct {
	Name     string `json:"name"`
	Type     string `json:"type"`
	Exported bool   `json:"exported"`
}

type MethodInfo struct {
	Name      string `json:"name"`
	Signature string `json:"signature"`
}

// Analyzer holds the workspace state
type Analyzer struct {
	workspaceRoot string
	pkgCache      map[string]*packages.Package
	cfg           *packages.Config
}

func NewAnalyzer(root string) (*Analyzer, error) {
	cfg := &packages.Config{
		Mode: packages.NeedTypes | packages.NeedTypesInfo |
			packages.NeedSyntax | packages.NeedImports | packages.NeedDeps,
		Dir: root,
	}
	return &Analyzer{
		workspaceRoot: root,
		pkgCache:      make(map[string]*packages.Package),
		cfg:           cfg,
	}, nil
}

func (a *Analyzer) LoadPackage(path string) (*packages.Package, error) {
	if cached, ok := a.pkgCache[path]; ok {
		return cached, nil
	}

	pkgs, err := packages.Load(a.cfg, path)
	if err != nil {
		return nil, fmt.Errorf("failed to load package %s: %w", path, err)
	}
	if len(pkgs) == 0 {
		return nil, fmt.Errorf("package not found: %s", path)
	}
	if len(pkgs[0].Errors) > 0 {
		var errs []string
		for _, e := range pkgs[0].Errors {
			errs = append(errs, e.Error())
		}
		return nil, fmt.Errorf("package errors: %s", strings.Join(errs, "; "))
	}

	a.pkgCache[path] = pkgs[0]
	return pkgs[0], nil
}

// ResolveType resolves a fully qualified type string like "github.com/pkg.Type"
func (a *Analyzer) ResolveType(qualifiedType string) (types.Type, error) {
	// Split "github.com/pkg/path.TypeName" into package path and type name
	lastDot := strings.LastIndex(qualifiedType, ".")
	if lastDot == -1 {
		return nil, fmt.Errorf("invalid qualified type: %s (expected pkg.Type format)", qualifiedType)
	}

	pkgPath := qualifiedType[:lastDot]
	typeName := qualifiedType[lastDot+1:]

	pkg, err := a.LoadPackage(pkgPath)
	if err != nil {
		return nil, err
	}

	obj := pkg.Types.Scope().Lookup(typeName)
	if obj == nil {
		return nil, fmt.Errorf("type %s not found in package %s", typeName, pkgPath)
	}

	return obj.Type(), nil
}

// ValidateFieldPath validates a field path on a root type
func (a *Analyzer) ValidateFieldPath(rootType types.Type, path string) (types.Type, error) {
	if path == "" {
		return rootType, nil
	}

	parts := strings.Split(path, ".")
	current := rootType

	for _, part := range parts {
		// Dereference pointers automatically
		if ptr, ok := current.(*types.Pointer); ok {
			current = ptr.Elem()
		}

		// Handle named types
		if named, ok := current.(*types.Named); ok {
			current = named.Underlying()
		}

		switch t := current.(type) {
		case *types.Struct:
			found := false
			for i := 0; i < t.NumFields(); i++ {
				field := t.Field(i)
				if field.Name() == part {
					current = field.Type()
					found = true
					break
				}
			}
			if !found {
				return nil, fmt.Errorf("field %q not found on type %s", part, rootType)
			}
		default:
			return nil, fmt.Errorf("cannot access field %q on non-struct type %T", part, current)
		}
	}

	return current, nil
}

// FindInterfaceImplementations finds all types implementing an interface
func (a *Analyzer) FindInterfaceImplementations(pkgPath, ifaceName string) ([]string, error) {
	pkg, err := a.LoadPackage(pkgPath)
	if err != nil {
		return nil, err
	}

	obj := pkg.Types.Scope().Lookup(ifaceName)
	if obj == nil {
		return nil, fmt.Errorf("interface %s not found in package %s", ifaceName, pkgPath)
	}

	iface, ok := obj.Type().Underlying().(*types.Interface)
	if !ok {
		return nil, fmt.Errorf("%s is not an interface", ifaceName)
	}

	var impls []string

	// Search all cached packages for implementations
	for pkgPathKey, cachedPkg := range a.pkgCache {
		scope := cachedPkg.Types.Scope()
		for _, name := range scope.Names() {
			scopeObj := scope.Lookup(name)
			if tn, ok := scopeObj.(*types.TypeName); ok {
				t := tn.Type()
				// Check both T and *T
				if types.Implements(t, iface) {
					impls = append(impls, pkgPathKey+"."+name)
				} else if types.Implements(types.NewPointer(t), iface) {
					impls = append(impls, pkgPathKey+"."+name)
				}
			}
		}
	}

	return impls, nil
}

// GetTypeInfo returns field and method info for a type
func (a *Analyzer) GetTypeInfo(pkgPath, typeName string) (*TypeInfoResult, error) {
	pkg, err := a.LoadPackage(pkgPath)
	if err != nil {
		return nil, err
	}

	obj := pkg.Types.Scope().Lookup(typeName)
	if obj == nil {
		return nil, fmt.Errorf("type %s not found in package %s", typeName, pkgPath)
	}

	result := &TypeInfoResult{}
	t := obj.Type()

	// Get methods
	if named, ok := t.(*types.Named); ok {
		for i := 0; i < named.NumMethods(); i++ {
			m := named.Method(i)
			result.Methods = append(result.Methods, MethodInfo{
				Name:      m.Name(),
				Signature: m.Type().String(),
			})
		}
	}

	// Get underlying type info
	switch u := t.Underlying().(type) {
	case *types.Struct:
		result.Kind = "struct"
		for i := 0; i < u.NumFields(); i++ {
			f := u.Field(i)
			result.Fields = append(result.Fields, FieldInfo{
				Name:     f.Name(),
				Type:     f.Type().String(),
				Exported: f.Exported(),
			})
		}
	case *types.Interface:
		result.Kind = "interface"
	case *types.Basic:
		result.Kind = "primitive"
	default:
		result.Kind = "alias"
	}

	return result, nil
}

func main() {
	scanner := bufio.NewScanner(os.Stdin)
	// Increase buffer size for large requests
	scanner.Buffer(make([]byte, 1024*1024), 1024*1024)
	encoder := json.NewEncoder(os.Stdout)

	var analyzer *Analyzer

	for scanner.Scan() {
		var req Request
		if err := json.Unmarshal(scanner.Bytes(), &req); err != nil {
			encoder.Encode(Response{
				JSONRPC: "2.0",
				ID:      0,
				Error:   &RPCError{Code: -32700, Message: fmt.Sprintf("Parse error: %v", err)},
			})
			continue
		}

		var result interface{}
		var rpcErr *RPCError

		switch req.Method {
		case "initialize":
			var params InitializeParams
			if err := json.Unmarshal(req.Params, &params); err != nil {
				rpcErr = &RPCError{Code: -32602, Message: fmt.Sprintf("Invalid params: %v", err)}
				break
			}
			var err error
			analyzer, err = NewAnalyzer(params.WorkspaceRoot)
			if err != nil {
				rpcErr = &RPCError{Code: -32000, Message: err.Error()}
			} else {
				result = InitializeResult{Initialized: true}
			}

		case "validateExpression":
			if analyzer == nil {
				rpcErr = &RPCError{Code: -32002, Message: "Analyzer not initialized"}
				break
			}
			var params ValidateExprParams
			if err := json.Unmarshal(req.Params, &params); err != nil {
				rpcErr = &RPCError{Code: -32602, Message: fmt.Sprintf("Invalid params: %v", err)}
				break
			}
			rootType, err := analyzer.ResolveType(params.RootType)
			if err != nil {
				result = ValidateExprResult{Valid: false, Error: err.Error()}
				break
			}
			resultType, err := analyzer.ValidateFieldPath(rootType, params.Expression)
			if err != nil {
				result = ValidateExprResult{Valid: false, Error: err.Error()}
			} else {
				result = ValidateExprResult{Valid: true, ResultType: resultType.String()}
			}

		case "findImplementations":
			if analyzer == nil {
				rpcErr = &RPCError{Code: -32002, Message: "Analyzer not initialized"}
				break
			}
			var params FindImplsParams
			if err := json.Unmarshal(req.Params, &params); err != nil {
				rpcErr = &RPCError{Code: -32602, Message: fmt.Sprintf("Invalid params: %v", err)}
				break
			}
			impls, err := analyzer.FindInterfaceImplementations(params.PackagePath, params.InterfaceName)
			if err != nil {
				rpcErr = &RPCError{Code: -32000, Message: err.Error()}
			} else {
				result = FindImplsResult{Implementations: impls}
			}

		case "getTypeInfo":
			if analyzer == nil {
				rpcErr = &RPCError{Code: -32002, Message: "Analyzer not initialized"}
				break
			}
			var params GetTypeInfoParams
			if err := json.Unmarshal(req.Params, &params); err != nil {
				rpcErr = &RPCError{Code: -32602, Message: fmt.Sprintf("Invalid params: %v", err)}
				break
			}
			info, err := analyzer.GetTypeInfo(params.PackagePath, params.TypeName)
			if err != nil {
				rpcErr = &RPCError{Code: -32000, Message: err.Error()}
			} else {
				result = info
			}

		case "loadPackage":
			if analyzer == nil {
				rpcErr = &RPCError{Code: -32002, Message: "Analyzer not initialized"}
				break
			}
			var params struct {
				PackagePath string `json:"packagePath"`
			}
			if err := json.Unmarshal(req.Params, &params); err != nil {
				rpcErr = &RPCError{Code: -32602, Message: fmt.Sprintf("Invalid params: %v", err)}
				break
			}
			_, err := analyzer.LoadPackage(params.PackagePath)
			if err != nil {
				rpcErr = &RPCError{Code: -32000, Message: err.Error()}
			} else {
				result = map[string]bool{"loaded": true}
			}

		case "shutdown":
			os.Exit(0)

		default:
			rpcErr = &RPCError{Code: -32601, Message: fmt.Sprintf("Method not found: %s", req.Method)}
		}

		resp := Response{
			JSONRPC: "2.0",
			ID:      req.ID,
		}
		if rpcErr != nil {
			resp.Error = rpcErr
		} else {
			resp.Result = result
		}
		encoder.Encode(resp)
	}

	if err := scanner.Err(); err != nil {
		fmt.Fprintf(os.Stderr, "Error reading stdin: %v\n", err)
		os.Exit(1)
	}
}
