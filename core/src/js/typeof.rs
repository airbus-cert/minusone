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
            && (text == "typeof window" || text == "typeof document" || text == "typeof browser")
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
