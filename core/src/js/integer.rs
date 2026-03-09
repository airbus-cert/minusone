use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::Raw;
use crate::js::Value::Num;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use js::array::flatten_array;
use js::JavaScript::Array;
use js::Value::Str;
use log::{debug, trace, warn};

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
        if view.kind() != "number" {
            return Ok(());
        }
        let token = view.text()?;

        if token.len() > 2 && (token.starts_with("0x") || token.starts_with("0X")) {
            if let Ok(n) = u64::from_str_radix(&token[2..], 16) {
                trace!("ParseInt (L): hex {} => {}", token, n);
                node.reduce(Raw(Num(n as i64)));
            }
        } else if token.len() > 2 && (token.starts_with("0o") || token.starts_with("0O")) {
            if let Ok(n) = u64::from_str_radix(&token[2..], 8) {
                trace!("ParseInt (L): octal {} => {}", token, n);
                node.reduce(Raw(Num(n as i64)));
            }
        } else if token.len() > 2 && (token.starts_with("0b") || token.starts_with("0B")) {
            if let Ok(n) = u64::from_str_radix(&token[2..], 2) {
                trace!("ParseInt (L): binary {} => {}", token, n);
                node.reduce(Raw(Num(n as i64)));
            }
        } else if let Ok(n) = token.parse::<i64>() {
            trace!("ParseInt (L): decimal {} => {}", token, n);
            node.reduce(Raw(Num(n)));
        }

        Ok(())
    }
}

/// Infers unary `-` expressions applied to a known integer, e.g. `-2` becomes `Raw(Num(-2))`.
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
                    if let Some(result) = n.checked_neg() {
                        trace!("NegInt (L): -{} = {}", n, result);
                        node.reduce(Raw(Num(result)));
                    } else {
                        warn!("NegInt (L): overflow -{}", n);
                    }
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
/// use minusone::js::integer::{ParseInt, AddInt};
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = 4 + 5 - 2;").unwrap();
/// tree.apply_mut(&mut (ParseInt::default(), AddInt::default())).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "var x = 7;");
/// ```
#[derive(Default)]
pub struct AddInt;

