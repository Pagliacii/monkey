use crate::{ast, lexer, token};

const LOWEST: u8 = 1;
const EQUALS: u8 = 2; // ==
const LESSGREATER: u8 = 3; // > or <
const SUM: u8 = 4; // +
const PRODUCT: u8 = 5; // *
const PREFIX: u8 = 6; // -X or !X
const CALL: u8 = 7; // myFunction(X)
const INDEX: u8 = 8; // array[index]

pub struct Parser<'a> {
    l: &'a mut lexer::Lexer,
    errors: Vec<String>,

    cur_token: token::Token,
    peek_token: token::Token,
}

impl<'a> Parser<'a> {
    pub fn new(l: &'a mut lexer::Lexer) -> Self {
        let mut parser = Self {
            l,
            errors: Vec::new(),
            cur_token: token::Token::new(token::TokenType::ILLEGAL, ""),
            peek_token: token::Token::new(token::TokenType::ILLEGAL, ""),
        };
        parser.next_token();
        parser.next_token();
        parser
    }

    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    pub fn parse_program(&mut self) -> ast::Program {
        let mut program = ast::Program::new();

        while self.cur_token.token_type() != token::TokenType::EOF {
            if let Some(statement) = self.parse_statement() {
                program.add_statement(statement);
            }
            self.next_token();
        }

        program
    }

    fn peek_error(&mut self, token_type: token::TokenType) {
        let message = format!(
            "expected next token to be {:?}, got {:?} instead",
            token_type,
            self.peek_token.token_type()
        );
        self.errors.push(message);
    }

    fn next_token(&mut self) {
        self.cur_token = std::mem::replace(&mut self.peek_token, self.l.next_token());
    }

    fn parse_statement(&mut self) -> Option<ast::Statement> {
        match self.cur_token.token_type() {
            token::TokenType::LET => self.parse_let_statement(),
            token::TokenType::RETURN => self.parse_return_statement(),
            _ => self.parse_expression_statement(),
        }
    }

    fn expect_peek(&mut self, token_type: token::TokenType) -> bool {
        if self.peek_token_is(token_type) {
            self.next_token();
            true
        } else {
            self.peek_error(token_type);
            false
        }
    }

    fn peek_token_is(&self, token_type: token::TokenType) -> bool {
        self.peek_token.token_type() == token_type
    }

    fn cur_token_is(&self, token_type: token::TokenType) -> bool {
        self.cur_token.token_type() == token_type
    }

    fn parse_let_statement(&mut self) -> Option<ast::Statement> {
        let token = self.cur_token.clone();

        if !self.expect_peek(token::TokenType::IDENT) {
            return None;
        }

        let name = ast::Identifier::new(self.cur_token.clone(), self.cur_token.literal());

        if !self.expect_peek(token::TokenType::ASSIGN) {
            return None;
        }

        self.next_token();
        let value = self.parse_expression(LOWEST)?;

        if self.peek_token_is(token::TokenType::SEMICOLON) {
            self.next_token();
        }

        Some(ast::Statement::Let { token, name, value })
    }

    fn parse_return_statement(&mut self) -> Option<ast::Statement> {
        let token = self.cur_token.clone();

        self.next_token();
        let value = self.parse_expression(LOWEST)?;

        if self.peek_token_is(token::TokenType::SEMICOLON) {
            self.next_token();
        }

        Some(ast::Statement::Return { token, value })
    }

    fn parse_expression_statement(&mut self) -> Option<ast::Statement> {
        let token = self.cur_token.clone();
        let expression = self.parse_expression(LOWEST)?;

        if self.peek_token_is(token::TokenType::SEMICOLON) {
            self.next_token();
        }

        Some(ast::Statement::Expression { token, expression })
    }

