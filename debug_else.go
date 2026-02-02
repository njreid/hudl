package main

import (
	"fmt"
	"regexp"
)

func main() {
	input := `if "cond" { div } else { span }`
	elseRegex := regexp.MustCompile(`}\s*else`)
	output := elseRegex.ReplaceAllString(input, "}\nelse")
	fmt.Printf("Input:  '%s'\nOutput: '%s'\n", input, output)
}
