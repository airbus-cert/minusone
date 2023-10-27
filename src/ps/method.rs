use rule::RuleMut;
use ps::Powershell;
use tree::{NodeMut, BranchFlow};
use error::MinusOneResult;
use ps::Powershell::{Array, Raw};
use ps::Value::{Num, Str};


/// Compute the length of predictable Array or string
///
/// # Example
/// ```
/// extern crate tree_sitter;
/// extern crate tree_sitter_powershell;
/// extern crate minusone;
///
/// use minusone::ps::build_powershell_tree;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::linter::Linter;
/// use minusone::ps::string::ParseString;
/// use minusone::ps::method::Length;
///
/// let mut tree = build_powershell_tree("'foo'.length").unwrap();
/// tree.apply_mut(&mut (
///     Length::default(),
///     Forward::default(),
///     ParseString::default()
///     )
/// ).unwrap();
///
/// let mut ps_litter_view = Linter::new();
/// ps_litter_view.print(&tree.root().unwrap()).unwrap();
///
/// assert_eq!(ps_litter_view.output, "3");
/// ```
#[derive(Default)]
pub struct Length;

impl<'a> RuleMut<'a> for Length {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()> {
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()> {
        let view = node.view();
        if view.kind() == "member_access" {
            if let (Some(primary_expression), Some(operator), Some(member_name)) = (view.child(0), view.child(1), view.child(2)) {
                match (primary_expression.data(), operator.text()?, member_name.text()?.to_lowercase().as_str()) {
                    (Some(Array(value)), ".", "length") => node.set(Raw(Num(value.len() as i32))),
                    (Some(Raw(Str(s))), ".", "length") => node.set(Raw(Num(s.len() as i32))),
                    _ => ()
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use ps::method::Length;
    use ps::build_powershell_tree;
    use ps::integer::ParseInt;
    use ps::forward::Forward;
    use ps::array::{ComputeArrayExpr, ParseArrayLiteral};
    use ps::Powershell::Raw;
    use ps::Value::Num;
    use ps::string::ParseString;

    #[test]
    fn test_array_length() {
        let mut tree = build_powershell_tree("@(1,2,3).length").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ComputeArrayExpr::default(),
            ParseArrayLiteral::default(),
            Length::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Raw(Num(3))
        );
    }

    #[test]
    fn test_str_length() {
        let mut tree = build_powershell_tree("'foo'.length").unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            Forward::default(),
            ComputeArrayExpr::default(),
            ParseArrayLiteral::default(),
            Length::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Raw(Num(3))
        );
    }
}