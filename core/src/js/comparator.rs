use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::*;
use crate::js::Value::*;
use crate::js::array::flatten_array;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use log::{trace, warn};
use num::FromPrimitive;
use std::cmp::Ordering;
use std::cmp::Ordering::*;

/// Infers `===` (strict equality) and `!==` (strict inequality).
/// No type coercion is applied: distinct types always yield `false`/`true`.
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::integer::ParseInt;
/// use minusone::js::comparator::StrictEq;
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = 1 === 1;").unwrap();
/// tree.apply_mut(&mut (ParseInt::default(), StrictEq::default())).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "var x = true;");
/// ```
#[derive(Default)]
pub struct StrictEq;

impl<'a> RuleMut<'a> for StrictEq {
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
        if view.kind() != "binary_expression" {
            return Ok(());
        }

        if let (Some(left), Some(op), Some(right)) = (view.child(0), view.child(1), view.child(2)) {
            let op_str = op.text()?;
            if op_str != "===" && op_str != "!==" {
                return Ok(());
            }

            let eq: Option<bool> = match (left.data(), right.data()) {
                (Some(Raw(Num(l))), Some(Raw(Num(r)))) => {
                    trace!("StrictEq (L): {} {} {} = {}", l, op_str, r, l == r);
                    Some(l == r)
                }
                (Some(Raw(BigInt(l))), Some(Raw(BigInt(r)))) => {
                    trace!("StrictEq (L): {}n {} {}n = {}", l, op_str, r, l == r);
                    Some(l == r)
                }
                (Some(Raw(Str(l))), Some(Raw(Str(r)))) => {
                    trace!("StrictEq (L): {:?} {} {:?} = {}", l, op_str, r, l == r);
                    Some(l == r)
                }
                (Some(Raw(Bool(l))), Some(Raw(Bool(r)))) => {
                    trace!("StrictEq (L): {} {} {} = {}", l, op_str, r, l == r);
                    Some(l == r)
                }
                (Some(Undefined), Some(Undefined)) => {
                    let res = op_str == "===";
                    trace!("StrictEq (L): undefined {} undefined = {}", op_str, res);
                    Some(res)
                }
                (Some(Null), Some(Null)) => {
                    let res = op_str == "===";
                    trace!("StrictEq (L): null {} null = {}", op_str, res);
                    Some(res)
                }
                // NaN is never strictly equal to anything, including itself ??
                (Some(NaN), _) | (_, Some(NaN)) => {
                    trace!("StrictEq (L): NaN {} _ = false", op_str);
                    Some(false)
                }
                // cross-type: always false for ===
                (Some(Raw(Num(_))), Some(Raw(Str(_))))
                | (Some(Raw(Str(_))), Some(Raw(Num(_))))
                | (Some(Raw(Num(_))), Some(Raw(Bool(_))))
                | (Some(Raw(Bool(_))), Some(Raw(Num(_))))
                | (Some(Raw(Str(_))), Some(Raw(Bool(_))))
                | (Some(Raw(Bool(_))), Some(Raw(Str(_))))
                | (Some(Raw(BigInt(_))), Some(Raw(Num(_))))
                | (Some(Raw(Num(_))), Some(Raw(BigInt(_))))
                | (Some(Raw(BigInt(_))), Some(Raw(Str(_))))
                | (Some(Raw(Str(_))), Some(Raw(BigInt(_))))
                | (Some(Raw(BigInt(_))), Some(Raw(Bool(_))))
                | (Some(Raw(Bool(_))), Some(Raw(BigInt(_))))
                | (Some(Raw(_)), Some(Undefined))
                | (Some(Undefined), Some(Raw(_))) => {
                    trace!("StrictEq (L): cross-type {} = false", op_str);
                    Some(false)
                }
                _ => None,
            };

            if let Some(is_eq) = eq {
                let result = if op_str == "===" { is_eq } else { !is_eq };
                trace!("StrictEq (L): result = {}", result);
                node.reduce(Raw(Bool(result)));
            }
        }

        Ok(())
    }
}

/// Infers `==` (loose equality) and `!=` (loose inequality).
/// Applies JavaScript's Abstract Equality Comparison (type coercion).
///
/// Rules applied:
/// - `Bool` -> `Num` (true=1, false=0)
/// - `Str` -> `Num` (via trim + parse; unparseable -> NaN -> false)
/// - `Array` -> `Str` (via flatten_array)
/// - `NaN` == anything -> false
/// - `undefined` == `undefined` -> true
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::integer::ParseInt;
/// use minusone::js::comparator::LooseEq;
/// use minusone::js::string::ParseString;
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = 1 == \"1\";").unwrap();
/// tree.apply_mut(&mut (ParseInt::default(), ParseString::default(), LooseEq::default())).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "var x = true;");
/// ```
#[derive(Default)]
pub struct LooseEq;

