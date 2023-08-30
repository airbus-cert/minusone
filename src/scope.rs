use std::collections::HashMap;
use tree::{Node, NodeMut};
use error::MinusOneResult;

#[derive(Clone)]
pub struct Variable<T: Clone> {
    inferred_type: Option<T>,
}

impl<T: Clone> Variable<T> {
    pub fn new(inferred_type: Option<T>) -> Self {
        Variable {
            inferred_type
        }
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

    pub fn assign(&mut self, var: &Node<T>, value: &Node<T>) -> MinusOneResult<()> {
        if let Some(var_value) = self.vars.get_mut(var.text()?) {
            // We will set the state of the variable
            // even if it's None
            var_value.inferred_type = value.data().map(|value| value.clone());
        } else {
            println!("assign {}",var.text()?);
            self.vars.insert(var.text()?.to_string(), Variable::new(value.data().map(|value| value.clone())));
        }
        Ok(())
    }

    pub fn attach(&self, var: &mut NodeMut<T>) -> MinusOneResult<()> {
        let view = var.view();

        println!("attach {}",view.text()?);
        if let Some(var_value) = self.vars.get(view.text()?) {
            if let Some(inferred_value) = &var_value.inferred_type {
                var.set(inferred_value.clone())
            }
        }

        Ok(())
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
}