use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::*;
use crate::js::Value::*;
use crate::js::array::flatten_array;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use log::trace;

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
        use JavaScript::{NaN, Undefined};

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
                (Some(Raw(Str(l))), Some(Raw(Str(r)))) => {
                    trace!("StrictEq (L): {:?} {} {:?} = {}", l, op_str, r, l == r);
                    Some(l == r)
                }
                (Some(Raw(Bool(l))), Some(Raw(Bool(r)))) => {
                    trace!("StrictEq (L): {} {} {} = {}", l, op_str, r, l == r);
                    Some(l == r)
                }
                (Some(Undefined), Some(Undefined)) => {
                    trace!("StrictEq (L): undefined {} undefined = true", op_str);
                    Some(true)
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
        use JavaScript::{NaN, Undefined};

        let view = node.view();
        if view.kind() != "binary_expression" {
            return Ok(());
        }

        if let (Some(left), Some(op), Some(right)) = (view.child(0), view.child(1), view.child(2)) {
            let op_str = op.text()?;
            if op_str != "==" && op_str != "!=" {
                return Ok(());
            }

            let eq: Option<bool> = match (left.data(), right.data()) {
                (Some(Raw(Num(l))), Some(Raw(Num(r)))) => {
                    trace!("LooseEq (L): {} {} {} = {}", l, op_str, r, l == r);
                    Some(l == r)
                }
                (Some(Raw(Str(l))), Some(Raw(Str(r)))) => {
                    trace!("LooseEq (L): {:?} {} {:?} = {}", l, op_str, r, l == r);
                    Some(l == r)
                }
                (Some(Raw(Bool(l))), Some(Raw(Bool(r)))) => {
                    trace!("LooseEq (L): {} {} {} = {}", l, op_str, r, l == r);
                    Some(l == r)
                }
                (Some(Undefined), Some(Undefined)) => {
                    trace!("LooseEq (L): undefined {} undefined = true", op_str);
                    Some(true)
                }
                // NaN == anything -> false
                (Some(NaN), _) | (_, Some(NaN)) => {
                    trace!("LooseEq (L): NaN {} _ = false", op_str);
                    Some(false)
                }
                // undefined == non-null primitive -> false
                (Some(Undefined), Some(Raw(_))) | (Some(Raw(_)), Some(Undefined)) => {
                    trace!("LooseEq (L): undefined {} primitive = false", op_str);
                    Some(false)
                }

                // Bool == Num
                (Some(Raw(Bool(b))), Some(Raw(Num(n)))) => {
                    let bnum = *b as i64;
                    trace!(
                        "LooseEq (L): {} (->{}) {} {} = {}",
                        b,
                        bnum,
                        op_str,
                        n,
                        bnum == *n
                    );
                    Some(bnum == *n)
                }
                (Some(Raw(Num(n))), Some(Raw(Bool(b)))) => {
                    let bnum = *b as i64;
                    trace!(
                        "LooseEq (L): {} {} {} (->{}) = {}",
                        n,
                        op_str,
                        b,
                        bnum,
                        *n == bnum
                    );
                    Some(*n == bnum)
                }

                // Str == Num
                (Some(Raw(Str(s))), Some(Raw(Num(n)))) => match js_str_to_num(s) {
                    Some(snum) => {
                        trace!(
                            "LooseEq (L): {:?} (->{}) {} {} = {}",
                            s,
                            snum,
                            op_str,
                            n,
                            snum == *n
                        );
                        Some(snum == *n)
                    }
                    None => {
                        trace!("LooseEq (L): {:?} (->NaN) {} {} = false", s, op_str, n);
                        Some(false)
                    }
                },
                (Some(Raw(Num(n))), Some(Raw(Str(s)))) => match js_str_to_num(s) {
                    Some(snum) => {
                        trace!(
                            "LooseEq (L): {} {} {:?} (->{}) = {}",
                            n,
                            op_str,
                            s,
                            snum,
                            *n == snum
                        );
                        Some(*n == snum)
                    }
                    None => {
                        trace!("LooseEq (L): {} {} {:?} (->NaN) = false", n, op_str, s);
                        Some(false)
                    }
                },

                // bool == Str: bool -> num, str -> num
                (Some(Raw(Bool(b))), Some(Raw(Str(s)))) => {
                    let bnum = *b as i64;
                    match js_str_to_num(s) {
                        Some(snum) => {
                            trace!(
                                "LooseEq (L): {} (->{}) {} {:?} (->{}) = {}",
                                b,
                                bnum,
                                op_str,
                                s,
                                snum,
                                bnum == snum
                            );
                            Some(bnum == snum)
                        }
                        None => {
                            trace!(
                                "LooseEq (L): {} (->{}) {} {:?} (->NaN) = false",
                                b, bnum, op_str, s
                            );
                            Some(false)
                        }
                    }
                }
                (Some(Raw(Str(s))), Some(Raw(Bool(b)))) => {
                    let bnum = *b as i64;
                    match js_str_to_num(s) {
                        Some(snum) => {
                            trace!(
                                "LooseEq (L): {:?} (->{}) {} {} (->{}) = {}",
                                s,
                                snum,
                                op_str,
                                b,
                                bnum,
                                snum == bnum
                            );
                            Some(snum == bnum)
                        }
                        None => {
                            trace!(
                                "LooseEq (L): {:?} (->NaN) {} {} (->{}) = false",
                                s, op_str, b, bnum
                            );
                            Some(false)
                        }
                    }
                }

                // Array == Str: flatten array then compare
                (Some(Array(arr)), Some(Raw(Str(s)))) => {
                    let flat = flatten_array(arr);
                    trace!(
                        "LooseEq (L): [..](->{:?}) {} {:?} = {}",
                        flat,
                        op_str,
                        s,
                        flat == *s
                    );
                    Some(flat == *s)
                }
                (Some(Raw(Str(s))), Some(Array(arr))) => {
                    let flat = flatten_array(arr);
                    trace!(
                        "LooseEq (L): {:?} {} [..](->{:?}) = {}",
                        s,
                        op_str,
                        flat,
                        *s == flat
                    );
                    Some(*s == flat)
                }

                // Array == Num: flatten -> parse -> compare
                (Some(Array(arr)), Some(Raw(Num(n)))) => {
                    let flat = flatten_array(arr);
                    match js_str_to_num(&flat) {
                        Some(anum) => {
                            trace!(
                                "LooseEq (L): [..](->{}) {} {} = {}",
                                anum,
                                op_str,
                                n,
                                anum == *n
                            );
                            Some(anum == *n)
                        }
                        None => {
                            trace!("LooseEq (L): [..](->NaN) {} {} = false", op_str, n);
                            Some(false)
                        }
                    }
                }
                (Some(Raw(Num(n))), Some(Array(arr))) => {
                    let flat = flatten_array(arr);
                    match js_str_to_num(&flat) {
                        Some(anum) => {
                            trace!(
                                "LooseEq (L): {} {} [..](->{}) = {}",
                                n,
                                op_str,
                                anum,
                                *n == anum
                            );
                            Some(*n == anum)
                        }
                        None => {
                            trace!("LooseEq (L): {} {} [..](->NaN) = false", n, op_str);
                            Some(false)
                        }
                    }
                }

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

            // TODO: check if this system can be unified with other operations
            let cmp_num = |l: i64, r: i64| match op_str {
                "<" => l < r,
                ">" => l > r,
                "<=" => l <= r,
                ">=" => l >= r,
                _ => unreachable!(),
            };

            let cmp_str = |l: &str, r: &str| match op_str {
                "<" => l < r,
                ">" => l > r,
                "<=" => l <= r,
                ">=" => l >= r,
                _ => unreachable!(),
            };

            let result: Option<bool> = match (left.data(), right.data()) {
                // NaN comparisons are always false
                (Some(NaN), _) | (_, Some(NaN)) => {
                    trace!("CmpOrd (L): NaN {} _ = false", op_str);
                    Some(false)
                }

                // Num x Num
                (Some(Raw(Num(l))), Some(Raw(Num(r)))) => {
                    let res = cmp_num(*l, *r);
                    trace!("CmpOrd (L): {} {} {} = {}", l, op_str, r, res);
                    Some(res)
                }

                // Str x Str: lexicographic
                (Some(Raw(Str(l))), Some(Raw(Str(r)))) => {
                    let res = cmp_str(l, r);
                    trace!("CmpOrd (L): {:?} {} {:?} = {}", l, op_str, r, res);
                    Some(res)
                }

                // Bool x Bool: convert to number
                (Some(Raw(Bool(l))), Some(Raw(Bool(r)))) => {
                    let res = cmp_num(*l as i64, *r as i64);
                    trace!(
                        "CmpOrd (L): {} (->{}) {} {} (->{}) = {}",
                        l, *l as i64, op_str, r, *r as i64, res
                    );
                    Some(res)
                }

                // Bool x Num
                (Some(Raw(Bool(b))), Some(Raw(Num(n)))) => {
                    let bnum = *b as i64;
                    let res = cmp_num(bnum, *n);
                    trace!("CmpOrd (L): {} (->{}) {} {} = {}", b, bnum, op_str, n, res);
                    Some(res)
                }
                (Some(Raw(Num(n))), Some(Raw(Bool(b)))) => {
                    let bnum = *b as i64;
                    let res = cmp_num(*n, bnum);
                    trace!("CmpOrd (L): {} {} {} (->{}) = {}", n, op_str, b, bnum, res);
                    Some(res)
                }

                // Str x Num: parse string
                (Some(Raw(Str(s))), Some(Raw(Num(n)))) => match js_str_to_num(s) {
                    Some(snum) => {
                        let res = cmp_num(snum, *n);
                        trace!(
                            "CmpOrd (L): {:?} (->{}) {} {} = {}",
                            s, snum, op_str, n, res
                        );
                        Some(res)
                    }
                    None => {
                        trace!("CmpOrd (L): {:?} (->NaN) {} {} = false", s, op_str, n);
                        Some(false)
                    }
                },
                (Some(Raw(Num(n))), Some(Raw(Str(s)))) => match js_str_to_num(s) {
                    Some(snum) => {
                        let res = cmp_num(*n, snum);
                        trace!(
                            "CmpOrd (L): {} {} {:?} (->{}) = {}",
                            n, op_str, s, snum, res
                        );
                        Some(res)
                    }
                    None => {
                        trace!("CmpOrd (L): {} {} {:?} (->NaN) = false", n, op_str, s);
                        Some(false)
                    }
                },

                // Bool x Str: bool -> num, str -> num
                (Some(Raw(Bool(b))), Some(Raw(Str(s)))) => {
                    let bnum = *b as i64;
                    match js_str_to_num(s) {
                        Some(snum) => {
                            let res = cmp_num(bnum, snum);
                            trace!(
                                "CmpOrd (L): {} (->{}) {} {:?} (->{}) = {}",
                                b, bnum, op_str, s, snum, res
                            );
                            Some(res)
                        }
                        None => {
                            trace!(
                                "CmpOrd (L): {} (->{}) {} {:?} (->NaN) = false",
                                b, bnum, op_str, s
                            );
                            Some(false)
                        }
                    }
                }
                (Some(Raw(Str(s))), Some(Raw(Bool(b)))) => {
                    let bnum = *b as i64;
                    match js_str_to_num(s) {
                        Some(snum) => {
                            let res = cmp_num(snum, bnum);
                            trace!(
                                "CmpOrd (L): {:?} (->{}) {} {} (->{}) = {}",
                                s, snum, op_str, b, bnum, res
                            );
                            Some(res)
                        }
                        None => {
                            trace!(
                                "CmpOrd (L): {:?} (->NaN) {} {} (->{}) = false",
                                s, op_str, b, bnum
                            );
                            Some(false)
                        }
                    }
                }

                _ => None,
            };

            if let Some(res) = result {
                node.reduce(Raw(Bool(res)));
            }
        }

        Ok(())
    }
}

