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


pub fn parse_i32(v: &Value) -> Option<i32> {
    match v {
        Num(num) => Some(*num),
        Str(num_str) => num_str.parse::<i32>().ok()
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
                    if let (Some(from), Some(to)) = (parse_i32(left_value), parse_i32(right_value)) {
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