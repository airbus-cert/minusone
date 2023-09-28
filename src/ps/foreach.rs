use rule::RuleMut;
use ps::Powershell;
use tree::{NodeMut, Node};
use error::{MinusOneResult, Error};
use ps::Powershell::{Array, PSItem, Raw};
use ps::Value::Str;

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

/// This rule will stop on the special var $_
/// And check if it's used into a foreach command
/// And then will infer the result of the previous command
/// in the pipe into a PItem value
///
/// # Example
/// ```
/// extern crate tree_sitter;
/// extern crate tree_sitter_powershell;
/// extern crate minusone;
///
/// use minusone::ps::from_powershell_src;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::integer::ParseInt;
/// use minusone::ps::litter::Litter;
/// use minusone::ps::cast::Cast;
/// use minusone::ps::foreach::{PSItemInferrator, ForEach};
/// use minusone::ps::join::JoinOperator;
/// use minusone::ps::array::ParseArrayLiteral;
///
/// let mut tree = from_powershell_src("-join ((0x61, 0x62, 0x63)|% {[char]$_})").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(),
///     Forward::default(),
///     Cast::default(),
///     PSItemInferrator::default(),
///     ForEach::default(),
///     JoinOperator::default(),
///     ParseArrayLiteral::default()
///     )
/// ).unwrap();
///
/// let mut ps_litter_view = Litter::new();
/// ps_litter_view.print(&tree.root().unwrap()).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"abc\"");
/// ```
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
                                match previous.data() {
                                    Some(Array(values)) => {
                                        node.set(PSItem(values.clone()));
                                    },
                                    Some(Raw(value)) => {
                                        node.set(PSItem(vec![value.clone()]));
                                    },
                                    _ => ()
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

/// This rule will infer the foreach command by it self
///
/// # Example
/// ```
/// extern crate tree_sitter;
/// extern crate tree_sitter_powershell;
/// extern crate minusone;
///
/// use minusone::ps::from_powershell_src;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::integer::ParseInt;
/// use minusone::ps::litter::Litter;
/// use minusone::ps::cast::Cast;
/// use minusone::ps::foreach::{PSItemInferrator, ForEach};
/// use minusone::ps::join::JoinOperator;
/// use minusone::ps::array::ParseArrayLiteral;
/// use minusone::ps::string::ParseString;
///
/// let mut tree = from_powershell_src("-join ((0x61, 0x62, 0x63)|% {'z'; [char]$_})").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(),
///     ParseString::default(),
///     Forward::default(),
///     Cast::default(),
///     PSItemInferrator::default(),
///     ForEach::default(),
///     JoinOperator::default(),
///     ParseArrayLiteral::default()
///     )
/// ).unwrap();
///
/// let mut ps_litter_view = Litter::new();
/// ps_litter_view.print(&tree.root().unwrap()).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"zazbzc\"");
/// ```
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
                                let mut previous_values = Vec::new();
                                match previous_command.data() {
                                    Some(Array(values)) => previous_values.extend(values.clone()),
                                    // array of size 1
                                    Some(Raw(value)) => previous_values.push(value.clone()),
                                    _ => ()
                                }
                                let script_block_body = script_block_expression
                                    .child(1).ok_or(Error::invalid_child())? // script_block node
                                    .named_child("script_block_body");

                                if let Some(script_block_body_node) = script_block_body {
                                    if let Some(statement_list) = script_block_body_node.named_child("statement_list") {
                                        // determine the number of loop
                                        // by looping over the size of the array

                                        let mut result = Vec::new();
                                        for i in 0..previous_values.len() {
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
                                        if result.len() > 0 {
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

#[cfg(test)]
mod test {
    use ps::array::ParseArrayLiteral;
    use ps::forward::Forward;
    use ps::integer::ParseInt;
    use ps::foreach::{PSItemInferrator, ForEach};
    use ps::Powershell::Array;
    use ps::from_powershell_src;
    use ps::Value::{Num, Str};
    use ps::string::ParseString;
    use ps::cast::Cast;

    #[test]
    fn test_foreach_transparent() {
        let mut tree = from_powershell_src("(1,2,3) | % {$_}").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ParseArrayLiteral::default(),
            PSItemInferrator::default(),
            ForEach::default(),
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Num(1), Num(2), Num(3)])
        );
    }

    #[test]
    fn test_foreach_transparent_with_mixed_array() {
        let mut tree = from_powershell_src("(\"a\",2,3) | % {$_}").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ParseString::default(),
            ParseArrayLiteral::default(),
            PSItemInferrator::default(),
            ForEach::default(),
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Str("a".to_string()), Num(2), Num(3)])
        );
    }

    #[test]
    fn test_foreach_transparent_with_one_element() {
        let mut tree = from_powershell_src("(1) | % {$_}").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ParseString::default(),
            ParseArrayLiteral::default(),
            PSItemInferrator::default(),
            ForEach::default(),
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Num(1)])
        );
    }

    #[test]
    fn test_foreach_cast_with_one_element() {
        let mut tree = from_powershell_src("(0x61) | % {[char]$_}").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ParseString::default(),
            ParseArrayLiteral::default(),
            PSItemInferrator::default(),
            ForEach::default(),
            Cast::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Str("a".to_string())])
        );
    }

    #[test]
    fn test_foreach_cast_with_array() {
        let mut tree = from_powershell_src("(0x61, 0x62) | % {[char]$_}").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ParseString::default(),
            ParseArrayLiteral::default(),
            PSItemInferrator::default(),
            ForEach::default(),
            Cast::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Str("a".to_string()), Str("b".to_string())])
        );
    }

    #[test]
    fn test_foreach_cast_with_array_and_static_result() {
        let mut tree = from_powershell_src("(0x61, 0x62) | % {'z'; [char]$_}").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ParseString::default(),
            ParseArrayLiteral::default(),
            PSItemInferrator::default(),
            ForEach::default(),
            Cast::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Str("z".to_string()), Str("a".to_string()), Str("z".to_string()), Str("b".to_string())])
        );
    }
}