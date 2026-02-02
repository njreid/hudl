package hudl

import (
	"fmt"
	"strings"
)

// GenerateGo outputs the complete Go source file content for a given AST.
func GenerateGo(root *Root, packageName string, funcName string) (string, error) {
	var sb strings.Builder

	// Header
	sb.WriteString(fmt.Sprintf("package %s\n\n", packageName))
	sb.WriteString("import (\n")
	sb.WriteString("\t\"io\"\n")
	// TODO: Add imports from root.Imports
	sb.WriteString(")\n\n")

	// Function signature
	// TODO: Add params from root.Param
	sb.WriteString(fmt.Sprintf("func %s(w io.Writer) error {\n", funcName))

	// Body
	for _, node := range root.Nodes {
		if err := generateNode(&sb, node); err != nil {
			return "", err
		}
	}

	sb.WriteString("\treturn nil\n")
	sb.WriteString("}\n")

	return sb.String(), nil
}

func generateNode(sb *strings.Builder, node Node) error {
	switch n := node.(type) {
	case Element:
		return generateElement(sb, n)
	case Text:
		return generateText(sb, n)
	default:
		return fmt.Errorf("unknown node type: %T", node)
	}
}

func generateElement(sb *strings.Builder, el Element) error {
	// Open Tag Start
	sb.WriteString(fmt.Sprintf("\tif _, err := io.WriteString(w, \"<%s\"); err != nil { return err }\n", el.Tag))

	// Attributes
	// Merge ID and Classes into attributes if not present
	if el.ID != "" {
		sb.WriteString(fmt.Sprintf("\tif _, err := io.WriteString(w, \" id=\\\"%s\\\"\"); err != nil { return err }\n", el.ID))
	}
	if len(el.Classes) > 0 {
		classes := strings.Join(el.Classes, " ")
		sb.WriteString(fmt.Sprintf("\tif _, err := io.WriteString(w, \" class=\\\"%s\\\"\"); err != nil { return err }\n", classes))
	}

	for k, v := range el.Attributes {
		// Handle simple values vs expressions
		// For Phase 1, assume simple strings.
		// TODO: Escape values
		sb.WriteString(fmt.Sprintf("\tif _, err := io.WriteString(w, \" %s=\\\"%s\\\"\"); err != nil { return err }\n", k, v))
	}

	// Open Tag End
	sb.WriteString("\tif _, err := io.WriteString(w, \">\"); err != nil { return err }\n")

	// Children
	for _, child := range el.Children {
		if err := generateNode(sb, child); err != nil {
			return err
		}
	}

	// Close Tag
	// TODO: Handle self-closing
	sb.WriteString(fmt.Sprintf("\tif _, err := io.WriteString(w, \"</%s>\"); err != nil { return err }\n", el.Tag))
	
	return nil
}

func generateText(sb *strings.Builder, t Text) error {
	// TODO: Handle expressions (backticks)
	// For now, treat everything as literal string
	// Escape quotes in the content string for Go source
	content := strings.ReplaceAll(t.Content, "\"", "\\\"")
	sb.WriteString(fmt.Sprintf("\tif _, err := io.WriteString(w, \"%s\"); err != nil { return err }\n", content))
	return nil
}
