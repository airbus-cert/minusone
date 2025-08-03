use crate::error::MinusOneResult;
use crate::ps::Powershell::{Array, Raw};
use crate::ps::Value::Str;
use crate::ps::{Powershell, Value};
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};

/// This function get char at index position
/// even if the index is negative
/// to seemless powershell engine
fn get_at_index(s: &str, index: i64) -> Option<String> {
    let mut uz_index = index as usize;

    // negative value is allowed by powershell
    if index < 0 {
        uz_index = (s.len() as i64 + index) as usize;
    }

    s.chars().nth(uz_index).map(|c| c.to_string())
}

fn get_array_at_index(s: &Vec<Value>, index: i64) -> Option<&Value> {
    let mut uz_index = index as usize;

    // negative value is allowed by powershell
    if index < 0 {
        uz_index = (s.len() as i64 + index) as usize;
    }

    s.get(uz_index)
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
///
/// use minusone::ps::build_powershell_tree;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::integer::ParseInt;
/// use minusone::ps::linter::Linter;
/// use minusone::ps::string::ParseString;
/// use minusone::ps::access::AccessString;
/// use minusone::ps::join::JoinOperator;
/// use minusone::ps::array::ParseArrayLiteral;
///
/// let mut tree = build_powershell_tree("-join 'abc'[2,1,0]").unwrap();
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
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"cba\"");
/// ```
#[derive(Default)]
pub struct AccessString;

impl<'a> RuleMut<'a> for AccessString {
    type Language = Powershell;

    fn enter(
        &mut self,
        _node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        Ok(())
    }

