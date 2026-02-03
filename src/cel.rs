//! CEL (Common Expression Language) integration for Hudl templates.
//!
//! This module provides:
//! - CEL expression parsing and validation
//! - Runtime evaluation with protobuf-style data
//! - Custom functions (raw, size, has)

use cel_interpreter::{Context, Program, Value as CelValue};
use cel_interpreter::objects::{Key, Map as CelMap};
use std::collections::HashMap;
use std::sync::Arc;

/// A compiled CEL expression ready for evaluation.
#[derive(Clone)]
pub struct CompiledExpr {
    program: Arc<Program>,
    source: String,
}

impl CompiledExpr {
    /// Compile a CEL expression string.
    pub fn compile(source: &str) -> Result<Self, String> {
        let program = Program::compile(source)
            .map_err(|e| format!("CEL parse error: {:?}", e))?;
        Ok(CompiledExpr {
            program: Arc::new(program),
            source: source.to_string(),
        })
    }

    /// Get the source expression.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Get the variables referenced by this expression.
    pub fn references(&self) -> Vec<String> {
        self.program
            .references()
            .variables()
            .into_iter()
            .map(|v| v.to_string())
            .collect()
    }

    /// Evaluate the expression with the given context.
    pub fn evaluate(&self, ctx: &EvalContext) -> Result<CelValue, String> {
        let cel_ctx = ctx.to_cel_context();
        self.program
            .execute(&cel_ctx)
            .map_err(|e| format!("CEL evaluation error: {:?}", e))
    }
}

/// Evaluation context containing variables and their values.
#[derive(Default)]
pub struct EvalContext {
    variables: HashMap<String, CelValue>,
}

impl EvalContext {
    /// Create a new empty context.
    pub fn new() -> Self {
        EvalContext {
            variables: HashMap::new(),
        }
    }

    /// Add a string variable.
    pub fn add_string(&mut self, name: &str, value: impl Into<String>) {
        self.variables.insert(name.to_string(), CelValue::String(Arc::new(value.into())));
    }

    /// Add an integer variable.
    pub fn add_int(&mut self, name: &str, value: i64) {
        self.variables.insert(name.to_string(), CelValue::Int(value));
    }

    /// Add a boolean variable.
    pub fn add_bool(&mut self, name: &str, value: bool) {
        self.variables.insert(name.to_string(), CelValue::Bool(value));
    }

    /// Add a float variable.
    pub fn add_float(&mut self, name: &str, value: f64) {
        self.variables.insert(name.to_string(), CelValue::Float(value));
    }

    /// Add a null variable.
    pub fn add_null(&mut self, name: &str) {
        self.variables.insert(name.to_string(), CelValue::Null);
    }

    /// Add a list variable.
    pub fn add_list(&mut self, name: &str, values: Vec<CelValue>) {
        self.variables.insert(name.to_string(), CelValue::List(Arc::new(values)));
    }

    /// Add a map variable (with string keys).
    pub fn add_map(&mut self, name: &str, values: HashMap<String, CelValue>) {
        let map: HashMap<Key, CelValue> = values
            .into_iter()
            .map(|(k, v)| (Key::String(Arc::new(k)), v))
            .collect();
        self.variables.insert(name.to_string(), CelValue::Map(CelMap { map: Arc::new(map) }));
    }

    /// Add a CEL value directly.
    pub fn add_value(&mut self, name: &str, value: CelValue) {
        self.variables.insert(name.to_string(), value);
    }

    /// Create a child context with additional variables (for loops).
    pub fn child(&self) -> Self {
        EvalContext {
            variables: self.variables.clone(),
        }
    }

    /// Convert to cel-interpreter Context.
    fn to_cel_context(&self) -> Context {
        let mut ctx = Context::default();
        for (name, value) in &self.variables {
            ctx.add_variable(name, value.clone()).ok();
        }
        ctx
    }
}

/// Convert CBOR Value to CEL Value.
pub fn cbor_to_cel(cbor: &serde_cbor::Value) -> CelValue {
    match cbor {
        serde_cbor::Value::Null => CelValue::Null,
        serde_cbor::Value::Bool(b) => CelValue::Bool(*b),
        serde_cbor::Value::Integer(i) => CelValue::Int(*i as i64),
        serde_cbor::Value::Float(f) => CelValue::Float(*f),
        serde_cbor::Value::Text(s) => CelValue::String(Arc::new(s.clone())),
        serde_cbor::Value::Bytes(b) => {
            // Convert bytes to list of ints
            CelValue::List(Arc::new(b.iter().map(|byte| CelValue::Int(*byte as i64)).collect()))
        }
        serde_cbor::Value::Array(arr) => {
            CelValue::List(Arc::new(arr.iter().map(cbor_to_cel).collect()))
        }
        serde_cbor::Value::Map(map) => {
            let cel_map: HashMap<Key, CelValue> = map
                .iter()
                .filter_map(|(k, v)| {
                    // CEL maps use Key type (string keys are most common)
                    if let serde_cbor::Value::Text(key) = k {
                        Some((Key::String(Arc::new(key.clone())), cbor_to_cel(v)))
                    } else {
                        None
                    }
                })
                .collect();
            CelValue::Map(CelMap { map: Arc::new(cel_map) })
        }
        _ => CelValue::Null,
    }
}

