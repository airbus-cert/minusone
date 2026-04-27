use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::*;
use crate::js::Value::*;
use crate::js::objects::objectify::constructor_name;
use crate::js::utils::*;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use half::f16;
use log::{error, trace};

/// Centralized dispatcher for static Math.x(...) builtins.
///
/// This includes:
/// - `Math.abs(x)`
/// - `Math.acos(x)`
/// - `Math.acosh(x)`
/// - `Math.asin(x)`
/// - `Math.asinh(x)`
/// - `Math.atan(x)`
/// - `Math.atan2(x)`
/// - `Math.atanh(x)`
/// - `Math.cbrt(x)`
/// - `Math.ceil(x)`
/// - `Math.clz32(x)`
/// - `Math.cos(x)`
/// - `Math.cosh(x)`
/// - `Math.exp(x)`
/// - `Math.expm1(x)`
/// - `Math.f16round(x)`
/// - `Math.floor(x)`
/// - `Math.fround(x)`
/// - `Math.hypot(x, ...)`
/// - `Math.imul(x, y)`
/// - `Math.log(x)`
/// - `Math.log10(x)`
/// - `Math.log1p(x)`
/// - `Math.log2(x)`
/// - `Math.max(x, ...)`
/// - `Math.min(x, ...)`
/// - `Math.pow(x)`
/// - `Math.random(x)`
/// - `Math.round(x)`
/// - `Math.sign(x)`
/// - `Math.sin(x)`
/// - `Math.sinh(x)`
/// - `Math.sqrt(x)`
/// - `Math.sumPrecise([x, ...])`
/// - `Math.tan(x)`
/// - `Math.tanh(x)`
/// - `Math.trunc(x)`
type MathBuiltinHandler = fn(&[JavaScript]) -> Option<JavaScript>;
const MATH_BUILTINS: &[(&str, MathBuiltinHandler)] = &[
    ("abs", math_builtin_abs),
    ("acos", math_builtin_acos),
    ("acosh", math_builtin_acosh),
    ("asin", math_builtin_asin),
    ("asinh", math_builtin_asinh),
    ("atan", math_builtin_atan),
    ("atan2", math_builtin_atan2),
    ("atanh", math_builtin_atanh),
    ("cbrt", math_builtin_cbrt),
    ("ceil", math_builtin_ceil),
    ("clz32", math_builtin_clz32),
    ("cos", math_builtin_cos),
    ("cosh", math_builtin_cosh),
    ("exp", math_builtin_exp),
    ("expm1", math_builtin_expm1),
    ("f16round", math_builtin_f16round),
    ("floor", math_builtin_floor),
    ("fround", math_builtin_fround),
    ("hypot", math_builtin_hypot),
    ("imul", math_builtin_imul),
    ("log", math_builtin_log),
    ("log10", math_builtin_log10),
    ("log1p", math_builtin_log1p),
    ("log2", math_builtin_log2),
    ("max", math_builtin_max),
    ("min", math_builtin_min),
    ("pow", math_builtin_pow),
    ("random", |_| Some(Raw(Num(rand::random::<f64>())))),
    ("round", math_builtin_round),
    ("sign", math_builtin_sign),
    ("sin", math_builtin_sin),
    ("sinh", math_builtin_sinh),
    ("sqrt", math_builtin_sqrt),
    ("sumPrecise", math_builtin_sum_precise),
    ("tan", math_builtin_tan),
    ("tanh", math_builtin_tanh),
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

// Log
fn math_builtin_log(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num(n.ln()))),
        _ => Some(NaN),
    }
}

fn math_builtin_log1p(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num((1.0 + n).ln()))),
        _ => Some(NaN),
    }
}

fn math_builtin_log2(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num(n.log2()))),
        _ => Some(NaN),
    }
}

fn math_builtin_log10(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num(n.log10()))),
        _ => Some(NaN),
    }
}

// Exponential
fn math_builtin_exp(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num(n.exp()))),
        _ => Some(NaN),
    }
}

fn math_builtin_expm1(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num(n.exp() - 1.0))),
        _ => Some(NaN),
    }
}

// min/max
fn math_builtin_max(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(Raw(Num(f64::NEG_INFINITY)));
    }
    let mut max = args[0].as_js_num();
    for arg in args.iter().skip(1) {
        max = match (max, arg.as_js_num()) {
            (Raw(Num(a)), Raw(Num(b))) => Raw(Num(a.max(b))),
            _ => return Some(NaN),
        }
    }
    Some(max)
}