impl<'a> RuleMut<'a> for LooseEq {
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
        if view.kind() != "binary_expression" {
            return Ok(());
        }

        if let (Some(left), Some(op), Some(right)) = (view.child(0), view.child(1), view.child(2)) {
            let op_str = op.text()?;
            if op_str != "==" && op_str != "!=" {
                return Ok(());
            }

            let eq = match (left.data(), right.data()) {
                (Some(l), Some(r)) => loose_eq(l, r),
                _ => None,
            };

            if let Some(is_eq) = eq {
                let result = if op_str == "==" { is_eq } else { !is_eq };
                trace!("LooseEq (L): result = {}", result);
                node.reduce(Raw(Bool(result)));
            }
        }

        Ok(())
    }
}

fn loose_eq(left: &JavaScript, right: &JavaScript) -> Option<bool> {
    match (left, right) {
        // same-type fast path
        (Raw(Num(l)), Raw(Num(r))) => Some(l == r),
        (Raw(Str(l)), Raw(Str(r))) => Some(l == r),
        (Raw(Bool(l)), Raw(Bool(r))) => Some(l == r),
        (Raw(BigInt(l)), Raw(BigInt(r))) => Some(l == r),

        (Undefined, Undefined) => Some(true),
        (Null, Null) => Some(true),
        (Null, Undefined) => Some(true),
        (Undefined, Null) => Some(true),

        // NaN
        (NaN, _) | (_, NaN) => Some(false),

        // bool -> number, then retry
        (Raw(Bool(b)), other) => {
            let n = if *b { 1.0 } else { 0.0 };
            loose_eq(&Raw(Num(n)), other)
        }
        (other, Raw(Bool(b))) => {
            let n = if *b { 1.0 } else { 0.0 };
            loose_eq(other, &Raw(Num(n)))
        }

        // string x number
        (Raw(Str(s)), Raw(Num(n))) => Some(js_str_to_num(s) == Some(*n)),
        (Raw(Num(n)), Raw(Str(s))) => Some(js_str_to_num(s) == Some(*n)),

        // string x bigint
        (Raw(Str(s)), Raw(BigInt(b))) => parse_js_bigint(s).map(|x| x == *b).or(Some(false)),
        (Raw(BigInt(b)), Raw(Str(s))) => parse_js_bigint(s).map(|x| x == *b).or(Some(false)),

        // number x bigint
        (Raw(Num(n)), Raw(BigInt(b))) => Some(number_bigint_eq(*n, b)),
        (Raw(BigInt(b)), Raw(Num(n))) => Some(number_bigint_eq(*n, b)),

        // array/object -> primitive, then retry
        (Array(arr), other) => {
            let flat = flatten_array(arr, None);
            loose_eq(&Raw(Str(flat)), other)
        }
        (other, Array(arr)) => {
            let flat = flatten_array(arr, None);
            loose_eq(other, &Raw(Str(flat)))
        }

        // undefined x primitive
        (Undefined, Raw(_)) | (Raw(_), Undefined) => Some(false),

        _ => None,
    }
}

fn number_bigint_eq(n: f64, b: &num_bigint::BigInt) -> bool {
    if !n.is_finite() {
        return false;
    }
    if n.fract() != 0.0 {
        return false;
    }
    match f64_integer_to_bigint(n) {
        Some(n_big) => &n_big == b,
        None => false,
    }
}

fn f64_integer_to_bigint(n: f64) -> Option<num_bigint::BigInt> {
    if !n.is_finite() || n.fract() != 0.0 {
        return None;
    }
    num_bigint::BigInt::from_f64(n)
}

fn parse_js_bigint(s: &str) -> Option<num_bigint::BigInt> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    num_bigint::BigInt::parse_bytes(s.as_bytes(), 10)
}

/// Infers comp operators (`<`, `>`, `<=`, `>=`) for all known types.
///
/// Rules applied:
/// - `Bool` -> `Num`
/// - `Str`  -> `Num` when compared with a numeric operand (unparseable -> false)
/// - `Str` x `Str` -> lexicographic (UTF-16 code-unit order)
/// - `NaN` op anything -> false
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::integer::ParseInt;
/// use minusone::js::comparator::CmpOrd;
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = 3 < 5;").unwrap();
/// tree.apply_mut(&mut (ParseInt::default(), CmpOrd::default())).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "var x = true;");
/// ```
#[derive(Default)]
pub struct CmpOrd;

impl<'a> RuleMut<'a> for CmpOrd {
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
        if view.kind() != "binary_expression" {
            return Ok(());
        }

