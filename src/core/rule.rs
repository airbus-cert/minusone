use core::tree::{NodeMut, Node};


pub trait RuleMut<'a> {
    type Language;
    fn enter(&mut self, node: &mut NodeMut<'a, Self::Language>);
    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>);
}

impl<'a, U, T: RuleMut<'a, Language = U>, R: RuleMut<'a, Language = U>> RuleMut<'a> for (T, R) {
    type Language = U;

    fn enter(&mut self, node : &mut NodeMut<'a, Self::Language>) {
        self.0.enter(node);
        self.1.enter(node);

    }

    fn leave(&mut self, node : &mut NodeMut<'a, Self::Language>) {
        self.0.leave(node);
        self.1.leave(node);
    }
}

pub trait Rule<'a> {
    type Language;
    fn enter(&mut self, node : &Node<'a, Self::Language>);
    fn leave(&mut self, node : &Node<'a, Self::Language>);
}

