use error::MinusOneResult;
use ps::Powershell;
use ps::Powershell::{HashEntry, HashMap, Raw};
use ps::Value::Str;
use rule::RuleMut;
use std::collections::BTreeMap;
use tree::{ControlFlow, NodeMut};

#[derive(Default)]
pub struct ParseHash;

impl<'a> RuleMut<'a> for ParseHash {
    type Language = Powershell;

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

        if view.kind() == "hash_entry" {
            if let (Some(key_expression), Some(pipeline)) = (view.child(0), view.child(2)) {
                if let Some(Raw(value)) = pipeline.data() {
                    if let Some(Raw(key)) = key_expression.data() {
                        node.set(Powershell::HashEntry(key.normalize(), value.clone()));
                    } else if let Some(key_child) = key_expression.child(0) {
                        if key_child.kind() == "simple_name" {
                            node.set(Powershell::HashEntry(
                                Str(key_child.text()?.to_lowercase()),
                                value.clone(),
                            ));
                        }
                    }
                }
            }
        } else if view.kind() == "hash_literal_body" {
            let mut result = BTreeMap::new();
            //manage the map itself
            for child in view.iter() {
                if let Some(HashEntry(k, v)) = child.data() {
                    result.insert(Str(k.to_string().to_lowercase()), v.clone());
                }
            }
            node.set(HashMap(result))
        }

        Ok(())
    }
}
