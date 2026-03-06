use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::tree::BranchFlow::Predictable;
use crate::tree::ControlFlow::Continue;
use crate::tree::{ControlFlow, Node, Strategy};

#[derive(Default)]
pub struct JavaScriptStrategy;

impl Strategy<JavaScript> for JavaScriptStrategy {
    fn control(&self, _node: Node<JavaScript>) -> MinusOneResult<ControlFlow> {
        Ok(Continue(Predictable))
    }
}
