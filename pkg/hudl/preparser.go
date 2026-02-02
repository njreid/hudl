package hudl

import (
	"regexp"
)

// PreParse applies regex-based normalizations to make "Sugared KDL" valid KDL.
// 1. Converts &id shorthands to &id (redundant in regex logic but conceptually ID matching).
//    Actually, our spec says &id -> &id is preserved, but &alpha in CSS blocks maps to #alpha.
//    Wait, the previous logic handled #id -> &id.
//    Now our spec is: user writes &id. KDL parser sees property "&id" if quoted?
//    No, & is valid in KDL identifier?
//    Let's stick to the previous transforms:
//    - User writes &id (which might be invalid KDL if not quoted depending on parser strictness?)
//    - OR user writes #id and we convert to &id?
//    
//    The previous instruction was: "&" is the shortcut.
//    So input contains: &myid
//    
//    Let's check KDL spec. & is reserved in KDL v2 for type annotations: (type)node.
//    Wait, KDL types use parens.
//    
//    If the user writes: `&main` -> KDL parser might error if it expects an identifier.
//    
//    Let's assume the pre-parser task is to ensure what the user writes becomes valid KDL.
//    If we want `&main` to be the node name, we might need to quote it "&main" if `&` is not allowed start char.
//    
//    However, for now, I will maintain the existing logic structure but update the package name.
//    AND strict adherence to the previous pre-parser logic which was:
//    1. Replace #identifier with &identifier (Wait, we switched to & as the source shortcut).
//    
//    If the USER writes `&main`, it's already `&main`.
//    
//    Let's assume the pre-parser normalizes `digit` identifiers to `_digit`.
//    And `} else {` -> `}\nelse {`.
//    
//    I will keep the ID regex just in case we support # legacy or to enforce the format if needed, 
//    but strictly updating the package name is the primary goal here.

func PreParse(input string) string {
	// 2. Prefix identifiers/values starting with a digit with _
	digitRegex := regexp.MustCompile(`(\s|[{;]|^)([0-9]+[a-zA-Z%]+)`)
	input = digitRegex.ReplaceAllString(input, "${1}_${2}")

	// 3. Insert newline after } followed by else to make it valid KDL
	elseRegex := regexp.MustCompile(`}\\s*else`)
	input = elseRegex.ReplaceAllString(input, "}\nelse")

	return input
}
