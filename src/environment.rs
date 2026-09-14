use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::object::Object;

pub type EnvironmentRef = Rc<RefCell<Environment>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Environment {
    store: HashMap<String, Object>,
    outer: Option<EnvironmentRef>,
}

impl Environment {
    pub fn new() -> Self {
        Self {
            store: HashMap::new(),
            outer: None,
        }
    }

    pub fn new_enclosed(outer: EnvironmentRef) -> Self {
        Self {
            store: HashMap::new(),
            outer: Some(outer),
        }
    }

    pub fn get(&self, name: &str) -> Option<Object> {
        match self.store.get(name) {
            Some(obj) => Some(obj.clone()),
            None => self
                .outer
                .as_ref()
                .and_then(|outer| outer.borrow().get(name)),
        }
    }

    pub fn set(&mut self, name: impl Into<String>, value: Object) -> Object {
        self.store.insert(name.into(), value.clone());
        value
    }
}
