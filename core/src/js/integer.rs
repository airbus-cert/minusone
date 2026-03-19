use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::{NaN, Raw};
use crate::js::Value::{BigInt, Num};
use std::ops::{Shl, Shr};

use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use log::{error, trace, warn};
use num::ToPrimitive;

/// Parses JavaScript numeric literals (decimal, hex, octal, binary) into `Raw(Num(_))`.
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::integer::ParseInt;
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = 0x1F;").unwrap();
/// tree.apply_mut(&mut ParseInt::default()).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "var x = 31;");
/// ```
#[derive(Default)]
pub struct ParseInt;

impl<'a> RuleMut<'a> for ParseInt {
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
        if view.kind() != "number" && view.kind() != "identifier" {
            return Ok(());
        }
        let token = view.text()?;

        match view.kind() {
            "number" => {
                node.reduce(Self::from_str(token));
            }
            "identifier" => {
                if view.text()? == "Infinity" {
                    trace!("ParseInt (L): Infinity");
                    node.reduce(Raw(Num(f64::INFINITY)));
                }
            }
            _ => {}
        }

        Ok(())
    }
}

impl ParseInt {
    pub fn from_str(input: &str) -> JavaScript {
        let negate = input.starts_with("-");
        let bigint = input.ends_with("n");
        let input = if negate { &input[1..] } else { input };
        let input = if bigint {
            trace!("ParseInt (L): found BigInt literal {}", input);
            &input[..input.len() - 1]
        } else {
            input
        };
        let input = if !input.starts_with("_") && !input.ends_with("_") {
            input.replace("_", "")
        } else {
            input.to_string()
        };

        if bigint {
            return Self::bigint_from_str(&input, negate);
        }

        if input.len() > 2 && (input.starts_with("0x") || input.starts_with("0X")) {
            if let Ok(n) = u64::from_str_radix(&input[2..], 16) {
                trace!("ParseInt (L): hex {} => {}", input, n);
                return if negate { NaN } else { Raw(Num(n as f64)) };
            }
        } else if input.len() > 2 && (input.starts_with("0o") || input.starts_with("0O")) {
            if let Ok(n) = u64::from_str_radix(&input[2..], 8) {
                trace!("ParseInt (L): octal {} => {}", input, n);
                return if negate { NaN } else { Raw(Num(n as f64)) };
            }
        } else if input.len() > 2 && (input.starts_with("0b") || input.starts_with("0B")) {
            if let Ok(n) = u64::from_str_radix(&input[2..], 2) {
                trace!("ParseInt (L): binary {} => {}", input, n);
                return if negate { NaN } else { Raw(Num(n as f64)) };
            }
        } else {
            if input.starts_with("0") {
                if let Ok(n) = u64::from_str_radix(&input[1..], 8) {
                    trace!("ParseInt (L): octal {} => {}", input, n);
                    return if negate {
                        Raw(Num(-(n as f64)))
                    } else {
                        Raw(Num(n as f64))
                    };
                }
            }

            // JS fallback to decimal parsing on fail
            if let Ok(n) = input.parse::<f64>() {
                trace!("ParseInt (L): decimal {} => {}", input, n);
                return Raw(Num(if negate { -n } else { n }));
            }
        }
        warn!(
            "ParseInt (L): Unable to parse {}{}, falling back to NaN",
            if negate { "" } else { "-" },
            input
        );
        NaN
    }

    pub fn bigint_from_str(input: &str, negate: bool) -> JavaScript {
        if input.len() > 2 && (input.starts_with("0x") || input.starts_with("0X")) {
            if let Some(n) = num::BigInt::parse_bytes(input[2..].as_bytes(), 16) {
                trace!("ParseInt (L): hex BigInt {} => {}", input, n);
                return Raw(BigInt(if negate { -n } else { n }));
            }
        } else if input.len() > 2 && (input.starts_with("0o") || input.starts_with("0O")) {
            if let Some(n) = num::BigInt::parse_bytes(input[2..].as_bytes(), 8) {
                trace!("ParseInt (L): octal BigInt {} => {}", input, n);
                return Raw(BigInt(if negate { -n } else { n }));
            }
        } else if input.len() > 2 && (input.starts_with("0b") || input.starts_with("0B")) {
            if let Some(n) = num::BigInt::parse_bytes(input[2..].as_bytes(), 2) {
                trace!("ParseInt (L): binary BigInt {} => {}", input, n);
                return Raw(BigInt(if negate { -n } else { n }));
            }
        } else if input.len() >= 2 && input.starts_with("0") {
            error!(
                "ParseInt (L): BigInt literals cannot start with 0, this will crash the JS engine but found {}n",
                input
            );
        } else {
            if let Some(n) = num::BigInt::parse_bytes(input.as_bytes(), 10) {
                trace!("ParseInt (L): decimal BigInt {} => {}", input, n);
                return Raw(BigInt(if negate { -n } else { n }));
            }
        }
        warn!(
            "ParseInt (L): Unable to parse BigInt {}, falling back to NaN",
            input
        );
        NaN
    }
}