pub fn math_builtin_min(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(Raw(Num(f64::INFINITY)));
    }
    let mut min = args[0].as_js_num();
    for arg in args.iter().skip(1) {
        min = match (min, arg.as_js_num()) {
            (Raw(Num(a)), Raw(Num(b))) => Raw(Num(a.min(b))),
            _ => return Some(NaN),
        }
    }
    Some(min)
}

// Power
fn math_builtin_pow(args: &[JavaScript]) -> Option<JavaScript> {
    if args.len() < 2 {
        return Some(NaN);
    }
    match (args[0].as_js_num(), args[1].as_js_num()) {
        (Raw(Num(x)), Raw(Num(y))) => Some(Raw(Num(x.powf(y)))),
        _ => Some(NaN),
    }
}

// Count leading zeros
fn math_builtin_clz32(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(Raw(Num(32.0)));
    }
    match args[0].as_js_num() {
        Raw(Num(x)) => Some(Raw(Num(to_js_uint32(x).leading_zeros() as f64))),
        _ => Some(Raw(Num(32.0))),
    }
}

// Hyperbolic functions
fn math_builtin_sinh(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num(n.sinh()))),
        _ => Some(NaN),
    }
}

fn math_builtin_cosh(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num(n.cosh()))),
        _ => Some(NaN),
    }
}

fn math_builtin_tanh(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num(n.tanh()))),
        _ => Some(NaN),
    }
}

// Arc hyperbolic functions
fn math_builtin_asinh(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num(n.asinh()))),
        _ => Some(NaN),
    }
}

fn math_builtin_acosh(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num(n.acosh()))),
        _ => Some(NaN),
    }
}

fn math_builtin_atanh(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args.first()?.as_js_num() {
        Raw(Num(n)) => Some(Raw(Num(n.atanh()))),
        _ => Some(NaN),
    }
}

// Hypotenuse
fn math_builtin_hypot(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(Raw(Num(0.0)));
    }
    let mut sum_sq = 0.0_f64;
    for arg in args {
        match arg.as_js_num() {
            Raw(Num(x)) => sum_sq += x * x,
            _ => return Some(NaN),
        }
    }
    Some(Raw(Num(sum_sq.sqrt())))
}

// Multiplication
fn math_builtin_imul(args: &[JavaScript]) -> Option<JavaScript> {
    if args.len() < 2 {
        return Some(Raw(Num(0.0)));
    }
    match (args[0].as_js_num(), args[1].as_js_num()) {
        (Raw(Num(x)), Raw(Num(y))) => {
            let a = to_js_uint32(x) as i32;
            let b = to_js_uint32(y) as i32;
            Some(Raw(Num(a.wrapping_mul(b) as f64)))
        }
        _ => Some(NaN),
    }
}

// Sign
fn math_builtin_sign(args: &[JavaScript]) -> Option<JavaScript> {
    if args.is_empty() {
        return Some(NaN);
    }
    match args[0].as_js_num() {
        Raw(Num(n)) => {
            // it doesn't sign 0.0 to 1.0, but keeps it as 0.0
            if n == 0.0 {
                Some(Raw(Num(n)))
            } else {
                Some(Raw(Num(n.signum())))
            }
        }
        _ => Some(NaN),
    }
}

// Sum
#[derive(PartialEq)]
enum SumState {
    MinusZero,
    PlusInfinity,
    MinusInfinity,
    Finite,
}
/// See https://tc39.es/proposal-math-sum/
fn math_builtin_sum_precise(args: &[JavaScript]) -> Option<JavaScript> {
    let items = match args.first() {
        Some(Array(items)) => items,
        _ => {
            error!(
                "Math.sumPrecise: expected an 'Array' but got '{}'. This should crash the engine, skipping...",
                args.first()
                    .map_or("undefined", |arg| constructor_name(arg))
            );
            return None;
        }
    };

    let mut state = SumState::MinusZero;
    let mut partials: Vec<f64> = Vec::new();

    for elem in items {
        let n = match elem {
            Raw(Num(x)) => *x,
            NaN => {
                return Some(NaN);
            }
            other => {
                error!(
                    "Math.sumPrecise: expected a 'Number' but got '{}'. This should crash the engine, skipping...",
                    constructor_name(other)
                );
                return None;
            }
        };

        if n.is_nan() {
            return Some(NaN);
        } else if n == f64::INFINITY {
            if state == SumState::MinusInfinity {
                return Some(NaN);
            } else {
                state = SumState::PlusInfinity;
            }
        } else if n == f64::NEG_INFINITY {
            if state == SumState::PlusInfinity {
                return Some(NaN);
            } else {
                state = SumState::MinusInfinity;
            }
        } else {
            let is_neg_zero = n == 0.0 && n.is_sign_negative();
            if !is_neg_zero && (state == SumState::MinusZero || state == SumState::Finite) {
                state = SumState::Finite;
                grow_expansion(&mut partials, n);
            }
        }
    }

    Some(match state {
        SumState::PlusInfinity => Raw(Num(f64::INFINITY)),
        SumState::MinusInfinity => Raw(Num(f64::NEG_INFINITY)),
        SumState::MinusZero => Raw(Num(-0.0)),
        SumState::Finite => Raw(Num(sum_partials(&partials))),
    })
}