fn js_str_to_num(s: &str) -> Option<i64> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        Some(0)
    } else {
        trimmed.parse::<i64>().ok()
    }
}

#[cfg(test)]
mod tests_js_comparator {
    use super::*;
    use crate::js::bool::ParseBool;
    use crate::js::build_javascript_tree;
    use crate::js::integer::ParseInt;
    use crate::js::linter::Linter;
    use crate::js::string::ParseString;

    fn deobfuscate_comparator(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseString::default(),
            ParseBool::default(),
            StrictEq::default(),
            LooseEq::default(),
            CmpOrd::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_strict_eq_num() {
        assert_eq!(deobfuscate_comparator("var x = 1 === 1;"), "var x = true;");
        assert_eq!(deobfuscate_comparator("var x = 1 === 2;"), "var x = false;");
    }

    #[test]
    fn test_strict_eq_cross_type() {
        assert_eq!(
            deobfuscate_comparator("var x = 1 === \"1\";"),
            "var x = false;",
        );
    }

    #[test]
    fn test_strict_neq_num() {
        assert_eq!(deobfuscate_comparator("var x = 1 !== 2;"), "var x = true;",);
    }

    #[test]
    fn test_loose_eq_num() {
        assert_eq!(deobfuscate_comparator("var x = 42 == 42;"), "var x = true;",);
    }

    #[test]
    fn test_loose_eq_str_num() {
        assert_eq!(deobfuscate_comparator("\"42\" == 42;"), "true;",);
        assert_eq!(deobfuscate_comparator("\"abc\" == 0;"), "false;",);
    }

    #[test]
    fn test_loose_eq_bool_num() {
        assert_eq!(deobfuscate_comparator("true == 1;"), "true;",);
        assert_eq!(deobfuscate_comparator("false == 0;"), "true;",);
        assert_eq!(deobfuscate_comparator("true == 2;"), "false;",);
    }

    #[test]
    fn test_loose_eq_empty_str_num() {
        assert_eq!(deobfuscate_comparator("\"\" == 0;"), "true;",);
    }

    #[test]
    fn test_loose_neq() {
        assert_eq!(deobfuscate_comparator("1 != 2;"), "true;",);
    }

    #[test]
    fn test_cmp_num() {
        assert_eq!(deobfuscate_comparator("3 < 5;"), "true;");
        assert_eq!(deobfuscate_comparator("5 > 3;"), "true;");
        assert_eq!(deobfuscate_comparator("3 <= 3;"), "true;");
        assert_eq!(deobfuscate_comparator("4 >= 5;"), "false;");
    }

    #[test]
    fn test_cmp_str_lex() {
        assert_eq!(deobfuscate_comparator("\"abc\" < \"abd\";"), "true;",);
    }

    #[test]
    fn test_cmp_bool_num() {
        assert_eq!(deobfuscate_comparator("true > 0;"), "true;",);
    }

    #[test]
    fn test_cmp_str_num() {
        assert_eq!(deobfuscate_comparator("\"10\" > 9;"), "true;",);
        assert_eq!(deobfuscate_comparator("\"abc\" < 5;"), "false;",);
    }
}
