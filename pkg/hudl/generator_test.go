package hudl

import (
	"strings"
	"testing"
)

func TestGenerateGo(t *testing.T) {
	root := &Root{
		Nodes: []Node{
			Element{
				Tag: "div",
				ID: "main",
				Classes: []string{"container"},
				Children: []Node{
					Element{
						Tag: "h1",
						Children: []Node{
							Text{Content: "Hello World"},
						},
					},
				},
			},
		},
	}

	code, err := GenerateGo(root, "views", "RenderMain")
	if err != nil {
		t.Fatalf("GenerateGo failed: %v", err)
	}

	// Basic assertions on generated code
	if !strings.Contains(code, "package views") {
		t.Error("Missing package declaration")
	}
	if !strings.Contains(code, "func RenderMain(w io.Writer) error") {
		t.Error("Missing function signature")
	}
	if !strings.Contains(code, `io.WriteString(w, "<div")`) {
		t.Error("Missing div tag write")
	}
	if !strings.Contains(code, `id=\"main\"`) {
		t.Error("Missing id attribute")
	}
	if !strings.Contains(code, `class=\"container\"`) {
		t.Error("Missing class attribute")
	}
	if !strings.Contains(code, `Hello World`) {
		t.Error("Missing text content")
	}
}
