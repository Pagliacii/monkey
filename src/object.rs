use std::collections::HashMap;

use crate::{ast, environment};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ObjectType {
    Integer,
    Boolean,
    Null,
    ReturnValue,
    Error,
    Function,
    String,
    Builtin,
    Array,
    Hash,
    Quote,
    Macro,
}

impl std::fmt::Display for ObjectType {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjectType::Integer => write!(formatter, "INTEGER"),
            ObjectType::Boolean => write!(formatter, "BOOLEAN"),
            ObjectType::Null => write!(formatter, "NULL"),
            ObjectType::ReturnValue => write!(formatter, "RETURN_VALUE"),
            ObjectType::Error => write!(formatter, "ERROR"),
            ObjectType::Function => write!(formatter, "FUNCTION"),
            ObjectType::String => write!(formatter, "STRING"),
            ObjectType::Builtin => write!(formatter, "BUILTIN"),
            ObjectType::Array => write!(formatter, "ARRAY"),
            ObjectType::Hash => write!(formatter, "HASH"),
            ObjectType::Quote => write!(formatter, "QUOTE"),
            ObjectType::Macro => write!(formatter, "MACRO"),
        }
    }
}

type BuiltinFunction = fn(Vec<Object>) -> Object;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Object {
    Integer(i64),
    Boolean(bool),
    Null,
    ReturnValue(Box<Object>),
    Error(String),
    Function {
        parameters: Vec<ast::Identifier>,
        body: ast::BlockStatement,
        env: environment::EnvironmentRef,
    },
    String(String),
    Builtin(BuiltinFunction),
    Array(Vec<Object>),
    Hash(HashMap<HashKey, HashPair>),
    Quote(ast::Expression),
    Macro {
        parameters: Vec<ast::Identifier>,
        body: ast::BlockStatement,
        env: environment::EnvironmentRef,
    },
}

impl Object {
    pub fn object_type(&self) -> ObjectType {
        match self {
            Object::Integer(_) => ObjectType::Integer,
            Object::Boolean(_) => ObjectType::Boolean,
            Object::Null => ObjectType::Null,
            Object::ReturnValue(_) => ObjectType::ReturnValue,
            Object::Error(_) => ObjectType::Error,
            Object::Function { .. } => ObjectType::Function,
            Object::String(_) => ObjectType::String,
            Object::Builtin(_) => ObjectType::Builtin,
            Object::Array(_) => ObjectType::Array,
            Object::Hash(_) => ObjectType::Hash,
            Object::Quote(_) => ObjectType::Quote,
            Object::Macro { .. } => ObjectType::Macro,
        }
    }

    pub fn inspect(&self) -> String {
        match self {
            Object::Integer(i) => i.to_string(),
            Object::Boolean(b) => b.to_string().to_uppercase(),
            Object::Null => "NULL".to_string(),
            Object::ReturnValue(obj) => obj.inspect(),
            Object::Error(msg) => format!("ERROR: {msg}"),
            Object::Function {
                parameters, body, ..
            } => {
                let params = parameters
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                format!("fn({}) {{\n{}\n}}", params, body)
            }
            Object::String(s) => s.clone(),
            Object::Builtin(_) => "builtin function".to_string(),
            Object::Array(elements) => {
                let elems = elements
                    .iter()
                    .map(|e| e.inspect())
                    .collect::<Vec<String>>()
                    .join(", ");
                format!("[{}]", elems)
            }
            Object::Hash(pairs) => {
                let pairs_str = pairs
                    .values()
                    .map(|v| format!("{v}"))
                    .collect::<Vec<String>>()
                    .join(", ");
                format!("{{{}}}", pairs_str)
            }
            Object::Quote(expr) => format!("QUOTE({})", expr),
            Object::Macro {
                parameters, body, ..
            } => {
                let params = parameters
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                format!("macro({}) {{\n{}\n}}", params, body)
            }
        }
    }

    pub fn hash_key(&self) -> Result<HashKey, String> {
        match self {
            Object::Boolean(b) => Ok(HashKey::new(ObjectType::Boolean, if *b { 1 } else { 0 })),
            Object::Integer(i) => Ok(HashKey::new(ObjectType::Integer, *i as u64)),
            Object::String(s) => {
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                Hash::hash(s, &mut hasher);
                Ok(HashKey::new(ObjectType::String, hasher.finish()))
            }
            _ => Err(format!("unusable as hash key: {}", self.object_type())),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct HashKey {
    object_type: ObjectType,
    value: u64,
}

impl HashKey {
    pub fn new(object_type: ObjectType, value: u64) -> Self {
        Self { object_type, value }
    }

    pub fn object_type(&self) -> &ObjectType {
        &self.object_type
    }

    pub fn value(&self) -> u64 {
        self.value
    }
}

impl std::fmt::Display for HashKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}:{}", self.object_type, self.value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HashPair {
    key: Object,
    value: Object,
}

impl HashPair {
    pub fn new(key: Object, value: Object) -> Self {
        Self { key, value }
    }

    pub fn key(&self) -> &Object {
        &self.key
    }

    pub fn value(&self) -> &Object {
        &self.value
    }
}

impl std::fmt::Display for HashPair {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}: {}",
            self.key.inspect(),
            self.value.inspect()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_hash_key() {
        let hello1 = Object::String("Hello World".to_string());
        let hello2 = Object::String("Hello World".to_string());
        let diff1 = Object::String("My name is johnny".to_string());
        let diff2 = Object::String("My name is johnny".to_string());
        assert_eq!(hello1.hash_key(), hello2.hash_key());
        assert_eq!(diff1.hash_key(), diff2.hash_key());
        assert_ne!(hello1.hash_key(), diff1.hash_key());
    }
}
