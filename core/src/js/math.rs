use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::*;
use crate::js::Value::*;
use crate::js::utils::{get_positional_arguments, method_name};
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use log::trace;

/// Centralized dispatcher for static Math.x(...) builtins.
///
/// This includes:
/// - `Math.abs(x)`
type MathBuiltinHandler = fn(&[JavaScript]) -> Option<JavaScript>;
const MATH_BUILTINS: &[(&str, MathBuiltinHandler)] = &[("abs", math_builtin_abs)];

#[derive(Default)]
pub struct MathBuiltins;

impl<'a> RuleMut<'a> for MathBuiltins {
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

        let Some(object) = callee.child(0).or_else(|| callee.named_child("object")) else {
            return Ok(());
        };
        let Some(method) = method_name(&callee) else {
            return Ok(());
        };

        if object.kind() != "identifier" || object.text() != Ok("Math") {
            return Ok(());
        }

        let args = view.named_child("arguments");
        let positional_args = get_positional_arguments(args);
        let mut arg_values = Vec::with_capacity(positional_args.len());
        for arg in positional_args {
            let Some(value) = arg.data().cloned() else {
                return Ok(());
            };
            arg_values.push(value);
        }

        let Some(result) = dispatch_math_builtin(&method, &arg_values) else {
            return Ok(());
        };

        trace!("MathBuiltins: reducing Math.{}(...) to {}", method, result);
        node.reduce(result);
        Ok(())
    }
}

fn dispatch_math_builtin(method: &str, args: &[JavaScript]) -> Option<JavaScript> {
    MATH_BUILTINS
        .iter()
        .find_map(|(name, handler)| (*name == method).then(|| handler(args)))
        .flatten()
}

fn math_builtin_abs(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num(n.abs()))),
        _ => Some(NaN),
    }
}

#[cfg(test)]
mod test_maths {
    use crate::js::build_javascript_tree;
    use crate::js::integer::{AddInt, ParseInt, PosNeg};
    use crate::js::linter::Linter;
    use crate::js::math::MathBuiltins;
    use crate::js::specials::ParseSpecials;
    use crate::js::string::ParseString;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseString::default(),
            ParseSpecials::default(),
            PosNeg::default(),
            AddInt::default(),
            MathBuiltins::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_math_abs() {
        assert_eq!(deobfuscate("Math.abs(-5)"), "5");
        assert_eq!(deobfuscate("Math.abs(3)"), "3");
        assert_eq!(deobfuscate("Math.abs(0)"), "0");
        assert_eq!(deobfuscate("Math.abs(-0)"), "0");
        assert_eq!(deobfuscate("Math.abs(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.abs()"), "NaN");
        assert_eq!(deobfuscate("Math.abs(null)"), "0");
    }
}
