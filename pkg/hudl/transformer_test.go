package hudl

import (
	"reflect"
	"testing"
)

func TestTransform(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected Element // We check the first root element usually
	}{
		{
			name:  "Basic Div",
			input: "div",
			expected: Element{
				Tag:        "div",
				Classes:    []string{},
				Attributes: map[string]string{},
			},
		},
		{
			name:  "ID Shorthand",
			input: "&header",
			expected: Element{
				Tag:        "div", // Implicit div
				ID:         "header",
				Classes:    []string{},
				Attributes: map[string]string{},
			},
		},
		{
			name:  "Class Shorthand",
			input: ".btn",
			expected: Element{
				Tag:        "div", // Implicit div
				Classes:    []string{"btn"},
				Attributes: map[string]string{},
			},
		},
		{
			name:  "Explicit Tag with ID and Classes",
			input: "span&main-title.text-bold.red",
			expected: Element{
				Tag:        "span",
				ID:         "main-title",
				Classes:    []string{"text-bold", "red"},
				Attributes: map[string]string{},
			},
		},
		{
			name:  "Attributes and Text",
			input: `a href="/home" "Go Home"`,
			expected: Element{
				Tag:        "a",
				Classes:    []string{},
				Attributes: map[string]string{"href": "/home"},
				Children: []Node{
					Text{Content: "Go Home"},
				},
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			// 1. Parse (using our real parser + regex)
			doc, err := Parse(tt.input)
			if err != nil {
				t.Fatalf("Parse error: %v", err)
			}

			// 2. Transform
			root, err := Transform(doc)
			if err != nil {
				t.Fatalf("Transform error: %v", err)
			}

			if len(root.Nodes) == 0 {
				t.Fatal("Result has no nodes")
			}
			
			// We assume the test input produces exactly one element as root (or we check the first one)
			got, ok := root.Nodes[0].(Element)
			if !ok {
				t.Fatalf("Expected Element, got %T", root.Nodes[0])
			}

			// Validate
			if got.Tag != tt.expected.Tag {
				t.Errorf("Tag: got %q, want %q", got.Tag, tt.expected.Tag)
			}
			if got.ID != tt.expected.ID {
				t.Errorf("ID: got %q, want %q", got.ID, tt.expected.ID)
			}
			if !reflect.DeepEqual(got.Classes, tt.expected.Classes) {
				t.Errorf("Classes: got %v, want %v", got.Classes, tt.expected.Classes)
			}
			if len(got.Attributes) != len(tt.expected.Attributes) {
				t.Errorf("Attr len: got %d, want %d", len(got.Attributes), len(tt.expected.Attributes))
			}
			for k, v := range tt.expected.Attributes {
				if got.Attributes[k] != v {
					t.Errorf("Attr[%s]: got %q, want %q", k, got.Attributes[k], v)
				}
			}
			// Check children text if expected
			if len(tt.expected.Children) > 0 {
				if len(got.Children) != len(tt.expected.Children) {
					t.Errorf("Children len: got %d, want %d", len(got.Children), len(tt.expected.Children))
				} else {
					// Assume Text node for simple check
					wantText := tt.expected.Children[0].(Text).Content
					gotText := got.Children[0].(Text).Content
					if gotText != wantText {
						t.Errorf("Text content: got %q, want %q", gotText, wantText)
					}
				}
			}
		})
	}
}
