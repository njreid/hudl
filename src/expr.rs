#[derive(Debug, PartialEq, Clone)]
pub enum Expr {
    Literal(Literal),
    Variable(String), // Dotted path like user.profile.Name
    Binary(Box<Expr>, Op, Box<Expr>),
    Unary(UnaryOp, Box<Expr>),
    Call(String, Vec<Expr>), // Standalone function: len(x)
    MethodCall(Box<Expr>, String, Vec<Expr>), // Method call: expr.Method(args)
}

#[derive(Debug, PartialEq, Clone)]
pub enum Literal {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Op {
    Eq, Neq, Gt, Lt, Gte, Lte,
    And, Or,
    Add, Sub, Mul, Div,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum UnaryOp {
    Not, Neg,
}

pub fn parse(input: &str) -> Result<Expr, String> {
    let mut parser = Parser::new(input);
    let expr = parser.parse_expression(0)?;
    parser.skip_whitespace();
    if parser.pos < parser.input.len() {
        return Err(format!("Unexpected character at end of expression: '{}'", &parser.input[parser.pos..]));
    }
    Ok(expr)
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Parser { input, pos: 0 }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    #[allow(dead_code)]
    fn peek_n(&self, n: usize) -> Option<&str> {
        if self.pos + n <= self.input.len() {
            Some(&self.input[self.pos..self.pos + n])
        } else {
            None
        }
    }

    fn consume(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn consume_n(&mut self, n: usize) {
        self.pos += n;
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.consume();
            } else {
                break;
            }
        }
    }

    fn parse_expression(&mut self, min_prec: u8) -> Result<Expr, String> {
        self.skip_whitespace();
        let mut left = self.parse_postfix()?;

        loop {
            self.skip_whitespace();
            let op = match self.peek_op() {
                Some(op) => op,
                None => break,
            };

            let prec = get_precedence(op);
            if prec < min_prec {
                break;
            }

            self.consume_op(op);

            let right = self.parse_expression(prec + 1)?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }

        Ok(left)
    }

    fn parse_postfix(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_atom()?;

        // Handle method calls: expr.Method(args)
        loop {
            self.skip_whitespace();
            if self.peek() != Some('.') {
                break;
            }

            // Check if this is a method call (. followed by ident and ()
            // vs just a field access which is handled in Variable
            let saved_pos = self.pos;
            self.consume(); // consume '.'
            self.skip_whitespace();

            // Parse the method/field name
            if !self.peek().map(is_ident_start).unwrap_or(false) {
                self.pos = saved_pos;
                break;
            }

            let name_start = self.pos;
            while let Some(c) = self.peek() {
                if is_ident_char(c) {
                    self.consume();
                } else {
                    break;
                }
            }
            let name = self.input[name_start..self.pos].to_string();

            self.skip_whitespace();

            // If followed by '(', it's a method call
            if self.peek() == Some('(') {
                self.consume();
                let mut args = Vec::new();
                self.skip_whitespace();
                if self.peek() != Some(')') {
                    loop {
                        args.push(self.parse_expression(0)?);
                        self.skip_whitespace();
                        if self.peek() == Some(',') {
                            self.consume();
                            self.skip_whitespace();
                        } else {
                            break;
                        }
                    }
                }
                self.skip_whitespace();
                if self.consume() != Some(')') {
                    return Err("Expected ')' in method call".to_string());
                }
                expr = Expr::MethodCall(Box::new(expr), name, args);
            } else {
                // It's a field access - extend the variable path
                // But expr might not be a Variable, so we need to handle this differently
                // For now, convert to MethodCall with no args to represent field access
                // Actually, let's revert and keep it as Variable extension
                self.pos = saved_pos;
                break;
            }
        }

        Ok(expr)
    }

    fn parse_atom(&mut self) -> Result<Expr, String> {
        self.skip_whitespace();
        let c = self.peek().ok_or("Unexpected end of input")?;

        if c == '(' {
            self.consume();
            let expr = self.parse_expression(0)?;
            self.skip_whitespace();
            if self.consume() != Some(')') {
                return Err("Expected ')'".to_string());
            }
            Ok(expr)
        } else if c == '!' {
            self.consume();
            let expr = self.parse_atom()?;
            Ok(Expr::Unary(UnaryOp::Not, Box::new(expr)))
        } else if c == '-' {
            self.consume();
            let expr = self.parse_atom()?; // Could be negative number, but UnaryOp::Neg handles it
            Ok(Expr::Unary(UnaryOp::Neg, Box::new(expr)))
        } else if c == '"' || c == '\'' {
            self.parse_string()
        } else if c.is_digit(10) {
            self.parse_number()
        } else if is_ident_start(c) {
            self.parse_ident_or_call()
        } else {
            Err(format!("Unexpected character: {}", c))
        }
    }

