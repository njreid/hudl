package hudl

import (
	"fmt"
	"strings"

	"github.com/calico32/kdl-go"
)

// Transform converts a raw KDL document into a semantic Hudl AST.
func Transform(doc *kdl.Document) (*Root, error) {
	root := &Root{
		Param: make(map[string]string),
	}

	for _, node := range doc.Nodes {
		// Handle top-level constructs
		switch node.Name() {
		case "import":
			// TODO: Handle imports in Phase 1.5
			continue
		case "el":
			// The main template body.
			children, err := transformNodes(node.Children().Nodes)
			if err != nil {
				return nil, err
			}
			root.Nodes = append(root.Nodes, children...)
		default:
			// Allow loose mode for testing elements directly
			n, err := transformNode(node)
			if err != nil {
				return nil, err
			}
			root.Nodes = append(root.Nodes, n)
		}
	}

	return root, nil
}

func transformNodes(nodes []*kdl.Node) ([]Node, error) {
	var result []Node
	for _, n := range nodes {
		transformed, err := transformNode(n)
		if err != nil {
			return nil, err
		}
		result = append(result, transformed)
	}
	return result, nil
}

func transformNode(n *kdl.Node) (Node, error) {
	name := n.Name()

	// 1. Check for Shorthands &id and .class
	tag := "div"
	id := ""
	classes := []string{}
	
	remaining := name
	if !strings.HasPrefix(name, "&") && !strings.HasPrefix(name, ".") {
		end := strings.IndexAny(name, "&.")
		if end == -1 {
			tag = name
			remaining = ""
		} else {
			tag = name[:end]
			remaining = name[end:]
		}
	}
	
	for len(remaining) > 0 {
		if strings.HasPrefix(remaining, "&") {
			remaining = remaining[1:]
			end := strings.IndexAny(remaining, ".")
			if end == -1 {
				id = remaining
				remaining = ""
			} else {
				id = remaining[:end]
				remaining = remaining[end:]
			}
		} else if strings.HasPrefix(remaining, ".") {
			remaining = remaining[1:]
			end := strings.IndexAny(remaining, "&.")
			if end == -1 {
				classes = append(classes, remaining)
				remaining = ""
			} else {
				classes = append(classes, remaining[:end])
				remaining = remaining[end:]
			}
		} else {
			break
		}
	}

	// 2. Extract Attributes (Properties)
	attrs := make(map[string]string)
	for key, val := range n.Properties() {
		// val is kdl.Value. Stringify it.
		// If it's a string, we might get quotes depending on implementation.
		// Let's assume standard formatting.
		// We might need to unquote if it comes with quotes.
		// For now, %v is a safe bet to see what we get.
		attrs[key] = fmt.Sprintf("%v", val)
	}

	// 3. Extract Text Content (Positional Args)
	args := n.Arguments()
	var textContent string
	hasText := false

	if len(args) > 0 {
		lastArg := args[len(args)-1]
		// Determine if this is text content.
		// Spec says last positional arg is inner text.
		// We convert it to string.
		textContent = fmt.Sprintf("%v", lastArg)
		hasText = true
	}

	// 4. Transform Children
	children, err := transformNodes(n.Children().Nodes)
	if err != nil {
		return nil, err
	}
	
	// Add text content as a child if present
	if hasText {
		textNode := Text{Content: textContent}
		children = append(children, textNode)
	}

	// Construct Element
	el := Element{
		Tag:        tag,
		ID:         id,
		Classes:    classes,
		Attributes: attrs,
		Children:   children,
	}
	
	return el, nil
}