    fn parse_expression(&mut self, precedence: u8) -> Option<ast::Expression> {
        let mut left_exp = match self.cur_token.token_type() {
            token::TokenType::IDENT => Some(self.parse_identifier()),
            token::TokenType::INT => self.parse_integer_literal(),
            token::TokenType::BANG | token::TokenType::MINUS => self.parse_prefix_expression(),
            token::TokenType::TRUE | token::TokenType::FALSE => self.parse_boolean(),
            token::TokenType::LPAREN => self.parse_grouped_expression(),
            token::TokenType::IF => self.parse_if_expression(),
            token::TokenType::FUNCTION => self.parse_function_literal(),
            token::TokenType::STRING => self.parse_string_literal(),
            token::TokenType::LBRACKET => self.parse_array_literal(),
            token::TokenType::LBRACE => self.parse_hash_literal(),
            token::TokenType::MACRO => self.parse_macro_literal(),
            _ => {
                let message = format!(
                    "no prefix parse function for {:?} found",
                    self.cur_token.token_type()
                );
                self.errors.push(message);
                None
            }
        }?;

        while !self.peek_token_is(token::TokenType::SEMICOLON)
            && precedence < self.peek_precedence()
        {
            match self.peek_token.token_type() {
                token::TokenType::PLUS
                | token::TokenType::MINUS
                | token::TokenType::SLASH
                | token::TokenType::ASTERISK
                | token::TokenType::EQ
                | token::TokenType::NOT_EQ
                | token::TokenType::LT
                | token::TokenType::GT => {
                    self.next_token();
                    left_exp = self.parse_infix_expression(left_exp)?;
                }
                token::TokenType::LPAREN => {
                    self.next_token();
                    left_exp = self.parse_call_expression(left_exp)?;
                }
                token::TokenType::LBRACKET => {
                    self.next_token();
                    left_exp = self.parse_index_expression(left_exp)?;
                }
                _ => break,
            };
        }

        Some(left_exp)
    }

    fn parse_identifier(&self) -> ast::Expression {
        ast::Expression::Identifier(ast::Identifier::new(
            self.cur_token.clone(),
            self.cur_token.literal(),
        ))
    }

    fn parse_integer_literal(&mut self) -> Option<ast::Expression> {
        let literal = self.cur_token.literal();
        let value = match literal.parse::<i64>() {
            Ok(value) => value,
            Err(error) => {
                self.errors
                    .push(format!("could not parse {literal} as integer: {error}"));
                return None;
            }
        };

        Some(ast::Expression::IntegerLiteral {
            token: self.cur_token.clone(),
            value,
        })
    }

    fn parse_prefix_expression(&mut self) -> Option<ast::Expression> {
        let token = self.cur_token.clone();
        let operator = token.literal();
        self.next_token();
        let right = self.parse_expression(PREFIX)?;
        Some(ast::Expression::PrefixExpr {
            token,
            operator,
            right: Box::new(right),
        })
    }

    fn get_token_precedence(&self, token_type: &token::TokenType) -> u8 {
        match token_type {
            token::TokenType::EQ | token::TokenType::NOT_EQ => EQUALS,
            token::TokenType::LT | token::TokenType::GT => LESSGREATER,
            token::TokenType::PLUS | token::TokenType::MINUS => SUM,
            token::TokenType::SLASH | token::TokenType::ASTERISK => PRODUCT,
            token::TokenType::LPAREN => CALL,
            token::TokenType::LBRACKET => INDEX,
            _ => LOWEST,
        }
    }

    fn peek_precedence(&self) -> u8 {
        self.get_token_precedence(&self.peek_token.token_type())
    }

    fn cur_precedence(&self) -> u8 {
        self.get_token_precedence(&self.cur_token.token_type())
    }

    fn parse_infix_expression(&mut self, left: ast::Expression) -> Option<ast::Expression> {
        let token = self.cur_token.clone();
        let operator = token.literal();
        let precedence = self.cur_precedence();
        self.next_token();
        let right = self.parse_expression(precedence)?;

        Some(ast::Expression::InfixExpr {
            token,
            left: Box::new(left),
            operator,
            right: Box::new(right),
        })
    }

    fn parse_call_expression(&mut self, function: ast::Expression) -> Option<ast::Expression> {
        let token = self.cur_token.clone();
        let arguments = self.parse_expression_list(token::TokenType::RPAREN)?;
        Some(ast::Expression::CallExpr {
            token,
            function: Box::new(function),
            arguments,
        })
    }

