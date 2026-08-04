#[cfg(test)]
mod test_objectify {
    use crate::js::build_javascript_tree;
    use crate::js::functions::fncall::FnCall;
    use crate::js::linter::Linter;
    use crate::js::objects::object::*;
    use crate::js::string::*;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            ParseObject::default(),
            ObjectField::default(),
            FnCall::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_string_builtins() {
        // length
        assert_eq!(deobfuscate("'minusone'.length"), "8");

        // tags
        assert_eq!(deobfuscate("'minusone'.big()"), "'<big>minusone</big>'");
        assert_eq!(
            deobfuscate("'minusone'.blink()"),
            "'<blink>minusone</blink>'"
        );
        assert_eq!(deobfuscate("'minusone'.bold()"), "'<b>minusone</b>'");
        assert_eq!(deobfuscate("'minusone'.fixed()"), "'<tt>minusone</tt>'");
        assert_eq!(deobfuscate("'minusone'.italics()"), "'<i>minusone</i>'");
        assert_eq!(
            deobfuscate("'minusone'.small()"),
            "'<small>minusone</small>'"
        );
        assert_eq!(
            deobfuscate("'minusone'.strike()"),
            "'<strike>minusone</strike>'"
        );
        assert_eq!(deobfuscate("'minusone'.sub()"), "'<sub>minusone</sub>'");
        assert_eq!(deobfuscate("'minusone'.sup()"), "'<sup>minusone</sup>'");
        assert_eq!(
            deobfuscate("'minusone'.fontcolor()"),
            "'<font color=\"undefined\">minusone</font>'"
        );
    }
}