/// Infers unary `-` and `+` expressions applied to a known integer, e.g. `-2` becomes `Raw(Num(-2))`.
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::integer::{ParseInt, NegInt};
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = -5;").unwrap();
/// tree.apply_mut(&mut (ParseInt::default(), NegInt::default())).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "var x = -5;");
/// ```
#[derive(Default)]
pub struct NegInt;

impl<'a> RuleMut<'a> for NegInt {
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
        if view.kind() != "unary_expression" {
            return Ok(());
        }
        if let (Some(op), Some(operand)) = (view.child(0), view.child(1)) {
            if op.text()? == "-" {
                if let Some(Raw(Num(n))) = operand.data() {
                    if *n == f64::INFINITY {
                        trace!("NegInt (L): -Infinity = -Infinity");
                        node.reduce(Raw(Num(f64::NEG_INFINITY)));
                        return Ok(());
                    }
                    if *n == f64::NEG_INFINITY {
                        trace!("NegInt (L): -Infinity = -Infinity");
                        node.reduce(Raw(Num(f64::INFINITY)));
                        return Ok(());
                    }

                    let result = -n;
                    trace!("NegInt (L): -{} = {}", n, result);
                    node.reduce(Raw(Num(result)));
                } else if let Some(Raw(BigInt(n))) = operand.data() {
                    let result = -n;
                    trace!("NegInt (L): -{}n = {}n", n, result);
                    node.reduce(Raw(BigInt(result)));
                }
            } else if op.text()? == "+" {
                if let Some(Raw(Num(n))) = operand.data() {
                    trace!("NegInt (L): +{} = {}", n, n);
                    node.reduce(Raw(Num(*n)));
                } else if let Some(Raw(BigInt(n))) = operand.data() {
                    error!(
                        "NegInt (L): unary + on BigInt is not allowed in JS, but found +{}n. This should crash the JS engine",
                        n
                    );
                }
            }
        }
        Ok(())
    }
}

/// Infers `+` and `-` binary expressions when both operands are known integers.
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::integer::{ParseInt, SubAddInt};
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = 1 + 1;").unwrap();
/// tree.apply_mut(&mut (ParseInt::default(), SubAddInt::default())).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "var x = 2;");
/// ```
#[derive(Default)]
pub struct SubAddInt;

