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
