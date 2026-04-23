use crate::error::MinusOneResult;
use crate::tree::{Storage, Tree};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PrinterMode {
    Pretty,
    Compact,
    Unchanged,
}

pub trait Printer {
    type Language;

    fn print<S>(&mut self, tree: &Tree<'_, S>, mode: PrinterMode) -> MinusOneResult<String>
    where
        S: Storage<Component = Self::Language> + Default;

    fn code<S>(&mut self, tree: &Tree<'_, S>) -> MinusOneResult<String>
    where
        S: Storage<Component = Self::Language> + Default,
    {
        self.print(tree, PrinterMode::Pretty)
    }
}

pub fn code_string<P, S>(printer: &mut P, tree: &Tree<'_, S>) -> MinusOneResult<String>
where
    P: Printer,
    S: Storage<Component = P::Language> + Default,
{
    printer.code(tree)
}

pub fn code_string_with_mode<P, S>(
    printer: &mut P,
    tree: &Tree<'_, S>,
    mode: PrinterMode,
) -> MinusOneResult<String>
where
    P: Printer,
    S: Storage<Component = P::Language> + Default,
{
    printer.print(tree, mode)
}
