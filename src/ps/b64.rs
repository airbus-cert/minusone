extern crate base64;

use self::base64::{engine::general_purpose, Engine as _};
use error::MinusOneResult;
use ps::Powershell;
use ps::Powershell::{Array, Raw};
use ps::Value::{Num, Str};
use rule::RuleMut;
use tree::{BranchFlow, NodeMut};

#[derive(Default)]
pub struct DecodeBase64;

impl<'a> RuleMut<'a> for DecodeBase64 {
    type Language = Powershell;

    fn enter(
        &mut self,
        _node: &mut NodeMut<'a, Self::Language>,
        _flow: BranchFlow,
    ) -> MinusOneResult<()> {
        Ok(())
    }

    fn leave(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        _flow: BranchFlow,
    ) -> MinusOneResult<()> {
        let view = node.view();
        if view.kind() == "invokation_expression" {
            if let (Some(type_lit), Some(op), Some(member_name), Some(args_list)) =
                (view.child(0), view.child(1), view.child(2), view.child(3))
            {
                match (
                    type_lit.text()?.to_lowercase().as_str(),
                    op.text()?,
                    member_name.text()?.to_lowercase().as_str(),
                ) {
                    ("[system.convert]", "::", "frombase64string")
                    | ("[convert]", "::", "frombase64string") => {
                        // get the argument list if present
                        if let Some(argument_expression_list) =
                            args_list.named_child("argument_expression_list")
                        {
                            if let Some(arg_1) = argument_expression_list.child(0) {
                                if let Some(Raw(Str(s))) = arg_1.data() {
                                    let bytes = general_purpose::STANDARD.decode(s).unwrap();
                                    node.set(Array(bytes.iter().map(|b| Num(*b as i32)).collect()));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }
}
