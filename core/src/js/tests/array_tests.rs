#[cfg(test)]
mod tests_js_array {
    use crate::js::array::*;
    use crate::js::build_javascript_tree;
    use crate::js::forward::Forward;
    use crate::js::integer::{ParseInt, PosNeg, Substract};
    use crate::js::iterator::IteratorBuiltins;
    use crate::js::linter::Linter;
    use crate::js::objects::object::ObjectField;
    use crate::js::specials::{AddSubSpecials, ParseSpecials};
    use crate::js::string::BracketCharAt;
    use crate::js::string::ParseString;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseString::default(),
            ParseArray::default(),
            ParseSpecials::default(),
            CombineArrays::default(),
            Forward::default(),
            Substract::default(),
            ArrayBuiltins::default(),
            IteratorBuiltins::default(),
            PosNeg::default(),
            ObjectField::default(),
            GetArrayElement::default(),
            ArrayPlusMinus::default(),
            AddSubSpecials::default(),
            BracketCharAt::default(),
            ObjectField::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_array_parsing() {
        assert_eq!(
            deobfuscate("var x = [1, 2, [3, '4']]"),
            "var x = [1, 2, [3, '4']]"
        );
    }

    #[test]
    fn test_combine_arrays() {
        assert_eq!(
            deobfuscate("var x = [0, 1,7] + [3, [7, '2', [88]]]"),
            "var x = '0,1,73,7,2,88'"
        );
    }

    #[test]
    fn test_get_array_element() {
        assert_eq!(
            deobfuscate("var x = ([1, [2, '3'], 4][1])[0];"),
            "var x = 2;"
        );
    }

    #[test]
    fn test_array_plus_minus() {
        assert_eq!(deobfuscate("var x = +[['455']];"), "var x = 455;");

        assert_eq!(deobfuscate("var x = +['a'];"), "var x = NaN;");

        assert_eq!(deobfuscate("var x = [8] - 1;"), "var x = 7;");
    }

    #[test]
    fn test_jsfuck_from_array_access() {
        assert_eq!(deobfuscate("var x = ([][[]]+[])[1];"), "var x = 'n';");
    }

    #[test]
    fn test_dont_reduce_array_lookup_when_used_as_callee() {
        assert_eq!(deobfuscate("var x = [][[]]();"), "var x = [][[]]();");
    }

    #[test]
    fn test_sparse_array_indexing() {
        assert_eq!(
            deobfuscate("var x = [,,, 'hello',,][3];"),
            "var x = 'hello';"
        );
        assert_eq!(
            deobfuscate("var x = [,,, 'hello',,][4];"),
            "var x = undefined;"
        );
    }

    #[test]
    fn test_sparse_array_length() {
        assert_eq!(deobfuscate("var x = [,,, 'hello',,].length;"), "var x = 5;");
    }

    #[test]
    fn test_builtin_at() {
        assert_eq!(deobfuscate("var x = [0,1,2].at()"), "var x = 0");
        assert_eq!(deobfuscate("var x = [0,1,2].at(2)"), "var x = 2");
        assert_eq!(deobfuscate("var x = [0,1,2].at(3)"), "var x = undefined");
        assert_eq!(deobfuscate("var x = [0,1,2].at(-1)"), "var x = 2");
        assert_eq!(deobfuscate("var x = [0,1,2].at(-3)"), "var x = 0");
        assert_eq!(deobfuscate("var x = [0,1,2].at(-4)"), "var x = undefined");
    }

    #[test]
    fn test_builtin_concat() {
        assert_eq!(deobfuscate("var x = [0].concat()"), "var x = [0]");
        assert_eq!(deobfuscate("var x = [0].concat(1)"), "var x = [0, 1]");
        assert_eq!(
            deobfuscate("var x = [0,1,2].concat(1, 'a', ['b', ['c']])"),
            "var x = [0, 1, 2, 1, 'a', 'b', ['c']]"
        );
    }

