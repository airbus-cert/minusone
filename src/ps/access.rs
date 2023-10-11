use rule::RuleMut;
use ps::Powershell;
use tree::{NodeMut, BranchFlow};
use error::MinusOneResult;
use ps::Powershell::{Raw, Array};
use ps::Value::Str;

/// This function get char at index position
/// even if the index is negative
/// to seemless powershell engine
fn get_at_index(s: &str, index: i32) -> Option<String> {
    let mut uz_index = index as usize;

    // negative value is allowed by powershell
    if index < 0 {
        uz_index = (s.len() as i32 + index) as usize;
    }

    s.chars().nth(uz_index).map(|c| c.to_string())

}


/// Extract element as array using [] operator
///
/// "foo"[0] => "f"
/// "foo"[1,3] => "fo"
/// "foo"[-1] => "o"
///
/// # Example
/// ```
/// extern crate tree_sitter;
/// extern crate tree_sitter_powershell;
/// extern crate minusone;
///
/// use minusone::ps::from_powershell_src;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::integer::ParseInt;
/// use minusone::ps::linter::Linter;
/// use minusone::ps::string::ParseString;
/// use minusone::ps::access::AccessString;
/// use minusone::ps::join::JoinOperator;
/// use minusone::ps::array::ParseArrayLiteral;
///
/// let mut tree = from_powershell_src("-join 'abc'[2,1,0]").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(),
///     Forward::default(),
///     ParseString::default(),
///     ParseArrayLiteral::default(),
///     AccessString::default(),
///     JoinOperator::default()
///     )
/// ).unwrap();
///
/// let mut ps_litter_view = Linter::new();
/// ps_litter_view.print(&tree.root().unwrap()).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"cba\"");
/// ```
#[derive(Default)]
pub struct AccessString;

impl<'a> RuleMut<'a> for AccessString {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()>{
        let view = node.view();
        if view.kind() == "element_access"  {
            if let (Some(element), Some(expression)) = (view.child(0), view.child(2)) {
                match (element.data(), expression.data()) {
                    // We will handle the case of indexing string by an array
                    // ex: "foo"[1,2] => ["o", "o"]
                    (Some(Raw(Str(string_element))), Some(Array(index))) => {
                        let mut result = vec![];
                        for index_value in index {
                            if let Some(parsed_index_value) = index_value.clone().to_i32() {
                                if let Some(string_result) = get_at_index(string_element, parsed_index_value) {
                                    result.push(Str(string_result));
                                }
                            }
                        }
                        node.set(Array(result));
                    },
                    // "foo"[0]
                    (Some(Raw(Str(string_element))), Some(Raw(index_value))) => {
                        if let Some(parsed_index_value) = index_value.clone().to_i32() {
                            if let Some(string_result) = get_at_index(string_element, parsed_index_value) {
                                node.set(Raw(Str(string_result)));
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

#[cfg(test)]
mod test {
    use super::*;
    use ps::from_powershell_src;
    use ps::integer::ParseInt;
    use ps::string::ParseString;
    use ps::forward::Forward;
    use ps::array::ParseArrayLiteral;

    #[test]
    fn test_access_string_element_from_int() {
        let mut tree = from_powershell_src("'abc'[0, 1]").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ParseString::default(),
            AccessString::default(),
            ParseArrayLiteral::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Str("a".to_string()), Str("b".to_string())])
        );
    }

    #[test]
    fn test_access_string_element_from_negative_int() {
        let mut tree = from_powershell_src("'abc'[-2, -1]").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ParseString::default(),
            AccessString::default(),
            ParseArrayLiteral::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Str("b".to_string()), Str("c".to_string())])
        );
    }

    #[test]
    fn test_access_string_element_from_negative_string() {
        let mut tree = from_powershell_src("'abc'['-2']").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ParseString::default(),
            AccessString::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Raw(Str("b".to_string()))
        );
    }

    #[test]
    fn test_access_string_element_from_string() {
        let mut tree = from_powershell_src("'abc'['0']").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ParseString::default(),
            AccessString::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Raw(Str("a".to_string()))
        );
    }

    #[test]
    fn test_access_string_multi_element_from_int() {
        let mut tree = from_powershell_src("'abc'[1, 2]").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ParseString::default(),
            AccessString::default(),
            ParseArrayLiteral::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Str("b".to_string()), Str("c".to_string())])
        );
    }
}