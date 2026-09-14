use std::cell::RefCell;
use std::rc::Rc;

use crate::{
    ast, builtins, environment,
    object::{self, HashPair},
};

const NULL: object::Object = object::Object::Null;
const TRUE: object::Object = object::Object::Boolean(true);
const FALSE: object::Object = object::Object::Boolean(false);

macro_rules! new_error {
    ($($args:tt)*) => {
        $crate::object::Object::Error(format!($($args)*))
    };
}

macro_rules! return_if_error {
    ($obj:expr) => {
        if matches!(&$obj, object::Object::Error(_)) {
            return $obj;
        }
    };
}

pub fn eval(program: ast::Program, env: &environment::EnvironmentRef) -> object::Object {
    let mut result = NULL;
    for statement in program.statements() {
        result = eval_statement(statement, env);
        match result {
            object::Object::ReturnValue(value) => return *value,
            object::Object::Error(_) => return result,
            _ => {}
        }
    }
    result
}

fn eval_statement(statement: &ast::Statement, env: &environment::EnvironmentRef) -> object::Object {
    match statement {
        ast::Statement::Expression { expression, .. } => eval_expression(expression, env),
        ast::Statement::Block(block) => eval_block_statement(block, env),
        ast::Statement::Return { value, .. } => {
            let val = eval_expression(value, env);
            return_if_error!(val);
            object::Object::ReturnValue(Box::new(val))
        }
        ast::Statement::Let { name, value, .. } => {
            let val = eval_expression(value, env);
            return_if_error!(val);
            env.borrow_mut().set(name.value(), val)
        }
    }
}

pub(crate) fn eval_block_statement(
    block: &ast::BlockStatement,
    env: &environment::EnvironmentRef,
) -> object::Object {
    let mut result = NULL;
    for statement in block.statements() {
        result = eval_statement(statement, env);
        match result {
            object::Object::ReturnValue(_) | object::Object::Error(_) => return result,
            _ => {}
        }
    }
    result
}

fn eval_expression(
    expression: &ast::Expression,
    env: &environment::EnvironmentRef,
) -> object::Object {
    match expression {
        ast::Expression::IntegerLiteral { value, .. } => object::Object::Integer(*value),
        ast::Expression::BooleanLiteral { value, .. } => {
            if *value {
                TRUE
            } else {
                FALSE
            }
        }
        ast::Expression::PrefixExpr {
            operator, right, ..
        } => {
            let right = eval_expression(right.as_ref(), env);
            return_if_error!(right);
            eval_prefix_expression(operator, right)
        }
        ast::Expression::InfixExpr {
            left,
            operator,
            right,
            ..
        } => {
            let left = eval_expression(left.as_ref(), env);
            return_if_error!(left);
            let right = eval_expression(right.as_ref(), env);
            return_if_error!(right);
            eval_infix_expression(operator, left, right)
        }
        ast::Expression::IfLiteral {
            condition,
            consequence,
            alternative,
            ..
        } => {
            let condition = eval_expression(condition.as_ref(), env);
            return_if_error!(condition);
            if is_truthy(&condition) {
                eval_block_statement(consequence, env)
            } else if let Some(alt) = alternative {
                eval_block_statement(alt, env)
            } else {
                NULL
            }
        }
        ast::Expression::Identifier(identifier) => {
            let value = env.borrow().get(identifier.value());
            match value {
                Some(value) => value,
                None => match builtins::BUILTINS.get(identifier.value()) {
                    Some(builtin) => object::Object::Builtin(*builtin),
                    None => new_error!("identifier not found: {}", identifier.value()),
                },
            }
        }
        ast::Expression::FunctionLiteral {
            parameters, body, ..
        } => object::Object::Function {
            parameters: parameters.clone(),
            body: body.clone(),
            env: Rc::clone(env),
        },
        ast::Expression::CallExpr {
            function,
            arguments,
            ..
        } => {
            if function.token_literal() == "quote" {
                if arguments.len() != 1 {
                    return new_error!(
                        "wrong number of arguments. got={}, want=1",
                        arguments.len()
                    );
                }
                return object::Object::Quote(eval_unquote_calls(arguments[0].clone(), env));
            }
            let function = eval_expression(function.as_ref(), env);
            return_if_error!(function);
            let args = eval_expressions(arguments, env);
            if args.len() == 1 && matches!(&args[0], object::Object::Error(_)) {
                return args[0].clone();
            }
            apply_function(function, args)
        }
        ast::Expression::StringLiteral { value, .. } => object::Object::String(value.clone()),
        ast::Expression::ArrayLiteral { elements, .. } => {
            let elements = eval_expressions(elements, env);
            if elements.len() == 1 {
                return_if_error!(elements[0].clone());
            }
            object::Object::Array(elements)
        }
        ast::Expression::IndexExpr { left, index, .. } => {
            let left = eval_expression(left, env);
            return_if_error!(left);
            let index = eval_expression(index, env);
            return_if_error!(index);
            eval_index_expression(left, index)
        }
        ast::Expression::HashLiteral { pairs, .. } => {
            let mut hash_pairs = std::collections::HashMap::new();
            for pair in pairs {
                let key = eval_expression(pair.key(), env);
                return_if_error!(key);
                let Ok(hash_key) = key.hash_key() else {
                    return new_error!("unusable as hash key: {}", key.object_type());
                };

                let value = eval_expression(pair.value(), env);
                return_if_error!(value);
                hash_pairs.insert(hash_key, HashPair::new(key, value));
            }
            object::Object::Hash(hash_pairs)
        }
        ast::Expression::MacroLiteral {
            parameters, body, ..
        } => object::Object::Macro {
            parameters: parameters.clone(),
            body: body.clone(),
            env: Rc::clone(env),
        },
    }
}

