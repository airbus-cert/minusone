use rule::RuleMut;
use ps::Powershell;
use tree::{NodeMut, Node};
use error::{MinusOneResult, Error};
use ps::litter::Litter;
use ps::Powershell::{Array, PSItem, Raw};
use ps::Value::{Num, Str};

fn find_previous_expr<'a>(command: &Node<'a, Powershell>) -> MinusOneResult<Option<Node<'a, Powershell>>> {
    let pipeline = command.parent().ok_or(Error::invalid_child())?;
    // find in the pipeline at which index i am
    let mut index = 0;
    for pipeline_element in pipeline.range(Some(0), None, Some(2)) {
        if &pipeline_element == command {
            break;
        }
        index += 2; // gap is 2 to jump over the '|' token
    }

    if index < 2 {
        Ok(None)
    }
    else {
        Ok(pipeline.child(index - 2))
    }
}

fn parse_command(command: &Node<Powershell>) -> MinusOneResult<Option<String>> {
    if let Some(command_name) = command.named_child("command_name") {
        if command.child(0).unwrap().kind() == "command_invokation_operator" {
            if let Some(Raw(Str(inferred_name))) = command_name.data() {
                Ok(Some(inferred_name.to_lowercase()))
            }
            else{
                Ok(None)
            }
        }
        else {
            Ok(Some(command_name.text()?.to_lowercase()))
        }
    }
    else {
        Ok(None)
    }
}

#[derive(Default)]
pub struct PSItemInferrator;

impl<'a> RuleMut<'a> for PSItemInferrator {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        let view = node.view();
        // find usage of magic variable
        if view.kind() == "variable" && view.text()? == "$_"{
            if let Some(script_block_expression) = view.get_parent_of_type("script_block_expression") {
                if let Some(command) = script_block_expression.get_parent_of_type("command") {
                    // it's part of a foreach command
                    if let Some(command_name) = parse_command(&command)? {
                        if command_name == "%" || command_name == "foreach-object" {
                            if let Some(previous) = find_previous_expr(&command)? {
                                // the previous in the pipeline
                                if let Some(Array(values)) = previous.data() {
                                    node.set(PSItem(values.clone()));
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[derive(Default)]
pub struct ForEach;

impl<'a> RuleMut<'a> for ForEach {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        let view = node.view();
        // find usage of magic variable
        if view.kind() == "command" {
            if let Some(command_name) = parse_command(&view)? {
                if command_name == "%" || command_name == "foreach-object" {
                    if let Some(command_elements) = view.named_child("command_elements") {
                        // we can only handle this pattern
                        if command_elements.child_count() == 1 && command_elements.child(0).unwrap().kind() == "script_block_expression" {
                            let script_block_expression = command_elements.child(0).unwrap();
                            if let Some(previous_command) = find_previous_expr(&view)? {
                                // if the previous pipeline was inferred as an array
                                if let Some(Array(values)) = previous_command.data() {
                                    let script_block_body = script_block_expression
                                        .child(1).ok_or(Error::invalid_child())? // script_block node
                                        .named_child("script_block_body");

                                    if let Some(script_block_body_node) = script_block_body {
                                        if let Some(statement_list) = script_block_body_node.named_child("statement_list") {
                                            // determine the number of loop
                                            // by looping over the size of the array

                                            let mut result = Vec::new();
                                            for i in 0..values.len() {
                                                for child_statement in statement_list.iter() {
                                                    if child_statement.kind() == "empty_statement" {
                                                        continue
                                                    }

                                                    match child_statement.data() {
                                                        Some(PSItem(values)) => {
                                                            result.push(values[i].clone());
                                                        },
                                                        Some(Raw(r)) => {
                                                            result.push(r.clone());
                                                        },
                                                        Some(Array(array_value)) => {
                                                            for v in array_value {
                                                                result.push(v.clone());
                                                            }
                                                        }
                                                        _ => {
                                                            // stop inferring we have not enough infos
                                                            return Ok(())
                                                        }
                                                    }
                                                }
                                            }
                                            node.set(Array(result));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
