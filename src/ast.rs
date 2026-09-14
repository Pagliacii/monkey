use std::fmt;

use crate::token;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Program {
    statements: Vec<Statement>,
}

impl Program {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn statements(&self) -> &[Statement] {
        &self.statements
    }

    pub fn add_statement(&mut self, statement: Statement) {
        self.statements.push(statement);
    }

    pub(crate) fn retain_statements(&mut self, predicate: impl FnMut(&Statement) -> bool) {
        self.statements.retain(predicate);
    }

    #[allow(dead_code)]
    pub fn token_literal(&self) -> String {
        self.statements
            .first()
            .map_or_else(String::new, Statement::token_literal)
    }
}

impl fmt::Display for Program {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for statement in &self.statements {
            write!(formatter, "{statement}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Statement {
    Let {
        token: token::Token,
        name: Identifier,
        value: Expression,
    },
    Return {
        token: token::Token,
        value: Expression,
    },
    Expression {
        token: token::Token,
        expression: Expression,
    },
    Block(BlockStatement),
}

impl Statement {
    pub fn token_literal(&self) -> String {
        match self {
            Self::Let { token, .. }
            | Self::Return { token, .. }
            | Self::Expression { token, .. }
            | Self::Block(BlockStatement { token, .. }) => token.literal(),
        }
    }
}

impl fmt::Display for Statement {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Let {
                token, name, value, ..
            } => write!(formatter, "{} {name} = {value};", token.literal()),
            Self::Return { token, value } => {
                write!(formatter, "{} {value};", token.literal())
            }
            Self::Expression { expression, .. } => write!(formatter, "{expression}"),
            Self::Block(BlockStatement { statements, .. }) => {
                for statement in statements {
                    write!(formatter, "{statement}")?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockStatement {
    token: token::Token,
    statements: Vec<Statement>,
}

impl BlockStatement {
    pub fn new(token: token::Token, statements: Vec<Statement>) -> Self {
        Self { token, statements }
    }

    pub fn statements(&self) -> &[Statement] {
        &self.statements
    }

    pub fn token_literal(&self) -> String {
        self.token.literal()
    }
}

impl fmt::Display for BlockStatement {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for statement in &self.statements {
            write!(formatter, "{statement}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expression {
    Identifier(Identifier),
    IntegerLiteral {
        token: token::Token,
        value: i64,
    },
    BooleanLiteral {
        token: token::Token,
        value: bool,
    },
    PrefixExpr {
        token: token::Token,
        operator: String,
        right: Box<Expression>,
    },
    InfixExpr {
        token: token::Token,
        left: Box<Expression>,
        operator: String,
        right: Box<Expression>,
    },
    IfLiteral {
        token: token::Token,
        condition: Box<Expression>,
        consequence: BlockStatement,
        alternative: Option<BlockStatement>,
    },
    FunctionLiteral {
        token: token::Token,
        parameters: Vec<Identifier>,
        body: BlockStatement,
    },
    CallExpr {
        token: token::Token,
        function: Box<Expression>,
        arguments: Vec<Expression>,
    },
    StringLiteral {
        token: token::Token,
        value: String,
    },
    ArrayLiteral {
        token: token::Token,
        elements: Vec<Expression>,
    },
    IndexExpr {
        token: token::Token,
        left: Box<Expression>,
        index: Box<Expression>,
    },
    HashLiteral {
        token: token::Token,
        pairs: Vec<HashPair>,
    },
    MacroLiteral {
        token: token::Token,
        parameters: Vec<Identifier>,
        body: BlockStatement,
    },
}

impl Expression {
    pub fn token_literal(&self) -> String {
        match self {
            Self::Identifier(identifier) => identifier.token_literal(),
            Self::IntegerLiteral { token, .. } => token.literal(),
            Self::PrefixExpr { token, .. } => token.literal(),
            Self::InfixExpr { token, .. } => token.literal(),
            Self::BooleanLiteral { token, .. } => token.literal(),
            Self::IfLiteral { token, .. } => token.literal(),
            Self::FunctionLiteral { token, .. } => token.literal(),
            Self::CallExpr { token, .. } => token.literal(),
            Self::StringLiteral { token, .. } => token.literal(),
            Self::ArrayLiteral { token, .. } => token.literal(),
            Self::IndexExpr { token, .. } => token.literal(),
            Self::HashLiteral { token, .. } => token.literal(),
            Self::MacroLiteral { token, .. } => token.literal(),
        }
    }
}

impl fmt::Display for Expression {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identifier(identifier) => write!(formatter, "{identifier}"),
            Self::IntegerLiteral { value, .. } => write!(formatter, "{value}"),
            Self::PrefixExpr {
                operator, right, ..
            } => write!(formatter, "({operator}{right})"),
            Self::InfixExpr {
                left,
                operator,
                right,
                ..
            } => write!(formatter, "({left} {operator} {right})"),
            Self::BooleanLiteral { value, .. } => write!(formatter, "{value}"),
            Self::IfLiteral {
                condition,
                consequence,
                alternative,
                ..
            } => {
                write!(formatter, "if {condition} {consequence}")?;
                if let Some(alt) = alternative {
                    write!(formatter, " else {alt}")?;
                }
                Ok(())
            }
            Self::FunctionLiteral {
                parameters, body, ..
            } => {
                let params = parameters
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(formatter, "fn({params}) {body}")
            }
            Self::CallExpr {
                function,
                arguments,
                ..
            } => {
                let args = arguments
                    .iter()
                    .map(|a| a.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(formatter, "{function}({args})")
            }
            Self::StringLiteral { value, .. } => write!(formatter, "\"{value}\""),
            Self::ArrayLiteral { elements, .. } => {
                let elems = elements
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(formatter, "[{elems}]")
            }
            Self::IndexExpr { left, index, .. } => write!(formatter, "({left})[{index}]"),
            Self::HashLiteral { pairs, .. } => {
                let pairs_str = pairs
                    .iter()
                    .map(|pair| format!("{}: {}", pair.key, pair.value))
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(formatter, "{{{pairs_str}}}")
            }
            Self::MacroLiteral {
                parameters,
                body,
                token,
            } => {
                let params = parameters
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(formatter, "{}({params}) {body}", token.literal())
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Identifier {
    token: token::Token,
    value: String,
}

impl Identifier {
    pub fn new(token: token::Token, value: impl Into<String>) -> Self {
        Self {
            token,
            value: value.into(),
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn token_literal(&self) -> String {
        self.token.literal()
    }
}

impl fmt::Display for Identifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.value)
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct HashPair {
    key: Expression,
    value: Expression,
}

impl HashPair {
    pub fn new(key: Expression, value: Expression) -> Self {
        Self { key, value }
    }

    pub fn key(&self) -> &Expression {
        &self.key
    }

    pub fn value(&self) -> &Expression {
        &self.value
    }
}

pub type ModifierFunc<'a> = dyn FnMut(&mut Expression) + 'a;

pub fn modify_program(program: &mut Program, modifier: &mut ModifierFunc<'_>) {
    for statement in &mut program.statements {
        modify_statement(statement, modifier);
    }
}

pub fn modify_statement(statement: &mut Statement, modifier: &mut ModifierFunc<'_>) {
    match statement {
        Statement::Let { value, .. }
        | Statement::Return { value, .. }
        | Statement::Expression {
            expression: value, ..
        } => modify_expression(value, modifier),
        Statement::Block(block) => modify_block(block, modifier),
    }
}

fn modify_block(block: &mut BlockStatement, modifier: &mut ModifierFunc<'_>) {
    for statement in &mut block.statements {
        modify_statement(statement, modifier);
    }
}

pub fn modify_expression(expression: &mut Expression, modifier: &mut ModifierFunc<'_>) {
    match expression {
        Expression::PrefixExpr { right, .. } => modify_expression(right, modifier),
        Expression::InfixExpr { left, right, .. } => {
            modify_expression(left, modifier);
            modify_expression(right, modifier);
        }
        Expression::IfLiteral {
            condition,
            consequence,
            alternative,
            ..
        } => {
            modify_expression(condition, modifier);
            modify_block(consequence, modifier);
            if let Some(alternative) = alternative {
                modify_block(alternative, modifier);
            }
        }
        Expression::FunctionLiteral { body, .. } => modify_block(body, modifier),
        Expression::CallExpr {
            function,
            arguments,
            ..
        } => {
            modify_expression(function, modifier);
            for argument in arguments {
                modify_expression(argument, modifier);
            }
        }
        Expression::ArrayLiteral { elements, .. } => {
            for element in elements {
                modify_expression(element, modifier);
            }
        }
        Expression::IndexExpr { left, index, .. } => {
            modify_expression(left, modifier);
            modify_expression(index, modifier);
        }
        Expression::HashLiteral { pairs, .. } => {
            for pair in pairs {
                modify_expression(&mut pair.key, modifier);
                modify_expression(&mut pair.value, modifier);
            }
        }
        Expression::Identifier(_)
        | Expression::IntegerLiteral { .. }
        | Expression::BooleanLiteral { .. }
        | Expression::StringLiteral { .. }
        | Expression::MacroLiteral { .. } => {}
    }
    modifier(expression);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_program(input: &str) -> Program {
        let mut lexer = crate::lexer::Lexer::new(input);
        let mut parser = crate::parser::Parser::new(&mut lexer);
        let program = parser.parse_program();
        assert!(parser.errors().is_empty(), "{:?}", parser.errors());
        program
    }

    fn turn_one_into_two(expression: &mut Expression) {
        if matches!(expression, Expression::IntegerLiteral { value: 1, .. }) {
            *expression = Expression::IntegerLiteral {
                token: token::Token::new(token::TokenType::INT, "2"),
                value: 2,
            };
        }
    }

    #[test]
    fn modifies_all_expression_children_in_place() {
        for (input, expected) in [
            ("1", "2"),
            ("-1", "-2"),
            ("1 + 1", "2 + 2"),
            ("if (1) { return 1; }", "if (2) { return 2; }"),
            (
                "if (1) { return 1; } else { return 1; }",
                "if (2) { return 2; } else { return 2; }",
            ),
            ("fn(value) { return 1; }", "fn(value) { return 2; }"),
            (
                "fn(value) { return 1; }(1, 1)",
                "fn(value) { return 2; }(2, 2)",
            ),
            ("[1, 1][1]", "[2, 2][2]"),
            ("{1: 1, 1 + 1: 1}", "{2: 2, 2 + 2: 2}"),
            ("[]", "[]"),
            ("{}", "{}"),
            ("fn() {}", "fn() {}"),
            (
                "[name, true, false, \"one\", 3]",
                "[name, true, false, \"one\", 3]",
            ),
        ] {
            let mut program = parse_program(input);
            let expected = parse_program(expected);
            let [Statement::Expression { expression, .. }] = program.statements.as_mut_slice()
            else {
                panic!("expected one expression");
            };
            let [
                Statement::Expression {
                    expression: expected,
                    ..
                },
            ] = expected.statements()
            else {
                panic!("expected one expression");
            };
            modify_expression(expression, &mut turn_one_into_two);
            assert_eq!(expression, expected, "{input}");
        }
    }

    #[test]
    fn modifies_program_and_block_statements_in_place() {
        let input = "let value = 1; return 1; (1 + 1);";
        let expected = "let value = 2; return 2; (2 + 2);";
        let mut program = parse_program(input);
        modify_program(&mut program, &mut turn_one_into_two);
        assert_eq!(program, parse_program(expected));

        let block = |input| {
            Statement::Block(BlockStatement::new(
                token::Token::new(token::TokenType::LBRACE, "{"),
                parse_program(input).statements,
            ))
        };
        let mut statement = block(input);
        modify_statement(&mut statement, &mut turn_one_into_two);
        assert_eq!(statement, block(expected));
    }

    #[test]
    fn visits_children_before_their_parent_and_can_replace_the_parent() {
        let mut program = parse_program("1 + 1;");
        let mut visited = Vec::new();
        modify_program(&mut program, &mut |expression| {
            visited.push(expression.to_string());
            turn_one_into_two(expression);
            if matches!(expression, Expression::InfixExpr { .. }) {
                *expression = Expression::BooleanLiteral {
                    token: token::Token::new(token::TokenType::TRUE, "true"),
                    value: true,
                };
            }
        });
        assert_eq!(visited, ["1", "1", "(2 + 2)"]);
        let [Statement::Expression { expression, .. }] = program.statements() else {
            panic!("expected one expression");
        };
        assert_eq!(
            expression,
            &Expression::BooleanLiteral {
                token: token::Token::new(token::TokenType::TRUE, "true"),
                value: true,
            }
        );
    }

    #[test]
    fn formats_let_statement() {
        let mut program = Program::new();
        program.add_statement(Statement::Let {
            token: token::Token::new(token::TokenType::LET, "let"),
            name: Identifier::new(token::Token::new(token::TokenType::IDENT, "myVar"), "myVar"),
            value: Expression::Identifier(Identifier::new(
                token::Token::new(token::TokenType::IDENT, "anotherVar"),
                "anotherVar",
            )),
        });

        assert_eq!(program.to_string(), "let myVar = anotherVar;");
    }

    #[test]
    fn test_modify() {
        let one = || Expression::IntegerLiteral {
            token: token::Token::new(token::TokenType::INT, "1"),
            value: 1,
        };
        let two = || Expression::IntegerLiteral {
            token: token::Token::new(token::TokenType::INT, "2"),
            value: 2,
        };

        let mut turn_one_into_two = |expression: &mut Expression| {
            if matches!(expression, Expression::IntegerLiteral { value: 1, .. }) {
                *expression = two();
            }
        };

        let mut modified_expr = one();
        modify_expression(&mut modified_expr, &mut turn_one_into_two);
        assert_eq!(modified_expr, two());

        let infix = |left: Expression, right: Expression| Expression::InfixExpr {
            token: token::Token::new(token::TokenType::PLUS, "+"),
            left: Box::new(left),
            operator: "+".to_owned(),
            right: Box::new(right),
        };
        for (mut expression, expected) in [
            (infix(one(), two()), infix(two(), two())),
            (infix(two(), one()), infix(two(), two())),
            (infix(one(), one()), infix(two(), two())),
            (infix(two(), two()), infix(two(), two())),
        ] {
            modify_expression(&mut expression, &mut turn_one_into_two);
            assert_eq!(expression, expected);
        }

        let prefix = |right: Expression| Expression::PrefixExpr {
            token: token::Token::new(token::TokenType::MINUS, "-"),
            operator: "-".to_owned(),
            right: Box::new(right),
        };
        for (mut expression, expected) in [
            (prefix(one()), prefix(two())),
            (prefix(two()), prefix(two())),
        ] {
            modify_expression(&mut expression, &mut turn_one_into_two);
            assert_eq!(expression, expected);
        }

        let index = |left: Expression, index: Expression| Expression::IndexExpr {
            token: token::Token::new(token::TokenType::LBRACKET, "["),
            left: Box::new(left),
            index: Box::new(index),
        };
        for (mut expression, expected) in [
            (index(one(), two()), index(two(), two())),
            (index(two(), one()), index(two(), two())),
            (index(one(), one()), index(two(), two())),
            (index(two(), two()), index(two(), two())),
        ] {
            modify_expression(&mut expression, &mut turn_one_into_two);
            assert_eq!(expression, expected);
        }

        let if_literal =
            |condition: Expression,
             consequence: BlockStatement,
             alternative: Option<BlockStatement>| Expression::IfLiteral {
                token: token::Token::new(token::TokenType::IF, "if"),
                condition: Box::new(condition),
                consequence,
                alternative,
            };
        let block = |input| {
            BlockStatement::new(
                token::Token::new(token::TokenType::LBRACE, "{"),
                parse_program(input).statements,
            )
        };
        for (mut expression, expected) in [
            (
                if_literal(one(), block("return 1;"), Some(block("return 1;"))),
                if_literal(two(), block("return 2;"), Some(block("return 2;"))),
            ),
            (
                if_literal(two(), block("return 1;"), Some(block("return 1;"))),
                if_literal(two(), block("return 2;"), Some(block("return 2;"))),
            ),
            (
                if_literal(one(), block("return 2;"), Some(block("return 1;"))),
                if_literal(two(), block("return 2;"), Some(block("return 2;"))),
            ),
            (
                if_literal(two(), block("return 2;"), Some(block("return 1;"))),
                if_literal(two(), block("return 2;"), Some(block("return 2;"))),
            ),
        ] {
            modify_expression(&mut expression, &mut turn_one_into_two);
            assert_eq!(expression, expected);
        }

        let return_statement = |value: Expression| Statement::Return {
            token: token::Token::new(token::TokenType::RETURN, "return"),
            value,
        };
        let mut modified_statement = return_statement(one());
        modify_statement(&mut modified_statement, &mut turn_one_into_two);
        assert_eq!(modified_statement, return_statement(two()));

        let let_statement = |name: &str, value: Expression| Statement::Let {
            token: token::Token::new(token::TokenType::LET, "let"),
            name: Identifier::new(token::Token::new(token::TokenType::IDENT, name), name),
            value,
        };
        let mut modified_statement = let_statement("myVar", one());
        modify_statement(&mut modified_statement, &mut turn_one_into_two);
        assert_eq!(modified_statement, let_statement("myVar", two()));

        let function_literal =
            |parameters: Vec<Identifier>, body: BlockStatement| Expression::FunctionLiteral {
                token: token::Token::new(token::TokenType::FUNCTION, "fn"),
                parameters,
                body,
            };
        let mut modified_expression = function_literal(
            vec![Identifier::new(
                token::Token::new(token::TokenType::IDENT, "value"),
                "value",
            )],
            block("return 1;"),
        );
        modify_expression(&mut modified_expression, &mut turn_one_into_two);
        assert_eq!(
            modified_expression,
            function_literal(
                vec![Identifier::new(
                    token::Token::new(token::TokenType::IDENT, "value"),
                    "value",
                )],
                block("return 2;")
            )
        );

        let array_literal = |elements: Vec<Expression>| Expression::ArrayLiteral {
            token: token::Token::new(token::TokenType::LBRACKET, "["),
            elements,
        };
        let mut modified_expression = array_literal(vec![one(), two()]);
        modify_expression(&mut modified_expression, &mut turn_one_into_two);
        assert_eq!(modified_expression, array_literal(vec![two(), two()]));

        let hash_literal = |pairs: Vec<HashPair>| Expression::HashLiteral {
            token: token::Token::new(token::TokenType::LBRACE, "{"),
            pairs,
        };
        let mut modified_expression = hash_literal(vec![
            HashPair::new(one(), two()),
            HashPair::new(two(), one()),
        ]);
        modify_expression(&mut modified_expression, &mut turn_one_into_two);
        assert_eq!(
            modified_expression,
            hash_literal(vec![
                HashPair::new(two(), two()),
                HashPair::new(two(), two())
            ])
        );

        let program_with = |expr: Expression| Program {
            statements: vec![Statement::Expression {
                token: token::Token::new(token::TokenType::INT, "1"),
                expression: expr,
            }],
        };
        let mut modified_program = program_with(one());
        modify_program(&mut modified_program, &mut turn_one_into_two);
        assert_eq!(modified_program, program_with(two()));
    }
}
