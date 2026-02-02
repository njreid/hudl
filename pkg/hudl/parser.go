package hudl

import (
	"strings"

	"github.com/calico32/kdl-go"
)

// Parse takes a raw Hudl template string, applies pre-parsing normalization,
// and returns a parsed KDL document.
func Parse(input string) (*kdl.Document, error) {
	// 1. Apply regex-based "sugaring" fixes
	normalized := PreParse(input)

	// 2. Parse strictly as KDL v2
	doc, err := kdl.Parse(strings.NewReader(normalized))
	if err != nil {
		return nil, err
	}

	return doc, nil
}
