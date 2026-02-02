package hudl

import (
	"strings"
	"testing"
)

// CompilerTestCase defines a single test scenario for the hudl compiler.
type CompilerTestCase struct {
	Name           string
	InputKDL       string
	ExpectedGoCode string // Partial match or key signature check
	ExpectedHTML   string
}

func TestCompilerSpecs(t *testing.T) {
	cases := []CompilerTestCase{
		{
			Name: "Basic Element",
			InputKDL: "el {\n\tdiv \"Hello World\"\n}",
			ExpectedHTML: "<div>Hello World</div>",
		},
		{
			Name: "ID and Class Shorthands",
			InputKDL: "el {\n\t&main.container.fluid {\n\t\th1 \"Title\"\n\t}\n}",
			ExpectedHTML: "<div id=\"main\" class=\"container fluid\"><h1>Title</h1></div>",
		},
		{
			Name: "Attributes",
			InputKDL: "el {\n\ta href=\" /login\" target=\"_blank\" \"Login\"\n}",
			ExpectedHTML: "<a href=\" /login\" target=\"_blank\">Login</a>",
		},
		{
			Name: "Inline CSS with Number fix",
			InputKDL: "el {\n\tcss {\n\t\t.btn { padding 10px; font-size 1.2rem; }\n\t\t&header { margin 0; }\n\t}\n\t.btn \"Click Me\"\n}",
			ExpectedHTML: "<style>.btn-x9s2{padding:10px;font-size:1.2rem;}#header{margin:0;}</style><div class=\"btn-x9s2\">Click Me</div>",
		},
		{
			Name: "Control Flow - If/Else",
			InputKDL: "// param: show bool\nel {\n\tif \"`show`\" {\n\t\tp \"Visible\"\n\t} else {\n\t\tp \"Hidden\"\n\t}\n}",
			ExpectedGoCode: "if show {",
			ExpectedHTML:   "<p>Visible</p>",
		},
		{
			Name: "Control Flow - Each",
			InputKDL: "// param: items []string\nel {\n\tul {\n\t\teach item of=\" `items`\" {\n\t\t\tli \"`item`\"\n\t\t}\n\t}\n}",
			ExpectedGoCode: "for _, item := range items {",
			ExpectedHTML:   "<ul><li>A</li><li>B</li></ul>",
		},
		{
			Name:           "Control Flow - Each with Index",
			InputKDL:       "// param: items []string\nel {\n\tul {\n\t\teach i item of=\" `items`\" {\n\t\t\tli \"`i`: `item`\"\n\t\t}\n\t}\n}",
			ExpectedGoCode: "for i, item := range items {",
			ExpectedHTML:   "<ul><li>0: A</li><li>1: B</li></ul>",
		},
		{
			Name: "Control Flow - Switch",
			InputKDL: "// param: role string\nel {\n\tswitch \"`role`\" {\n\t\tcase \"admin\" { span \"Admin\" }\n\t\tdefault { span \"User\" }\n\t}\n}",
			ExpectedGoCode: "switch role {",
			ExpectedHTML:   "<span>Admin</span>",
		},
	}

	for _, tc := range cases {
		t.Run(tc.Name, func(t *testing.T) {
			preParsed := PreParse(tc.InputKDL)
			if strings.Contains(preParsed, "#") {
				t.Errorf("PreParse failed to remove # shorthand")
			}
		})
	}
}
