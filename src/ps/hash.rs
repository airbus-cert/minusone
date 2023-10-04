use rule::RuleMut;
use ps::Powershell;
use tree::NodeMut;
use error::MinusOneResult;
use ps::Powershell::HashMap;

#[derive(Default)]
pub struct ParseHash;

impl<'a> RuleMut<'a> for ParseHash {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        let view = node.view();
        if view.kind() == "hash_literal_expression" {
            node.set(HashMap)
        }
        Ok(())
    }
}