fn sum_partials(partials: &[f64]) -> f64 {
    partials.iter().rev().fold(0.0_f64, |acc, &p| acc + p)
}

fn grow_expansion(partials: &mut Vec<f64>, mut x: f64) {
    let mut i = 0;
    for k in 0..partials.len() {
        let y = partials[k];
        let (hi, lo) = two_sum(x, y);
        if lo != 0.0 {
            partials[i] = lo;
            i += 1;
        }
        x = hi;
    }
    partials.truncate(i);
    if x != 0.0 {
        partials.push(x);
    }
}

// Møller-Knuth algorithm
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let a2 = s - b;
    let b2 = s - a2;
    (s, (a - a2) + (b - b2))
}

#[cfg(test)]
mod test_maths {
    use crate::js::array::ParseArray;
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
            ParseArray::default(),
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

    // Log
    #[test]
    fn test_math_log() {
        assert_eq!(deobfuscate("Math.log(1)"), "0");
        assert_eq!(deobfuscate("Math.log(Math.E)"), "1");
        assert_eq!(deobfuscate("Math.log(0)"), "-Infinity");
        assert_eq!(deobfuscate("Math.log(-1)"), "NaN");
        assert_eq!(deobfuscate("Math.log(8) / Math.log(2)"), "3");
        assert_eq!(deobfuscate("Math.log(625) / Math.log(5)"), "4");
        assert_eq!(deobfuscate("Math.log(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.log()"), "NaN");
    }

    #[test]
    fn test_math_log1p() {
        assert_eq!(deobfuscate("Math.log1p(0)"), "0");
        assert_eq!(deobfuscate("Math.log1p(Math.E - 1)"), "1");
        assert_eq!(deobfuscate("Math.log1p(-1)"), "-Infinity");
        assert_eq!(deobfuscate("Math.log1p(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.log1p()"), "NaN");
    }

    #[test]
    fn test_math_log2() {
        assert_eq!(deobfuscate("Math.log2(3)"), "1.584962500721156");
        assert_eq!(deobfuscate("Math.log2(2)"), "1");
        assert_eq!(deobfuscate("Math.log2(1)"), "0");
        assert_eq!(deobfuscate("Math.log2(0)"), "-Infinity");
        assert_eq!(deobfuscate("Math.log2(-1)"), "NaN");
        assert_eq!(deobfuscate("Math.log2(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.log2()"), "NaN");
    }

    #[test]
    fn test_math_log10() {
        assert_eq!(deobfuscate("Math.log10(100000)"), "5");
        assert_eq!(deobfuscate("Math.log10(2)"), "0.3010299956639812");
        assert_eq!(deobfuscate("Math.log10(1)"), "0");
        assert_eq!(deobfuscate("Math.log10(0)"), "-Infinity");
        assert_eq!(deobfuscate("Math.log10(-1)"), "NaN");
        assert_eq!(deobfuscate("Math.log10(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.log10()"), "NaN");
    }

    // Exponential
    #[test]
    fn test_math_exp() {
        assert_eq!(deobfuscate("Math.exp(0)"), "1");
        assert_eq!(deobfuscate("Math.exp(1)"), "2.718281828459045");
        assert_eq!(deobfuscate("Math.exp(-1)"), "0.36787944117144233");
        assert_eq!(deobfuscate("Math.exp(-Infinity)"), "0");
        assert_eq!(deobfuscate("Math.exp(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.exp()"), "NaN");
    }

    #[test]
    fn test_math_expm1() {
        assert_eq!(deobfuscate("Math.expm1(0)"), "0");
        assert_eq!(deobfuscate("Math.expm1(1)"), "1.718281828459045");
        assert_eq!(deobfuscate("Math.expm1(-1)"), "-0.6321205588285577");
        assert_eq!(deobfuscate("Math.expm1(-Infinity)"), "-1");
        assert_eq!(deobfuscate("Math.expm1(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.expm1()"), "NaN");
    }

