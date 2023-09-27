use rule::RuleMut;
use ps::Powershell;
use tree::NodeMut;
use error::{MinusOneResult, Error};
use ps::Powershell::{Array, Raw};
use ps::Value::{Str, Num};

/// This rule will infer the -join opoerator
/// in the context of comparison operator
///
/// @('f', 'o', 'o') -join '' => 'foo'
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
/// use minusone::ps::litter::Litter;
/// use minusone::ps::string::ParseString;
/// use minusone::ps::join::JoinComparison;
/// use minusone::ps::integer::ParseInt;
/// use minusone::ps::array::ParseArrayLiteral;
/// use minusone::ps::access::AccessString;
///
/// let mut tree = from_powershell_src("(\"3oFAIQdPcNvzU72CELRwGlMTDxfe1iVtp8OuWq-jsYyJHSakm69nb5XBZg4K0hr\")[29,51,10,1,47,27,38,27,25,32,62,27,40,40,29,1,51] -join ''").unwrap();
/// tree.apply_mut(&mut (
///     ParseString::default(),
///     Forward::default(),
///     ParseInt::default(),
///     ParseArrayLiteral::default(),
///     JoinComparison::default(),
///     AccessString::default()
/// )).unwrap();
///
/// let mut ps_litter_view = Litter::new();
/// ps_litter_view.print(&tree.root().unwrap()).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"invoke-expression\"");
/// ```
#[derive(Default)]
pub struct JoinComparison;

impl<'a> RuleMut<'a> for JoinComparison {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        let view = node.view();
        if view.kind() == "comparison_expression"  {
            if let (Some(left_expression), Some(operator), Some(right_expression)) = (view.child(0), view.child(1), view.child(2)) {
                match (left_expression.data(), operator.text()?, right_expression.data()) {
                    (Some(Array(src_array)), "-join", Some(Raw(Str(join_token)))) => {
                        let result = src_array
                            .iter()
                            .map(|e| e.to_string())
                            .collect::<Vec<String>>()
                            .join(join_token);
                        node.set(Raw(Str(result)));
                    },
                    _ => ()
                }
            }
        }
        Ok(())
    }
}


/// This rule will infer the [string]::join function
///
/// [string]::join('', ('a','b','c')) => 'abc'
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
/// use minusone::ps::litter::Litter;
/// use minusone::ps::string::ParseString;
/// use minusone::ps::join::JoinStringMethod;
/// use minusone::ps::integer::ParseInt;
/// use minusone::ps::array::ParseArrayLiteral;
///
/// let mut tree = from_powershell_src("[string]::join('', (\"a\",\"b\",\"c\"))").unwrap();
/// tree.apply_mut(&mut (
///     ParseString::default(),
///     Forward::default(),
///     ParseInt::default(),
///     ParseArrayLiteral::default(),
///     JoinStringMethod::default()
/// )).unwrap();
///
/// let mut ps_litter_view = Litter::new();
/// ps_litter_view.print(&tree.root().unwrap()).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"abc\"");
/// ```
#[derive(Default)]
pub struct JoinStringMethod;

impl<'a> RuleMut<'a> for JoinStringMethod {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        let view = node.view();
        if view.kind() == "invokation_expression" {
            // Invokation must be done using argument list
            if let (Some(primary_expression), Some(operator), Some(member_name), Some(arguments_list)) = (view.child(0), view.child(1), view.child(2), view.child(3)) {
                if primary_expression.kind() == "type_literal" {
                    match (primary_expression.child(1).ok_or(Error::invalid_child())?.text()?.to_lowercase().as_str(), operator.text()?.to_lowercase().as_str(), member_name.text()?.to_lowercase().as_str()) {
                        ("string", "::", "join") => {
                            // get the argument list if present
                            if let Some(argument_expression_list) = arguments_list.named_child("argument_expression_list") {
                                // if there is 2 arguments
                                if let (Some(arg_1), Some(arg_2)) = (argument_expression_list.child(0), argument_expression_list.child(2)) {
                                    // if arguments was inferred as Str, Array
                                    if let (Some(Raw(Str(join_token))), Some(Array(values))) = (arg_1.data(), arg_2.data()) {
                                        let result = values
                                            .iter()
                                            .map(|e| e.to_string())
                                            .collect::<Vec<String>>()
                                            .join(join_token);

                                        node.set(Raw(Str(result)));
                                    }
                                }
                            }
                        },
                        _ => ()
                    }
                }
            }
        }
        Ok(())
    }
}

/// This rule will infer the -join operator
/// in context of unary operatoe
///
/// -join @('a','b','c') => 'abc'
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
/// use minusone::ps::litter::Litter;
/// use minusone::ps::string::ParseString;
/// use minusone::ps::join::JoinOperator;
/// use minusone::ps::integer::ParseInt;
/// use minusone::ps::array::ParseArrayLiteral;
///
/// let mut tree = from_powershell_src("-join @(\"a\",\"b\", \"c\")").unwrap();
/// tree.apply_mut(&mut (
///     ParseString::default(),
///     Forward::default(),
///     ParseInt::default(),
///     ParseArrayLiteral::default(),
///     JoinOperator::default()
/// )).unwrap();
///
/// let mut ps_litter_view = Litter::new();
/// ps_litter_view.print(&tree.root().unwrap()).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"abc\"");
/// ```
#[derive(Default)]
pub struct JoinOperator;

impl<'a> RuleMut<'a> for JoinOperator {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        let view = node.view();
        if view.kind() == "expression_with_unary_operator" {
            if let (Some(operator), Some(unary_expression)) = (view.child(0), view.child(1)) {
                match (operator.text()?.to_lowercase().as_str(), unary_expression.data()) {
                    ("-join", Some(Array(values))) => {
                        let result = values.iter().map(|e| {
                            match e {
                                Str(s) => s.clone(),
                                Num(n) => n.to_string()
                            }
                        }).collect::<Vec<String>>().join(""); // by default the join operator join with an empty token

                        node.set(Raw(Str(result)));
                    }
                    _ => ()
                }
            }
        }
        Ok(())
    }
}