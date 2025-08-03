use crate::rule::RuleMut;
use crate::tree::{NodeMut, ControlFlow};
use crate::error::{MinusOneResult, Error};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum PowershellDetect {
    Static(bool),
    StaticCast(String)
}

pub struct Pattern {
    pub start_offset : usize,
    pub end_offset : usize
}

impl Pattern {
    pub fn new(start_offset: usize, end_offset: usize) -> Self {
        Pattern {
            start_offset,
            end_offset
        }
    }
}

pub trait Detection {
    fn get_nodes(&self) -> &Vec<Pattern>;
}

#[derive(Default)]
pub struct Static;

impl<'a> RuleMut<'a> for Static {
    type Language = PowershellDetect;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>, _flow: ControlFlow) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>, _flow: ControlFlow) -> MinusOneResult<()>{
        let view = node.view();
        match view.kind() {
            "decimal_integer_literal" | "hexadecimal_integer_literal" => node.set(PowershellDetect::Static(true)),
            // Forward static on simple node
            "unary_expression" | "range_expression" |
            "format_expression" | "comparison_expression" |
            "bitwise_expression" | "string_literal" |
            "logical_expression" | "integer_literal" |
            "argument_expression" | "range_argument_expression" |
            "format_argument_expression" | "comparison_argument_expression" |
            "bitwise_argument_expression" | "logical_argument_expression" |
            "command_name_expr" | "pipeline" |
            "statement_list" | "expression_with_unary_operator" => {
                if view.child_count() == 1 {
                    if let Some(PowershellDetect::Static(data)) = view.child(0).ok_or(Error::invalid_child())?.data() {
                        node.set(PowershellDetect::Static(data.clone()))
                    }
                }
            },
            // complex expression
            "parenthesized_expression" | "sub_expression" => {
                if view.child_count() == 3 {
                    if let Some(PowershellDetect::Static(data)) = view.child(1).ok_or(Error::invalid_child())?.data() {
                        node.set(PowershellDetect::Static(data.clone()))
                    }
                }
            },

            "additive_argument_expression" | "additive_expression" |
            "multiplicative_expression" | "multiplicative_argument_expression" | "array_literal_expression" => {
                match view.child_count() {
                    1 => {
                        if let Some(PowershellDetect::Static(data)) = view.child(0).ok_or(Error::invalid_child())?.data() {
                            node.set(PowershellDetect::Static(data.clone()))
                        }
                    },
                    3 => {
                        if let (Some(PowershellDetect::Static(left)), Some(PowershellDetect::Static(right))) =
                            (
                                view.child(0).ok_or(Error::invalid_child())?.data(),
                                view.child(2).ok_or(Error::invalid_child())?.data()
                            ) {
                            node.set(PowershellDetect::Static(*left && *right))
                        }
                    },
                    _ => ()
                }
            },
            "expandable_string_literal" => {
                if view.child_count() == 0 {
                    node.set(PowershellDetect::Static(true))
                }
            },
            "cast_expression" => {
                if let (Some(_type_literal), Some(unary_expression)) = (view.child(0), view.child(1)) {
                    if unary_expression.data() == Some(&PowershellDetect::Static(true)) {
                        node.set(PowershellDetect::Static(true))
                    }
                }
            }
            _ => ()
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct StaticArray {
    detected_nodes : Vec<Pattern>
}

impl<'a> RuleMut<'a> for StaticArray {
    type Language = PowershellDetect;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>, _flow: ControlFlow) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>, _flow: ControlFlow) -> MinusOneResult<()>{
        let view = node.view();
        if view.kind() == "array_literal_expression" && view.parent().ok_or(Error::invalid_child())?.kind() != "array_literal_expression" && view.child_count() > 1 {
            if let Some(PowershellDetect::Static(true)) = view.data() {
                self.detected_nodes.push(Pattern::new(view.start_abs(), view.end_abs()));
            }
        }
        Ok(())
    }
}

impl Detection for StaticArray {
    fn get_nodes(&self) -> &Vec<Pattern> {
        &self.detected_nodes
    }
}

#[derive(Default)]
pub struct StaticFormat {
    detected_nodes : Vec<Pattern>
}

impl<'a> RuleMut<'a> for StaticFormat {
    type Language = PowershellDetect;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>, _flow: ControlFlow) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>, _flow: ControlFlow) -> MinusOneResult<()>{
        let view = node.view();
        if view.kind() == "format_expression"  {
            if let (Some(expression), Some(range_expression )) = (view.child(0), view.child(2)) {
                match (expression.data(), range_expression.data()) {
                    (Some(PowershellDetect::Static(true)), Some(PowershellDetect::Static(true))) => {
                        self.detected_nodes.push(Pattern::new(view.start_abs(), view.end_abs()))
                    }
                    _ => ()
                }
            }
        }
        Ok(())
    }
}

impl Detection for StaticFormat {
    fn get_nodes(&self) -> &Vec<Pattern> {
        &self.detected_nodes
    }
}

pub type RuleSet = (
    Static,
    StaticArray,
    StaticFormat,
);