    fn parse_index_expression(&mut self, left: ast::Expression) -> Option<ast::Expression> {
        let token = self.cur_token.clone();
        self.next_token();
        let index = self.parse_expression(LOWEST)?;
        if !self.expect_peek(token::TokenType::RBRACKET) {
            return None;
        }
        Some(ast::Expression::IndexExpr {
            token,
            left: Box::new(left),
            index: Box::new(index),
        })
    }

    fn parse_boolean(&self) -> Option<ast::Expression> {
        let value = self.cur_token_is(token::TokenType::TRUE);
        Some(ast::Expression::BooleanLiteral {
            token: self.cur_token.clone(),
            value,
        })
    }

    fn parse_grouped_expression(&mut self) -> Option<ast::Expression> {
        self.next_token();
        let exp = self.parse_expression(LOWEST);
        if !self.expect_peek(token::TokenType::RPAREN) {
            return None;
        }
        exp
    }

    fn parse_if_expression(&mut self) -> Option<ast::Expression> {
        let token = self.cur_token.clone();
        if !self.expect_peek(token::TokenType::LPAREN) {
            return None;
        }
        self.next_token();
        let condition = self.parse_expression(LOWEST)?;
        if !self.expect_peek(token::TokenType::RPAREN) {
            return None;
        }
        if !self.expect_peek(token::TokenType::LBRACE) {
            return None;
        }
        let consequence = self.parse_block_statement();
        let alternative = if self.peek_token_is(token::TokenType::ELSE) {
            self.next_token();
            if !self.expect_peek(token::TokenType::LBRACE) {
                return None;
            }
            Some(self.parse_block_statement())
        } else {
            None
        };
        Some(ast::Expression::IfLiteral {
            token,
            condition: Box::new(condition),
            consequence,
            alternative,
        })
    }

    fn parse_block_statement(&mut self) -> ast::BlockStatement {
        let token = self.cur_token.clone();
        let mut statements = Vec::new();

        self.next_token();

        while !self.cur_token_is(token::TokenType::RBRACE)
            && !self.cur_token_is(token::TokenType::EOF)
        {
            if let Some(statement) = self.parse_statement() {
                statements.push(statement);
            }
            self.next_token();
        }

        ast::BlockStatement::new(token, statements)
    }

    fn parse_function_literal(&mut self) -> Option<ast::Expression> {
        let token = self.cur_token.clone();

        if !self.expect_peek(token::TokenType::LPAREN) {
            return None;
        }

        let parameters = self.parse_function_parameters()?;

        if !self.expect_peek(token::TokenType::LBRACE) {
            return None;
        }

        let body = self.parse_block_statement();

        Some(ast::Expression::FunctionLiteral {
            token,
            parameters,
            body,
        })
    }

    fn parse_function_parameters(&mut self) -> Option<Vec<ast::Identifier>> {
        let mut identifiers = Vec::new();

        if self.peek_token_is(token::TokenType::RPAREN) {
            self.next_token();
            return Some(identifiers);
        }

        self.next_token();

        identifiers.push(ast::Identifier::new(
            self.cur_token.clone(),
            self.cur_token.literal(),
        ));

        while self.peek_token_is(token::TokenType::COMMA) {
            self.next_token();
            self.next_token();
            identifiers.push(ast::Identifier::new(
                self.cur_token.clone(),
                self.cur_token.literal(),
            ));
        }

        if !self.expect_peek(token::TokenType::RPAREN) {
            return None;
        }

        Some(identifiers)
    }

    fn parse_string_literal(&mut self) -> Option<ast::Expression> {
        Some(ast::Expression::StringLiteral {
            token: self.cur_token.clone(),
            value: self.cur_token.literal(),
        })
    }

    fn parse_array_literal(&mut self) -> Option<ast::Expression> {
        let token = self.cur_token.clone();
        let elements = self.parse_expression_list(token::TokenType::RBRACKET)?;
        Some(ast::Expression::ArrayLiteral { token, elements })
    }

    fn parse_hash_literal(&mut self) -> Option<ast::Expression> {
        let token = self.cur_token.clone();
        let mut pairs = vec![];
        while !self.peek_token_is(token::TokenType::RBRACE) {
            // parse key expression
            self.next_token();
            let key = self.parse_expression(LOWEST)?;
            if !self.expect_peek(token::TokenType::COLON) {
                return None;
            }

            // parse value expression
            self.next_token();
            let value = self.parse_expression(LOWEST)?;

            pairs.push(ast::HashPair::new(key, value));

            if !self.peek_token_is(token::TokenType::RBRACE)
                && !self.expect_peek(token::TokenType::COMMA)
            {
                return None;
            }
        }

        if !self.expect_peek(token::TokenType::RBRACE) {
            return None;
        }

        Some(ast::Expression::HashLiteral { token, pairs })
    }