    // Min/max
    #[test]
    fn test_math_min() {
        assert_eq!(deobfuscate("Math.min(1, [2], 3)"), "1");
        assert_eq!(deobfuscate("Math.min(3, [2], 1)"), "1");
        assert_eq!(deobfuscate("Math.min(1, 2, NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.min(1, 2)"), "1");
        assert_eq!(deobfuscate("Math.min(1)"), "1");
        assert_eq!(deobfuscate("Math.min()"), "Infinity");
    }

    #[test]
    fn test_math_max() {
        assert_eq!(deobfuscate("Math.max(1, 2, 3)"), "3");
        assert_eq!(deobfuscate("Math.max(3, 2, 1)"), "3");
        assert_eq!(deobfuscate("Math.max(1, 2, NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.max(1, 2)"), "2");
        assert_eq!(deobfuscate("Math.max(1)"), "1");
        assert_eq!(deobfuscate("Math.max()"), "-Infinity");
    }

    // Power
    #[test]
    fn test_math_pow() {
        assert_eq!(deobfuscate("Math.pow(2, 3)"), "8");
        assert_eq!(deobfuscate("Math.pow(5, 0)"), "1");
        assert_eq!(deobfuscate("Math.pow(2, -1)"), "0.5");
        assert_eq!(deobfuscate("Math.pow(-2, 3)"), "-8");
        assert_eq!(deobfuscate("Math.pow(-2, 2)"), "4");
        assert_eq!(deobfuscate("Math.pow(-2, 0.5)"), "NaN");
        assert_eq!(deobfuscate("Math.pow(NaN, 2)"), "NaN");
        assert_eq!(deobfuscate("Math.pow(2, NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.pow()"), "NaN");
    }

    // Count leading zeros
    #[test]
    fn test_math_clz32() {
        assert_eq!(deobfuscate("Math.clz32(0)"), "32");
        assert_eq!(deobfuscate("Math.clz32(1)"), "31");
        assert_eq!(deobfuscate("Math.clz32(2)"), "30");
        assert_eq!(deobfuscate("Math.clz32(3)"), "30");
        assert_eq!(deobfuscate("Math.clz32(4)"), "29");
        assert_eq!(deobfuscate("Math.clz32(1024)"), "21");
        assert_eq!(deobfuscate("Math.clz32(4294967295)"), "0");
        assert_eq!(deobfuscate("Math.clz32(-1)"), "0");
        assert_eq!(deobfuscate("Math.clz32(NaN)"), "32");
        assert_eq!(deobfuscate("Math.clz32()"), "32");
        assert_eq!(deobfuscate("Math.clz32(Infinity)"), "32");
        assert_eq!(deobfuscate("Math.clz32(-Infinity)"), "32");
    }

    // Hyperbolic functions
    #[test]
    fn test_math_cosh() {
        assert_eq!(deobfuscate("Math.cosh(0)"), "1");
        assert_eq!(deobfuscate("Math.cosh(1)"), "1.5430806348152437");
        assert_eq!(deobfuscate("Math.cosh(-1)"), "1.5430806348152437");
        assert_eq!(deobfuscate("Math.cosh(2)"), "3.7621956910836314");
        assert_eq!(deobfuscate("Math.cosh(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.cosh()"), "NaN");
    }

    #[test]
    fn test_math_sinh() {
        assert_eq!(deobfuscate("Math.sinh(0)"), "0");
        assert_eq!(deobfuscate("Math.sinh(1)"), "1.1752011936438014");
        assert_eq!(deobfuscate("Math.sinh(-1)"), "-1.1752011936438014");
        assert_eq!(deobfuscate("Math.sinh(2)"), "3.626860407847019");
        assert_eq!(deobfuscate("Math.sinh(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.sinh()"), "NaN");
    }

    #[test]
    fn test_math_tanh() {
        assert_eq!(deobfuscate("Math.tanh(0)"), "0");
        assert_eq!(deobfuscate("Math.tanh(1)"), "0.7615941559557649");
        assert_eq!(deobfuscate("Math.tanh(-1)"), "-0.7615941559557649");
        assert_eq!(deobfuscate("Math.tanh(2)"), "0.9640275800758169");
        assert_eq!(deobfuscate("Math.tanh(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.tanh()"), "NaN");
    }

