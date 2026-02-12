#!/usr/bin/env bash
set -euo pipefail

# Install Hudl binaries and editor configuration

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="${HOME}/bin"
HELIX_CONFIG_DIR="${HOME}/.config/helix"
HELIX_RUNTIME_DIR="${HELIX_CONFIG_DIR}/runtime"

echo "Building Hudl..."

# Build the compiler
echo "  Building hudlc..."
cargo build --release --manifest-path "${SCRIPT_DIR}/Cargo.toml"

# Build the LSP
echo "  Building hudl-lsp..."
cargo build --release --manifest-path "${SCRIPT_DIR}/lsp/Cargo.toml"

# Create install directory if needed
mkdir -p "${INSTALL_DIR}"

# Copy binaries
echo "Installing binaries to ${INSTALL_DIR}..."
cp "${SCRIPT_DIR}/target/release/hudlc" "${INSTALL_DIR}/"
cp "${SCRIPT_DIR}/lsp/target/release/hudl-lsp" "${INSTALL_DIR}/"

echo "  ${INSTALL_DIR}/hudlc"
echo "  ${INSTALL_DIR}/hudl-lsp"

# Install Helix editor configuration
echo ""
echo "Installing Helix editor configuration..."

# Create Helix directories
mkdir -p "${HELIX_CONFIG_DIR}"
mkdir -p "${HELIX_RUNTIME_DIR}/queries/hudl"

# Copy query files for syntax highlighting
cp "${SCRIPT_DIR}/editor/helix/queries/hudl/"*.scm "${HELIX_RUNTIME_DIR}/queries/hudl/"
echo "  ${HELIX_RUNTIME_DIR}/queries/hudl/highlights.scm"
echo "  ${HELIX_RUNTIME_DIR}/queries/hudl/injections.scm"

# Merge or create languages.toml
HELIX_LANG_FILE="${HELIX_CONFIG_DIR}/languages.toml"
if [[ -f "${HELIX_LANG_FILE}" ]]; then
    # Check if hudl language is configured
    if grep -q '^\[\[language\]\]' "${HELIX_LANG_FILE}" && grep -q 'name = "hudl"' "${HELIX_LANG_FILE}"; then
        echo "  Hudl language already configured in ${HELIX_LANG_FILE}"
    else
        echo "  Appending Hudl language config to ${HELIX_LANG_FILE}"
        echo "" >> "${HELIX_LANG_FILE}"
        # Append just the language and language-server sections
        grep -A 10 '^\[\[language\]\]' "${SCRIPT_DIR}/editor/helix/languages.toml" | head -12 >> "${HELIX_LANG_FILE}"
    fi

    # Check if hudl grammar is configured (required for syntax highlighting)
    if grep -q '^\[\[grammar\]\]' "${HELIX_LANG_FILE}" && grep -A 2 '^\[\[grammar\]\]' "${HELIX_LANG_FILE}" | grep -q 'name = "hudl"'; then
        echo "  Hudl grammar already configured in ${HELIX_LANG_FILE}"
    else
        echo "  Appending Hudl grammar config to ${HELIX_LANG_FILE}"
        echo "" >> "${HELIX_LANG_FILE}"
        echo '[[grammar]]' >> "${HELIX_LANG_FILE}"
        echo 'name = "hudl"' >> "${HELIX_LANG_FILE}"
        echo "source = { path = \"${SCRIPT_DIR}/tree-sitter-hudl\" }" >> "${HELIX_LANG_FILE}"
    fi
else
    echo "  Creating ${HELIX_LANG_FILE}"
    cp "${SCRIPT_DIR}/editor/helix/languages.toml" "${HELIX_LANG_FILE}"
fi

# Build tree-sitter grammar
echo ""
echo "Building tree-sitter grammar..."
if command -v tree-sitter &> /dev/null; then
    cd "${SCRIPT_DIR}/tree-sitter-hudl"
    if [[ -f "grammar.js" ]]; then
        tree-sitter generate 2>/dev/null || echo "  Note: tree-sitter generate had issues (this is OK if src/ already exists)"
    fi
    cd "${SCRIPT_DIR}"
    echo "  Grammar generated in ${SCRIPT_DIR}/tree-sitter-hudl/src/"
else
    echo "  Note: tree-sitter CLI not found. Install with: npm install -g tree-sitter-cli"
    echo "  Then run: cd tree-sitter-hudl && tree-sitter generate"
fi

# Build and install Helix grammar
echo ""
echo "Building Helix grammar..."
HELIX_GRAMMARS_DIR="${HELIX_RUNTIME_DIR}/grammars"
mkdir -p "${HELIX_GRAMMARS_DIR}"

# Compile the tree-sitter grammar to a shared library
cd "${SCRIPT_DIR}/tree-sitter-hudl"
if [[ -f "src/parser.c" ]]; then
    # Compile parser.c and scanner.c into a shared library
    GRAMMAR_SO="${HELIX_GRAMMARS_DIR}/hudl.so"

    if command -v cc &> /dev/null; then
        echo "  Compiling hudl grammar..."
        cc -shared -fPIC -fno-exceptions -O2 \
            -I src \
            src/parser.c src/scanner.c \
            -o "${GRAMMAR_SO}" 2>/dev/null

        if [[ -f "${GRAMMAR_SO}" ]]; then
            echo "  Installed ${GRAMMAR_SO}"
        else
            echo "  Warning: Failed to compile grammar"
        fi
    else
        echo "  Note: C compiler (cc) not found"
        echo "  Install gcc or clang to compile the grammar"
    fi
else
    echo "  Note: parser.c not found. Run 'tree-sitter generate' first"
fi
cd "${SCRIPT_DIR}"

echo ""
echo "Done!"

# Check if ~/bin is in PATH
if [[ ":$PATH:" != *":${INSTALL_DIR}:"* ]]; then
    echo ""
    echo "Note: ${INSTALL_DIR} is not in your PATH."
    echo "Add this to your shell config:"
    echo "  export PATH=\"\${HOME}/bin:\${PATH}\""
fi
