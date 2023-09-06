use rule::RuleMut;
use ps::InferredValue;
use tree::NodeMut;
use error::{MinusOneResult, Error};

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
/// use minusone::ps::from_powershell_src;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::InferredValue::Number;
/// use minusone::ps::integer::ParseInt;
///
/// let mut tree = from_powershell_src("4").unwrap();
/// tree.apply_mut(&mut (ParseInt::default(), Forward::default())).unwrap();
///
/// assert_eq!(*(tree.root().unwrap().child(0).expect("At least one child").data().expect("A data in the first child")), Number(4));
/// ```
impl<'a> RuleMut<'a> for Forward {
    type Language = InferredValue;

    /// Nothing to do during top down exploration
    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        Ok(())
    }

    /// Forward the inferred type to the top node
    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()> {
        match node.view().kind() {
            "unary_expression" | "array_literal_expression" |
            "range_expression" | "format_expression" |
            "multiplicative_expression" | "additive_expression" |
            "comparison_expression" | "bitwise_expression" |
            "string_literal" | "logical_expression" |
            "integer_literal" | "pipeline" => {
                if node.view().child_count() == 1 {
                    if let Some(child_data) = node.view().child(0).ok_or(Error::invalid_child())?.data() {
                        node.set(child_data.clone());
                    }
                }
            }
            "sub_expression" => {
                if let Some(expression) = node.view().named_child("statements") {
                    // A sub expression must have only one statement to be reduced
                    if expression.child_count() == 1 {
                        if let Some(expression_data) = expression.child(0).ok_or(Error::invalid_child())?.data() {
                            node.set(expression_data.clone())
                        }
                    }
                }
            },
            "parenthesized_expression" => {
                if let Some(pipeline) = node.view().child(1) {
                    if let Some(pipeline_data) = pipeline.data() {
                        node.set(pipeline_data.clone())
                    }
                }
            }
            _ => ()
        }

        Ok(())
    }
}