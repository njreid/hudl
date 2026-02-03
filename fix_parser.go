package main

import (
	"os"
)

func main() {
	content := `use kdl::{KdlDocument, KdlError};
use regex::Regex;

pub fn parse(input: &str) -> Result<KdlDocument, KdlError> {
    let normalized = pre_parse(input);
    // println!("--- NORMALIZED ---\n{}\n------------------", normalized);
    normalized.parse()
}

fn pre_parse(input: &str) -> String {
    let mut result = input.to_string();

    // 1. Prefix numeric identifiers with _
    let digit_regex = Regex::new(r"(\s|[{;]|^)([0-9]+[a-zA-Z%]+)").unwrap();
    result = digit_regex.replace_all(&result, "${1}_${2}").to_string();

    // 2. Condensed else: } else -> }\nelse
    let else_regex = Regex::new(r"\}\s*else").unwrap();
    result = else_regex.replace_all(&result, "}\nelse").to_string();

    // 3. Quote property values: key=val -> key=\"val\"
    let prop_regex = Regex::new(r"(\w+)=([a-zA-Z_\-][a-zA-Z0-9_\-]*)").unwrap();
    result = prop_regex.replace_all(&result, r#"$1=\"$2\""#).to_string();

    // 4. Quote selectors containing dots or starting with & or .
    let selector_regex = Regex::new(r"(^|[\s{};])([&.][\w.&-]*|[a-zA-Z_\-][\w\-]*[\.][\w.&-]*)").unwrap();
    result = selector_regex.replace_all(&result, r#"$1\"$2\""#).to_string();

    // 5. Handle backticks: `expr` -> "expr"
    let backtick_regex = Regex::new(r"(`[^`]*`)").unwrap();
    result = backtick_regex.replace_all(&result, |caps: &regex::Captures| {
        format!(\"\"{}\"", &caps[0])
    }).to_string();

    // 6. Handle CSS At-rules: @rule -> "@rule"
    let at_rule_regex = Regex::new(r"(^|[\s{};])(@[a-zA-Z_\-]+)").unwrap();
    result = at_rule_regex.replace_all(&result, r#"$1\"$2\""#).to_string();

    result
}
`
	os.WriteFile("src/parser.rs", []byte(content), 0644)
}