    #[test]
    fn test_builtin_copy_within() {
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5, 6].copyWithin()"),
            "var x = [0, 1, 2, 3, 4, 5, 6]"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5, 6].copyWithin(2)"),
            "var x = [0, 1, 0, 1, 2, 3, 4]"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5, 6].copyWithin('2')"),
            "var x = [0, 1, 0, 1, 2, 3, 4]"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5, 6].copyWithin('??')"),
            "var x = [0, 1, 2, 3, 4, 5, 6]"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5, 6].copyWithin(2, 3)"),
            "var x = [0, 1, 3, 4, 5, 6, 6]"
        ); // crash here
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5, 6].copyWithin(2, '??')"),
            "var x = [0, 1, 0, 1, 2, 3, 4]"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5, 6].copyWithin(2, 0, 3)"),
            "var x = [0, 1, 0, 1, 2, 5, 6]"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5, 6].copyWithin(2, 0, '??')"),
            "var x = [0, 1, 2, 3, 4, 5, 6]"
        );
    }

    #[test]
    fn test_builtin_entries() {
        assert_eq!(
            deobfuscate("var x = [0, 1, 2].entries()"),
            "var x = [object Array Iterator]"
        );

        // Order of the fields is random ??
        let result = deobfuscate("var x = [0, 1, 2].entries().next()");
        if result.starts_with("var x = {v") {
            assert_eq!(result, "var x = {value: [0, 0], done: false}");
        } else {
            assert_eq!(result, "var x = {done: false, value: [0, 0]}");
        }

        assert_eq!(
            deobfuscate("var x = [0, 1, 2].entries().next().value"),
            "var x = [0, 0]"
        );
    }

    #[test]
    fn test_builtin_fill() {
        assert_eq!(deobfuscate("var x = [].fill()"), "var x = []");
        assert_eq!(
            deobfuscate("var x = [1, 2, 3].fill()"),
            "var x = [undefined, undefined, undefined]"
        );
        assert_eq!(
            deobfuscate("var x = [1, 2, 3].fill(0)"),
            "var x = [0, 0, 0]"
        );
        assert_eq!(
            deobfuscate("var x = [1, 2, 3].fill(0, 1)"),
            "var x = [1, 0, 0]"
        );
        assert_eq!(
            deobfuscate("var x = [1, 2, 3].fill(0, '??')"),
            "var x = [0, 0, 0]"
        );
        assert_eq!(
            deobfuscate("var x = [1, 2, 3, 4, 5].fill(0, 1, 4)"),
            "var x = [1, 0, 0, 0, 5]"
        );
        assert_eq!(
            deobfuscate("var x = [1, 2, 3, 4, 5].fill(0, 1, '??')"),
            "var x = [1, 2, 3, 4, 5]"
        );
        assert_eq!(
            deobfuscate("var x = [1, 2, 3, 4, 5].fill(0, '??', 4)"),
            "var x = [0, 0, 0, 0, 5]"
        );
        assert_eq!(
            deobfuscate("var x = [1, 2, 3, 4, 5].fill(0, '??', '??')"),
            "var x = [1, 2, 3, 4, 5]"
        );
    }

    #[test]
    fn test_builtin_flat() {
        assert_eq!(
            deobfuscate("var x = [0, 1, [2, [3, [4, 5]]]].flat()"),
            "var x = [0, 1, 2, [3, [4, 5]]]"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, [2, [3, [4, 5]]]].flat(0)"),
            "var x = [0, 1, [2, [3, [4, 5]]]]"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, [2, [3, [4, 5]]]].flat(1)"),
            "var x = [0, 1, 2, [3, [4, 5]]]"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, [2, [3, [4, 5]]]].flat(2)"),
            "var x = [0, 1, 2, 3, [4, 5]]"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, [2, [3, [4, 5]]]].flat('??')"),
            "var x = [0, 1, [2, [3, [4, 5]]]]"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, [2, [3, [4, 5]]]].flat(-1)"),
            "var x = [0, 1, [2, [3, [4, 5]]]]"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, [2, [3, [4, 5]]]].flat(Infinity)"),
            "var x = [0, 1, 2, 3, 4, 5]"
        );
    }

    #[test]
    fn test_builtin_includes() {
        assert_eq!(deobfuscate("var x = [0, 1, 2].includes()"), "var x = false");
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, undefined].includes()"),
            "var x = true"
        );
        assert_eq!(deobfuscate("var x = [0, 1, 2].includes(2)"), "var x = true");
        assert_eq!(
            deobfuscate("var x = [0,1,2].includes('2')"),
            "var x = false"
        );
    }

    #[test]
    fn test_builtin_index_of() {
        assert_eq!(deobfuscate("var x = [0, 1, 2].indexOf()"), "var x = -1");
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, undefined].indexOf()"),
            "var x = 3"
        );
        assert_eq!(deobfuscate("var x = [0, 1, 2].indexOf(2)"), "var x = 2");
        assert_eq!(deobfuscate("var x = [0,1,2].indexOf('2')"), "var x = -1");
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 0, 1, 2].indexOf(2)"),
            "var x = 2"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 0, 1, 2].indexOf(2, 2)"),
            "var x = 2"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 0, 1, 2].indexOf(2, 3)"),
            "var x = 5"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 0, 1, 2].indexOf(2, '??')"),
            "var x = 2"
        );
    }

    #[test]
    fn test_builtin_last_index_of() {
        assert_eq!(deobfuscate("var x = [0, 1, 2].lastIndexOf()"), "var x = -1");
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, undefined].lastIndexOf()"),
            "var x = 3"
        );
        assert_eq!(deobfuscate("var x = [0, 1, 2].lastIndexOf(2)"), "var x = 2");
        assert_eq!(
            deobfuscate("var x = [0,1,2].lastIndexOf('2')"),
            "var x = -1"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 0, 1, 2].lastIndexOf(2)"),
            "var x = 5"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 0, 1, 2].lastIndexOf(2, 2)"),
            "var x = 2"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 0, 1, 2].lastIndexOf(2, 3)"),
            "var x = 2"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 0, 1, 2].lastIndexOf(2, '??')"),
            "var x = -1"
        );
    }

    /*#[test]
    fn test_builtin_pop() {
        assert_eq!(deobfuscate("var x = [0].pop()"), "var x = 0");
        assert_eq!(deobfuscate("var x = [].pop()"), "var x = undefined");
    }*/

    /*#[test]
    fn test_builtin_push() {
        assert_eq!(deobfuscate("var x = [0,1,2,3].push()"), "var x = 4");
        assert_eq!(deobfuscate("var x = [0,1,2,3].push(4)"), "var x = 5");
        assert_eq!(
            deobfuscate("var x = [0,1,2,3].push(undefined)"),
            "var x = 5"
        );
        assert_eq!(
            deobfuscate("var x = [0,1,2,3].push(4,5,6,7,8,9)"),
            "var x = 10"
        );
    }*/

    /*#[test]
    fn test_convert_builtin_to_string() {
        assert_eq!(
            deobfuscate("var x = [0].pop + ''"),
            "var x = 'function pop() { [native code] }'"
        );
        assert_eq!(
            deobfuscate("var x = undefined + [0].pop"),
            "var x = 'undefinedfunction pop() { [native code] }'"
        );
    }*/

    /*#[test]
    fn test_builtin_reverse() {
        assert_eq!(
            deobfuscate("var x = [0,1,2,3].reverse()"),
            "var x = [3, 2, 1, 0]"
        );
        assert_eq!(deobfuscate("var x = [0].reverse()"), "var x = [0]");
        assert_eq!(deobfuscate("var x = [].reverse()"), "var x = []");
    }*/

    #[test]
    fn test_builtin_to_reversed() {
        assert_eq!(
            deobfuscate("var x = [0,1,2,3].toReversed()"),
            "var x = [3, 2, 1, 0]"
        );
        assert_eq!(deobfuscate("var x = [0].toReversed()"), "var x = [0]");
        assert_eq!(deobfuscate("var x = [].toReversed()"), "var x = []");
    }

    /*#[test]
    fn test_builtin_shift() {
        assert_eq!(deobfuscate("var x = [0].shift()"), "var x = 0");
        assert_eq!(deobfuscate("var x = [].shift()"), "var x = undefined");
    }*/

    #[test]
    fn test_builtin_slice() {
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5].slice(1, 4);"),
            "var x = [1, 2, 3];"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5].slice(2);"),
            "var x = [2, 3, 4, 5];"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5].slice(-3);"),
            "var x = [3, 4, 5];"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5].slice(-4, -1);"),
            "var x = [2, 3, 4];"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5].slice(2, 1);"),
            "var x = [];"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5].slice(10);"),
            "var x = [];"
        );
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4, 5].slice();"),
            "var x = [0, 1, 2, 3, 4, 5];"
        );
    }

    /*#[test]
    fn test_builtin_sort() {
        assert_eq!(
            deobfuscate("var x = [0, 8, 7, 3].sort()"),
            "var x = [0, 3, 7, 8]"
        );
        assert_eq!(deobfuscate("var x = [0].sort()"), "var x = [0]");
        assert_eq!(deobfuscate("var x = [].sort()"), "var x = []");
        assert_eq!(
            deobfuscate("var x = [9, 10, 11].sort()"),
            "var x = [10, 11, 9]"
        ); // to_string moment...
    }*/

    #[test]
    fn test_builtin_to_sorted() {
        assert_eq!(
            deobfuscate("var x = [0, 8, 7, 3].toSorted()"),
            "var x = [0, 3, 7, 8]"
        );
        assert_eq!(deobfuscate("var x = [0].toSorted()"), "var x = [0]");
        assert_eq!(deobfuscate("var x = [].toSorted()"), "var x = []");
        assert_eq!(
            deobfuscate("var x = [9, 10, 11].toSorted()"),
            "var x = [10, 11, 9]"
        ); // to_string moment...
    }

    /*#[test]
    fn test_builtin_unshift() {
        assert_eq!(deobfuscate("var x = [0,1,2,3].unshift()"), "var x = 4");
        assert_eq!(deobfuscate("var x = [0,1,2,3].unshift(4)"), "var x = 5");
        assert_eq!(
            deobfuscate("var x = [0,1,2,3].unshift(undefined)"),
            "var x = 5"
        );
        assert_eq!(
            deobfuscate("var x = [0,1,2,3].unshift(4,5,6,7,8,9)"),
            "var x = 10"
        );
    }*/

    #[test]
    fn test_builtin_values() {
        assert_eq!(
            deobfuscate("var x = [0, 1, 2].values()"),
            "var x = [object Array Iterator]"
        );

        let result = deobfuscate("var x = [0, 1, 2].values().next()");
        if result.starts_with("var x = {v") {
            assert_eq!(result, "var x = {value: 0, done: false}");
        } else {
            assert_eq!(result, "var x = {done: false, value: 0}");
        }

        assert_eq!(
            deobfuscate("var x = [0, 1, 2].values().next().value"),
            "var x = 0"
        );
    }

    #[test]
    fn test_builtin_length() {
        assert_eq!(deobfuscate("var x = [0, 1, 2].length"), "var x = 3");
        assert_eq!(deobfuscate("var x = [].length"), "var x = 0");
    }
}