    fn peek_op(&self) -> Option<Op> {
        let s = &self.input[self.pos..];
        if s.starts_with("==") { Some(Op::Eq) }
        else if s.starts_with("!=") { Some(Op::Neq) }
        else if s.starts_with(">=") { Some(Op::Gte) }
        else if s.starts_with("<=") { Some(Op::Lte) }
        else if s.starts_with("&&") { Some(Op::And) }
        else if s.starts_with("||") { Some(Op::Or) }
        else if s.starts_with('>') { Some(Op::Gt) }
        else if s.starts_with('<') { Some(Op::Lt) }
        else if s.starts_with('+') { Some(Op::Add) }
        else if s.starts_with('-') { Some(Op::Sub) }
        else if s.starts_with('*') { Some(Op::Mul) }
        else if s.starts_with('/') { Some(Op::Div) }
        else { None }
    }

    fn consume_op(&mut self, op: Op) {
        let len = match op {
            Op::Eq | Op::Neq | Op::Gte | Op::Lte | Op::And | Op::Or => 2,
            _ => 1,
        };
        self.consume_n(len);
    }

    fn parse_string(&mut self) -> Result<Expr, String> {
        let quote = self.consume().unwrap();
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c == quote {
                self.consume();
                return Ok(Expr::Literal(Literal::String(s)));
            }
            if c == '\\' {
                self.consume();
                if let Some(next) = self.consume() {
                    s.push(next);
                }
            } else {
                self.consume();
                s.push(c);
            }
        }
        Err("Unterminated string".to_string())
    }

    fn parse_number(&mut self) -> Result<Expr, String> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_digit(10) {
                self.consume();
            } else {
                break;
            }
        }
        let s = &self.input[start..self.pos];
        if let Ok(i) = s.parse::<i64>() {
            Ok(Expr::Literal(Literal::Int(i)))
        } else {
            Err("Invalid number".to_string())
        }
    }

    fn parse_ident_or_call(&mut self) -> Result<Expr, String> {
        let start = self.pos;
        // First, parse the base identifier (no dots)
        while let Some(c) = self.peek() {
            if is_ident_char(c) {
                self.consume();
            } else {
                break;
            }
        }
        let base_ident = self.input[start..self.pos].to_string();

        match base_ident.as_str() {
            "true" => return Ok(Expr::Literal(Literal::Bool(true))),
            "false" => return Ok(Expr::Literal(Literal::Bool(false))),
            "null" => return Ok(Expr::Literal(Literal::Null)),
            _ => {}
        }

        self.skip_whitespace();

        // Check if this is a standalone function call (no dots before the paren)
        if self.peek() == Some('(') {
            // Function call like len(x)
            self.consume();
            let mut args = Vec::new();
            self.skip_whitespace();
            if self.peek() != Some(')') {
                loop {
                    args.push(self.parse_expression(0)?);
                    self.skip_whitespace();
                    if self.peek() == Some(',') {
                        self.consume();
                        self.skip_whitespace();
                    } else {
                        break;
                    }
                }
            }
            self.skip_whitespace();
            if self.consume() != Some(')') {
                return Err("Expected ')'".to_string());
            }
            return Ok(Expr::Call(base_ident, args));
        }

        // Check for dotted path (variable or potential method call target)
        let mut full_path = base_ident;
        while self.peek() == Some('.') {
            // Look ahead to see if this is field access or method call
            let saved = self.pos;
            self.consume(); // consume '.'
            self.skip_whitespace();

            if !self.peek().map(is_ident_start).unwrap_or(false) {
                self.pos = saved;
                break;
            }

            let part_start = self.pos;
            while let Some(c) = self.peek() {
                if is_ident_char(c) {
                    self.consume();
                } else {
                    break;
                }
            }
            let part = &self.input[part_start..self.pos];

            self.skip_whitespace();

            // If followed by '(', this is a method call - don't include in path
            if self.peek() == Some('(') {
                self.pos = saved; // Reset to before the dot
                break;
            }

            // Otherwise, it's part of the variable path
            full_path.push('.');
            full_path.push_str(part);
        }

        Ok(Expr::Variable(full_path))
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn get_precedence(op: Op) -> u8 {
    match op {
        Op::Or => 1,
        Op::And => 2,
        Op::Eq | Op::Neq => 3,
        Op::Gt | Op::Lt | Op::Gte | Op::Lte => 4,
        Op::Add | Op::Sub => 5,
        Op::Mul | Op::Div => 6,
    }
}