impl<'a> RuleMut<'a> for SubAddInt {
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
            match (left.data(), op.text()?, right.data()) {
                (Some(Raw(Num(l))), "+", Some(Raw(Num(r)))) => {
                    let result = l + r;
                    trace!("AddInt (L): {} + {} = {}", l, r, result);
                    node.reduce(Raw(Num(result)));
                }
                (Some(Raw(BigInt(l))), "+", Some(r)) => {
                    if let Raw(BigInt(r)) = r {
                        let result = l + r;
                        trace!("AddInt (L): {}n + {}n = {}n", l, r, result);
                        node.reduce(Raw(BigInt(result)));
                    } else if !r.is_string() {
                        error!(
                            "AddInt (L): tried to add BigInt and non-BigInt: {}n + {}. This should crash the Js engine",
                            l, r
                        );
                    }
                }
                (Some(l), "+", Some(Raw(BigInt(r)))) => {
                    if let Raw(BigInt(l)) = l {
                        let result = l + r;
                        trace!("AddInt (L): {}n + {}n = {}n", l, r, result);
                        node.reduce(Raw(BigInt(result)));
                    } else if !l.is_string() {
                        error!(
                            "AddInt (L): tried to add non-BigInt and BigInt: {} + {}n. This should crash the Js engine",
                            l, r
                        );
                    }
                }
                (Some(Raw(Num(l))), "-", Some(Raw(Num(r)))) => {
                    let result = l - r;
                    trace!("AddInt (L): {} - {} = {}", l, r, result);
                    node.reduce(Raw(Num(result)));
                }
                (Some(Raw(BigInt(l))), "-", Some(r)) => {
                    if let Raw(BigInt(r)) = r {
                        let result = l - r;
                        trace!("AddInt (L): {}n - {}n = {}n", l, r, result);
                        node.reduce(Raw(BigInt(result)));
                    } else {
                        error!(
                            "AddInt (L): tried to subtract BigInt and non-BigInt: {}n - {}. This should crash the Js engine",
                            l, r
                        );
                    }
                }
                (Some(l), "-", Some(Raw(BigInt(r)))) => {
                    if let Raw(BigInt(l)) = l {
                        let result = l - r;
                        trace!("AddInt (L): {}n - {}n = {}n", l, r, result);
                        node.reduce(Raw(BigInt(result)));
                    } else {
                        error!(
                            "AddInt (L): tried to subtract non-BigInt and BigInt: {} - {}n. This should crash the Js engine",
                            l, r
                        );
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// Infers `*`, `/`, and `%` binary expressions when both operands are known integers.
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::integer::{ParseInt, MultInt};
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = 3 * 4;").unwrap();
/// tree.apply_mut(&mut (ParseInt::default(), MultInt::default())).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "var x = 12;");
/// ```
#[derive(Default)]
pub struct MultInt;

impl<'a> RuleMut<'a> for MultInt {
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
            match (left.data(), op.text()?, right.data()) {
                (Some(Raw(Num(l))), "*", Some(Raw(Num(r)))) => {
                    let result = l * r;
                    trace!("MultInt (L): {} * {} = {}", l, r, result);
                    node.reduce(Raw(Num(result)));
                }
                (Some(Raw(BigInt(l))), "*", Some(r)) => {
                    if let Raw(BigInt(r)) = r {
                        let result = l * r;
                        trace!("MultInt (L): {}n * {}n = {}n", l, r, result);
                        node.reduce(Raw(BigInt(result)));
                    } else {
                        error!(
                            "MultInt (L): tried to multiply BigInt and non-BigInt: {}n * {}. This should crash the Js engine",
                            l, r
                        );
                    }
                }
                (Some(l), "*", Some(Raw(BigInt(r)))) => {
                    if let Raw(BigInt(l)) = l {
                        let result = l * r;
                        trace!("MultInt (L): {}n * {}n = {}n", l, r, result);
                        node.reduce(Raw(BigInt(result)));
                    } else {
                        error!(
                            "MultInt (L): tried to multiply non-BigInt and BigInt: {} * {}n. This should crash the Js engine",
                            l, r
                        );
                    }
                }
                (Some(Raw(Num(l))), "/", Some(Raw(Num(r)))) => {
                    let result = l / r;
                    trace!("MultInt (L): {} / {} = {}", l, r, result);
                    node.reduce(Raw(Num(result)));
                }
                (Some(Raw(BigInt(l))), "/", Some(r)) => {
                    if let Raw(BigInt(r)) = r {
                        let result = l / r;
                        trace!("MultInt (L): {}n / {}n = {}n", l, r, result);
                        node.reduce(Raw(BigInt(result)));
                    } else {
                        error!(
                            "MultInt (L): tried to divide BigInt and non-BigInt: {}n / {}. This should crash the Js engine",
                            l, r
                        );
                    }
                }
                (Some(l), "/", Some(Raw(BigInt(r)))) => {
                    if let Raw(BigInt(l)) = l {
                        let result = l / r;
                        trace!("MultInt (L): {}n / {}n = {}n", l, r, result);
                        node.reduce(Raw(BigInt(result)));
                    } else {
                        error!(
                            "MultInt (L): tried to divide non-BigInt and BigInt: {} / {}n. This should crash the Js engine",
                            l, r
                        );
                    }
                }
                (Some(Raw(Num(l))), "%", Some(Raw(Num(r)))) => {
                    if *r != 0.0 {
                        trace!("MultInt (L): {} % {} = {}", l, r, l % r);
                        node.reduce(Raw(Num(l % r)));
                    } else {
                        warn!("MultInt (L): modulo by zero {} % {}", l, r);
                    }
                }
                (Some(Raw(BigInt(l))), "%", Some(r)) => {
                    if let Raw(BigInt(r)) = r {
                        let result = l % r;
                        trace!("MultInt (L): {}n % {}n = {}n", l, r, result);
                        node.reduce(Raw(BigInt(result)));
                    } else {
                        error!(
                            "MultInt (L): tried to apply mod on BigInt and non-BigInt: {}n % {}. This should crash the Js engine",
                            l, r
                        );
                    }
                }
                (Some(l), "%", Some(Raw(BigInt(r)))) => {
                    if let Raw(BigInt(l)) = l {
                        let result = l % r;
                        trace!("MultInt (L): {}n % {}n = {}n", l, r, result);
                        node.reduce(Raw(BigInt(result)));
                    } else {
                        error!(
                            "MultInt (L): tried to apply mod on non-BigInt and BigInt: {} % {}n. This should crash the Js engine",
                            l, r
                        );
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// Infer `**` operations on integers if both operands are known integers (exponentiation operator)
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::integer::{ParseInt, PowInt};
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = 2 ** 3;").unwrap();
/// tree.apply_mut(&mut (ParseInt::default(), PowInt::default())).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "var x = 8;");
/// ```
#[derive(Default)]
pub struct PowInt;

impl<'a> RuleMut<'a> for PowInt {
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
            match (left.data(), op.text()?, right.data()) {
                (Some(Raw(Num(l))), "**", Some(Raw(Num(r)))) => {
                    let result = l.powi(*r as i32);
                    trace!("PowInt (L): {} ** {} = {}", l, r, result);
                    node.reduce(Raw(Num(result)));
                }
                (Some(Raw(BigInt(l))), "**", Some(r)) => {
                    if let Raw(BigInt(r)) = r {
                        if let Some(exp) = r.to_u32() {
                            let result = l.pow(exp);
                            trace!("PowInt (L): {}n ** {}n = {}n", l, r, result);
                            node.reduce(Raw(BigInt(result)));
                        } else {
                            warn!("PowInt (L): exponent too large: {}n", r);
                        }
                    } else {
                        error!(
                            "PowInt (L): tried to pow BigInt and non-BigInt: {}n ** {}. This should crash the Js engine",
                            l, r
                        );
                    }
                }
                (Some(l), "**", Some(Raw(BigInt(r)))) => {
                    if let Raw(BigInt(l)) = l {
                        if let Some(exp) = r.to_u32() {
                            let result = l.pow(exp);
                            trace!("PowInt (L): {}n ** {}n = {}n", l, r, result);
                            node.reduce(Raw(BigInt(result)));
                        } else {
                            warn!("PowInt (L): exponent too large: {}n", r);
                        }
                    } else {
                        error!(
                            "PowInt (L): tried to pow non-BigInt and BigInt: {} ** {}n. This should crash the Js engine",
                            l, r
                        );
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// infer `<<` `>>` and `>>>` operations on integers if both operands are known integers (bitwise shift operators)
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::integer::{ParseInt, ShiftInt};
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = 1 << 3;").unwrap();
/// tree.apply_mut(&mut (ParseInt::default(), ShiftInt::default())).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
/// assert_eq!(linter.output, "var x = 8;");
/// ```
#[derive(Default)]
pub struct ShiftInt;

impl<'a> RuleMut<'a> for ShiftInt {
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
            match (left.data(), op.text()?, right.data()) {
                // JS always truncate to 32 bits to do shifts
                (Some(Raw(Num(l))), ">>", Some(Raw(Num(r)))) => {
                    let shift = (*r as i32 as u32) % 32;
                    let result = (*l as i32).wrapping_shr(shift);
                    trace!(
                        "ShiftInt (L): 0X{:x} >> 0x{:x} = 0X{:x}",
                        *l as i32, *r as i64, result
                    );
                    node.reduce(Raw(Num(result as f64)));
                }
                (Some(Raw(BigInt(l))), ">>", Some(r)) => {
                    if let Raw(BigInt(r)) = r {
                        if let Some(shift) = r.to_u32() {
                            let result = l.shr(shift);
                            trace!("ShiftInt (L): {}n >> {}n = {}n", l, r, result);
                            node.reduce(Raw(BigInt(result)));
                        } else {
                            warn!("ShiftInt (L): shift too large: {}n", r);
                        }
                    } else {
                        error!(
                            "ShiftInt (L): tried to shift right BigInt and non-BigInt: {}n >> {}. This should crash the Js engine",
                            l, r
                        );
                    }
                }
                (Some(l), ">>", Some(Raw(BigInt(r)))) => {
                    if let Raw(BigInt(l)) = l {
                        if let Some(shift) = r.to_u32() {
                            let result = l.shr(shift);
                            trace!("ShiftInt (L): {}n >> {}n = {}n", l, r, result);
                            node.reduce(Raw(BigInt(result)));
                        } else {
                            warn!("ShiftInt (L): shift too large: {}n", r);
                        }
                    } else {
                        error!(
                            "ShiftInt (L): tried to shift right BigInt and non-BigInt: {}n >> {}. This should crash the Js engine",
                            l, r
                        );
                    }
                }
                (Some(Raw(Num(l))), "<<", Some(Raw(Num(r)))) => {
                    let shift = (*r as i32 as u32) % 32;
                    let result = (*l as i32).wrapping_shl(shift);
                    trace!(
                        "ShiftInt (L): 0X{:x} << 0X{:x} = 0X{:x}",
                        *l as i32, *r as i64, result
                    );
                    node.reduce(Raw(Num(result as f64)));
                }
                (Some(Raw(BigInt(l))), "<<", Some(r)) => {
                    if let Raw(BigInt(r)) = r {
                        if let Some(shift) = r.to_u32() {
                            let result = l.shl(shift);
                            trace!("ShiftInt (L): {}n << {}n = {}n", l, r, result);
                            node.reduce(Raw(BigInt(result)));
                        } else {
                            warn!("ShiftInt (L): shift too large: {}n", r);
                        }
                    } else {
                        error!(
                            "ShiftInt (L): tried to shift left BigInt and non-BigInt: {}n << {}. This should crash the Js engine",
                            l, r
                        );
                    }
                }
                (Some(l), "<<", Some(Raw(BigInt(r)))) => {
                    if let Raw(BigInt(l)) = l {
                        if let Some(shift) = r.to_u32() {
                            let result = l.shl(shift);
                            trace!("ShiftInt (L): {}n << {}n = {}n", l, r, result);
                            node.reduce(Raw(BigInt(result)));
                        } else {
                            warn!("ShiftInt (L): shift too large: {}n", r);
                        }
                    } else {
                        error!(
                            "ShiftInt (L): tried to shift left BigInt and non-BigInt: {}n << {}. This should crash the Js engine",
                            l, r
                        );
                    }
                }
                (Some(Raw(Num(l))), ">>>", Some(Raw(Num(r)))) => {
                    // f64 -> u32 then u32 -> i32 is required to avoid saturating the cast
                    let shift = (*r as i32 as u32) % 32;
                    let result = (*l as i32 as u32).wrapping_shr(shift);
                    trace!(
                        "ShiftInt (L): 0X{:x} >>> 0X{:x} = 0X{:x}",
                        *l as i32, *r as i64, result
                    );
                    node.reduce(Raw(Num(result as f64)));
                }

                _ => {}
            }
        }
        Ok(())
    }
}

/// Infers bitwise operators `&`, `|`, `^`, and `~` operations on integers if both operands are known integers
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::integer::{ParseInt, BitwiseInt};
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = 0x4 ^ 0x8;").unwrap();
/// tree.apply_mut(&mut (ParseInt::default(), BitwiseInt::default())).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
/// assert_eq!(linter.output, "var x = 12;");
/// ```
#[derive(Default)]
pub struct BitwiseInt;

impl<'a> RuleMut<'a> for BitwiseInt {
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

        match view.kind() {
            "binary_expression" => {
                if let (Some(left), Some(op), Some(right)) =
                    (view.child(0), view.child(1), view.child(2))
                {
                    match (left.data(), op.text()?, right.data()) {
                        (Some(Raw(Num(l))), "&", Some(Raw(Num(r)))) => {
                            let l = *l as i64;
                            let r = *r as i64;
                            trace!("BitwiseInt (L): {} & {} = {}", l, r, l & r);
                            node.reduce(Raw(Num((l & r) as f64)));
                        }
                        (Some(Raw(BigInt(l))), "&", Some(r)) => {
                            if let Raw(BigInt(r)) = r {
                                let result = l & r;
                                trace!("BitwiseInt (L): {}n & {}n = {}n", l, r, result);
                                node.reduce(Raw(BigInt(result)));
                            } else {
                                error!(
                                    "BitwiseInt (L): tried to AND BigInt and non-BigInt: {}n & {}. This should crash the Js engine",
                                    l, r
                                );
                            }
                        }
                        (Some(l), "&", Some(Raw(BigInt(r)))) => {
                            if let Raw(BigInt(l)) = l {
                                let result = l & r;
                                trace!("BitwiseInt (L): {}n & {}n = {}n", l, r, result);
                                node.reduce(Raw(BigInt(result)));
                            } else {
                                error!(
                                    "BitwiseInt (L): tried to AND non-BigInt and BigInt: {} & {}n. This should crash the Js engine",
                                    l, r
                                );
                            }
                        }
                        (Some(Raw(Num(l))), "|", Some(Raw(Num(r)))) => {
                            let l = *l as i64;
                            let r = *r as i64;
                            trace!("BitwiseInt (L): {} | {} = {}", l, r, l | r);
                            node.reduce(Raw(Num((l | r) as f64)));
                        }
                        (Some(Raw(BigInt(l))), "|", Some(r)) => {
                            if let Raw(BigInt(r)) = r {
                                let result = l | r;
                                trace!("BitwiseInt (L): {}n | {}n = {}n", l, r, result);
                                node.reduce(Raw(BigInt(result)));
                            } else {
                                error!(
                                    "BitwiseInt (L): tried to OR BigInt and non-BigInt: {}n | {}. This should crash the Js engine",
                                    l, r
                                );
                            }
                        }
                        (Some(l), "|", Some(Raw(BigInt(r)))) => {
                            if let Raw(BigInt(l)) = l {
                                let result = l | r;
                                trace!("BitwiseInt (L): {}n | {}n = {}n", l, r, result);
                                node.reduce(Raw(BigInt(result)));
                            } else {
                                error!(
                                    "BitwiseInt (L): tried to OR non-BigInt and BigInt: {} | {}n. This should crash the Js engine",
                                    l, r
                                );
                            }
                        }
                        (Some(Raw(Num(l))), "^", Some(Raw(Num(r)))) => {
                            let l = *l as i64;
                            let r = *r as i64;
                            trace!("BitwiseInt (L): {} ^ {} = {}", l, r, l ^ r);
                            node.reduce(Raw(Num((l ^ r) as f64)));
                        }
                        (Some(Raw(BigInt(l))), "^", Some(r)) => {
                            if let Raw(BigInt(r)) = r {
                                let result = l ^ r;
                                trace!("BitwiseInt (L): {}n ^ {}n = {}n", l, r, result);
                                node.reduce(Raw(BigInt(result)));
                            } else {
                                error!(
                                    "BitwiseInt (L): tried to XOR BigInt and non-BigInt: {}n ^ {}. This should crash the Js engine",
                                    l, r
                                );
                            }
                        }
                        (Some(l), "^", Some(Raw(BigInt(r)))) => {
                            if let Raw(BigInt(l)) = l {
                                let result = l ^ r;
                                trace!("BitwiseInt (L): {}n ^ {}n = {}n", l, r, result);
                                node.reduce(Raw(BigInt(result)));
                            } else {
                                error!(
                                    "BitwiseInt (L): tried to XOR non-BigInt and BigInt: {} ^ {}n. This should crash the Js engine",
                                    l, r
                                );
                            }
                        }
                        _ => {}
                    }
                }
            }
            "unary_expression" => {
                if let (Some(op), Some(operand)) = (view.child(0), view.child(1)) {
                    if op.text()? == "~" {
                        if let Some(Raw(Num(n))) = operand.data() {
                            let n = *n as i64;
                            trace!("BitwiseInt (L): ~{} = {}", n, !n);
                            node.reduce(Raw(Num((!n) as f64)));
                        } else if let Some(Raw(BigInt(n))) = operand.data() {
                            let result = !n;
                            trace!("BitwiseInt (L): ~{}n = {}n", n, result);
                            node.reduce(Raw(BigInt(result)));
                        }
                    }
                }
            }
            _ => (),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests_js_integer {
    use super::*;
    use crate::js::build_javascript_tree;
    use crate::js::linter::Linter;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            NegInt::default(),
            SubAddInt::default(),
            MultInt::default(),
            PowInt::default(),
            ShiftInt::default(),
            BitwiseInt::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_parse_int() {
        assert_eq!(deobfuscate("var x = 31;"), "var x = 31;");
        assert_eq!(deobfuscate("var x = 0x1F;"), "var x = 31;");
        assert_eq!(deobfuscate("var x = 0o37;"), "var x = 31;");
        assert_eq!(deobfuscate("var x = 0b11111;"), "var x = 31;");
        assert_eq!(deobfuscate("var x = 017;"), "var x = 15;");
        assert_eq!(deobfuscate("var x = 0017;"), "var x = 15;");
        assert_eq!(deobfuscate("var x = 019;"), "var x = 19;");
    }

    #[test]
    fn test_parse_bigint() {
        assert_eq!(deobfuscate("var x = 31n;"), "var x = 31n;");
        assert_eq!(deobfuscate("var x = 0x1Fn;"), "var x = 31n;");
        assert_eq!(deobfuscate("var x = 0o37n;"), "var x = 31n;");
        assert_eq!(deobfuscate("var x = 0b11111n;"), "var x = 31n;");
    }

    #[test]
    fn test_pos_neg_int() {
        assert_eq!(deobfuscate("var x = +42 + +5;"), "var x = 47;");
        assert_eq!(deobfuscate("var x = -42 - -5;"), "var x = -37;");
    }

    #[test]
    fn test_add_sub_int() {
        assert_eq!(deobfuscate("var x = 1 + 1;"), "var x = 2;");
        assert_eq!(deobfuscate("var x = 5 - 2;"), "var x = 3;");
        assert_eq!(
            deobfuscate("var x = 1 - 25 + 47 - 6 - 2 -99 + 120 + 33;"),
            "var x = 69;"
        );
    }

    #[test]
    fn test_mult_div_mod_int() {
        assert_eq!(deobfuscate("var x = 3 * 4;"), "var x = 12;");
        assert_eq!(deobfuscate("var x = 10 / 2;"), "var x = 5;");
        assert_eq!(deobfuscate("var x = 10 % 3;"), "var x = 1;");
        assert_eq!(deobfuscate("var x = 10 * 2 / 5 % 2;"), "var x = 0;");
    }

    #[test]
    fn test_op_priority() {
        assert_eq!(deobfuscate("var x = 1 + 3 * 36;"), "var x = 109;");
        assert_eq!(deobfuscate("var x = 1 + 9 * 6 % 28 - 3 * 7;"), "var x = 6;");
    }

    #[test]
    fn test_pow_int() {
        assert_eq!(deobfuscate("var x = 50 ** 8;"), "var x = 39062500000000;");
    }

    #[test]
    fn test_shift_int() {
        assert_eq!(deobfuscate("var x = 1 << 3;"), "var x = 8;");
        assert_eq!(deobfuscate("var x = 16 >> 2;"), "var x = 4;");
        assert_eq!(deobfuscate("let x = -16 >>> 2;"), "let x = 1073741820;"); // test fails
        assert_eq!(deobfuscate("var x = 1 << 3 >> 2;"), "var x = 2;");
        assert_eq!(deobfuscate("var x = 2 >> 31;"), "var x = 0;");
        assert_eq!(deobfuscate("var x = 2 >> 32;"), "var x = 2;");
        assert_eq!(deobfuscate("var x = 2 >> 33;"), "var x = 1;");
        assert_eq!(deobfuscate("let x = -16 >> 2;"), "let x = -4;");
    }

    #[test]
    fn test_bitwise_int() {
        assert_eq!(deobfuscate("var x = 0x4 & 0x8;"), "var x = 0;");
        assert_eq!(deobfuscate("var x = 0x4 | 0x8;"), "var x = 12;");
        assert_eq!(deobfuscate("var x = 0x4 ^ 0x8;"), "var x = 12;");
        assert_eq!(deobfuscate("var x = ~0x4;"), "var x = -5;");
        assert_eq!(
            deobfuscate("var x = 0x15487596 ^ 0x5216598 | 0x36598745 & ~0x21215487;"),
            "var x = 377066318;",
        );
    }
}
