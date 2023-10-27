use error::MinusOneResult;
use ps::Powershell;
use ps::Powershell::{Array, Raw, Type};
use ps::Value::{Num, Str};
use rule::RuleMut;
use tree::{BranchFlow, NodeMut};

#[derive(Default)]
pub struct FromUTF;

impl<'a> RuleMut<'a> for FromUTF {
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
            if let (Some(member_access), Some(op), Some(member_name)) =
                (view.child(0), view.child(1), view.child(2))
            {
                if member_access.kind() != "member_access" {
                    return Ok(());
                }

                match (
                    member_access.child(0).unwrap().data(),
                    member_access
                        .child(2)
                        .unwrap()
                        .text()?
                        .to_lowercase()
                        .as_str(),
                    op.text()?,
                    member_name.text()?.to_lowercase().as_str(),
                ) {
                    (Some(Type(typename)), member_name, ".", "getstring")
                    | (Some(Type(typename)), member_name, ".", "getstring")
                        if (typename == "system.text.encoding" || typename == "text.encoding")
                            && (member_name == "utf8" || member_name == "unicode") =>
                    {
                        let arg_list = view.child(3).unwrap().child(1).unwrap().child(0).unwrap();

                        match arg_list.data() {
                            Some(Array(a)) => {
                                let mut int_vec = Vec::new();
                                for value in a.iter() {
                                    if let Num(n) = value {
                                        int_vec.push(*n as u8);
                                    }
                                }
                                if let Ok(s) = String::from_utf8(int_vec) {
                                    node.set(Raw(Str(s)));
                                }
                            }
                            _ => {}
                        }
                    }
                    (Some(Type(typename)), "utf16", ".", "getstring")
                        if typename == "system.text.encoding" || typename == "text.encoding" =>
                    {
                        let arg_list = view.child(3).unwrap().child(1).unwrap().child(0).unwrap();

                        match arg_list.data() {
                            Some(Array(a)) => {
                                let mut int_vec = Vec::new();
                                for value in a.iter() {
                                    if let Num(n) = value {
                                        int_vec.push(*n as u8);
                                    }
                                }

                                let int_vec: Vec<u16> = int_vec
                                    .chunks_exact(2)
                                    .into_iter()
                                    .map(|a| u16::from_ne_bytes([a[0], a[1]]))
                                    .collect();
                                let int_vec = int_vec.as_slice();

                                if let Ok(s) = String::from_utf16(&int_vec) {
                                    node.set(Raw(Str(s)));
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}
