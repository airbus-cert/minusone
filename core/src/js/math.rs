use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::*;
use crate::js::Value::*;
use crate::js::utils::{get_positional_arguments, method_name};
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use half::f16;
use log::trace;

/// Centralized dispatcher for static Math.x(...) builtins.
///
/// This includes:
/// - `Math.abs(x)`
/// - `Math.acos(x)`
/// - `Math.asin(x)`
/// - `Math.atan(x)`
/// - `Math.atan2(x)`
/// - `Math.cbrt(x)`
/// - `Math.cos(x)`
/// - `Math.sin(x)`
/// - `Math.sqrt(x)`
/// - `Math.tan(x)`
/// - `Math.ceil(x)`
/// - `Math.f16round(x)`
/// - `Math.floor(x)`
/// - `Math.fround(x)`
/// - `Math.round(x)`
/// - `Math.trunc(x)`
type MathBuiltinHandler = fn(&[JavaScript]) -> Option<JavaScript>;
const MATH_BUILTINS: &[(&str, MathBuiltinHandler)] = &[
    ("abs", math_builtin_abs),
    ("acos", math_builtin_acos),
    ("asin", math_builtin_asin),
    ("atan", math_builtin_atan),
    ("atan2", math_builtin_atan2),
    ("cbrt", math_builtin_cbrt),
    ("cos", math_builtin_cos),
    ("sin", math_builtin_sin),
    ("sqrt", math_builtin_sqrt),
    ("tan", math_builtin_tan),
    ("ceil", math_builtin_ceil),
    ("f16round", math_builtin_f16round),
    ("floor", math_builtin_floor),
    ("fround", math_builtin_fround),
    ("round", math_builtin_round),
    ("trunc", math_builtin_trunc),
];

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

        let result = match result {
            Raw(Num(n)) if n.is_nan() => NaN,
            any => any,
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

// Absolute value
fn math_builtin_abs(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num(n.abs()))),
        _ => Some(NaN),
    }
}

// Angles
fn math_builtin_acos(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num(n.acos()))),
        _ => Some(NaN),
    }
}

fn math_builtin_asin(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num(n.asin()))),
        _ => Some(NaN),
    }
}

fn math_builtin_atan(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num(n.atan()))),
        _ => Some(NaN),
    }
}

fn math_builtin_atan2(args: &[JavaScript]) -> Option<JavaScript> {
    if args.len() < 2 {
        return Some(NaN);
    }
    match (args[0].as_js_num(), args[1].as_js_num()) {
        (Raw(Num(y)), Raw(Num(x))) => Some(Raw(Num(y.atan2(x)))),
        _ => Some(NaN),
    }
}

fn math_builtin_cos(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num(n.cos()))),
        _ => Some(NaN),
    }
}

fn math_builtin_sin(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num(n.sin()))),
        _ => Some(NaN),
    }
}

fn math_builtin_tan(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num(n.tan()))),
        _ => Some(NaN),
    }
}

// Roots
fn math_builtin_cbrt(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num(n.cbrt()))),
        _ => Some(NaN),
    }
}

fn math_builtin_sqrt(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num(n.sqrt()))),
        _ => Some(NaN),
    }
}

// Rounding
fn math_builtin_ceil(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num(n.ceil()))),
        _ => Some(NaN),
    }
}

fn math_builtin_f16round(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num(f16::from_f64(n).to_f64()))),
        _ => Some(NaN),
    }
}

fn math_builtin_floor(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num(n.floor()))),
        _ => Some(NaN),
    }
}

pub fn math_builtin_fround(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num((n as f32) as f64))),
        _ => Some(NaN),
    }
}

fn math_builtin_round(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        // rust f64::round(x) != js Math.round(x)
        Raw(Num(n)) => Some(Raw(Num((n + 0.5).floor()))),
        _ => Some(NaN),
    }
}

fn math_builtin_trunc(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num(n.trunc()))),
        _ => Some(NaN),
    }
}