    fn parse_macro_literal(&mut self) -> Option<ast::Expression> {
        let token = self.cur_token.clone();

        if !self.expect_peek(token::TokenType::LPAREN) {
            return None;
        }

        let parameters = self.parse_function_parameters()?;

        if !self.expect_peek(token::TokenType::LBRACE) {
            return None;
        }

        let body = self.parse_block_statement();

        Some(ast::Expression::MacroLiteral {
            token,
            parameters,
            body,
        })
    }

    fn parse_expression_list(
        &mut self,
        token_type: token::TokenType,
    ) -> Option<Vec<ast::Expression>> {
        let mut list = Vec::new();

        if self.peek_token_is(token_type) {
            self.next_token();
            return Some(list);
        }

        self.next_token();
        list.push(self.parse_expression(LOWEST)?);

        while self.peek_token_is(token::TokenType::COMMA) {
            self.next_token();
            self.next_token();
            list.push(self.parse_expression(LOWEST)?);
        }

        if !self.expect_peek(token_type) {
            return None;
        }

        Some(list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug)]
    enum ExpectedLiteral {
        Integer(i64),
        Boolean(bool),
        Identifier(&'static str),
    }

    #[test]
    fn parses_let_statements() {
        use ExpectedLiteral;
        let tests = [
            ("let x = 5;", "x", ExpectedLiteral::Integer(5)),
            ("let y = true;", "y", ExpectedLiteral::Boolean(true)),
            (
                "let foobar = y;",
                "foobar",
                ExpectedLiteral::Identifier("y"),
            ),
        ];

        for (input, expected_name, expected_value) in tests {
            let program = parse(input);
            let [ast::Statement::Let { name, value, .. }] = program.statements() else {
                panic!("expected one let statement: {program:?}");
            };
            assert_eq!(name.value(), expected_name);
            test_literal_expression(value, expected_value);
        }
    }

    #[test]
    fn parses_return_statements() {
        let tests = [
            ("return 5;", 5),
            ("return 10;", 10),
            ("return 993322;", 993322),
        ];

        for (input, expected_value) in tests {
            let program = parse(input);
            let [ast::Statement::Return { value, .. }] = program.statements() else {
                panic!("expected one return statement: {program:?}");
            };
            test_integer_literal(value, expected_value);
        }
    }

    #[test]
    fn parses_identifier_expression() {
        let program = parse("foobar;");

        let [
            ast::Statement::Expression {
                expression: ast::Expression::Identifier(identifier),
                ..
            },
        ] = program.statements()
        else {
            panic!("expected one identifier expression: {program:?}");
        };

        assert_eq!(identifier.value(), "foobar");
        assert_eq!(identifier.token_literal(), "foobar");
    }

    #[test]
    fn parses_integer_literal_expression() {
        let program = parse("5;");

        let [
            ast::Statement::Expression {
                expression: ast::Expression::IntegerLiteral { token, value },
                ..
            },
        ] = program.statements()
        else {
            panic!("expected one integer literal expression: {program:?}");
        };

        assert_eq!(*value, 5);
        assert_eq!(token.literal(), "5");
    }

    #[test]
    fn parses_prefix_expressions() {
        use ExpectedLiteral::{Boolean, Integer};

        let prefix_tests = [
            ("!5;", "!", Integer(5)),
            ("-15;", "-", Integer(15)),
            ("!true", "!", Boolean(true)),
            ("!false", "!", Boolean(false)),
        ];
        for (input, expected_operator, expected_value) in prefix_tests {
            let program = parse(input);
            let [ast::Statement::Expression { expression, .. }] = program.statements() else {
                panic!("expected one prefix expression: {program:?}");
            };
            test_prefix_expression(expression, expected_operator, expected_value);
        }
    }

