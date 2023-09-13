use rule::RuleMut;
use ps::Powershell;
use tree::NodeMut;
use error::MinusOneResult;
use ps::litter::Litter;

#[derive(Default)]
pub struct CastBehavior;

impl<'a> RuleMut<'a> for CastBehavior {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        let view = node.view();
        if view.kind() == "command" {
            if let Some(command_name) = view.child(0) {
                if command_name.text() == Ok("%") {
                    let mut litter = Litter::new();
                    litter.print(&view)?;
                    match litter.output.as_str() {
                        "% { [char][int]$_ }" => {

                        }
                        _ => ()
                    }
                }
            }
        }

        Ok(())
    }
}