impl<'a> RuleMut<'a> for AddInt {
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
                    if let Some(result) = l.checked_add(*r) {
                        trace!("AddInt (L): {} + {} = {}", l, r, result);
                        node.reduce(Raw(Num(result)));
                    } else {
                        warn!("AddInt (L): overflow {} + {}", l, r);
                    }
                }
                (Some(Raw(Num(l))), "-", Some(Raw(Num(r)))) => {
                    if let Some(result) = l.checked_sub(*r) {
                        trace!("AddInt (L): {} - {} = {}", l, r, result);
                        node.reduce(Raw(Num(result)));
                    } else {
                        warn!("AddInt (L): overflow {} - {}", l, r);
                    }
                }
                (Some(Array(l)), "+", Some(Raw(Num(r)))) => {
                    let r = r.to_string();
                    let l = flatten_array(l);
                    let result = l.clone() + &r;
                    trace!("AddInt (L): {} + {} = {}", l, r, result);
                    node.reduce(Raw(Str(result)));
                }
                (Some(Raw(Num(l))), "+", Some(Array(r))) => {
                    let l = l.to_string();
                    let r = flatten_array(r);
                    let result = l.clone() + &r.clone();
                    trace!("AddInt (L): {} + {} = {}", l, r, result);
                    node.reduce(Raw(Str(result)));
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
                    if let Some(result) = l.checked_mul(*r) {
                        trace!("MultInt (L): {} * {} = {}", l, r, result);
                        node.reduce(Raw(Num(result)));
                    } else {
                        warn!("MultInt (L): overflow {} * {}", l, r);
                    }
                }
                (Some(Raw(Num(l))), "/", Some(Raw(Num(r)))) => {
                    if let Some(result) = l.checked_div(*r) {
                        trace!("MultInt (L): {} / {} = {}", l, r, result);
                        node.reduce(Raw(Num(result)));
                    } else {
                        warn!("MultInt (L): division by zero {} / {}", l, r);
                    }
                }
                (Some(Raw(Num(l))), "%", Some(Raw(Num(r)))) => {
                    if *r != 0 {
                        trace!("MultInt (L): {} % {} = {}", l, r, l % r);
                        node.reduce(Raw(Num(l % r)));
                    } else {
                        warn!("MultInt (L): modulo by zero {} % {}", l, r);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// Infer ** operations on integers if both operands are known integers (exponentiation operator)
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
                    if let Some(result) = l.checked_pow(*r as u32) {
                        trace!("PowInt (L): {} ** {} = {}", l, r, result);
                        node.reduce(Raw(Num(result)));
                    } else {
                        warn!("PowInt (L): overflow {} ** {}", l, r);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// infer << >> and >>> operations on integers if both operands are known integers (bitwise shift operators)
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
//// assert_eq!(linter.output, "var x = 8;");
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
                (Some(Raw(Num(l))), ">>", Some(Raw(Num(r)))) => {
                    let shift = (*r as i32 as u32) % 32;
                    let result = (*l as i32).wrapping_shr(shift) as i64;
                    trace!("ShiftInt (L): {} >> {} = {}", l, r, result);
                    node.reduce(Raw(Num(result)));
                }
                (Some(Raw(Num(l))), "<<", Some(Raw(Num(r)))) => {
                    let shift = (*r as i32 as u32) % 32;
                    let result = (*l as i32).wrapping_shl(shift) as i64;
                    trace!("ShiftInt (L): {} << {} = {}", l, r, result);
                    node.reduce(Raw(Num(result)));
                }
                (Some(Raw(Num(l))), ">>>", Some(Raw(Num(r)))) => {
                    let shift = (*r as i32 as u32) % 32;
                    let result = (*l as u32).wrapping_shr(shift) as i64;
                    trace!("ShiftInt (L): {} >>> {} = {}", l, r, result);
                    node.reduce(Raw(Num(result)));
                }
                _ => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::js::build_javascript_tree;
    use crate::js::linter::Linter;
    use crate::tree::{HashMapStorage, Tree};

    fn lint(tree: &Tree<HashMapStorage<JavaScript>>) -> String {
        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_parse_decimal() {
        let mut tree = build_javascript_tree("var x = 42;").unwrap();
        tree.apply_mut(&mut ParseInt::default()).unwrap();
        assert_eq!(lint(&tree), "var x = 42;");
    }

    #[test]
    fn test_parse_hex() {
        let mut tree = build_javascript_tree("var x = 0x1F;").unwrap();
        tree.apply_mut(&mut ParseInt::default()).unwrap();
        assert_eq!(lint(&tree), "var x = 31;");
    }

    #[test]
    fn test_add() {
        let mut tree = build_javascript_tree("var x = 4 + 5 - 2;").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), AddInt::default()))
            .unwrap();
        assert_eq!(lint(&tree), "var x = 7;");
    }

    #[test]
    fn test_neg() {
        let mut tree = build_javascript_tree("var x = -5;").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), NegInt::default()))
            .unwrap();
        assert_eq!(lint(&tree), "var x = -5;");
    }

    #[test]
    fn test_mult_negative() {
        let mut tree = build_javascript_tree("var x = 1 * -2;").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), NegInt::default(), MultInt::default()))
            .unwrap();
        assert_eq!(lint(&tree), "var x = -2;");
    }

    #[test]
    fn test_mult() {
        let mut tree = build_javascript_tree("var x = 3 * 4;").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), MultInt::default()))
            .unwrap();
        assert_eq!(lint(&tree), "var x = 12;");
    }

    #[test]
    fn test_pow() {
        let mut tree = build_javascript_tree("var x = 2 ** 3;").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), PowInt::default()))
            .unwrap();
        assert_eq!(lint(&tree), "var x = 8;");
    }

    #[test]
    fn test_shift_right() {
        let mut tree = build_javascript_tree("var x = 16 >> 2;").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), ShiftInt::default()))
            .unwrap();
        assert_eq!(lint(&tree), "var x = 4;");

        let mut tree = build_javascript_tree("var x = 2 >> 31;").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), ShiftInt::default()))
            .unwrap();
        assert_eq!(lint(&tree), "var x = 0;");

        let mut tree = build_javascript_tree("var x = 2 >> 32;").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), ShiftInt::default()))
            .unwrap();
        assert_eq!(lint(&tree), "var x = 2;");

        let mut tree = build_javascript_tree("var x = 2 >> 33;").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), ShiftInt::default()))
            .unwrap();
        assert_eq!(lint(&tree), "var x = 1;");

        let mut tree = build_javascript_tree("let x = -16 >> 2;").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), NegInt::default(), ShiftInt::default()))
            .unwrap();
        assert_eq!(lint(&tree), "let x = -4;");
    }

    #[test]
    fn test_shift_left() {
        let mut tree = build_javascript_tree("var x = 1 << 3;").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), ShiftInt::default()))
            .unwrap();
        assert_eq!(lint(&tree), "var x = 8;");

        let mut tree = build_javascript_tree("var x = 2 << 31;").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), ShiftInt::default()))
            .unwrap();
        assert_eq!(lint(&tree), "var x = 0;");

        let mut tree = build_javascript_tree("var x = 2 << 32;").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), ShiftInt::default()))
            .unwrap();
        assert_eq!(lint(&tree), "var x = 2;");
    }

    #[test]
    fn test_shift_unsigned_right() {
        let mut tree = build_javascript_tree("let x = -16 >>> 2;").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), NegInt::default(), ShiftInt::default()))
            .unwrap();
        assert_eq!(lint(&tree), "let x = 1073741820;");
    }

    #[test]
    fn test_combined() {
        let mut tree = build_javascript_tree("var x = 0x0A + 3 * 2;").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), AddInt::default(), MultInt::default()))
            .unwrap();
        assert_eq!(lint(&tree), "var x = 16;");
    }
}

// Todo: Bitwise operators (&, |, ^, ~)