    #[test]
    fn parses_infix_expressions() {
        use ExpectedLiteral::{Boolean, Integer};

        let infix_tests = [
            ("5 + 5;", Integer(5), "+", Integer(5)),
            ("5 - 5;", Integer(5), "-", Integer(5)),
            ("5 * 5;", Integer(5), "*", Integer(5)),
            ("5 / 5;", Integer(5), "/", Integer(5)),
            ("5 > 5;", Integer(5), ">", Integer(5)),
            ("5 < 5;", Integer(5), "<", Integer(5)),
            ("5 == 5;", Integer(5), "==", Integer(5)),
            ("5 != 5;", Integer(5), "!=", Integer(5)),
            ("true == true", Boolean(true), "==", Boolean(true)),
            ("true != false", Boolean(true), "!=", Boolean(false)),
            ("false == false", Boolean(false), "==", Boolean(false)),
        ];

        for (input, expected_left, expected_operator, expected_right) in infix_tests {
            let program = parse(input);
            let [ast::Statement::Expression { expression, .. }] = program.statements() else {
                panic!("expected one infix expression: {program:?}");
            };
            test_infix_expression(expression, expected_left, expected_operator, expected_right);
        }
    }

    #[test]
    fn parses_operator_precedence() {
        let tests = [
            ("-a * b", "((-a) * b)"),
            ("!-a", "(!(-a))"),
            ("a + b + c", "((a + b) + c)"),
            ("a + b - c", "((a + b) - c)"),
            ("a * b * c", "((a * b) * c)"),
            ("a * b / c", "((a * b) / c)"),
            ("a + b / c", "(a + (b / c))"),
            ("a + b * c + d / e - f", "(((a + (b * c)) + (d / e)) - f)"),
            ("3 + 4; -5 * 5", "(3 + 4)((-5) * 5)"),
            ("5 > 4 == 3 < 4", "((5 > 4) == (3 < 4))"),
            ("5 < 4 != 3 > 4", "((5 < 4) != (3 > 4))"),
            (
                "3 + 4 * 5 == 3 * 1 + 4 * 5",
                "((3 + (4 * 5)) == ((3 * 1) + (4 * 5)))",
            ),
            ("true", "true"),
            ("false", "false"),
            ("3 > 5 == false", "((3 > 5) == false)"),
            ("3 < 5 == true", "((3 < 5) == true)"),
            ("1 + (2 + 3) + 4", "((1 + (2 + 3)) + 4)"),
            ("(5 + 5) * 2", "((5 + 5) * 2)"),
            ("2 / (5 + 5)", "(2 / (5 + 5))"),
            ("-(5 + 5)", "(-(5 + 5))"),
            ("!(true == true)", "(!(true == true))"),
            ("a + add(b * c) + d", "((a + add((b * c))) + d)"),
            (
                "add(a, b, 1, 2 * 3, 4 + 5, add(6, 7 * 8))",
                "add(a, b, 1, (2 * 3), (4 + 5), add(6, (7 * 8)))",
            ),
            (
                "add(a + b + c * d / f + g)",
                "add((((a + b) + ((c * d) / f)) + g))",
            ),
            (
                "a * [1, 2, 3, 4][b * c] * d",
                "((a * ([1, 2, 3, 4])[(b * c)]) * d)",
            ),
            (
                "add(a * b[2], b[1], 2 * [1, 2][1])",
                "add((a * (b)[2]), (b)[1], (2 * ([1, 2])[1]))",
            ),
        ];

        for test in tests.iter() {
            let program = parse(test.0);
            let actual = program.to_string();
            assert_eq!(actual, test.1, "expected={:?}, got={:?}", test.1, actual);
        }
    }

    #[test]
    fn parses_boolean_expressions() {
        let tests = [("true;", true), ("false;", false)];
        for test in tests.iter() {
            let program = parse(test.0);
            let [
                ast::Statement::Expression {
                    expression: ast::Expression::BooleanLiteral { value, .. },
                    ..
                },
            ] = program.statements()
            else {
                panic!("expected one boolean expression: {program:?}");
            };

            assert_eq!(*value, test.1, "expected={:?}, got={:?}", test.1, *value);
        }
    }

