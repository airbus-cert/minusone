use rule::RuleMut;
use ps::{Powershell, Value};
use tree::NodeMut;
use error::MinusOneResult;
use ps::Powershell::{Raw, Array};
use ps::array::parse_i32;
use ps::Value::Str;

/// This function get char at index position
/// even if the index is negative
/// to seemless powershell engine
fn get_at_index(s: &str, index: i32) -> Option<String> {
    let mut uz_index = index as usize;

    // negative value is allowed by powershell
    if index < 0 {
        uz_index = (s.len() as i32 + index) as usize;
    }

    s.chars().nth(uz_index).map(|c| c.to_string())

}


#[derive(Default)]
pub struct AccessString;

impl<'a> RuleMut<'a> for AccessString {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
        let view = node.view();
        if view.kind() == "element_access"  {
            if let (Some(element), Some(expression)) = (view.child(0), view.child(2)) {
                match (element.data(), expression.data()) {
                    // We will handle the case of indexing string by an array
                    // ex: "foo"[1,2] => ["o", "o"]
                    (Some(Raw(Str(string_element))), Some(Array(index))) => {
                        let mut result = vec![];
                        for index_value in index {
                            if let Some(parsed_index_value) = parse_i32(index_value) {
                                if let Some(string_result) = get_at_index(string_element, parsed_index_value) {
                                    result.push(Str(string_result));
                                }
                            }
                        }
                        node.set(Array(result));
                    },
                    (Some(Raw(Str(string_element))), Some(Raw(index_value))) => {
                        if let Some(parsed_index_value) = parse_i32(index_value) {
                            if let Some(string_result) = get_at_index(string_element, parsed_index_value) {
                                let mut result = vec![];
                                result.push(Str(string_result));
                                node.set(Array(result));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use ps::from_powershell_src;
    use ps::integer::ParseInt;
    use ps::string::ParseString;
    use ps::forward::Forward;
    use ps::array::ParseArrayLiteral;

    #[test]
    fn test_access_string_element_from_int() {
        let mut tree = from_powershell_src("'abc'[0]").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ParseString::default(),
            AccessString::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Str("a".to_string())])
        );
    }

    #[test]
    fn test_access_string_element_from_negative_int() {
        let mut tree = from_powershell_src("'abc'[-2]").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ParseString::default(),
            AccessString::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Str("b".to_string())])
        );
    }

    #[test]
    fn test_access_string_element_from_negative_string() {
        let mut tree = from_powershell_src("'abc'['-2']").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ParseString::default(),
            AccessString::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Str("b".to_string())])
        );
    }

    #[test]
    fn test_access_string_element_from_string() {
        let mut tree = from_powershell_src("'abc'['0']").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ParseString::default(),
            AccessString::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Str("a".to_string())])
        );
    }

    #[test]
    fn test_access_string_multi_element_from_int() {
        let mut tree = from_powershell_src("'abc'[1, 2]").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ParseString::default(),
            AccessString::default(),
            ParseArrayLiteral::default()
        )).unwrap();

        assert_eq!(*tree.root().unwrap()
            .child(0).unwrap()
            .data().expect("Inferred type"), Array(vec![Str("b".to_string()), Str("c".to_string())])
        );
    }
}