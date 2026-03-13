use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::*;
use crate::js::Value::*;
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

#[cfg(test)]
mod tests_comparator {
    use super::*;
    use crate::js::integer::ParseInt;
    use crate::js::string::ParseString;
    use crate::js::{build_javascript_tree, lint};

    #[test]
    fn test_strict_eq_num() {
        let mut tree = build_javascript_tree("var x = 1 === 1;").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), StrictEq::default()))
            .unwrap();
        assert_eq!(lint(&tree), "var x = true;");

        let mut tree = build_javascript_tree("var x = 1 === 2;").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), StrictEq::default()))
            .unwrap();
        assert_eq!(lint(&tree), "var x = false;");
    }

    #[test]
    fn test_strict_eq_cross_type() {
        let mut tree = build_javascript_tree("var x = 1 === \"1\";").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseString::default(),
            StrictEq::default(),
        ))
        .unwrap();
        assert_eq!(lint(&tree), "var x = false;");
    }

    #[test]
    fn test_strict_neq_num() {
        let mut tree = build_javascript_tree("var x = 1 !== 2;").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), StrictEq::default()))
            .unwrap();
        assert_eq!(lint(&tree), "var x = true;");
    }
}