    #[test]
    fn parses_if_expression() {
        let program = parse("if (x < y) { x }");
        let [
            ast::Statement::Expression {
                expression:
                    ast::Expression::IfLiteral {
                        condition,
                        consequence,
                        alternative,
                        ..
                    },
                ..
            },
        ] = program.statements()
        else {
            panic!("expected one if expression: {program:?}");
        };
        test_infix_expression(
            condition,
            ExpectedLiteral::Identifier("x"),
            "<",
            ExpectedLiteral::Identifier("y"),
        );

        let [ast::Statement::Expression { expression, .. }] = consequence.statements() else {
            panic!("expected one statement in consequence: {consequence:?}");
        };
        test_literal_expression(expression, ExpectedLiteral::Identifier("x"));

        assert!(alternative.is_none());
    }

    #[test]
    fn parses_if_else_expression() {
        let program = parse("if (x < y) { x } else { y }");
        let [
            ast::Statement::Expression {
                expression:
                    ast::Expression::IfLiteral {
                        condition,
                        consequence,
                        alternative,
                        ..
                    },
                ..
            },
        ] = program.statements()
        else {
            panic!("expected one if expression: {program:?}");
        };
        test_infix_expression(
            condition,
            ExpectedLiteral::Identifier("x"),
            "<",
            ExpectedLiteral::Identifier("y"),
        );

        let [ast::Statement::Expression { expression, .. }] = consequence.statements() else {
            panic!("expected one statement in consequence: {consequence:?}");
        };
        test_literal_expression(expression, ExpectedLiteral::Identifier("x"));

        let alternative = alternative
            .as_ref()
            .expect("expected an alternative branch");
        let [ast::Statement::Expression { expression, .. }] = alternative.statements() else {
            panic!("expected one statement in alternative: {alternative:?}");
        };
        test_literal_expression(expression, ExpectedLiteral::Identifier("y"));
    }

    #[test]
    fn parses_function_literal() {
        let program = parse("fn(x, y) { x + y; }");
        let [
            ast::Statement::Expression {
                expression:
                    ast::Expression::FunctionLiteral {
                        parameters, body, ..
                    },
                ..
            },
        ] = program.statements()
        else {
            panic!("expected one function literal expression: {program:?}");
        };

        assert_eq!(parameters.len(), 2);
        test_literal_expression(
            &ast::Expression::Identifier(parameters[0].clone()),
            ExpectedLiteral::Identifier("x"),
        );
        test_literal_expression(
            &ast::Expression::Identifier(parameters[1].clone()),
            ExpectedLiteral::Identifier("y"),
        );

        let [ast::Statement::Expression { expression, .. }] = body.statements() else {
            panic!("expected one statement in function body: {body:?}");
        };
        test_infix_expression(
            expression,
            ExpectedLiteral::Identifier("x"),
            "+",
            ExpectedLiteral::Identifier("y"),
        );
    }

    #[test]
    fn parses_function_parameters() {
        let tests = [
            ("fn() {};", vec![]),
            ("fn(x) {};", vec!["x"]),
            ("fn(x, y, z) {};", vec!["x", "y", "z"]),
        ];
        for (input, expected_params) in tests {
            let program = parse(input);
            let [
                ast::Statement::Expression {
                    expression: ast::Expression::FunctionLiteral { parameters, .. },
                    ..
                },
            ] = program.statements()
            else {
                panic!("expected one function literal expression: {program:?}");
            };

            assert_eq!(parameters.len(), expected_params.len());
            for (param, expected) in parameters.iter().zip(expected_params) {
                test_literal_expression(
                    &ast::Expression::Identifier(param.clone()),
                    ExpectedLiteral::Identifier(expected),
                );
            }
        }
    }

    #[test]
    fn parses_call_expression() {
        let program = parse("add(1, 2 * 3, 4 + 5);");
        let [
            ast::Statement::Expression {
                expression:
                    ast::Expression::CallExpr {
                        function,
                        arguments,
                        ..
                    },
                ..
            },
        ] = program.statements()
        else {
            panic!("expected one call expression: {program:?}");
        };

        test_identifier(function, "add");
        assert_eq!(arguments.len(), 3);
        test_literal_expression(&arguments[0], ExpectedLiteral::Integer(1));
        test_infix_expression(
            &arguments[1],
            ExpectedLiteral::Integer(2),
            "*",
            ExpectedLiteral::Integer(3),
        );
        test_infix_expression(
            &arguments[2],
            ExpectedLiteral::Integer(4),
            "+",
            ExpectedLiteral::Integer(5),
        );
    }