fn eval_index_expression(left: object::Object, index: object::Object) -> object::Object {
    match (left.object_type(), index.object_type()) {
        (object::ObjectType::Array, object::ObjectType::Integer) => {
            eval_array_index_expression(left, index)
        }
        (object::ObjectType::Hash, _) => eval_hash_index_expression(left, index),
        _ => new_error!("index operator not supported: {}", left.object_type()),
    }
}

fn eval_expressions(
    expressions: &[ast::Expression],
    env: &environment::EnvironmentRef,
) -> Vec<object::Object> {
    let mut result = Vec::new();
    for expression in expressions {
        let evaluated = eval_expression(expression, env);
        if matches!(&evaluated, object::Object::Error(_)) {
            return vec![evaluated];
        }
        result.push(evaluated);
    }
    result
}

fn apply_function(function: object::Object, args: Vec<object::Object>) -> object::Object {
    match function {
        object::Object::Function {
            parameters,
            body,
            env: func_env,
        } => {
            let extended_env = extend_function_env(func_env, parameters, args);
            let evaluated = eval_block_statement(&body, &extended_env);
            match evaluated {
                object::Object::ReturnValue(value) => *value,
                _ => evaluated,
            }
        }
        object::Object::Builtin(builtin) => builtin(args),
        _ => new_error!("not a function: {}", function.object_type()),
    }
}

fn extend_function_env(
    func_env: environment::EnvironmentRef,
    parameters: Vec<ast::Identifier>,
    args: Vec<object::Object>,
) -> environment::EnvironmentRef {
    let extended_env = Rc::new(RefCell::new(environment::Environment::new_enclosed(
        func_env,
    )));
    for (param, arg) in parameters.into_iter().zip(args.into_iter()) {
        extended_env.borrow_mut().set(param.value(), arg);
    }
    extended_env
}

fn is_truthy(obj: &object::Object) -> bool {
    match obj {
        object::Object::Boolean(b) => *b,
        object::Object::Null => false,
        _ => true,
    }
}

fn eval_prefix_expression(operator: &str, right: object::Object) -> object::Object {
    match operator {
        "!" => eval_bang_operator_expression(right),
        "-" => eval_minus_prefix_operator_expression(right),
        _ => new_error!("unknown operator: {}{}", operator, right.object_type()),
    }
}

