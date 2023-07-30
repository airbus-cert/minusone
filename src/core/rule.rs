use core::entity::{Entity};

pub trait Rule {
    type Language;
    fn enter(&self, entity: &mut Entity<Self::Language>);
    fn leave(&self, entity: &mut Entity<Self::Language>);
}

impl<U, T: Rule<Language = U>> Rule for (T,) {
    type Language = U;

    fn enter(&self, entity: &mut Entity<Self::Language>) {
        unimplemented!()
    }

    fn leave(&self, entity: &mut Entity<Self::Language>) {
        unimplemented!()
    }
}


