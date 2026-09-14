use crate::object::Object;
use std::{collections::HashMap, sync::LazyLock};

type BuiltinFunction = fn(Vec<Object>) -> Object;

pub static BUILTINS: LazyLock<HashMap<&'static str, BuiltinFunction>> = LazyLock::new(|| {
    let mut builtins = HashMap::new();
    builtins.insert("len", builtin_len as BuiltinFunction);
    builtins.insert("first", builtin_first as BuiltinFunction);
    builtins.insert("last", builtin_last as BuiltinFunction);
    builtins.insert("rest", builtin_rest as BuiltinFunction);
    builtins.insert("push", builtin_push as BuiltinFunction);
    builtins.insert("puts", builtin_puts as BuiltinFunction);
    builtins
});

fn builtin_len(args: Vec<Object>) -> Object {
    if args.len() != 1 {
        return Object::Error(format!(
            "wrong number of arguments. got={}, want=1",
            args.len()
        ));
    }

    match &args[0] {
        Object::String(s) => Object::Integer(s.len() as i64),
        Object::Array(elements) => Object::Integer(elements.len() as i64),
        _ => Object::Error(format!(
            "argument to `len` not supported, got {}",
            args[0].object_type()
        )),
    }
}

fn builtin_first(args: Vec<Object>) -> Object {
    if args.len() != 1 {
        return Object::Error(format!(
            "wrong number of arguments. got={}, want=1",
            args.len()
        ));
    }

    match &args[0] {
        Object::Array(elements) => {
            if elements.is_empty() {
                Object::Null
            } else {
                elements[0].clone()
            }
        }
        _ => Object::Error(format!(
            "argument to `first` must be ARRAY, got {}",
            args[0].object_type()
        )),
    }
}

fn builtin_last(args: Vec<Object>) -> Object {
    if args.len() != 1 {
        return Object::Error(format!(
            "wrong number of arguments. got={}, want=1",
            args.len()
        ));
    }

    match &args[0] {
        Object::Array(elements) => {
            if elements.is_empty() {
                Object::Null
            } else {
                elements[elements.len() - 1].clone()
            }
        }
        _ => Object::Error(format!(
            "argument to `last` must be ARRAY, got {}",
            args[0].object_type()
        )),
    }
}

fn builtin_rest(args: Vec<Object>) -> Object {
    if args.len() != 1 {
        return Object::Error(format!(
            "wrong number of arguments. got={}, want=1",
            args.len()
        ));
    }

    match &args[0] {
        Object::Array(elements) => {
            if elements.is_empty() {
                Object::Null
            } else {
                Object::Array(elements[1..].to_vec())
            }
        }
        _ => Object::Error(format!(
            "argument to `rest` must be ARRAY, got {}",
            args[0].object_type()
        )),
    }
}

fn builtin_push(args: Vec<Object>) -> Object {
    if args.len() != 2 {
        return Object::Error(format!(
            "wrong number of arguments. got={}, want=2",
            args.len()
        ));
    }

    match &args[0] {
        Object::Array(elements) => {
            let mut new_elements = elements.clone();
            new_elements.push(args[1].clone());
            Object::Array(new_elements)
        }
        _ => Object::Error(format!(
            "argument to `push` must be ARRAY, got {}",
            args[0].object_type()
        )),
    }
}

fn builtin_puts(args: Vec<Object>) -> Object {
    for arg in args {
        println!("{}", arg.inspect());
    }
    Object::Null
}
