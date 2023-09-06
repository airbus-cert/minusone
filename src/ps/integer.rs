use rule::RuleMut;
use ps::InferredValue;
use tree::NodeMut;
use error::MinusOneResult;
use ps::InferredValue::Number;

/// Parse int will interpret integer node into Rust world
#[derive(Default)]
pub struct ParseInt;

impl<'a> RuleMut<'a> for ParseInt {
    type Language = InferredValue;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        Ok(())
    }

    /// We will infer parse integer operation during down to top traveling of the tree
    /// We will manage to import numbers in normal format (ex: 123), hex (0x42),
    /// with unary operation (ex: -5)
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
    /// use minusone::ps::InferredValue::Number;
    /// use minusone::ps::integer::{ParseInt, AddInt};
    ///
    /// let mut test1 = from_powershell_src("4").unwrap();
    /// test1.apply_mut(&mut (ParseInt::default(), Forward::default())).unwrap();
    ///
    /// assert_eq!(*(test1.root().unwrap().child(0).expect("At least one child").data().expect("A data in the first child")), Number(4));
    ///
    /// let mut test2 = from_powershell_src("0x42").unwrap();
    /// test2.apply_mut(&mut (ParseInt::default(), AddInt::default(), Forward::default())).unwrap();
    ///
    /// assert_eq!(*(test2.root().unwrap().child(0).expect("At least one child").data().expect("A data in the first child")), Number(0x42));
    ///
    /// let mut test3 = from_powershell_src("-5").unwrap();
    /// test3.apply_mut(&mut (ParseInt::default(), AddInt::default(), Forward::default())).unwrap();
    ///
    /// assert_eq!(*(test3.root().unwrap().child(0).expect("At least one child").data().expect("A data in the first child")), Number(-5));
    /// ```
    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        let view = node.view();
        let token = view.text()?;
        match view.kind() {
            "hexadecimal_integer_literal" => {
                if let Ok(number) = u32::from_str_radix(&token[2..], 16) {
                    node.set(Number(number as i32));
                }
            },
            "decimal_integer_literal" => {
                if let Ok(number) = token.parse::<i32>() {
                    node.set(Number(number));
                }
            },
            "expression_with_unary_operator" => {
                if let (Some(operator), Some(expression)) = (view.child(0), view.child(1)) {
                    match (operator.text()?, expression.data()) {
                        ("-", Some(Number(num))) => node.set(Number(-num)),
                        ("+", Some(Number(num))) => node.set(Number(*num)),
                        _ => ()
                    }
                }
            }
            _ => ()
        }

        Ok(())
    }
}

/// This rule will infer integer operation + -
#[derive(Default)]
pub struct AddInt;

impl<'a> RuleMut<'a> for AddInt {
    type Language = InferredValue;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        Ok(())
    }

    /// We will infer integer operation during down to top traveling of the tree
    /// We will manage basic operation + and -
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
    /// use minusone::ps::InferredValue::Number;
    /// use minusone::ps::integer::{ParseInt, AddInt};
    ///
    /// let mut test1 = from_powershell_src("4 + 5").unwrap();
    /// test1.apply_mut(&mut (ParseInt::default(), AddInt::default(), Forward::default())).unwrap();
    ///
    /// assert_eq!(*(test1.root().unwrap().child(0).expect("At least one child").data().expect("A data in the first child")), Number(9));
    ///
    /// let mut test2 = from_powershell_src("4 - 5").unwrap();
    /// test2.apply_mut(&mut (ParseInt::default(), AddInt::default(), Forward::default())).unwrap();
    ///
    /// assert_eq!(*(test2.root().unwrap().child(0).expect("At least one child").data().expect("A data in the first child")), Number(-1));
    ///
    /// let mut test3 = from_powershell_src("4 + -5").unwrap();
    /// test3.apply_mut(&mut (ParseInt::default(), AddInt::default(), Forward::default())).unwrap();
    ///
    /// assert_eq!(*(test3.root().unwrap().child(0).expect("At least one child").data().expect("A data in the first child")), Number(-1));
    /// ```
    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        let node_view = node.view();
        if node_view.kind() == "additive_expression"  {
            if let (Some(left_op), Some(operator), Some(right_op)) = (node_view.child(0), node_view.child(1), node_view.child(2)) {
                match (left_op.data(), operator.text()?, right_op.data()) {
                    (Some(Number(number_left)), "+", Some(Number(number_right))) => node.set(Number(number_left + number_right)),
                    (Some(Number(number_left)), "-", Some(Number(number_right))) => node.set(Number(number_left - number_right)),
                    _ => {}
                }
            }
        }
        Ok(())
    }
}


/// This rule will infer integer operation + -
#[derive(Default)]
pub struct MultInt;

/// We will infer integer operation during down to top traveling of the tree
/// We will manage basic operation + and -
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
/// use minusone::ps::InferredValue::Number;
/// use minusone::ps::integer::{ParseInt, MultInt};
///
/// let mut test1 = from_powershell_src("4 * 5").unwrap();
/// test1.apply_mut(&mut (ParseInt::default(), MultInt::default(), Forward::default())).unwrap();
///
/// assert_eq!(*(test1.root().unwrap().child(0).expect("At least one child").data().expect("A data in the first child")), Number(20));
///
/// let mut test2 = from_powershell_src("4 / 5").unwrap();
/// test2.apply_mut(&mut (ParseInt::default(), MultInt::default(), Forward::default())).unwrap();
///
/// assert_eq!(*(test2.root().unwrap().child(0).expect("At least one child").data().expect("A data in the first child")), Number(0));
///
/// let mut test3 = from_powershell_src("108*116/108").unwrap();
/// test3.apply_mut(&mut (ParseInt::default(), MultInt::default(), Forward::default())).unwrap();
///
/// assert_eq!(*(test3.root().unwrap().child(0).expect("At least one child").data().expect("A data in the first child")), Number(116));
/// ```
impl<'a> RuleMut<'a> for MultInt {
    type Language = InferredValue;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        let node_view = node.view();
        if node_view.kind() == "multiplicative_expression"  {
            if let (Some(left_op), Some(operator), Some(right_op)) = (node_view.child(0), node_view.child(1), node_view.child(2)) {
                match (left_op.data(), operator.text()?, right_op.data()) {
                    (Some(Number(number_left)), "*", Some(Number(number_right))) => node.set(Number(number_left * number_right)),
                    (Some(Number(number_left)), "/", Some(Number(number_right))) => node.set(Number((number_left / number_right) as i32)),
                    _ => {}
                }
            }
        }
        Ok(())
    }
}