fn eval_infix_expression(
    operator: &str,
    left: object::Object,
    right: object::Object,
) -> object::Object {
    match (&left, &right) {
        (object::Object::Integer(l), object::Object::Integer(r)) => match operator {
            "+" => object::Object::Integer(l + r),
            "-" => object::Object::Integer(l - r),
            "*" => object::Object::Integer(l * r),
            "/" => object::Object::Integer(l / r),
            "<" => object::Object::Boolean(l < r),
            ">" => object::Object::Boolean(l > r),
            "==" => object::Object::Boolean(l == r),
            "!=" => object::Object::Boolean(l != r),
            _ => new_error!(
                "unknown operator: {} {} {}",
                left.object_type(),
                operator,
                right.object_type()
            ),
        },
        (object::Object::Boolean(l), object::Object::Boolean(r)) => match operator {
            "==" => object::Object::Boolean(l == r),
            "!=" => object::Object::Boolean(l != r),
            _ => new_error!(
                "unknown operator: {} {} {}",
                left.object_type(),
                operator,
                right.object_type()
            ),
        },
        (object::Object::String(l), object::Object::String(r)) => match operator {
            "+" => object::Object::String(format!("{l}{r}")),
            // "==" => object::Object::Boolean(l == r),
            // "!=" => object::Object::Boolean(l != r),
            _ => new_error!(
                "unknown operator: {} {} {}",
                left.object_type(),
                operator,
                right.object_type()
            ),
        },
        _ => new_error!(
            "type mismatch: {} {} {}",
            left.object_type(),
            operator,
            right.object_type()
        ),
    }
}

fn eval_bang_operator_expression(right: object::Object) -> object::Object {
    match right {
        object::Object::Boolean(b) => {
            if b {
                FALSE
            } else {
                TRUE
            }
        }
        object::Object::Null => TRUE,
        _ => FALSE,
    }
}

fn eval_minus_prefix_operator_expression(right: object::Object) -> object::Object {
    match &right {
        object::Object::Integer(i) => object::Object::Integer(-i),
        _ => new_error!("unknown operator: -{}", right.object_type()),
    }
}

fn eval_array_index_expression(array: object::Object, index: object::Object) -> object::Object {
    if let (object::Object::Array(elements), object::Object::Integer(idx)) = (&array, &index) {
        let max = elements.len() as i64 - 1;
        if *idx < 0 || *idx > max {
            NULL
        } else {
            elements[*idx as usize].clone()
        }
    } else {
        new_error!(
            "index operator not supported: {} {}",
            array.object_type(),
            index.object_type()
        )
    }
}

fn eval_hash_index_expression(hash: object::Object, index: object::Object) -> object::Object {
    if let object::Object::Hash(pairs) = hash {
        match index.hash_key() {
            Ok(hash_key) => match pairs.get(&hash_key) {
                Some(pair) => pair.value().clone(),
                None => NULL,
            },
            Err(_) => new_error!("unusable as hash key: {}", index.object_type()),
        }
    } else {
        new_error!(
            "index operator not supported: {} {}",
            hash.object_type(),
            index.object_type()
        )
    }
}

fn eval_unquote_calls(
    mut quoted: ast::Expression,
    env: &environment::EnvironmentRef,
) -> ast::Expression {
    ast::modify_expression(&mut quoted, &mut |expression| {
        let ast::Expression::CallExpr {
            function,
            arguments,
            ..
        } = expression
        else {
            return;
        };
        if !matches!(
            function.as_ref(),
            ast::Expression::Identifier(identifier) if identifier.value() == "unquote"
        ) {
            return;
        }
        let [argument] = arguments.as_slice() else {
            return;
        };
        if let Some(replacement) = convert_object_to_ast_node(eval_expression(argument, env)) {
            *expression = replacement;
        }
    });
    quoted
}

