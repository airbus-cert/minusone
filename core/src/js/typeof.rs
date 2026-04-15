use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::{Array, Bytes, Function, NaN, Null, Object, Raw, Regex, Undefined};
use crate::js::Value::{BigInt, Bool, Num, Str};
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};

impl JavaScript {
    // If a new type is added, try `typeof ...` in the console
    pub fn r#typeof(&self) -> &str {
        match self {
            Raw(raw) => match raw {
                Num(_) => "number",
                Str(_) => "string",
                Bool(_) => "boolean",
                BigInt(_) => "bigint",
            },
            Array(_) => "object",
            Regex { .. } => "object",
            Function { .. } => "function",
            Undefined => "undefined",
            NaN => "number",
            Null => "object", // what ?
            Bytes(_) => "string",
            Object { .. } => "object",
        }
    }
}

/// Infer unary typeof calls
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::integer::ParseInt;
/// use minusone::js::r#typeof::Typeof;
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = typeof 5;").unwrap();
/// tree.apply_mut(&mut (ParseInt::default(), Typeof::default())).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "var x = 'number';");
/// ```
#[derive(Default)]
pub struct Typeof;

impl<'a> RuleMut<'a> for Typeof {
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
        if let (Some(left), Some(right)) = (view.child(0), view.child(1)) {
            if left.text()? == "typeof" {
                if let Some(r) = right.data() {
                    let r = r.clone();
                    node.reduce(Raw(Str(r.r#typeof().to_string())));
                }
            }
        }
        Ok(())
    }
}
