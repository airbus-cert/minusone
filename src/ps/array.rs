use rule::RuleMut;
use ps::{Powershell, Value};
use tree::NodeMut;
use error::MinusOneResult;
use ps::Powershell::{Raw, Array};
use ps::Value::{Num, Str};

/// Parse array literal
///
/// It will parse 1,2,3,"5" as a range for powershell
///
/// It will not have direct impact on Powershell litter
/// It's an internal representation
#[derive(Default)]
pub struct ParseArrayLiteral;

impl<'a> RuleMut<'a> for ParseArrayLiteral {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()> {
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()> {
        let view = node.view();
        if view.kind() == "array_literal_expression" {
            if let (Some(left_node), Some(right_node)) = (view.child(0), view.child(2)) {
                match (left_node.data(), right_node.data()) {
                    // Case when we are not beginning to built the range
                    (Some(Raw(left_value)), Some(Raw(right_value))) => node.set(Array(vec![left_value.clone(), right_value.clone()])),
                    // update an existing array
                    (Some(Array(left_value)), Some(Raw(right_value))) => {
                        let mut new_range = left_value.clone();
                        new_range.push(right_value.clone());
                        node.set(Array(new_range))
                    }
                    _ => ()
                }
            }
        }
        Ok(())
    }
}

/// This rule will generate
/// a range value from operator ..
#[derive(Default)]
pub struct ParseRange;

impl<'a> RuleMut<'a> for ParseRange {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()> {
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()> {
        let view = node.view();
        if view.kind() == "range_expression" {
            if let (Some(left_node), Some(right_node)) = (view.child(0), view.child(2)) {
                if let (Some(Raw(left_value)), Some(Raw(right_value))) = (left_node.data(), right_node.data()) {
                    if let (Some(from), Some(to)) = (<Value as Into<Option<i32>>>::into(left_value.clone()), <Value as Into<Option<i32>>>::into(right_value.clone())) {
                        let mut result = Vec::new();
                        for i in from .. to + 1 {
                            result.push(Num(i));
                        }
                        node.set(Array(result));
                    }
                }
            }
        }
        Ok(())
    }
}


#[derive(Default)]
pub struct ComputeArrayExpr;

impl<'a> RuleMut<'a> for ComputeArrayExpr {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()> {
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()> {
        let view = node.view();
        if view.kind() == "array_expression" {
            if let Some(statement_list) = view.named_child("statements") {
                let mut result = Vec::new();
                for statement in statement_list.iter() {
                    if statement.kind() == "empty_statement" {
                        continue
                    }

                    match statement.data() {
                        Some(Array(values)) => {
                            result.extend(values.clone());
                        },
                        Some(Raw(value)) => {
                            result.push(value.clone());
                        }
                        _ => {
                            // stop inferring
                            return Ok(());
                        }
                    }
                }
                node.set(Array(result));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use ps::from_powershell_src;
    use ps::integer::{ParseInt, AddInt};
    use ps::forward::Forward;
    use ps::array::{ParseArrayLiteral, ComputeArrayExpr};
    use ps::Powershell::Array;
    use ps::Value::{Num, Str};
    use ps::string::ParseString;

    #[test]
    fn test_init_num_array() {
        let mut tree = from_powershell_src("@(1,2,3)").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ComputeArrayExpr::default(),
            ParseArrayLiteral::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Num(1), Num(2), Num(3)])
        );
    }

    #[test]
    fn test_init_mix_array() {
        let mut tree = from_powershell_src("@(1,2,'3')").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseString::default(),
            Forward::default(),
            ComputeArrayExpr::default(),
            ParseArrayLiteral::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Num(1), Num(2), Str("3".to_string())])
        );
    }

    #[test]
    fn test_init_str_array() {
        let mut tree = from_powershell_src("@('a','b','c')").unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            Forward::default(),
            ComputeArrayExpr::default(),
            ParseArrayLiteral::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Str("a".to_string()), Str("b".to_string()), Str("c".to_string())])
        );
    }

    #[test]
    fn test_init_int_array_without_at() {
        let mut tree = from_powershell_src("1,2,3").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ComputeArrayExpr::default(),
            ParseArrayLiteral::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Num(1), Num(2), Num(3)])
        );
    }

    #[test]
    fn test_init_array_with_multi_statement() {
        let mut tree = from_powershell_src("@(1,2,3; 4 + 6)").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            AddInt::default(),
            Forward::default(),
            ComputeArrayExpr::default(),
            ParseArrayLiteral::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Num(1), Num(2), Num(3), Num(10)])
        );
    }
}