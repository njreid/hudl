package hudl

// NodeType identifies the type of a Hudl node.
type NodeType int

const (
	NodeElement NodeType = iota
	NodeText
	NodeControlFlow // if, each, switch
	NodeComment     // (Not strictly needed if we just drop them, but good for completeness)
	NodeImport      // Top-level imports
)

// Node represents a node in the Hudl AST.
type Node interface {
	Type() NodeType
}

// Root represents the parsed template file.
type Root struct {
	Imports []string
	Param   map[string]string // e.g. "user" -> "models.User"
	Nodes   []Node
}

// Element represents an HTML element.
type Element struct {
	Tag        string
	ID         string
	Classes    []string
	Attributes map[string]string
	Children   []Node
	IsSelfClosing bool // e.g. <img />, <input />
}

func (e Element) Type() NodeType { return NodeElement }

// Text represents a text node (string literal or expression).
type Text struct {
	Content string
	IsExpr  bool // If true, content is a Go expression inside quotes/backticks
}

func (t Text) Type() NodeType { return NodeText }

// CSSBlock represents a scoped CSS block.
// It will be compiled into a <style> tag with scoped selectors.
type CSSBlock struct {
	Rules []CSSRule
}

// CSSRule represents a single selector block inside css { ... }
type CSSRule struct {
	Selector string
	Props    map[string]string
}

// TODO: ControlFlow structures (If, Each, Switch) will be added in Phase 3.
// For Phase 1, we will focus on Elements and Text.