    #[test]
    fn test_math_acosh() {
        assert_eq!(deobfuscate("Math.acosh(1)"), "0");
        assert_eq!(deobfuscate("Math.acosh(2)"), "1.3169578969248166");
        assert_eq!(deobfuscate("Math.acosh(3)"), "1.762747174039086");
        assert_eq!(deobfuscate("Math.acosh(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.acosh(-1)"), "NaN");
    }

    #[test]
    fn test_math_asinh() {
        assert_eq!(deobfuscate("Math.asinh(0)"), "0");
        assert_eq!(deobfuscate("Math.asinh(1)"), "0.881373587019543");
        assert_eq!(deobfuscate("Math.asinh(-1)"), "-0.881373587019543");
        assert_eq!(deobfuscate("Math.asinh(2)"), "1.4436354751788103");
        assert_eq!(deobfuscate("Math.asinh(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.asinh()"), "NaN");
    }

    #[test]
    fn test_math_atanh() {
        assert_eq!(deobfuscate("Math.atanh(0)"), "0");
        assert_eq!(deobfuscate("Math.atanh(0.5)"), "0.5493061443340548");
        assert_eq!(deobfuscate("Math.atanh(-0.5)"), "-0.5493061443340548");
        assert_eq!(deobfuscate("Math.atanh(1)"), "Infinity");
        assert_eq!(deobfuscate("Math.atanh(-1)"), "-Infinity");
        assert_eq!(deobfuscate("Math.atanh(2)"), "NaN");
        assert_eq!(deobfuscate("Math.atanh(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.atanh()"), "NaN");
    }

    // Hypotenuse
    #[test]
    fn test_math_hypot() {
        assert_eq!(deobfuscate("Math.hypot(3, 4)"), "5");
        assert_eq!(deobfuscate("Math.hypot(5, 12)"), "13");
        assert_eq!(deobfuscate("Math.hypot(3, 4, 5)"), "7.0710678118654755");
        assert_eq!(deobfuscate("Math.hypot(-5)"), "5");
        assert_eq!(deobfuscate("Math.hypot()"), "0");
        assert_eq!(deobfuscate("Math.hypot(NaN)"), "NaN");
    }

    // Multiplication
    #[test]
    fn test_math_imul() {
        assert_eq!(deobfuscate("Math.imul(3, 4)"), "12");
        assert_eq!(deobfuscate("Math.imul(-5, 12)"), "-60");
        assert_eq!(deobfuscate("Math.imul(0xffffffff, 5)"), "-5");
        assert_eq!(deobfuscate("Math.imul(0xfffffffe, 5)"), "-10");
        assert_eq!(deobfuscate("Math.imul(1, 2, 3)"), "2");
        assert_eq!(deobfuscate("Math.imul(1)"), "0");
        assert_eq!(deobfuscate("Math.imul()"), "0");
    }

    // Sign
    #[test]
    fn test_math_sign() {
        assert_eq!(deobfuscate("Math.sign(3)"), "1");
        assert_eq!(deobfuscate("Math.sign(-3)"), "-1");
        assert_eq!(deobfuscate("Math.sign(0)"), "0");
        assert_eq!(deobfuscate("Math.sign(-0)"), "-0");
        assert_eq!(deobfuscate("Math.sign(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.sign()"), "NaN");
        assert_eq!(deobfuscate("Math.sign(Infinity)"), "1");
        assert_eq!(deobfuscate("Math.sign(-Infinity)"), "-1");
    }

    // Sum
    #[test]
    fn test_math_sum_precise() {
        assert_eq!(deobfuscate("Math.sumPrecise([])"), "-0");
        assert_eq!(deobfuscate("Math.sumPrecise([-0, -0])"), "-0");
        assert_eq!(deobfuscate("Math.sumPrecise([1, 2, 3])"), "6");
        assert_eq!(deobfuscate("Math.sumPrecise([1e20, 0.1, -1e20])"), "0.1");
        assert_eq!(
            deobfuscate("Math.sumPrecise([0.1, 0.2])"),
            "0.30000000000000004"
        );
        assert_eq!(deobfuscate("Math.sumPrecise([NaN, 1])"), "NaN");
        assert_eq!(deobfuscate("Math.sumPrecise([Infinity, -Infinity])"), "NaN");
        assert_eq!(deobfuscate("Math.sumPrecise([Infinity, 1])"), "Infinity");
        assert_eq!(deobfuscate("Math.sumPrecise([-Infinity])"), "-Infinity");
    }
}