fn convert_object_to_ast_node(value: object::Object) -> Option<ast::Expression> {
    use crate::token::{Token, TokenType};

    match value {
        object::Object::Integer(value) => Some(ast::Expression::IntegerLiteral {
            token: Token::new(TokenType::INT, value.to_string()),
            value,
        }),
        object::Object::Boolean(value) => Some(ast::Expression::BooleanLiteral {
            token: Token::new(
                if value {
                    TokenType::TRUE
                } else {
                    TokenType::FALSE
                },
                value.to_string(),
            ),
            value,
        }),
        object::Object::Quote(expression) => Some(expression),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::{lexer, parser};

    fn test_eval(input: &str) -> object::Object {
        let mut l = lexer::Lexer::new(input.to_string());
        let mut p = parser::Parser::new(&mut l);
        let program = p.parse_program();
        let env = Rc::new(RefCell::new(environment::Environment::new()));
        eval(program, &env)
    }

    fn test_integer_object(obj: &object::Object, expected: i64) {
        match obj {
            object::Object::Integer(i) => assert_eq!(*i, expected),
            _ => panic!("object is not Integer. got={:?}", obj),
        }
    }

    fn test_null_object(obj: &object::Object) {
        assert!(
            matches!(obj, object::Object::Null),
            "object is not Null. got={obj:?}"
        );
    }

    fn test_boolean_object(obj: &object::Object, expected: bool) {
        match obj {
            object::Object::Boolean(b) => assert_eq!(*b, expected),
            _ => panic!("object is not Boolean. got={:?}", obj),
        }
    }

    #[test]
    fn test_eval_integer_expression() {
        let tests = [
            ("5", 5),
            ("10", 10),
            ("-5", -5),
            ("-10", -10),
            ("5 + 5 + 5 + 5 - 10", 10),
            ("2 * 2 * 2 * 2 * 2", 32),
            ("-50 + 100 + -50", 0),
            ("5 * 2 + 10", 20),
            ("5 + 2 * 10", 25),
            ("20 + 2 * -10", 0),
            ("50 / 2 * 2 + 10", 60),
            ("2 * (5 + 10)", 30),
            ("3 * 3 * 3 + 10", 37),
            ("3 * (3 * 3) + 10", 37),
            ("(5 + 10 * 2 + 15 /3) *2 + -10", 50),
        ];
        for (input, expected) in tests {
            let evaluated = test_eval(input);
            test_integer_object(&evaluated, expected);
        }
    }

    #[test]
    fn test_eval_boolean_expression() {
        let tests = [
            ("true", true),
            ("false", false),
            ("1 < 2", true),
            ("1 > 2", false),
            ("1 < 1", false),
            ("1 > 1", false),
            ("1 == 1", true),
            ("1 != 1", false),
            ("1 == 2", false),
            ("1 != 2", true),
            ("true == true", true),
            ("false == false", true),
            ("true == false", false),
            ("true != false", true),
            ("false != true", true),
            ("(1 < 2) == true", true),
            ("(1 < 2) == false", false),
            ("(1 > 2) == true", false),
            ("(1 > 2) == false", true),
        ];

        for (input, expected) in tests {
            let evaluated = test_eval(input);
            test_boolean_object(&evaluated, expected);
        }
    }

    #[test]
    fn test_bang_operator() {
        let tests = [
            ("!true", false),
            ("!false", true),
            ("!5", false),
            ("!!true", true),
            ("!!false", false),
            ("!!5", true),
        ];

        for (input, expected) in tests {
            let evaluated = test_eval(input);
            test_boolean_object(&evaluated, expected);
        }
    }

    #[test]
    fn test_if_else_expressions() {
        let tests = [
            ("if (true) { 10 }", Some(10)),
            ("if (false) { 10 }", None),
            ("if (1) { 10 }", Some(10)),
            ("if (1 < 2) { 10 }", Some(10)),
            ("if (1 > 2) { 10 }", None),
            ("if (1 > 2) { 10 } else { 20 }", Some(20)),
            ("if (1 < 2) { 10 } else { 20 }", Some(10)),
        ];

        for (input, expected) in tests {
            let evaluated = test_eval(input);
            match expected {
                Some(expected) => test_integer_object(&evaluated, expected),
                None => assert_eq!(evaluated, NULL),
            }
        }
    }

    #[test]
    fn test_return_statements() {
        let tests = [
            ("return 10;", 10),
            ("return 10; 9;", 10),
            ("return 2 * 5; 9;", 10),
            ("9; return 2 * 5; 9;", 10),
            ("if (10 > 1) { if (10 > 1) { return 10; } return 1; }", 10),
        ];

        for (input, expected) in tests {
            let evaluated = test_eval(input);
            test_integer_object(&evaluated, expected);
        }
    }

    #[test]
    fn test_error_handling() {
        let tests = [
            ("5 + true;", "type mismatch: INTEGER + BOOLEAN"),
            ("5 + true; 5;", "type mismatch: INTEGER + BOOLEAN"),
            ("-true", "unknown operator: -BOOLEAN"),
            ("true + false;", "unknown operator: BOOLEAN + BOOLEAN"),
            ("5; true + false; 5", "unknown operator: BOOLEAN + BOOLEAN"),
            (
                "if (10 > 1) { true + false; }",
                "unknown operator: BOOLEAN + BOOLEAN",
            ),
            (
                "if (10 > 1) { if (10 > 1) { return true + false; } return 1; }",
                "unknown operator: BOOLEAN + BOOLEAN",
            ),
            ("foobar", "identifier not found: foobar"),
            (r#""Hello" - "World""#, "unknown operator: STRING - STRING"),
            (
                r#"{"name": "Monkey"}[fn(x) { x }];"#,
                "unusable as hash key: FUNCTION",
            ),
        ];

        for (input, expected_message) in tests {
            let evaluated = test_eval(input);
            match evaluated {
                object::Object::Error(message) => assert_eq!(message, expected_message),
                _ => panic!("no error object returned. got={:?}", evaluated),
            }
        }
    }

    #[test]
    fn test_let_statements() {
        let tests = [
            ("let a = 5; a;", 5),
            ("let a = 5 * 5; a;", 25),
            ("let a = 5; let b = a; b;", 5),
            ("let a = 5; let b = a; let c = a + b + 5; c;", 15),
        ];

        for (input, expected) in tests {
            let evaluated = test_eval(input);
            test_integer_object(&evaluated, expected);
        }
    }

    #[test]
    fn test_function_object() {
        let input = "fn(x) { x + 2; };";
        let object::Object::Function {
            parameters, body, ..
        } = test_eval(input)
        else {
            panic!("object is not Function. got={:?}", test_eval(input));
        };
        assert_eq!(parameters.len(), 1);
        assert_eq!(parameters[0].value(), "x");
        assert_eq!(body.to_string(), "(x + 2)");
    }

    #[test]
    fn function_retains_shared_environment() {
        let mut lexer = lexer::Lexer::new("fn() { answer; };");
        let mut parser = parser::Parser::new(&mut lexer);
        let program = parser.parse_program();
        assert!(parser.errors().is_empty(), "{:?}", parser.errors());

        let env = Rc::new(RefCell::new(environment::Environment::new()));
        let object::Object::Function { env: captured, .. } = eval(program, &env) else {
            panic!("expected a function object");
        };

        assert!(Rc::ptr_eq(&env, &captured));
        env.borrow_mut().set("answer", object::Object::Integer(42));
        drop(env);

        assert_eq!(
            captured.borrow().get("answer"),
            Some(object::Object::Integer(42))
        );
    }

    #[test]
    fn test_function_application() {
        let tests = [
            ("let identity = fn(x) { x; }; identity(5);", 5),
            ("let identity = fn(x) { return x; }; identity(5);", 5),
            ("let double = fn(x) { x * 2; }; double(5);", 10),
            ("let add = fn(x, y) { x + y; }; add(5, 5);", 10),
            ("let add = fn(x, y) { x + y; }; add(5 + 5, add(5, 5));", 20),
            ("fn(x) { x; }(5)", 5),
        ];

        for (input, expected) in tests {
            let evaluated = test_eval(input);
            test_integer_object(&evaluated, expected);
        }
    }

    #[test]
    fn test_closures() {
        let input = "
            let newAdder = fn(x) {
                fn(y) { x + y };
            };
            let addTwo = newAdder(2);
            addTwo(2);
        ";
        let evaluated = test_eval(input);
        test_integer_object(&evaluated, 4);
    }

    #[test]
    fn test_string_literal() {
        let input = r#""Hello World!""#;
        let evaluated = test_eval(input);
        if let object::Object::String(s) = evaluated {
            assert_eq!(s, "Hello World!");
        } else {
            panic!("object is not String. got={:?}", evaluated);
        }
    }

    #[test]
    fn test_string_concatenation() {
        let input = r#""Hello" + " " + "World!""#;
        let evaluated = test_eval(input);
        if let object::Object::String(s) = evaluated {
            assert_eq!(s, "Hello World!");
        } else {
            panic!("object is not String. got={:?}", evaluated);
        }
    }

    #[test]
    fn test_builtin_functions() {
        let tests = [
            (r#"len("")"#, object::Object::Integer(0)),
            (r#"len("four")"#, object::Object::Integer(4)),
            (r#"len("hello world")"#, object::Object::Integer(11)),
            (
                r#"len(1)"#,
                object::Object::Error("argument to `len` not supported, got INTEGER".to_string()),
            ),
            (
                r#"len("one", "two")"#,
                object::Object::Error("wrong number of arguments. got=2, want=1".to_string()),
            ),
        ];

        for (input, expected) in tests {
            let evaluated = test_eval(input);
            assert_eq!(evaluated, expected);
        }
    }

    #[test]
    fn test_array_literals() {
        let input = "[1, 2 * 2, 3 + 3]";
        let evaluated = test_eval(input);
        let object::Object::Array(elements) = evaluated else {
            panic!("object is not Array. got={:?}", evaluated);
        };
        assert_eq!(elements.len(), 3);
        test_integer_object(&elements[0], 1);
        test_integer_object(&elements[1], 4);
        test_integer_object(&elements[2], 6);
    }

    #[test]
    fn test_array_index_expressions() {
        let tests = [
            ("[1, 2, 3][0]", Some(1)),
            ("[1, 2, 3][1]", Some(2)),
            ("[1, 2, 3][2]", Some(3)),
            ("let i = 0; [1][i];", Some(1)),
            ("[1, 2, 3][1 + 1];", Some(3)),
            ("let myArray = [1, 2, 3]; myArray[2];", Some(3)),
            (
                "let myArray = [1, 2, 3]; myArray[0] + myArray[1] + myArray[2];",
                Some(6),
            ),
            (
                "let myArray = [1, 2, 3]; let i = myArray[0]; myArray[i]",
                Some(2),
            ),
            ("[1, 2, 3][3]", None),
            ("[1, 2, 3][-1]", None),
        ];

        for (input, expected) in tests {
            let evaluated = test_eval(input);
            match expected {
                Some(value) => test_integer_object(&evaluated, value),
                None => test_null_object(&evaluated),
            }
        }
    }

    #[test]
    fn test_hash_literal() {
        use std::collections::HashMap;
        let input = r#"let two = "two";
        {
            "one": 10 - 9,
            two: 1 + 1,
            "thr" + "ee": 6 / 2,
            4: 4,
            true: 5,
            false: 6
        }"#;
        let evaluated = test_eval(input);
        let object::Object::Hash(pairs) = evaluated else {
            panic!("object is not Hash. got={:?}", evaluated);
        };
        let expected: HashMap<object::HashKey, i64> = HashMap::from([
            (
                object::Object::String("one".to_string())
                    .hash_key()
                    .unwrap(),
                1,
            ),
            (
                object::Object::String("two".to_string())
                    .hash_key()
                    .unwrap(),
                2,
            ),
            (
                object::Object::String("three".to_string())
                    .hash_key()
                    .unwrap(),
                3,
            ),
            (object::Object::Integer(4).hash_key().unwrap(), 4),
            (object::Object::Boolean(true).hash_key().unwrap(), 5),
            (object::Object::Boolean(false).hash_key().unwrap(), 6),
        ]);
        assert_eq!(pairs.len(), expected.len());
        for (expected_key, expected_value) in expected {
            let pair = pairs.get(&expected_key).unwrap();
            test_integer_object(pair.value(), expected_value);
        }
    }

    #[test]
    fn test_hash_index_expressions() {
        let tests = [
            (r#"{"foo": 5}["foo"]"#, Some(5)),
            (r#"{"foo": 5}["bar"]"#, None),
            (r#"let key = "foo"; {"foo": 5}[key]"#, Some(5)),
            (r#"{}["foo"]"#, None),
            (r#"{5: 5}[5]"#, Some(5)),
            (r#"{true: 5}[true]"#, Some(5)),
            (r#"{false: 5}[false]"#, Some(5)),
        ];
        for (input, expected) in tests {
            let evaluated = test_eval(input);
            match expected {
                Some(value) => test_integer_object(&evaluated, value),
                None => test_null_object(&evaluated),
            }
        }
    }

    #[test]
    fn test_quote() {
        let tests = [
            ("quote(5)", "5"),
            ("quote(5 + 8)", "(5 + 8)"),
            ("quote(foobar)", "foobar"),
            ("quote(foobar + barfoo)", "(foobar + barfoo)"),
        ];
        for (input, expected) in tests {
            let evaluated = test_eval(input);
            let object::Object::Quote(expr) = evaluated else {
                panic!("object is not Quote. got={:?}", evaluated);
            };
            assert_eq!(expr.to_string(), expected);
        }
    }

    #[test]
    fn test_quote_unquote() {
        let tests = [
            ("quote(unquote(4))", "4"),
            ("quote(unquote(4 + 4))", "8"),
            ("quote(8 + unquote(4 + 4))", "(8 + 8)"),
            ("quote(unquote(4 + 4) + 8)", "(8 + 8)"),
            ("quote(unquote(-4))", "-4"),
            ("quote(unquote(true))", "true"),
            ("quote(unquote(false))", "false"),
            ("quote(unquote(1 > 2))", "false"),
            ("let value = 8; quote(unquote(value));", "8"),
            ("quote(unquote(quote(4 + 4)))", "(4 + 4)"),
            (
                "let quoted = quote(4 + 4); quote(unquote(quoted) + unquote(2 + 2));",
                "((4 + 4) + 4)",
            ),
            ("quote(add(unquote(1 + 1)))", "add(2)"),
            ("quote([unquote(1 + 1)])", "[2]"),
            ("quote(unquote(true))", "true"),
            ("quote(unquote(false))", "false"),
            ("quote({unquote(1 + 1): unquote(2 + 2)})", "{2: 4}"),
        ];
        for (input, expected) in tests {
            let evaluated = test_eval(input);
            let object::Object::Quote(expr) = evaluated else {
                panic!("object is not Quote. got={:?}", evaluated);
            };
            assert_eq!(expr.to_string(), expected);
        }
    }

    #[test]
    fn preserves_unquote_calls_that_cannot_be_replaced() {
        for (input, expected) in [
            ("quote(unquote())", "unquote()"),
            ("quote(unquote(1, 2))", "unquote(1, 2)"),
            ("quote(unquote(missing))", "unquote(missing)"),
            ("quote(unquote(1 + true))", "unquote((1 + true))"),
            ("quote(unquote([1, 2]))", "unquote([1, 2])"),
            ("quote(unquote(if (false) { 1; }))", "unquote(if false 1)"),
            ("quote(\"unquote\"(4))", "\"unquote\"(4)"),
        ] {
            let evaluated = test_eval(input);
            let object::Object::Quote(expression) = evaluated else {
                panic!("expected Quote for {input}: {evaluated:?}");
            };
            assert_eq!(expression.to_string(), expected, "{input}");
        }
    }
}