/// Convert CEL Value to string for HTML output.
pub fn cel_to_string(value: &CelValue) -> String {
    match value {
        CelValue::Null => String::new(),
        CelValue::Bool(b) => b.to_string(),
        CelValue::Int(i) => i.to_string(),
        CelValue::UInt(u) => u.to_string(),
        CelValue::Float(f) => f.to_string(),
        CelValue::String(s) => s.to_string(),
        CelValue::Bytes(b) => String::from_utf8_lossy(b).to_string(),
        CelValue::List(l) => format!("{:?}", l),
        CelValue::Map(m) => format!("{:?}", m),
        _ => format!("{:?}", value),
    }
}

/// Check if a CEL value is truthy (for conditionals).
pub fn is_truthy(value: &CelValue) -> bool {
    match value {
        CelValue::Null => false,
        CelValue::Bool(b) => *b,
        CelValue::Int(i) => *i != 0,
        CelValue::UInt(u) => *u != 0,
        CelValue::Float(f) => *f != 0.0,
        CelValue::String(s) => !s.is_empty(),
        CelValue::List(l) => !l.is_empty(),
        CelValue::Map(m) => !m.map.is_empty(),
        _ => true,
    }
}

/// HTML-escape a string.
pub fn html_escape(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&#x27;"),
            _ => result.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_simple() {
        let expr = CompiledExpr::compile("1 + 2").unwrap();
        let ctx = EvalContext::new();
        let result = expr.evaluate(&ctx).unwrap();
        assert_eq!(result, CelValue::Int(3));
    }

    #[test]
    fn test_variable_access() {
        let expr = CompiledExpr::compile("name").unwrap();
        let mut ctx = EvalContext::new();
        ctx.add_string("name", "Alice");
        let result = expr.evaluate(&ctx).unwrap();
        assert_eq!(cel_to_string(&result), "Alice");
    }

    #[test]
    fn test_field_access() {
        let expr = CompiledExpr::compile("user.name").unwrap();
        let mut ctx = EvalContext::new();
        let mut user = HashMap::new();
        user.insert("name".to_string(), CelValue::String(Arc::new("Bob".to_string())));
        ctx.add_map("user", user);
        let result = expr.evaluate(&ctx).unwrap();
        assert_eq!(cel_to_string(&result), "Bob");
    }

    #[test]
    fn test_comparison() {
        let expr = CompiledExpr::compile("count > 0").unwrap();
        let mut ctx = EvalContext::new();
        ctx.add_int("count", 5);
        let result = expr.evaluate(&ctx).unwrap();
        assert_eq!(result, CelValue::Bool(true));
    }

    #[test]
    fn test_truthy() {
        assert!(!is_truthy(&CelValue::Null));
        assert!(!is_truthy(&CelValue::Bool(false)));
        assert!(is_truthy(&CelValue::Bool(true)));
        assert!(!is_truthy(&CelValue::Int(0)));
        assert!(is_truthy(&CelValue::Int(1)));
        assert!(!is_truthy(&CelValue::String(Arc::new(String::new()))));
        assert!(is_truthy(&CelValue::String(Arc::new("hello".to_string()))));
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(html_escape("a & b"), "a &amp; b");
        assert_eq!(html_escape("\"quoted\""), "&quot;quoted&quot;");
    }

    #[test]
    fn test_ternary() {
        let expr = CompiledExpr::compile("active ? \"yes\" : \"no\"").unwrap();
        let mut ctx = EvalContext::new();
        ctx.add_bool("active", true);
        let result = expr.evaluate(&ctx).unwrap();
        assert_eq!(cel_to_string(&result), "yes");

        ctx.add_bool("active", false);
        let result = expr.evaluate(&ctx).unwrap();
        assert_eq!(cel_to_string(&result), "no");
    }

    #[test]
    fn test_size_function() {
        let expr = CompiledExpr::compile("size(items) > 0").unwrap();
        let mut ctx = EvalContext::new();
        ctx.add_list("items", vec![CelValue::Int(1), CelValue::Int(2)]);
        let result = expr.evaluate(&ctx).unwrap();
        assert_eq!(result, CelValue::Bool(true));
    }
}
