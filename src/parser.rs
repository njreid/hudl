use kdl::{KdlDocument, KdlError};
use regex::Regex;

pub fn parse(input: &str) -> Result<KdlDocument, KdlError> {
    // let normalized = pre_parse(input);
    // println!("--- NORMALIZED ---\n{}\n------------------", normalized);
    input.parse()
}

fn pre_parse(input: &str) -> String {
    let mut result = input.to_string();

    let digit_regex = Regex::new(r"(\s|[{;]|^)([0-9]+[a-zA-Z%]+)").unwrap();
    result = digit_regex.replace_all(&result, "${1}_${2}").to_string();

    let else_regex = Regex::new(r"\}\s*else").unwrap();
    result = else_regex.replace_all(&result, "}\nelse").to_string();

    let prop_regex = Regex::new(r"(\w+)=([a-zA-Z_\-][a-zA-Z0-9_\-]*)").unwrap();
    result = prop_regex.replace_all(&result, "$1=\"$2\"").to_string();

    let selector_regex = Regex::new(r"(^|[\s{};])([&.][\w.&-]*|[a-zA-Z_\-][\w\-]*[\.][\w.&-]*)").unwrap();
    result = selector_regex.replace_all(&result, "$1\"$2\"").to_string();

    let backtick_regex = Regex::new(r#"`([^`]*)`"#).unwrap();
    result = backtick_regex.replace_all(&result, "\"$0\"").to_string();

    let at_rule_regex = Regex::new(r"(^|[\s{};])(@[a-zA-Z_\-]+)").unwrap();
    result = at_rule_regex.replace_all(&result, "$1\"$2\"").to_string();

    result
}
