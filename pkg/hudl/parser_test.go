package hudl

import (
	"testing"
)

func TestParse(t *testing.T) {
	input := `
el {
	div "Hello"
	&main {
		p "Content"
	}
	css {
		.btn { width 10px; }
	}
}
`
	doc, err := Parse(input)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	if len(doc.Nodes) == 0 {
		t.Errorf("Expected nodes, got 0")
	}

	// Basic check: root node should be 'el'
	if doc.Nodes[0].Name() != "el" {
		t.Errorf("Expected root node 'el', got '%s'", doc.Nodes[0].Name())
	}
}
