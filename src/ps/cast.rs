use rule::RuleMut;
use ps::Powershell;
use tree::NodeMut;
use error::{MinusOneResult, Error};
use ps::Value::{Num, Str};
use ps::Powershell::Raw;

/// Handle static cast operations
/// For example [char]0x74 => 't'
#[derive(Default)]
pub struct Cast;

/// We will infer cast value using down to top exploring
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
/// use minusone::ps::InferredValue::{Number, Str};
/// use minusone::ps::integer::ParseInt;
/// use minusone::ps::cast::Cast;
/// use minusone::ps::string::{ConcatString, ParseString};
///
/// let mut test1 = from_powershell_src("[char]0x74").unwrap();
/// test1.apply_mut(&mut (ParseInt::default(), Cast::default(), Forward::default())).unwrap();
///
/// assert_eq!(*(test1.root().unwrap().child(0).expect("expecting a child").data().expect("expecting a data in the first child")), Str("t".to_string()));
///
/// let mut test2 = from_powershell_src("[char]0x74 + [char]0x6f + [char]0x74 + [char]0x6f").unwrap();
/// test2.apply_mut(&mut (ParseInt::default(), Cast::default(), Forward::default(), ConcatString::default())).unwrap();
///
/// assert_eq!(*(test2.root().unwrap().child(0).expect("expecting a child").data().expect("expecting a data in the first child")), Str("toto".to_string()));
///
/// let mut test3 = from_powershell_src("[int]'65'").unwrap();
/// test3.apply_mut(&mut (ParseInt::default(), Cast::default(), Forward::default(), ParseString::default())).unwrap();
///
/// assert_eq!(*(test3.root().unwrap().child(0).expect("expecting a child").data().expect("expecting a data in the first child")), Number(65));
/// ```
impl<'a> RuleMut<'a> for Cast {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        let view = node.view();
        match view.kind() {
            "cast_expression" => {
                if let (Some(type_literal), Some(expression)) = (view.child(0), view.child(1)) {
                    match (type_literal
                        .child(1).ok_or(Error::invalid_child())? // type_spec
                        .child(0).ok_or(Error::invalid_child())? // type_name
                        .child(0).ok_or(Error::invalid_child())?.text()?.to_lowercase().as_str(),
                           expression.data()) // type_identifier
                    {
                        ("int", Some(Raw(Str(token)))) => {
                            if let Ok(number) = token.parse::<i32>() {
                                node.set(Raw(Num(number)));
                            }
                        },
                        ("byte", Some(Raw(Str(token)))) => {
                            if let Ok(number) = token.parse::<u8>() {
                                node.set(Raw(Num(number as i32)));
                            }
                        },
                        ("char", Some(Raw(Num(num)))) => {
                            let mut result = String::new();
                            result.push(char::from(*num as u8));
                            node.set(Raw(Str(result)));
                        },
                        _ => ()
                    }
                }
            },

            // Forward inferred type in case of cast expression
            "expression_with_unary_operator" => {
                if let Some(child) = view.child(0) {
                    if child.kind() == "cast_expression" {
                        if let Some(data) = child.data() {
                            node.set(data.clone());
                        }
                    }
                }
            }
            _ => ()
        }

        Ok(())
    }
}