        if let (Some(left), Some(op), Some(right)) = (view.child(0), view.child(1), view.child(2)) {
            let op_str = op.text()?;
            if op_str != "<" && op_str != ">" && op_str != "<=" && op_str != ">=" {
                return Ok(());
            }

            let result = match (left.data(), right.data()) {
                (Some(l), Some(r)) => loose_ord(l, r, op_str),
                _ => None,
            };

            if let Some(res) = result {
                trace!(
                    "CmpOrd (L): {} {} {} -> {}",
                    left.text()?,
                    op_str,
                    right.text()?,
                    res
                );
                node.reduce(Raw(Bool(res)));
            } else {
                warn!(
                    "CmpOrd (L): unable to compare {} {} {}",
                    left.text()?,
                    op_str,
                    right.text()?
                );
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
enum OrdPrim {
    Num(f64),
    Str(String),
    BigInt(num_bigint::BigInt),
}

#[derive(Clone, Debug)]
enum Numeric {
    Num(f64),
    BigInt(num_bigint::BigInt),
}

fn to_ord_prim(value: &JavaScript) -> Option<OrdPrim> {
    match value {
        Raw(Num(n)) => Some(OrdPrim::Num(*n)),
        Raw(Str(s)) => Some(OrdPrim::Str(s.clone())),
        Raw(Bool(b)) => Some(OrdPrim::Num(if *b { 1.0 } else { 0.0 })),
        Raw(BigInt(b)) => Some(OrdPrim::BigInt(b.clone())),
        Array(arr) => Some(OrdPrim::Str(flatten_array(arr, None))),
        Undefined => Some(OrdPrim::Num(f64::NAN)),
        NaN => Some(OrdPrim::Num(f64::NAN)),
        _ => None,
    }
}

fn to_numeric(v: OrdPrim) -> Numeric {
    match v {
        OrdPrim::Num(n) => Numeric::Num(n),
        OrdPrim::BigInt(b) => Numeric::BigInt(b),
        OrdPrim::Str(s) => Numeric::Num(js_str_to_num(&s).unwrap_or(f64::NAN)),
    }
}

fn cmp_num_bigint(n: f64, b: &num_bigint::BigInt) -> Option<Ordering> {
    if n.is_nan() {
        return None;
    }

    if n == f64::INFINITY {
        return Some(Greater);
    }

    if n == f64::NEG_INFINITY {
        return Some(Less);
    }

    let frac = n.fract();

    if frac == 0.0 {
        let nb = num_bigint::BigInt::from_f64(n)?;
        return Some(nb.cmp(b));
    }

    let trunc = n.trunc();
    let tb = num_bigint::BigInt::from_f64(trunc)?;

    match tb.cmp(b) {
        Equal if frac > 0.0 => Some(Greater),
        Equal if frac < 0.0 => Some(Less),
        ord => Some(ord),
    }
}

fn cmp_numeric(op: &str, l: Numeric, r: Numeric) -> bool {
    let ord = match (&l, &r) {
        (Numeric::Num(ln), Numeric::Num(rn)) => match ln.partial_cmp(rn) {
            Some(ord) => ord,
            None => return false,
        },

        (Numeric::BigInt(lb), Numeric::BigInt(rb)) => lb.cmp(rb),

        (Numeric::Num(ln), Numeric::BigInt(rb)) => match cmp_num_bigint(*ln, rb) {
            Some(ord) => ord,
            None => return false,
        },

        (Numeric::BigInt(lb), Numeric::Num(rn)) => match cmp_num_bigint(*rn, lb) {
            Some(Less) => Greater,
            Some(Greater) => Less,
            Some(Equal) => Equal,
            None => return false,
        },
    };

    match op {
        "<" => ord == Less,
        ">" => ord == Greater,
        "<=" => ord != Greater,
        ">=" => ord != Less,
        _ => unreachable!(),
    }
}

fn loose_ord(left: &JavaScript, right: &JavaScript, op: &str) -> Option<bool> {
    let l = to_ord_prim(left)?;
    let r = to_ord_prim(right)?;

    if let (OrdPrim::Str(ls), OrdPrim::Str(rs)) = (&l, &r) {
        let res = match op {
            "<" => ls < rs,
            ">" => ls > rs,
            "<=" => ls <= rs,
            ">=" => ls >= rs,
            _ => unreachable!(),
        };
        trace!("CmpOrd (L): {:?} {} {:?} = {}", ls, op, rs, res);
        return Some(res);
    }

    let ln = to_numeric(l);
    let rn = to_numeric(r);
    let res = cmp_numeric(op, ln.clone(), rn.clone());

    trace!("CmpOrd (L): {:?} {} {:?} = {}", ln, op, rn, res);
    Some(res)
}

fn js_str_to_num(s: &str) -> Option<f64> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        Some(0.0)
    } else {
        trimmed.parse::<f64>().ok()
    }
}
