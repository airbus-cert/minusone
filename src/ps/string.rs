use rule::RuleMut;
use tree::{NodeMut, BranchFlow};
use error::MinusOneResult;
use ps::Value::{Str, Num, Bool};
use ps::Powershell;
use ps::Powershell::{Raw, Array};

#[derive(Default)]
pub struct ParseString;

impl<'a> RuleMut<'a> for ParseString {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()>{
        let view = node.view();

        match view.kind() {
            "verbatim_string_characters" => {
                let value = String::from(view.text()?);
                // Parse string by removing the double quote
                node.set(Raw(Str(String::from(&value[1..value.len() - 1]))));
            },
            "expandable_string_literal" => {
                // expand what is expandable
                let value = String::from(view.text()?);
                // Parse string by removing the double quote
                let mut result = String::from(&value[1..value.len() - 1]);

                // last child is the token \"
                for child in view.range(None, Some(view.child_count() - 1), None) {
                    if let Some(v) = child.data() {
                        match v {
                            Raw(Str(s)) => {
                                result = result.replace(child.text()?, s);
                            },
                            Raw(Num(n)) => {
                                result = result.replace(child.text()?, n.to_string().as_str());
                            },
                            Raw(Bool(true)) => {
                                result = result.replace(child.text()?, "True");
                            },
                            Raw(Bool(false)) => {
                                result = result.replace(child.text()?, "False");
                            }
                            Powershell::HashMap => {
                                result = result.replace(child.text()?, "System.Collections.Hashtable");
                            }
                            _ => ()
                        }
                    }
                    else {

                        // the expandable string have non inferred child
                        // so can't be inferred
                        return Ok(())
                    }
                }
                node.set(Raw(Str(result)));
            }
            _ => ()
        }
        Ok(())
    }
}


/// This rule will infer string concat operation
///
/// # Example
/// ```
/// extern crate tree_sitter;
/// extern crate tree_sitter_powershell;
/// extern crate minusone;
///
/// use minusone::tree::{HashMapStorage, Tree};
/// use minusone::ps::from_powershell_src;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::linter::Linter;
/// use minusone::ps::string::{ConcatString, ParseString};
///
/// let mut tree = from_powershell_src("'foo' + 'bar'").unwrap();
/// tree.apply_mut(&mut (ParseString::default(), Forward::default(), ConcatString::default())).unwrap();
///
/// let mut ps_litter_view = Linter::new();
/// ps_litter_view.print(&tree.root().unwrap()).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"foobar\"");
/// ```
#[derive(Default)]
pub struct ConcatString;

