use tree_sitter::{Node, Language};
use core::tree::ComponentDb;

pub trait RuleMut {
    type Language;
    fn enter(&mut self, node : Node, component: &mut ComponentDb<Self::Language>);
    fn leave(&mut self, node : Node, component: &mut ComponentDb<Self::Language>);
}

impl<U, T: RuleMut<Language = U>, R: RuleMut<Language = U>> RuleMut for (T, R) {
    type Language = U;

    fn enter(&mut self, node : Node, component: &mut ComponentDb<Self::Language>) {
        self.0.enter(node, component);
        self.1.enter(node, component);

    }

    fn leave(&mut self, node : Node, component: &mut ComponentDb<Self::Language>) {
        self.0.leave(node, component);
        self.1.leave(node, component);
    }
}

pub trait RuleEngineMut<T> {
    fn apply_mut(&self, rule: &mut impl RuleMut<Language=T>, db: &mut ComponentDb<T>);
}

impl<T> RuleEngineMut<T> for Node<'_> {
    fn apply_mut(&self, rule: &mut impl RuleMut<Language=T>, db: &mut dyn ComponentDb<T>) {
        rule.enter(*self, db);
        let mut cursor = self.walk();
        for child in self.children(&mut cursor) {
            child.apply_mut(rule, db);
        }
        rule.leave(*self, db);
    }
}

pub trait Rule {
    type Language;
    fn enter(&mut self, node : Node, component: &ComponentDb<Self::Language>);
    fn leave(&mut self, node : Node, component: &ComponentDb<Self::Language>);
}

pub trait RuleEngine<T> {
    fn apply(&self, rule: &mut impl Rule<Language=T>, db: &ComponentDb<T>);
}

impl<T> RuleEngine<T> for Node<'_> {
    fn apply(&self, rule: &mut impl Rule<Language=T>, db: &dyn ComponentDb<T>) {
        rule.enter(*self, db);
        let mut cursor = self.walk();
        for child in self.children(&mut cursor) {
            child.apply(rule, db);
        }
        rule.leave(*self, db);
    }
}