#[cfg(test)]
mod test_maths {
    use crate::js::build_javascript_tree;
    use crate::js::integer::{AddInt, MultInt, ParseInt, PosNeg};
    use crate::js::linter::Linter;
    use crate::js::math::MathBuiltins;
    use crate::js::objects::object::ObjectField;
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
            MultInt::default(),
            ObjectField::default(),
            MathBuiltins::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    // Absolute value
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

    // Angles
    #[test]
    fn test_math_acos() {
        assert_eq!(deobfuscate("Math.acos(1)"), "0");
        assert_eq!(deobfuscate("Math.acos(0)"), "1.5707963267948966");
        assert_eq!(deobfuscate("Math.acos(-1)"), "3.141592653589793");
        assert_eq!(deobfuscate("Math.acos(2)"), "NaN");
        assert_eq!(deobfuscate("Math.acos(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.acos()"), "NaN");
    }

    #[test]
    fn test_math_asin() {
        assert_eq!(deobfuscate("Math.asin(0)"), "0");
        assert_eq!(deobfuscate("Math.asin(1)"), "1.5707963267948966");
        assert_eq!(deobfuscate("Math.asin(-1)"), "-1.5707963267948966");
        assert_eq!(deobfuscate("Math.asin(2)"), "NaN");
        assert_eq!(deobfuscate("Math.asin(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.asin()"), "NaN");
    }

    #[test]
    fn test_math_atan() {
        assert_eq!(deobfuscate("Math.atan(0)"), "0");
        assert_eq!(deobfuscate("Math.atan(1)"), "0.7853981633974483");
        assert_eq!(deobfuscate("Math.atan(-1)"), "-0.7853981633974483");
        assert_eq!(deobfuscate("Math.atan(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.atan()"), "NaN");
    }

    #[test]
    fn test_math_atan2() {
        assert_eq!(deobfuscate("Math.atan2(0, 0)"), "0");
        assert_eq!(deobfuscate("Math.atan2(1, 0)"), "1.5707963267948966");
        assert_eq!(deobfuscate("Math.atan2(0, 1)"), "0");
        assert_eq!(deobfuscate("Math.atan2(-1, 0)"), "-1.5707963267948966");
        assert_eq!(deobfuscate("Math.atan2(0, -1)"), "3.141592653589793");
        assert_eq!(deobfuscate("Math.atan2(NaN, 1)"), "NaN");
        assert_eq!(deobfuscate("Math.atan2(1, NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.atan2()"), "NaN");
    }

    #[test]
    fn test_math_cos() {
        assert_eq!(deobfuscate("Math.cos(0)"), "1");
        assert_eq!(deobfuscate("Math.cos(Math.PI)"), "-1");
        assert_eq!(
            deobfuscate("Math.cos(Math.PI / 2)"),
            "0.00000000000000006123233995736766" // Will be wrong after fixing the scientific notation
        );
        assert_eq!(
            deobfuscate("Math.cos(-Math.PI / 2)"),
            "0.00000000000000006123233995736766" // Will be wrong after fixing the scientific notation
        );
        assert_eq!(deobfuscate("Math.cos(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.cos()"), "NaN");
    }

    #[test]
    fn test_math_sin() {
        assert_eq!(deobfuscate("Math.sin(0)"), "0");
        assert_eq!(deobfuscate("Math.sin(Math.PI / 2)"), "1");
        assert_eq!(deobfuscate("Math.sin(-Math.PI / 2)"), "-1");
        assert_eq!(
            deobfuscate("Math.sin(Math.PI)"),
            "0.00000000000000012246467991473532" // Will be wrong after fixing the scientific notation
        );
        assert_eq!(deobfuscate("Math.sin(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.sin()"), "NaN");
    }