    fn leave(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        let view = node.view();
        if view.kind() == "element_access" {
            if let (Some(element), Some(expression)) = (view.child(0), view.child(2)) {
                match (element.data(), expression.data()) {
                    // We will handle the case of indexing string by an array
                    // ex: "foo"[1,2] => ["o", "o"]
                    (Some(Raw(Str(string_element))), Some(Array(index))) => {
                        let mut result = vec![];
                        for index_value in index {
                            if let Some(parsed_index_value) = index_value.clone().to_i64() {
                                if let Some(string_result) =
                                    get_at_index(string_element, parsed_index_value)
                                {
                                    result.push(Str(string_result));
                                }
                            }
                        }
                        node.set(Array(result));
                    }
                    // "foo"[0]
                    (Some(Raw(Str(string_element))), Some(Raw(index_value))) => {
                        if let Some(parsed_index_value) = index_value.clone().to_i64() {
                            if let Some(string_result) =
                                get_at_index(string_element, parsed_index_value)
                            {
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

#[derive(Default)]
pub struct AccessArray;

impl<'a> RuleMut<'a> for AccessArray {
    type Language = Powershell;

    fn enter(
        &mut self,
        _node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        Ok(())
    }

    fn leave(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        let view = node.view();
        if view.kind() == "element_access" {
            if let (Some(element), Some(expression)) = (view.child(0), view.child(2)) {
                match (element.data(), expression.data()) {
                    (Some(Array(array_element)), Some(Array(index))) => {
                        let mut result = vec![];
                        for index_value in index {
                            if let Some(parsed_index_value) = index_value.clone().to_i64() {
                                if let Some(value) =
                                    get_array_at_index(array_element, parsed_index_value)
                                {
                                    result.push(value.clone());
                                }
                            }
                        }
                        node.set(Array(result));
                    }
                    // "foo"[0]
                    (Some(Array(array_element)), Some(Raw(index_value))) => {
                        if let Some(parsed_index_value) = index_value.clone().to_i64() {
                            if let Some(value) =
                                get_array_at_index(array_element, parsed_index_value)
                            {
                                node.set(Raw(value.clone()));
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

/// Extract element of hashmap using [] or . operator
/// Key values are case-insensitive
///
/// $foo = @{"Key1" = 1; Key2 = 2;};
/// $foo["Key1"] => 1
/// $foo.Key2 => 2
/// $foo."kEy1" => 1
/// $foo["kEy2"] => 2q
///
/// # Example
/// ```
/// extern crate tree_sitter;
/// extern crate tree_sitter_powershell;
///
/// use minusone::ps::build_powershell_tree;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::integer::{ParseInt, AddInt};
/// use minusone::ps::linter::Linter;
/// use minusone::ps::string::ParseString;
/// use minusone::ps::access::{AccessHashMap, AccessString};
/// use minusone::ps::join::JoinOperator;
/// use minusone::ps::array::ParseArrayLiteral;
/// use minusone::ps::hash::ParseHash;
///
/// let mut tree = build_powershell_tree("@{'Key' = 1}.kEy + @{'Name' = 2}['name'] + @{OK = 3}.'ok'").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(),
///     ParseHash::default(),
///     AddInt::default(),
///     Forward::default(),
///     ParseString::default(),
///     ParseArrayLiteral::default(),
///     AccessString::default(),
///     AccessHashMap::default()
///     )
/// ).unwrap();
///
/// let mut ps_litter_view = Linter::new();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "6");
/// ```
#[derive(Default)]
pub struct AccessHashMap;

impl<'a> RuleMut<'a> for AccessHashMap {
    type Language = Powershell;

    fn enter(
        &mut self,
        _node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        Ok(())
    }

    fn leave(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        let view = node.view();
        match view.kind() {
            "element_access" => {
                if let (Some(element), Some(expression)) = (view.child(0), view.child(2)) {
                    if let (Some(Powershell::HashMap(map)), Some(Raw(value))) =
                        (element.data(), expression.data())
                    {
                        let value_n = &value.normalize();
                        if map.contains_key(value_n) {
                            node.set(Raw(map[value_n].clone()))
                        }
                    }
                }
            }
            "member_access" => {
                if let (Some(element), Some(expression)) = (view.child(0), view.child(2)) {
                    if let Some(Powershell::HashMap(map)) = element.data() {
                        if let Some(Raw(value)) = expression.data() {
                            let value_n = &value.normalize();
                            if map.contains_key(value_n) {
                                node.set(Raw(map[value_n].clone()))
                            }
                        } else if let Some(child) = expression.child(0) {
                            if child.kind() == "simple_name" {
                                let value = Str(expression.text()?.to_lowercase());
                                if map.contains_key(&value) {
                                    node.set(Raw(map[&value].clone()))
                                }
                            }
                        }
                    }
                }
            }
            _ => (),
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ps::array::ParseArrayLiteral;
    use crate::ps::build_powershell_tree;
    use crate::ps::forward::Forward;
    use crate::ps::integer::ParseInt;
    use crate::ps::string::ParseString;

    #[test]
    fn test_access_string_element_from_int() {
        let mut tree = build_powershell_tree("'abc'[0, 1]").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ParseString::default(),
            AccessString::default(),
            ParseArrayLiteral::default(),
        ))
        .unwrap();

        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data()
                .expect("Inferred type"),
            Array(vec![Str("a".to_string()), Str("b".to_string())])
        );
    }

    #[test]
    fn test_access_string_element_from_negative_int() {
        let mut tree = build_powershell_tree("'abc'[-2, -1]").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ParseString::default(),
            AccessString::default(),
            ParseArrayLiteral::default(),
        ))
        .unwrap();

        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data()
                .expect("Inferred type"),
            Array(vec![Str("b".to_string()), Str("c".to_string())])
        );
    }

    #[test]
    fn test_access_string_element_from_negative_string() {
        let mut tree = build_powershell_tree("'abc'['-2']").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ParseString::default(),
            AccessString::default(),
        ))
        .unwrap();

        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data()
                .expect("Inferred type"),
            Raw(Str("b".to_string()))
        );
    }

    #[test]
    fn test_access_string_element_from_string() {
        let mut tree = build_powershell_tree("'abc'['0']").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ParseString::default(),
            AccessString::default(),
        ))
        .unwrap();

        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data()
                .expect("Inferred type"),
            Raw(Str("a".to_string()))
        );
    }

    #[test]
    fn test_access_string_multi_element_from_int() {
        let mut tree = build_powershell_tree("'abc'[1, 2]").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ParseString::default(),
            AccessString::default(),
            ParseArrayLiteral::default(),
        ))
        .unwrap();

        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data()
                .expect("Inferred type"),
            Array(vec![Str("b".to_string()), Str("c".to_string())])
        );
    }
}