    #[test]
    fn parses_string_literal_expression() {
        let program = parse(r#""hello world";"#);
        let [
            ast::Statement::Expression {
                expression: ast::Expression::StringLiteral { value, .. },
                ..
            },
        ] = program.statements()
        else {
            panic!("expected one string literal expression: {program:?}");
        };

        assert_eq!(value, "hello world");
    }

    #[test]
    fn parses_array_literal_expression() {
        let program = parse("[1, 2 * 2, 3 + 3]");
        let [
            ast::Statement::Expression {
                expression: ast::Expression::ArrayLiteral { elements, .. },
                ..
            },
        ] = program.statements()
        else {
            panic!("expected one array literal expression: {program:?}");
        };

        assert_eq!(elements.len(), 3);
        test_literal_expression(&elements[0], ExpectedLiteral::Integer(1));
        test_infix_expression(
            &elements[1],
            ExpectedLiteral::Integer(2),
            "*",
            ExpectedLiteral::Integer(2),
        );
        test_infix_expression(
            &elements[2],
            ExpectedLiteral::Integer(3),
            "+",
            ExpectedLiteral::Integer(3),
        );
    }

    #[test]
    fn parses_index_expressions() {
        let program = parse("myArray[1 + 1]");
        let [
            ast::Statement::Expression {
                expression: ast::Expression::IndexExpr { left, index, .. },
                ..
            },
        ] = program.statements()
        else {
            panic!("expected one index expression: {program:?}");
        };
        test_identifier(left, "myArray");
        test_infix_expression(
            index,
            ExpectedLiteral::Integer(1),
            "+",
            ExpectedLiteral::Integer(1),
        );
    }

    #[test]
    fn parses_hash_literals_string_keys() {
        use std::collections::HashMap;
        let program = parse(r#"{"one": 1, "two": 2, "three": 3}"#);
        let [
            ast::Statement::Expression {
                expression: ast::Expression::HashLiteral { pairs, .. },
                ..
            },
        ] = program.statements()
        else {
            panic!("expected one hash literal expression: {program:?}");
        };
        assert_eq!(pairs.len(), 3);

        let expected = HashMap::from([("one", 1), ("two", 2), ("three", 3)]);
        for pair in pairs {
            let ast::Expression::StringLiteral { value: key, .. } = pair.key() else {
                panic!("expected string literal as hash key: {}", pair.key());
            };
            let expected_value = expected.get(key.as_str()).expect("unexpected key");
            test_integer_literal(pair.value(), *expected_value);
        }
    }

    #[test]
    fn parses_empty_hash_literal() {
        let program = parse("{}");
        let [
            ast::Statement::Expression {
                expression: ast::Expression::HashLiteral { pairs, .. },
                ..
            },
        ] = program.statements()
        else {
            panic!("expected one hash literal expression: {program:?}");
        };
        assert_eq!(pairs.len(), 0);
    }

    #[test]
    fn parses_hash_literals_with_expressions() {
        use std::collections::HashMap;

        let program = parse(r#"{"one": 0 + 1, "two": 10 - 8, "three": 15 / 5}"#);
        let [
            ast::Statement::Expression {
                expression: ast::Expression::HashLiteral { pairs, .. },
                ..
            },
        ] = program.statements()
        else {
            panic!("expected one hash literal expression: {program:?}");
        };

        let expected = HashMap::from([
            (
                "one",
                (
                    ExpectedLiteral::Integer(0),
                    "+",
                    ExpectedLiteral::Integer(1),
                ),
            ),
            (
                "two",
                (
                    ExpectedLiteral::Integer(10),
                    "-",
                    ExpectedLiteral::Integer(8),
                ),
            ),
            (
                "three",
                (
                    ExpectedLiteral::Integer(15),
                    "/",
                    ExpectedLiteral::Integer(5),
                ),
            ),
        ]);
        for pair in pairs {
            let ast::Expression::StringLiteral { value: key, .. } = pair.key() else {
                panic!("expected string literal as hash key: {}", pair.key());
            };
            let (expected_left, expected_operator, expected_right) =
                expected.get(key.as_str()).expect("unexpected key");
            test_infix_expression(
                pair.value(),
                *expected_left,
                expected_operator,
                *expected_right,
            );
        }
    }

    #[test]
    fn parses_macro_literal() {
        let program = parse("macro(x, y) { x + y; }");
        let [
            ast::Statement::Expression {
                expression:
                    ast::Expression::MacroLiteral {
                        parameters, body, ..
                    },
                ..
            },
        ] = program.statements()
        else {
            panic!("expected one macro literal expression: {program:?}");
        };

        assert_eq!(parameters.len(), 2);
        test_literal_expression(
            &ast::Expression::Identifier(parameters[0].clone()),
            ExpectedLiteral::Identifier("x"),
        );
        test_literal_expression(
            &ast::Expression::Identifier(parameters[1].clone()),
            ExpectedLiteral::Identifier("y"),
        );

        let [ast::Statement::Expression { expression, .. }] = body.statements() else {
            panic!("expected one statement in macro body: {body:?}");
        };
        test_infix_expression(
            &expression,
            ExpectedLiteral::Identifier("x"),
            "+",
            ExpectedLiteral::Identifier("y"),
        );
    }

    fn test_integer_literal(got: &ast::Expression, expected_value: i64) {
        let ast::Expression::IntegerLiteral { token, value } = got else {
            panic!("expected an integer literal expression. got={got}");
        };
        assert_eq!(
            *value, expected_value,
            "integer literal value {value} isn't {expected_value}"
        );
        assert_eq!(token.literal(), format!("{value}"));
    }

    fn test_literal_expression(got: &ast::Expression, expected: ExpectedLiteral) {
        match expected {
            ExpectedLiteral::Integer(value) => test_integer_literal(got, value),
            ExpectedLiteral::Boolean(value) => test_boolean_literal(got, value),
            ExpectedLiteral::Identifier(value) => test_identifier(got, value),
        }
    }

    fn test_prefix_expression(
        got: &ast::Expression,
        expected_operator: &str,
        expected_value: ExpectedLiteral,
    ) {
        let ast::Expression::PrefixExpr {
            operator, right, ..
        } = got
        else {
            panic!("expression is not a prefix expression: {got}");
        };

        assert_eq!(
            *operator, expected_operator,
            "operator is not {expected_operator}. got={operator}"
        );
        test_literal_expression(right.as_ref(), expected_value);
    }

    fn test_infix_expression(
        got: &ast::Expression,
        expected_left: ExpectedLiteral,
        expected_operator: &str,
        expected_right: ExpectedLiteral,
    ) {
        let ast::Expression::InfixExpr {
            left,
            operator,
            right,
            ..
        } = got
        else {
            panic!("expression is not an infix expression: {got}");
        };

        test_literal_expression(left.as_ref(), expected_left);
        assert_eq!(
            *operator, expected_operator,
            "operator is not {expected_operator}. got={operator}"
        );
        test_literal_expression(right.as_ref(), expected_right);
    }

    fn test_boolean_literal(got: &ast::Expression, expected_value: bool) {
        let ast::Expression::BooleanLiteral { token, value } = got else {
            panic!("expected a boolean literal expression. got={got}");
        };
        assert_eq!(
            *value, expected_value,
            "boolean literal value {value} isn't {expected_value}"
        );
        assert_eq!(token.literal(), format!("{value}"));
    }

    fn test_identifier(got: &ast::Expression, expected_value: &str) {
        let ast::Expression::Identifier(identifier) = got else {
            panic!("expected an identifier expression. got={got}");
        };
        let value = identifier.value();
        assert_eq!(
            value, expected_value,
            "identifier value {value} isn't {expected_value}"
        );
        assert_eq!(identifier.token_literal(), expected_value);
    }

    fn parse(input: &str) -> ast::Program {
        let mut lexer = lexer::Lexer::new(input);
        let mut parser = Parser::new(&mut lexer);
        let program = parser.parse_program();

        assert!(
            parser.errors().is_empty(),
            "parser errors:\n{}",
            parser.errors().join("\n")
        );

        program
    }
}