impl<'a> RuleMut<'a> for ConcatString {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()>{
        let view = node.view();
        if view.kind() == "additive_expression" ||  view.kind() == "additive_argument_expression" {
            if let (Some(left_op), Some(operator), Some(right_op)) = (view.child(0), view.child(1), view.child(2)) {
                match (left_op.data(), operator.text()?, right_op.data()) {
                     (Some(Raw(Str(string_left))), "+", Some(Raw(Str(string_right)))) => node.set(Raw(Str(String::from(string_left) + string_right))),
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct StringReplaceMethod;

impl<'a> RuleMut<'a> for StringReplaceMethod {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()>{
        let view = node.view();
        if view.kind() == "invokation_expression" {
            if let (Some(expression), Some(operator), Some(member_name), Some(arguments_list)) = (view.child(0), view.child(1), view.child(2), view.child(3)) {
                match (expression.data(), operator.text()?, member_name.text()?.to_lowercase().as_str()) {
                     (Some(Raw(Str(src))), ".", "replace") => {
                         if let Some(argument_expression_list) = arguments_list.named_child("argument_expression_list") {
                            if let (Some(arg_1), Some(arg_2)) = (argument_expression_list.child(0), argument_expression_list.child(2)) {
                                if let (Some(Raw(Str(from))), Some(Raw(to))) = (arg_1.data(), arg_2.data()) {
                                    node.set(Raw(Str(src.replace(from, &to.to_string()))));
                                }
                            }
                         }
                     },
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct StringReplaceOp;

impl<'a> RuleMut<'a> for StringReplaceOp {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()>{
        let view = node.view();
        if view.kind() == "comparison_expression" {
            if let (Some(left_expression), Some(operator), Some(right_expression)) = (view.child(0), view.child(1), view.child(2)) {
                match (left_expression.data(), operator.text()?.to_lowercase().as_str(), right_expression.data()) {
                    (Some(Raw(Str(src))), "-replace", Some(Array(params)))
                    | (Some(Raw(Str(src))), "-creplace", Some(Array(params))) =>  {
                        // -replace operator need two params
                        if let (Some(Str(old)), Some(Str(new))) = (params.get(0), params.get(1)) {
                            node.set(Raw(Str(src.replace(old, new))));
                        }
                    }
                    _ => ()
                }
            }
        }
        Ok(())
    }
}


/// This rule will infer format operator
///
/// # Example
/// ```
/// extern crate tree_sitter;
/// extern crate tree_sitter_powershell;
/// extern crate minusone;
///
/// use minusone::tree::{HashMapStorage, Tree};
/// use minusone::ps::from_powershell_src;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::linter::Linter;
/// use minusone::ps::string::{ParseString, FormatString};
/// use minusone::ps::array::ParseArrayLiteral;
///
/// let mut tree = from_powershell_src("\"{1} {0}\" -f 'world', 'hello'").unwrap();
/// tree.apply_mut(&mut (
///     ParseString::default(),
///     Forward::default(),
///     FormatString::default(),
///     ParseArrayLiteral::default())
/// ).unwrap();
///
/// let mut ps_litter_view = Linter::new();
/// ps_litter_view.print(&tree.root().unwrap()).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"hello world\"");
/// ```
#[derive(Default)]
pub struct FormatString;

impl<'a> RuleMut<'a> for FormatString {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>, _flow: BranchFlow) -> MinusOneResult<()>{
        let view = node.view();
        if view.kind() == "format_expression" {
            if let (Some(format_str_node), Some(format_args_node)) = (view.child(0), view.child(2)) {
                match (format_str_node.data(), format_args_node.data()) {
                    (Some(Raw(Str(format_str))), Some(Array(format_args))) => {
                        let mut result = format_str.clone();
                        let mut index = 0;
                        for new in format_args.iter() {
                            result = result.replace(format!("{{{}}}", index).as_str(), new.to_string().as_str());
                            index += 1;
                        }
                        node.set(Raw(Str(result)));
                    },
                    (Some(Raw(Str(format_str))), Some(Raw(format_arg))) => {
                        node.set(Raw(Str(format_str.replace("{0}", format_arg.to_string().as_str()))));
                    },
                    _ => ()
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use ps::from_powershell_src;
    use ps::forward::Forward;
    use ps::Powershell::Raw;
    use ps::Value::Str;
    use ps::string::{ParseString, ConcatString, StringReplaceOp, FormatString};
    use ps::array::ParseArrayLiteral;

    #[test]
    fn test_concat_two_elements() {
        let mut tree = from_powershell_src("'a' + 'b'").unwrap();
        tree.apply_mut(&mut (ParseString::default(), Forward::default(), ConcatString::default())).unwrap();
        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Raw(Str("ab".to_string()))
        );
    }

    #[test]
    fn test_infer_subexpression_elements() {
        let mut tree = from_powershell_src("\"foo$(\"b\"+\"a\"+\"r\")\"").unwrap();
        tree.apply_mut(&mut (ParseString::default(), Forward::default(), ConcatString::default())).unwrap();
        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Raw(Str("foobar".to_string()))
        );
    }

    #[test]
    fn test_replace_operator() {
        let mut tree = from_powershell_src("\"hello world\" -replace \"world\", \"toto\"").unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            Forward::default(),
            StringReplaceOp::default(),
            ParseArrayLiteral::default()
        )).unwrap();
        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Raw(Str("hello toto".to_string()))
        );
    }

    #[test]
    fn test_format_operator() {
        let mut tree = from_powershell_src("\"{1} {0}\" -f \"world\", \"hello\"").unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            Forward::default(),
            FormatString::default(),
            ParseArrayLiteral::default()
        )).unwrap();
        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Raw(Str("hello world".to_string()))
        );
    }
}