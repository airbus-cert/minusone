use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::Raw;
use crate::js::Value::Str;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use log::warn;

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

        if let Ok(text) = view.text()
            && (
                text == "typeof window" // if present -> browser
                || text == "typeof document" // if present -> browser
                || text == "typeof browser" // if present -> browser
                || text == "typeof XMLHttpRequest" // if present -> browser
                || text == "typeof navigator" // if present -> browser
                || text == "typeof self" // if present -> browser
                || text == "typeof globalThis" // if present -> browser
                || text == "typeof location" // if present -> browser
                || text == "typeof history" // if present -> browser
                || text == "typeof screen" // if present -> browser
                || text == "typeof localStorage" // if present -> browser
                || text == "typeof sessionStorage" // if present -> browser
                || text == "typeof Worker" // if present -> browser
                || text == "typeof Element" // if present -> browser
                || text == "typeof HTMLElement" // if present -> browser
                || text == "typeof module" // if present -> NodeJS
                || text == "typeof process"
                // if present -> browser
            )
        {
            warn!(
                "The script tried to detect if the environment is a browser by using '{}'.",
                text
            );
        }

        if let (Some(left), Some(right)) = (view.child(0), view.child(1))
            && left.text()? == "typeof"
            && let Some(r) = right.data()
        {
            let r = r.clone();
            node.reduce(Raw(Str(r.r#typeof().to_string())));
        }
        Ok(())
    }
}
