use std::collections::HashMap;

#[derive(Clone)]
pub struct Variable<T: Clone> {
    inferred_type: Option<T>,
    // use to handle variable attributes
    attributes: HashMap<String, T>
}

impl<T: Clone> Variable<T> {
    pub fn new(inferred_type: Option<T>) -> Self {
        Variable {
            inferred_type,
            attributes: HashMap::new()
        }
    }

    pub fn set_attr(&mut self, name: String, inferred_type: T) {
        self.attributes.insert(name, inferred_type);
    }
}

#[derive(Clone)]
pub struct Scope<T: Clone> {
    vars: HashMap<String, Variable<T>>
}

impl<T: Clone> Scope<T> {
    pub fn new() -> Self {
        Scope {
            vars: HashMap::new()
        }
    }

    pub fn assign(&mut self, var_name: &str, value: T) {
        if let Some(var_value) = self.vars.get_mut(var_name) {
            var_value.inferred_type = Some(value);
        } else {
            self.vars.insert(var_name.to_string(), Variable::new(Some(value)));
        }
    }

    pub fn forget(&mut self, var_name: &str) {
        if let Some(var_value) = self.vars.get_mut(var_name) {
            var_value.inferred_type = None;
        }
    }

    pub fn get_var_mut(&mut self, var_name: &str) -> Option<&mut T> {
        if let Some(data) = self.vars.get_mut(var_name) {
            return data.inferred_type.as_mut();
        }
        None
    }

    pub fn get_var(&self, var_name: &str) -> Option<&T> {
        if let Some(data) = self.vars.get(var_name) {
            return data.inferred_type.as_ref();
        }
        None
    }
}

pub struct ScopeManager<T: Clone> {
    scopes: Vec<Scope<T>>
}

impl<T: Clone> ScopeManager<T> {
    pub fn new() -> Self {
        ScopeManager {
            scopes: vec![Scope::new()] // default scope
        }
    }

    pub fn enter(&mut self) {
        self.scopes.push(self.scopes.last().expect("It must at least exist a last scope").clone())
    }

    pub fn leave(&mut self) {
        self.scopes.pop();
    }

    pub fn current(&mut self) -> &mut Scope<T> {
        self.scopes.last_mut().expect("It must at least exist a last scope")
    }

    pub fn reset(&mut self) {
        self.scopes = vec![Scope::new()]
    }

    pub fn is_known_in_stack(&self, var_name: &str) -> bool {
        // check in the stack without checking the current one
        for scope in &self.scopes[0..self.scopes.len() - 1] {
            if !scope.get_var(var_name).is_none() {
                return true;
            }
        }
        return false;
    }

    pub fn forget_everywhere(&mut self, var_name: &str) {
        for scope in self.scopes.iter_mut() {
            scope.forget(var_name)
        }
    }
}