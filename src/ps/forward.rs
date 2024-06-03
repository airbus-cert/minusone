use rule::RuleMut;
use ps::Powershell;
use tree::{NodeMut, ControlFlow};
use error::{MinusOneResult, Error};
use ps::Powershell::Null;

/// The forward rule is use to forward
/// inferedtype in the most simple case : where there is nothing to do
#[derive(Default)]
pub struct Forward;

/// Forward will just forward inferred type in case of very simple
/// tree exploring
///
/// # Example
/// ```
/// extern crate tree_sitter;
/// extern crate tree_sitter_powershell;
/// extern crate minusone;
///
/// use minusone::tree::{HashMapStorage, Tree};
/// use minusone::ps::build_powershell_tree;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::linter::Linter;
/// use minusone::ps::integer::ParseInt;
/// let mut tree = build_powershell_tree("42").unwrap();
/// tree.apply_mut(&mut (
///     Forward::default(),
///     ParseInt::default(),
/// )).unwrap();
///
/// let mut ps_litter_view = Linter::new();
/// ps_litter_view.print(&tree.root().unwrap()).unwrap();
///
/// assert_eq!(ps_litter_view.output, "42");
/// ```
impl<'a> RuleMut<'a> for Forward {
    type Language = Powershell;

    /// Nothing to do during top down exploration
    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>, _flow: ControlFlow) -> MinusOneResult<()>{
        Ok(())
    }

    /// Forward the inferred type to the top node
    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>, _flow: ControlFlow) -> MinusOneResult<()> {
        let view = node.view();
        match view.kind() {
            "unary_expression" | "array_literal_expression" |
            "range_expression" | "format_expression" |
            "multiplicative_expression" | "additive_expression" |
            "comparison_expression" | "bitwise_expression" |
            "string_literal" | "logical_expression" |
            "integer_literal" | "argument_expression" |
            "range_argument_expression" | "format_argument_expression" |
            "multiplicative_argument_expression" | "additive_argument_expression" |
            "comparison_argument_expression" | "bitwise_argument_expression" |
            "logical_argument_expression" | "command_name_expr" | "expression_with_unary_operator" |
            "while_condition" | "member_name" => {
                if view.child_count() == 1 {
                    if let Some(child_data) = view.child(0).ok_or(Error::invalid_child())?.data() {
                        node.reduce(child_data.clone());
                    }
                }
            }
            "sub_expression" => {
                if let Some(expression) = view.named_child("statements") {
                    // A sub expression must have only one statement to be reduced
                    if expression.child_count() == 1 {
                        if let Some(expression_data) = expression.child(0).ok_or(Error::invalid_child())?.data() {
                            node.reduce(expression_data.clone())
                        }
                    }
                }
                else {
                    // an empty subexpression is considering as null output
                    node.reduce(Null)
                }
            },
            "parenthesized_expression" => {
                if let Some(expression) = view.child(1) {
                    if let Some(expression_data) = expression.data() {
                        node.reduce(expression_data.clone())
                    }
                }
            },

            // we infer pipeline type with the value of the last expression
            "pipeline" => {
                if let Some(expression) = view.child(view.child_count() - 1) {
                    if let Some(expression_data) = expression.data() {
                        node.reduce(expression_data.clone())
                    }
                }
            },
            "command" => {
                if let Some(sub_command) = view.child(0) {
                    if sub_command.kind() == "foreach_command" {
                        if let Some(expression_data) = sub_command.data() {
                            node.reduce(expression_data.clone())
                        }
                    }
                }
            },
            "type_literal" => {
                if let Some(expression) = view.child(1) {
                    if let Some(expression_data) = expression.data() {
                        node.reduce(expression_data.clone())
                    }
                }
            },
            _ => ()
        }

        Ok(())
    }
}