    #[test]
    fn test_math_tan() {
        assert_eq!(deobfuscate("Math.tan(0)"), "0");
        assert_eq!(deobfuscate("Math.tan(Math.PI / 4)"), "0.9999999999999999");
        assert_eq!(deobfuscate("Math.tan(-Math.PI / 4)"), "-0.9999999999999999");
        assert_eq!(
            deobfuscate("Math.tan(Math.PI)"),
            "-0.00000000000000012246467991473532" // Will be wrong after fixing the scientific notation
        );
        assert_eq!(deobfuscate("Math.tan(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.tan()"), "NaN");
    }

    // Roots
    #[test]
    fn test_math_cbrt() {
        assert_eq!(deobfuscate("Math.cbrt(27)"), "3");
        assert_eq!(deobfuscate("Math.cbrt(-8)"), "-2");
        assert_eq!(deobfuscate("Math.cbrt(0)"), "0");
        assert_eq!(deobfuscate("Math.cbrt(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.cbrt()"), "NaN");
    }

    #[test]
    fn test_math_sqrt() {
        assert_eq!(deobfuscate("Math.sqrt(16)"), "4");
        assert_eq!(deobfuscate("Math.sqrt(0)"), "0");
        assert_eq!(deobfuscate("Math.sqrt(-1)"), "NaN");
        assert_eq!(deobfuscate("Math.sqrt(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.sqrt()"), "NaN");
    }

    // Rounding
    #[test]
    fn test_math_ceil() {
        assert_eq!(deobfuscate("Math.ceil(3.2)"), "4");
        assert_eq!(deobfuscate("Math.ceil(-3.2)"), "-3");
        assert_eq!(deobfuscate("Math.ceil(0)"), "0");
        assert_eq!(deobfuscate("Math.ceil(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.ceil()"), "NaN");
    }

    #[test]
    fn test_math_f16round() {
        assert_eq!(deobfuscate("Math.f16round(5.5)"), "5.5");
        assert_eq!(deobfuscate("Math.f16round(5.05)"), "5.05078125");
        assert_eq!(deobfuscate("Math.f16round(5)"), "5");
        assert_eq!(deobfuscate("Math.f16round(-5.05)"), "-5.05078125");
        assert_eq!(deobfuscate("Math.f16round(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.f16round()"), "NaN");
    }

    #[test]
    fn test_math_floor() {
        assert_eq!(deobfuscate("Math.floor(3.2)"), "3");
        assert_eq!(deobfuscate("Math.floor(-3.2)"), "-4");
        assert_eq!(deobfuscate("Math.floor(0)"), "0");
        assert_eq!(deobfuscate("Math.floor(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.floor()"), "NaN");
    }

    #[test]
    fn test_math_fround() {
        assert_eq!(deobfuscate("Math.fround(5.5)"), "5.5");
        assert_eq!(deobfuscate("Math.fround(5.05)"), "5.050000190734863");
        assert_eq!(deobfuscate("Math.fround(5)"), "5");
        assert_eq!(deobfuscate("Math.fround(-5.05)"), "-5.050000190734863");
        assert_eq!(deobfuscate("Math.fround(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.fround()"), "NaN");
    }

    #[test]
    fn test_math_round() {
        assert_eq!(deobfuscate("Math.round(3.5)"), "4");
        assert_eq!(deobfuscate("Math.round(3.2)"), "3");
        assert_eq!(deobfuscate("Math.round(-3.5)"), "-3");
        assert_eq!(deobfuscate("Math.round(-3.2)"), "-3");
        assert_eq!(deobfuscate("Math.round(0)"), "0");
        assert_eq!(deobfuscate("Math.round(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.round()"), "NaN");
    }

    #[test]
    fn test_math_trunc() {
        assert_eq!(deobfuscate("Math.trunc(3.5)"), "3");
        assert_eq!(deobfuscate("Math.trunc(3.2)"), "3");
        assert_eq!(deobfuscate("Math.trunc(-3.5)"), "-3");
        assert_eq!(deobfuscate("Math.trunc(-3.2)"), "-3");
        assert_eq!(deobfuscate("Math.trunc(0)"), "0");
        assert_eq!(deobfuscate("Math.trunc(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.trunc()"), "NaN");
    }
}
