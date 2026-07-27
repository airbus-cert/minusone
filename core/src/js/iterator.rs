use crate::error::MinusOneResult;
use crate::js::JavaScript::*;
use crate::js::Value::*;
use crate::js::utils::{get_positional_arguments, method_name};
use crate::js::{IteratorKind, JavaScript};
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use log::trace;
use std::collections::HashMap;

type IteratorBuiltinHandler = fn(&mut JavaScript, &[JavaScript]) -> Option<JavaScript>;

const ITERATOR_BUILTINS: &[(&str, IteratorBuiltinHandler)] = &[("next", iterator_builtin_next)];

#[derive(Default)]
pub struct IteratorBuiltins;

impl<'a> RuleMut<'a> for IteratorBuiltins {
    type Language = JavaScript;

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
        if view.kind() != "call_expression" {
            return Ok(());
        }

        let Some(callee) = view.named_child("function").or_else(|| view.child(0)) else {
            return Ok(());
        };
        let Some(method) = method_name(&callee) else {
            return Ok(());
        };

        let Some(object) = callee.child(0).or_else(|| callee.named_child("object")) else {
            return Ok(());
        };

        // todo implement .data_mut() or find a way to alter var manager afterward
        let Some(input @ Iterator { .. }) = object.data() else {
            return Ok(());
        };

        let mut input = input.clone();

        let args = view.named_child("arguments");
        let positional_args = get_positional_arguments(args);
        let mut arg_values = Vec::with_capacity(positional_args.len());
        for arg in positional_args {
            let Some(value) = arg.data().cloned() else {
                return Ok(());
            };
            arg_values.push(value);
        }

        let Some(result) = dispatch_iterator_builtin(&method, &mut input, &arg_values) else {
            return Ok(());
        };

        trace!(
            "IteratorBuiltins: reducing [object Array Iterator].{}(...) to {}",
            method, result
        );
        node.reduce(result);
        Ok(())
    }
}

fn dispatch_iterator_builtin(
    method: &str,
    input: &mut JavaScript,
    args: &[JavaScript],
) -> Option<JavaScript> {
    ITERATOR_BUILTINS
        .iter()
        .find_map(|(name, handler)| (*name == method).then(|| handler(input, args)))
        .flatten()
}

fn iterator_builtin_next(iter: &mut JavaScript, _args: &[JavaScript]) -> Option<JavaScript> {
    if let Iterator {
        values,
        index,
        kind,
    } = iter
    {
        if *index < values.len() {
            let value = match kind {
                IteratorKind::Values => values[*index].clone(),
                IteratorKind::Entries => {
                    Array(vec![Raw(Num(*index as f64)), values[*index].clone()])
                }
            };

            *index += 1;

            let mut map = HashMap::new();
            map.insert("value".to_string(), value);
            map.insert("done".to_string(), Raw(Bool(false)));

            Some(Object {
                map,
                to_string_override: None,
            })
        } else {
            let mut map = HashMap::new();
            map.insert("value".to_string(), Undefined);
            map.insert("done".to_string(), Raw(Bool(true)));

            Some(Object {
                map,
                to_string_override: None,
            })
        }
    } else {
        None
    }
}
