# WASM Interoperability & Data Passing Design

To ensure high-performance data transfer between the Host (Go/Node.js) and the Guest (Hudl WASM), we need a serialization protocol that minimizes overhead and complexity.

## Selected Protocol: CBOR (Concise Binary Object Representation)

We have selected **CBOR** for the initial implementation of Hudl.

### Why CBOR?
1.  **Efficiency**: significantly more compact than JSON (binary format).
2.  **Schema-less**: Matches the dynamic nature of KDL/Hudl templates where views might accept varying data structures without rigid `.proto` definitions.
3.  **Ecosystem**: Excellent support in Rust (`serde_cbor`), Go (`fxamacker/cbor`), and Node.js (`cbor-x`).
4.  **Simplicity**: Easier to implement than Flatbuffers/Protobuf for our use case (no extra build step for schemas).

### Future Roadmap: wit-bindgen
As the WebAssembly Component Model matures, we will migrate to `wit-bindgen` for standard, high-level type interfaces.

---

## Host-Guest Interaction

### 1. Memory Layout
The Host and Guest share a linear memory buffer.
*   **Host**: Allocates memory in WASM instance. Writes CBOR bytes.
*   **Guest**: Reads CBOR bytes, deserializes to internal structs, renders HTML.
*   **Guest**: Writes HTML string to memory.
*   **Host**: Reads HTML string from memory.

### 2. Exported Functions (ABI)
Each view (e.g., `dashboard.hudl`) exports a function:

```rust
// Rust (Guest)
#[no_mangle]
pub extern "C" fn Dashboard(ptr: i32, len: i32) -> i32 {
    // 1. Read slice from memory(ptr, len)
    // 2. Deserialize CBOR -> Data Struct
    // 3. Render HTML
    // 4. Return pointer to result (packed ptr/len)
}
```

---

## Convenience Wrappers

We will generate package-specific wrappers to abstract the WASM complexity.

### Golang Wrapper (`hudl-go`)
*   **Usage**:
    ```go
    import "github.com/njreid/hudl-go"
    
    views, _ := hudl.Load("views.wasm")
    html, _ := views.Render("Dashboard", map[string]any{"User": user})
    ```
*   **Responsibilities**:
    *   Initialize `wazero`.
    *   Manage memory (malloc/free).
    *   Serialize Go structs to CBOR.
    *   Decode returned pointer/length.

### Node.js Wrapper (`hudl-node`)
*   **Usage**:
    ```javascript
    const { loadViews } = require('hudl-node');
    
    const views = await loadViews('./views.wasm');
    const html = views.render('Dashboard', { user });
    ```
*   **Responsibilities**:
    *   `WebAssembly.instantiate`.
    *   `TextEncoder`/`TextDecoder` for strings.
    *   CBOR encoding/decoding.
