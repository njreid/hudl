package hudl

import (
	"testing"
)

func TestPreParse(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{
			name:     "CSS Properties",
			input:    `width 100px; height 50%;`,
			expected: `width _100px; height _50%;`,
		},
		{
			name:     "CSS Keyframes",
			input:    `0% { opacity 0 } 100% { opacity 1 }`,
			expected: `_0% { opacity 0 } _100% { opacity 1 }`,
		},
		{
			name:     "Condensed If-Else",
			input:    `if "cond" { div } else { span }`,
			expected: "if \"cond\" { div }\nelse { span }",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := PreParse(tt.input)
			if got != tt.expected {
				t.Errorf("PreParse() = %v, want %v", got, tt.expected)
			}
		})
	}
}
