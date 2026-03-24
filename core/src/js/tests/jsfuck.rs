#[cfg(test)]
pub mod jsfuck_tests {
    use crate::js::array::*;
    use crate::js::b64::*;
    use crate::js::bool::ParseBool;
    use crate::js::bool::*;
    use crate::js::build_javascript_tree;
    use crate::js::comparator::*;
    use crate::js::forward::*;
    use crate::js::functions::fncall::*;
    use crate::js::functions::function::*;
    use crate::js::integer::*;
    use crate::js::linter::Linter;
    use crate::js::objects::object::*;
    use crate::js::specials::*;
    use crate::js::string::*;
    use crate::js::var::*;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseBool::default(),
            ParseString::default(),
            ParseFunction::default(),
            ParseArray::default(),
            ParseSpecials::default(),
            ParseObject::default(),
            NegInt::default(),
            SubAddInt::default(),
            MultInt::default(),
            PowInt::default(),
            ShiftInt::default(),
            BitwiseInt::default(),
            ObjectField::default(),
            NotBool::default(),
            BoolAlgebra::default(),
            AddBool::default(),
            CombineArrays::default(),
            CharAt::default(),
            Forward::default(),
            StringPlusMinus::default(),
            ArrayPlusMinus::default(),
            BoolPlusMinus::default(),
            Concat::default(),
            ConcatFunction::default(),
            Split::default(),
            GetArrayElement::default(),
            AddSubSpecials::default(),
            ToString::default(),
            B64::default(),
            Var::default(),
            FnCall::default(),
            StrictEq::default(),
            LooseEq::default(),
            CmpOrd,
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    /*'a':   '(false+"")[1]',
    'b':   '([]["entries"]()+"")[2]',
    'c':   '([]["flat"]+[])[3]',
    'd':   '(undefined+"")[2]',
    'e':   '(true+"")[3]',
    'f':   '(false+"")[0]',
    'g':   '(false+[0]+String)[20]',
    'h':   '(+(101))["to"+String["name"]](21)[1]',
    'i':   '([false]+undefined)[10]',
    'j':   '([]["entries"]()+"")[3]',
    'k':   '(+(20))["to"+String["name"]](21)',
    'l':   '(false+"")[2]',
    'm':   '(Number+"")[11]',
    'n':   '(undefined+"")[1]',
    'o':   '(true+[]["at"])[10]',
    'p':   '(+(211))["to"+String["name"]](31)[1]',
    'q':   '(+(2+[1]+[2]))["toString"](3+[1])[1]',
    'r':   '(true+"")[1]',
    's':   '(false+"")[3]',
    't':   '(true+"")[0]',
    'u':   '(undefined+"")[0]',
    'v':   '(+(31))["to"+String["name"]](32)',
    'w':   '(+(32))["to"+String["name"]](33)',
    'x':   '(+(101))["to"+String["name"]](34)[1]',
    'y':   '(NaN+[Infinity])[10]',
    'z':   '(+(35))["to"+String["name"]](36)',*/

    #[test]
    fn test_lower_alphabet() {
        assert_eq!("'a'", deobfuscate("(false+[])[1]"));
        assert_eq!("'b'", deobfuscate("([]['entries']()+'')[2]"));
        assert_eq!("'c'", deobfuscate("([]['flat']+[])[3]"));
        assert_eq!("'d'", deobfuscate("(undefined+[])[2]"));
        assert_eq!("'e'", deobfuscate("(true+[])[3]"));
        assert_eq!("'f'", deobfuscate("(false+[])[0]"));
        assert_eq!("'g'", deobfuscate("(false+[0]+String)[20]"));
        assert_eq!("'h'", deobfuscate("(+(101))['to'+String['name']](21)[1]"));
        assert_eq!("'i'", deobfuscate("([false]+undefined)[10]"));
        assert_eq!("'j'", deobfuscate("([]['entries']()+'')[3]"));
        assert_eq!("'k'", deobfuscate("(+(20))['to'+String['name']](21)"));
        assert_eq!("'l'", deobfuscate("(false+[])[2]"));
        assert_eq!("'m'", deobfuscate("(Number+[])[11]"));
        assert_eq!("'n'", deobfuscate("(undefined+[])[1]"));
        assert_eq!("'o'", deobfuscate("(true+[]['at'])[10]"));
        assert_eq!("'p'", deobfuscate("(+(211))['to'+String['name']](31)[1]"));
        assert_eq!("'q'", deobfuscate("(+(2+[1]+[2]))['toString'](3+[1])[1]"));
        assert_eq!("'r'", deobfuscate("(true+[])[1]"));
        assert_eq!("'s'", deobfuscate("(false+[])[3]"));
        assert_eq!("'t'", deobfuscate("(true+[])[0]"));
        assert_eq!("'u'", deobfuscate("(undefined+[])[0]"));
        assert_eq!("'v'", deobfuscate("(+(31))['to'+String['name']](32)"));
        assert_eq!("'w'", deobfuscate("(+(32))['to'+String['name']](33)"));
        assert_eq!("'x'", deobfuscate("(+(101))['to'+String['name']](34)[1]"));
        assert_eq!("'y'", deobfuscate("(NaN+[Infinity])[10]"));
        assert_eq!("'z'", deobfuscate("(+(35))['to'+String['name']](36)"));
    }
}
