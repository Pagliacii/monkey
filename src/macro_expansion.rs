use std::{cell::RefCell, rc::Rc};

use crate::{ast, environment, evaluator, object::Object};

pub fn define_macros(program: &mut ast::Program, env: &environment::EnvironmentRef) {
    program.retain_statements(|statement| {
        let ast::Statement::Let {
            name,
            value:
                ast::Expression::MacroLiteral {
                    parameters, body, ..
                },
            ..
        } = statement
        else {
            return true;
        };
        env.borrow_mut().set(
            name.value(),
            Object::Macro {
                parameters: parameters.clone(),
                body: body.clone(),
                env: Rc::clone(env),
            },
        );
        false
    });
}

pub fn expand_macros(
    program: &mut ast::Program,
    env: &environment::EnvironmentRef,
) -> Result<(), String> {
    let mut error = None;
    ast::modify_program(program, &mut |expression| {
        if error.is_some() {
            return;
        }
        let ast::Expression::CallExpr {
            function,
            arguments,
            ..
        } = expression
        else {
            return;
        };
        let ast::Expression::Identifier(identifier) = function.as_ref() else {
            return;
        };
        let value = env.borrow().get(identifier.value());
        let Some(Object::Macro {
            parameters,
            body,
            env: defining_env,
        }) = value
        else {
            return;
        };
        if parameters.len() != arguments.len() {
            error = Some(format!(
                "wrong number of arguments. got={}, want={}",
                arguments.len(),
                parameters.len()
            ));
            return;
        }

        let extended_env = Rc::new(RefCell::new(environment::Environment::new_enclosed(
            defining_env,
        )));
        for (parameter, argument) in parameters.iter().zip(arguments.iter()) {
            extended_env
                .borrow_mut()
                .set(parameter.value(), Object::Quote(argument.clone()));
        }
        let evaluated = evaluator::eval_block_statement(&body, &extended_env);
        let evaluated = match evaluated {
            Object::ReturnValue(value) => *value,
            other => other,
        };
        match evaluated {
            Object::Quote(replacement) => *expression = replacement,
            Object::Error(message) => error = Some(message),
            other => {
                error = Some(format!(
                    "macro must return QUOTE, got {}",
                    other.object_type()
                ))
            }
        }
    });
    match error {
        Some(message) => Err(message),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;
    use crate::{evaluator, lexer, object::Object, parser};

    fn parse(input: &str) -> ast::Program {
        let mut lexer = lexer::Lexer::new(input);
        let mut parser = parser::Parser::new(&mut lexer);
        let program = parser.parse_program();
        assert!(parser.errors().is_empty(), "{:?}", parser.errors());
        program
    }

    fn environment() -> environment::EnvironmentRef {
        Rc::new(RefCell::new(environment::Environment::new()))
    }

    #[test]
    fn test_macro_definition() {
        let mut program = parse(
            "let number = 1;
             let mymacro = macro(x, y) { x + y; };
             let another = macro() { quote(1); };
             let function = fn(x, y) { x + y; };",
        );
        let env = environment();
        define_macros(&mut program, &env);
        assert_eq!(
            program,
            parse("let number = 1; let function = fn(x, y) { x + y; };")
        );
        assert!(env.borrow().get("number").is_none());
        assert!(env.borrow().get("function").is_none());
        assert!(matches!(
            env.borrow().get("another"),
            Some(Object::Macro { .. })
        ));

        let Some(Object::Macro {
            parameters,
            body,
            env: captured,
        }) = env.borrow().get("mymacro")
        else {
            panic!("expected mymacro to be a macro");
        };
        assert_eq!(
            parameters
                .iter()
                .map(|parameter| parameter.value())
                .collect::<Vec<_>>(),
            ["x", "y"]
        );
        assert_eq!(body.to_string(), "(x + y)");
        assert!(Rc::ptr_eq(&captured, &env));
    }

    #[test]
    fn test_macro_expansion() {
        for (input, expected) in [
            (
                "let infixExpression = macro() { quote(1 + 2); }; infixExpression();",
                "1 + 2",
            ),
            (
                "let reverse = macro(a, b) { quote(unquote(b) - unquote(a)); }; reverse(2 + 2, 10 - 5);",
                "(10 - 5) - (2 + 2)",
            ),
            (
                "let identity = macro(value) { quote(unquote(value)); }; identity(missing + 1);",
                "missing + 1",
            ),
            (
                "let one = macro() { return quote(1); }; [one(), one()];",
                "[1, 1]",
            ),
            (
                "let one = macro() { quote(1); }; let value = one(); fn() { return one(); };",
                "let value = 1; fn() { return 1; };",
            ),
            (
                "let unless = macro(condition, consequence, alternative) { quote(if (!(unquote(condition))) { unquote(consequence); } else { unquote(alternative); }); }; unless(10 > 5, puts(\"not greater\"), puts(\"greater\"));",
                "if (!(10 > 5)) { puts(\"not greater\"); } else { puts(\"greater\"); }",
            ),
        ] {
            let mut program = parse(input);
            let env = environment();
            define_macros(&mut program, &env);
            expand_macros(&mut program, &env).unwrap();
            assert_eq!(program.to_string(), parse(expected).to_string(), "{input}");
        }
    }

    #[test]
    fn leaves_non_macro_calls_unchanged() {
        let input = "let function = fn(value) { value; }; function(1); missing(2);";
        let mut program = parse(input);
        let env = environment();
        env.borrow_mut().set("function", Object::Integer(7));
        define_macros(&mut program, &env);
        expand_macros(&mut program, &env).unwrap();
        assert_eq!(program, parse(input));
    }

    #[test]
    fn uses_the_definition_environment_without_leaking_parameters() {
        let defining_env = environment();
        defining_env.borrow_mut().set("value", Object::Integer(9));
        let mut definitions = parse(
            "let value_macro = macro() { quote(unquote(value)); }; let identity = macro(value) { quote(unquote(value)); };",
        );
        define_macros(&mut definitions, &defining_env);
        let calling_env = environment();
        calling_env.borrow_mut().set(
            "value_macro",
            defining_env.borrow().get("value_macro").unwrap(),
        );
        calling_env.borrow_mut().set("value", Object::Integer(99));
        let mut program = parse("value_macro();");
        expand_macros(&mut program, &calling_env).unwrap();
        assert_eq!(program.to_string(), "9");

        let mut program = parse("identity(1); identity(2);");
        expand_macros(&mut program, &defining_env).unwrap();
        assert_eq!(program.to_string(), parse("1; 2;").to_string());
        assert_eq!(defining_env.borrow().get("value"), Some(Object::Integer(9)));
    }

    #[test]
    fn reports_invalid_macro_calls() {
        for (input, expected) in [
            (
                "let identity = macro(value) { quote(unquote(value)); }; identity();",
                "wrong number of arguments. got=0, want=1",
            ),
            (
                "let identity = macro(value) { quote(unquote(value)); }; identity(1, 2);",
                "wrong number of arguments. got=2, want=1",
            ),
            (
                "let invalid = macro() { 1; }; invalid();",
                "macro must return QUOTE, got INTEGER",
            ),
            (
                "let invalid = macro() { missing; }; invalid();",
                "identifier not found: missing",
            ),
            (
                "let invalid = macro() {}; invalid();",
                "macro must return QUOTE, got NULL",
            ),
        ] {
            let mut program = parse(input);
            let env = environment();
            define_macros(&mut program, &env);
            assert_eq!(
                expand_macros(&mut program, &env),
                Err(expected.to_owned()),
                "{input}"
            );
        }
    }

    #[test]
    fn evaluates_the_expanded_program() {
        let mut program = parse(
            "let reverse = macro(a, b) { quote(unquote(b) - unquote(a)); }; reverse(2 + 2, 10 - 5);",
        );
        let macro_env = environment();
        define_macros(&mut program, &macro_env);
        expand_macros(&mut program, &macro_env).unwrap();
        assert_eq!(evaluator::eval(program, &environment()), Object::Integer(1));
    }
}
