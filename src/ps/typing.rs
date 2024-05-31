use rule::RuleMut;
use ps::Powershell;
use tree::{NodeMut, ControlFlow};
use error::MinusOneResult;
use ps::Powershell::Type;

#[derive(Default)]
pub struct ParseType;

impl<'a> RuleMut<'a> for ParseType {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>, _flow: ControlFlow) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>, _flow: ControlFlow) -> MinusOneResult<()>{
        let view = node.view();
        if view.kind() == "type_name" {
            node.set(Type(view.text()?.to_lowercase()));
        }
        else if view.kind() == "type_spec" {
            if let Some(type_identifier) = view.child(0) {
                if let Some(type_identifier_data) = type_identifier.data() {
                    node.set(type_identifier_data.clone())
                }
            }
        }
        else if view.kind() == "array_type_name" {
            if let Some(type_name) = view.child(0) {
                if let Some(Type(type_name_str)) = type_name.data() {
                    node.set(Type(type_name_str.to_string() + "[]"))
                }
            }
        }

        Ok(())
    }
}