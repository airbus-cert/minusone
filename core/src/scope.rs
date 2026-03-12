use std::collections::HashMap;

#[derive(Clone)]
pub struct Variable<T: Clone> {
    inferred_type: Option<T>,
    used: bool,
    local: bool,
}

impl<T: Clone> Variable<T> {
    pub fn new(inferred_type: Option<T>) -> Self {
        Variable {
            inferred_type,
            used: false,
            local: true,
        }
    }
}

#[derive(Clone)]
pub struct Scope<T: Clone> {
    vars: HashMap<String, Variable<T>>,
    pending: HashMap<String, Variable<T>>,
}

impl<T: Clone> Default for Scope<T> {
    fn default() -> Self {
        Self {
            vars: HashMap::new(),
            pending: HashMap::new(),
        }
    }
}

impl<T: Clone> Scope<T> {
    pub fn from(scope: &Scope<T>) -> Self {
        let mut s = Scope {
            vars: scope.vars.clone(),
            pending: scope.pending.clone(),
        };
        for (_, var) in s.vars.iter_mut().chain(s.pending.iter_mut()) {
            var.local = false;
        }
        s
    }

    pub fn assign(&mut self, var_name: &str, new_value: T, ongoing_transaction: bool) {
        let var_value = self.vars.get(var_name).cloned();

        let hashmap_to_update = if ongoing_transaction {
            &mut self.pending
        } else {
            &mut self.vars
        };

        if let Some(value) = hashmap_to_update.get_mut(var_name) {
            value.inferred_type = Some(new_value)
        } else if ongoing_transaction && let Some(mut var_value) = var_value {
            // If ongoing transaction and value is in self.vars, copy it from there instead of creating a new one
            var_value.inferred_type = Some(new_value);
            self.pending.insert(var_name.to_string(), var_value);
        } else {
            hashmap_to_update.insert(var_name.to_string(), Variable::new(Some(new_value)));
        }
    }

    pub fn forget(&mut self, var_name: &str, ongoing_transaction: bool) {
        let hashmap_to_update = if ongoing_transaction {
            &mut self.pending
        } else {
            &mut self.vars
        };

        if let Some(var_value) = hashmap_to_update.get_mut(var_name) {
            var_value.inferred_type = None;
        }
    }

    pub fn in_use(&mut self, var_name: &str, ongoing_transaction: bool) {
        let hashmap_to_update = if ongoing_transaction {
            &mut self.pending
        } else {
            &mut self.vars
        };

        if let Some(var_value) = hashmap_to_update.get_mut(var_name) {
            var_value.used = true;
        }
    }

    pub fn get_var_mut(&mut self, var_name: &str) -> Option<&mut T> {
        self.pending
            .get_mut(var_name)
            .or(self.vars.get_mut(var_name))
            .and_then(|data| data.inferred_type.as_mut())
    }

    pub fn get_var(&self, var_name: &str) -> Option<&T> {
        self.pending
            .get(var_name)
            .or(self.vars.get(var_name))
            .and_then(|data| data.inferred_type.as_ref())
    }

    pub fn get_var_names(&self) -> Vec<String> {
        self.vars.clone().keys().cloned().collect()
    }

    pub fn is_local(&self, var_name: &str) -> Option<bool> {
        if let Some(data) = self.vars.get(var_name) {
            return Some(data.local);
        }
        None
    }

    pub fn set_non_local(&mut self, var_name: &str) {
        if let Some(var_value) = self.vars.get_mut(var_name) {
            var_value.local = false;
        }
    }
}

pub struct ScopeManager<T: Clone> {
    scopes: Vec<Scope<T>>,
}

impl<T: Clone> Default for ScopeManager<T> {
    fn default() -> Self {
        ScopeManager {
            scopes: vec![Scope::default()],
        }
    }
}

impl<T: Clone> ScopeManager<T> {
    pub fn enter(&mut self) {
        self.scopes.push(Scope::from(self.current()))
    }

    pub fn leave(&mut self) {
        let mut last = self.scopes.pop().unwrap();

        // we will merge the pending scope of transcation
        for (name, value) in last.pending.iter_mut() {
            if !value.local {
                if let Some(inferred_type) = &value.inferred_type {
                    self.current_mut().assign(name, inferred_type.clone(), true);
                } else {
                    self.current_mut().forget(name, true);
                }
            }
        }

        // we will merge the vars scope
        for (name, value) in last.vars.iter_mut() {
            if !value.local {
                if let Some(inferred_type) = &value.inferred_type {
                    self.current_mut()
                        .assign(name, inferred_type.clone(), false);
                } else {
                    self.current_mut().forget(name, false);
                }
            }
        }
    }

    /// `leave_function()` != `leave()`, this only merges back variables that already existed in the parent scope before entering.
    /// it prevents `var` declarations from leaking past function boundaries
    pub fn leave_function(&mut self) {
        let last = self.scopes.pop().unwrap();
        for (name, value) in last.vars.iter() {
            if self.current().get_var(&name).is_some() || self.current().is_local(&name).is_some() {
                if let Some(inferred_type) = &value.inferred_type {
                    self.current_mut().assign(name, inferred_type.clone());
                } else {
                    self.current_mut().forget(name);
                }
            }
        }
    }
  
    pub fn flush_transaction(&mut self) {
        let pending = &mut self.current_mut().pending;
        if !pending.is_empty() {
            pending.clear();
        }
    }

    pub fn current_mut(&mut self) -> &mut Scope<T> {
        self.scopes
            .last_mut()
            .expect("It must at least exist a last scope")
    }

    pub fn current(&self) -> &Scope<T> {
        self.scopes
            .last()
            .expect("It must at least exist a last scope")
    }

    pub fn reset(&mut self) {
        self.scopes = vec![Scope::default()]
    }

    pub fn forget_everywhere(&mut self, var_name: &str, ongoing_transaction: bool) {
        for scope in self.scopes.iter_mut() {
            scope.forget(var_name, ongoing_transaction)
        }
    }
}
