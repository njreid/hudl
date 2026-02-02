use kdl::{KdlDocument, KdlError};
use regex::Regex;

pub fn parse(input: &str) -> Result<KdlDocument, KdlError> {
    // 1. Pre-parse (Sugaring)
    let normalized = pre_parse(input);

    // 2. Parse KDL
    normalized.parse()
}

fn pre_parse(input: &str) -> String {
    let mut result = input.to_string();

    // 1. Prefix numeric identifiers with _
    // Regex: (\s|[{;]|^)([0-9]+[a-zA-Z%]+) -> ${1}_${2}
    let digit_regex = Regex::new(r"(\s|[{;]|^)([0-9]+[a-zA-Z%]+)").unwrap();
    result = digit_regex.replace_all(&result, "${1}_${2}").to_string();

    // 2. Condensed else: } else -> }\nelse
    let else_regex = Regex::new(r"\}\s*else").unwrap();
    result = else_regex.replace_all(&result, "}\nelse").to_string();

    // 3. Quote property values: key=val -> key="val" if val is bare string
    // Regex: (\w+)=([a-zA-Z_\-][a-zA-Z0-9_\-]*)
    let prop_regex = Regex::new(r"(\w+)=([a-zA-Z_\-][a-zA-Z0-9_\-]*)").unwrap();
    result = prop_regex.replace_all(&result, r#"$1="$2""#).to_string();

    // 4. Quote selectors containing dots or starting with & or .
    // Regex: (^|[\s{};])([&.][\w.&-]*|[a-zA-Z_\-][\w\-]*[.][\w.&-]*)
    let selector_regex = Regex::new(r"(^|[\s{};])([&.][\w.&-]*|[a-zA-Z_\-][\w\-]*[.][\w.&-]*)").unwrap();
    result = selector_regex.replace_all(&result, r#"$1"$2""#).to_string();

    result
}
