use rule::RuleMut;
use tree::{NodeMut};
use error::MinusOneResult;
use ps::Value::Str;
use ps::Powershell;
use ps::Powershell::Raw;

#[derive(Default)]
pub struct ParseString;

impl<'a> RuleMut<'a> for ParseString {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        let node_view = node.view();

        match node_view.kind() {
            "expandable_string_literal" | "verbatim_string_characters" => {
                let value = String::from(node_view.text()?);
                // Parse string by removing the double quote
                node.set(Raw(Str(String::from(&value[1..value.len() - 1]))));
            }
            _ => ()
        }
        Ok(())
    }
}


/// This rule will infer string concat operation
///
/// # Example
/// ```
/// extern crate tree_sitter;
/// extern crate tree_sitter_powershell;
/// extern crate minusone;
///
/// use minusone::tree::{HashMapStorage, Tree};
/// use minusone::ps::from_powershell_src;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::litter::Litter;
/// use minusone::ps::string::{ConcatString, ParseString};
///
/// let mut tree = from_powershell_src("'foo' + 'bar'").unwrap();
/// tree.apply_mut(&mut (ParseString::default(), Forward::default(), ConcatString::default())).unwrap();
///
/// let mut ps_litter_view = Litter::new();
/// ps_litter_view.print(&tree.root().unwrap()).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"foobar\"");
/// ```
#[derive(Default)]
pub struct ConcatString;

impl<'a> RuleMut<'a> for ConcatString {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        let view = node.view();
        if view.kind() == "additive_expression" ||  view.kind() == "additive_argument_expression" {
            if let (Some(left_op), Some(operator), Some(right_op)) = (view.child(0), view.child(1), view.child(2)) {
                match (left_op.data(), operator.text()?, right_op.data()) {
                     (Some(Raw(Str(string_left))), "+", Some(Raw(Str(string_right)))) => node.set(Raw(Str(String::from(string_left) + string_right))),
                    _ => {}
                }
            }
        }
        Ok(())
    }
}