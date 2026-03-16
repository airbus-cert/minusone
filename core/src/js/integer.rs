use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::Array;
use crate::js::JavaScript::Raw;
use crate::js::Value::Num;
use crate::js::Value::Str;
use crate::js::array::flatten_array;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use log::{trace, warn};

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
                    if let Some(result) = n.checked_neg() {
                        trace!("NegInt (L): -{} = {}", n, result);
                        node.reduce(Raw(Num(result)));
                    } else {
                        warn!("NegInt (L): overflow -{}", n);
                    }
                }
            } else if op.text()? == "+" {
                if let Some(Raw(Num(n))) = operand.data() {
                    trace!("NegInt (L): +{} = {}", n, n);
                    node.reduce(Raw(Num(*n)));
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
/// let mut tree = build_javascript_tree("var x = 1 + 1;").unwrap();
/// tree.apply_mut(&mut (ParseInt::default(), AddInt::default())).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "var x = 2;");
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
                    let result = (*l as i32).wrapping_shr(shift) as i64;
                    trace!(
                        "ShiftInt (L): 0X{:x} >> 0x{:x} = 0X{:x}",
                        *l as i32, r, result
                    );
                    node.reduce(Raw(Num(result)));
                }
                (Some(Raw(Num(l))), "<<", Some(Raw(Num(r)))) => {
                    let shift = (*r as i32 as u32) % 32;
                    let result = (*l as i32).wrapping_shl(shift) as i64;
                    trace!("ShiftInt (L): 0X{:x} << 0X{:x} = 0X{:x}", l, r, result);
                    node.reduce(Raw(Num(result)));
                }
                (Some(Raw(Num(l))), ">>>", Some(Raw(Num(r)))) => {
                    let shift = (*r as i32 as u32) % 32;
                    let result = (*l as u32).wrapping_shr(shift) as i64;
                    trace!("ShiftInt (L): 0X{:x} >>> 0X{:x} = 0X{:x}", l, r, result);
                    node.reduce(Raw(Num(result)));
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
                            trace!("BitwiseInt (L): {} & {} = {}", l, r, l & r);
                            node.reduce(Raw(Num(l & r)));
                        }
                        (Some(Raw(Num(l))), "|", Some(Raw(Num(r)))) => {
                            trace!("BitwiseInt (L): {} | {} = {}", l, r, l | r);
                            node.reduce(Raw(Num(l | r)));
                        }
                        (Some(Raw(Num(l))), "^", Some(Raw(Num(r)))) => {
                            trace!("BitwiseInt (L): {} ^ {} = {}", l, r, l ^ r);
                            node.reduce(Raw(Num(l ^ r)));
                        }
                        _ => {}
                    }
                }
            }
            "unary_expression" => {
                if let (Some(op), Some(operand)) = (view.child(0), view.child(1)) {
                    if op.text()? == "~" {
                        if let Some(Raw(Num(n))) = operand.data() {
                            trace!("BitwiseInt (L): ~{} = {}", n, !n);
                            node.reduce(Raw(Num(!n)));
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
            AddInt::default(),
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
        assert_eq!(deobfuscate("let x = -16 >>> 2;"), "let x = 1073741820;");
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
