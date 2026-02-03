//! CEL Expression parsing tests
//!
//! Tests for parsing CEL (Common Expression Language) expressions used in Hudl templates.
//! CEL expressions appear in backticks and support:
//! - Field access: `user.name`
//! - Function calls: `size(items)`
//! - Method calls: `name.matches('^[a-z]+$')`
//! - Operators: `count > 0`, `a && b`
//! - Literals: strings, numbers, booleans

use hudlc::expr::{self, Expr, Literal, Op};

#[test]
fn test_parse_literals() {
    assert_eq!(expr::parse("123"), Ok(Expr::Literal(Literal::Int(123))));
    assert_eq!(
        expr::parse("\"hello\""),
        Ok(Expr::Literal(Literal::String("hello".to_string())))
    );
    assert_eq!(expr::parse("true"), Ok(Expr::Literal(Literal::Bool(true))));
    assert_eq!(expr::parse("false"), Ok(Expr::Literal(Literal::Bool(false))));
    assert_eq!(expr::parse("null"), Ok(Expr::Literal(Literal::Null)));
}

#[test]
fn test_parse_field_access() {
    // Simple field access (CEL style: snake_case)
    assert_eq!(
        expr::parse("user_name"),
        Ok(Expr::Variable("user_name".to_string()))
    );

    // Nested field access
    assert_eq!(
        expr::parse("user.profile.age"),
        Ok(Expr::Variable("user.profile.age".to_string()))
    );

    // Proto-style field names
    assert_eq!(
        expr::parse("tx.customer_email"),
        Ok(Expr::Variable("tx.customer_email".to_string()))
    );
}

#[test]
fn test_parse_comparison_operators() {
    assert_eq!(
        expr::parse("count > 0"),
        Ok(Expr::Binary(
            Box::new(Expr::Variable("count".to_string())),
            Op::Gt,
            Box::new(Expr::Literal(Literal::Int(0)))
        ))
    );

    assert_eq!(
        expr::parse("a >= b"),
        Ok(Expr::Binary(
            Box::new(Expr::Variable("a".to_string())),
            Op::Gte,
            Box::new(Expr::Variable("b".to_string()))
        ))
    );

    assert_eq!(
        expr::parse("x == y"),
        Ok(Expr::Binary(
            Box::new(Expr::Variable("x".to_string())),
            Op::Eq,
            Box::new(Expr::Variable("y".to_string()))
        ))
    );

    assert_eq!(
        expr::parse("x != y"),
        Ok(Expr::Binary(
            Box::new(Expr::Variable("x".to_string())),
            Op::Neq,
            Box::new(Expr::Variable("y".to_string()))
        ))
    );
}

#[test]
fn test_parse_arithmetic_operators() {
    assert_eq!(
        expr::parse("1 + 2"),
        Ok(Expr::Binary(
            Box::new(Expr::Literal(Literal::Int(1))),
            Op::Add,
            Box::new(Expr::Literal(Literal::Int(2)))
        ))
    );

    assert!(expr::parse("a - b").is_ok());
    assert!(expr::parse("a * b").is_ok());
    assert!(expr::parse("a / b").is_ok());
    assert!(expr::parse("-x").is_ok());
}

#[test]
fn test_parse_logical_operators() {
    assert!(expr::parse("a && b").is_ok());
    assert!(expr::parse("a || b").is_ok());
    assert!(expr::parse("!a").is_ok());
}

#[test]
fn test_parse_precedence() {
    // 1 + 2 * 3 -> 1 + (2 * 3)
    let res = expr::parse("1 + 2 * 3").unwrap();
    match res {
        Expr::Binary(left, op, right) => {
            assert_eq!(op, Op::Add);
            assert_eq!(*left, Expr::Literal(Literal::Int(1)));
            match *right {
                Expr::Binary(rl, rop, rr) => {
                    assert_eq!(rop, Op::Mul);
                    assert_eq!(*rl, Expr::Literal(Literal::Int(2)));
                    assert_eq!(*rr, Expr::Literal(Literal::Int(3)));
                }
                _ => panic!("Right should be binary"),
            }
        }
        _ => panic!("Top should be binary Add"),
    }
}

#[test]
fn test_parse_function_call() {
    // CEL built-in functions
    assert_eq!(
        expr::parse("size(items)"),
        Ok(Expr::Call(
            "size".to_string(),
            vec![Expr::Variable("items".to_string())]
        ))
    );

    assert_eq!(
        expr::parse("has(user.middle_name)"),
        Ok(Expr::Call(
            "has".to_string(),
            vec![Expr::Variable("user.middle_name".to_string())]
        ))
    );

    assert_eq!(
        expr::parse("string(count)"),
        Ok(Expr::Call(
            "string".to_string(),
            vec![Expr::Variable("count".to_string())]
        ))
    );
}

#[test]
fn test_parse_method_call() {
    // CEL method calls
    let res = expr::parse("name.matches('^[a-z]+$')").unwrap();
    match res {
        Expr::MethodCall(receiver, method, args) => {
            assert_eq!(method, "matches");
            assert_eq!(args.len(), 1);
            assert_eq!(*receiver, Expr::Variable("name".to_string()));
        }
        _ => panic!("Expected MethodCall, got {:?}", res),
    }
}

#[test]
fn test_parse_cel_list_operations() {
    // CEL list filter
    let res = expr::parse("users.filter(u, u.is_active)");
    assert!(res.is_ok(), "Should parse filter expression");

    // CEL list map
    let res = expr::parse("users.map(u, u.name)");
    assert!(res.is_ok(), "Should parse map expression");
}

#[test]
#[ignore] // TODO: Implement ternary operator with cel-rust integration
fn test_parse_cel_ternary() {
    // CEL ternary operator
    let res = expr::parse("is_active ? \"yes\" : \"no\"");
    assert!(res.is_ok(), "Should parse ternary expression");
}

#[test]
fn test_parse_cel_string_concatenation() {
    // CEL string concatenation
    let res = expr::parse("\"Hello, \" + name");
    assert!(res.is_ok(), "Should parse string concatenation");
}

#[test]
fn test_parse_cel_custom_function() {
    // Custom raw() function for unescaped HTML
    assert_eq!(
        expr::parse("raw(html_content)"),
        Ok(Expr::Call(
            "raw".to_string(),
            vec![Expr::Variable("html_content".to_string())]
        ))
    );
}

#[test]
fn test_parse_enum_value() {
    // CEL enum access (used in switch/case)
    assert_eq!(
        expr::parse("Status.STATUS_ACTIVE"),
        Ok(Expr::Variable("Status.STATUS_ACTIVE".to_string()))
    );
}
