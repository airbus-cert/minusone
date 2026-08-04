#[cfg(test)]
mod tests_js_integer {
    use crate::js::array::ParseArray;
    use crate::js::build_javascript_tree;
    use crate::js::forward::Forward;
    use crate::js::integer::*;
    use crate::js::linter::Linter;
    use crate::js::string::ParseString;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseString::default(),
            ParseArray::default(),
            NumberBuiltins::default(),
            PosNeg::default(),
            Forward::default(),
            AddInt::default(),
            Substract::default(),
            IncrDecr::default(),
            MultDivMod::default(),
            PowInt::default(),
            ShiftInt::default(),
            BitwiseInt::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_parse_int() {
        assert_eq!(deobfuscate("var x = 31;"), "var x = 31;");
        assert_eq!(deobfuscate("var x = 0x1F;"), "var x = 31;");
        assert_eq!(deobfuscate("var x = 0o37;"), "var x = 31;");
        assert_eq!(deobfuscate("var x = 0b11111;"), "var x = 31;");
        assert_eq!(deobfuscate("var x = 017;"), "var x = 15;");
        assert_eq!(deobfuscate("var x = 0017;"), "var x = 15;");
        assert_eq!(deobfuscate("var x = 019;"), "var x = 19;");
        assert_eq!(deobfuscate("var x = parseInt('10');"), "var x = 10;");
        assert_eq!(deobfuscate("var x = parseInt('10*3', 10);"), "var x = 10;");
        assert_eq!(
            deobfuscate("var x = parseInt('    3     ', 10);"),
            "var x = 3;"
        );
        assert_eq!(deobfuscate("var x = Number('10');"), "var x = 10;");
        assert_eq!(deobfuscate("var x = Number('10*3');"), "var x = NaN;");
        assert_eq!(deobfuscate("var x = Number('0x1f');"), "var x = 31;");
        assert_eq!(deobfuscate("var x = Number('');"), "var x = 0;");
        assert_eq!(deobfuscate("var x = parseInt('');"), "var x = NaN;");
    }

    #[test]
    fn test_parse_bigint() {
        assert_eq!(deobfuscate("var x = 31n;"), "var x = 31n;");
        assert_eq!(deobfuscate("var x = 0x1Fn;"), "var x = 31n;");
        assert_eq!(deobfuscate("var x = 0o37n;"), "var x = 31n;");
        assert_eq!(deobfuscate("var x = 0b11111n;"), "var x = 31n;");
        assert_eq!(deobfuscate("var x = 0b1_1111n;"), "var x = 31n;");
        assert_eq!(deobfuscate("var x = BigInt('0x1f');"), "var x = 31n;");
    }

    #[test]
    fn test_pos_neg_int() {
        assert_eq!(deobfuscate("var x = +42 + +5;"), "var x = 47;");
        assert_eq!(deobfuscate("var x = -42 - -5;"), "var x = -37;");
    }

    #[test]
    fn test_add_sub_int() {
        assert_eq!(deobfuscate("var x = 1 + 1;"), "var x = 2;");
        assert_eq!(deobfuscate("var x = 5 - 2;"), "var x = 3;");
        assert_eq!(
            deobfuscate("var x = 1 - 25 + 47 - 6 - 2 -99 + 120 + 33;"),
            "var x = 69;"
        );
    }

    #[test]
    fn test_add_sub_bigint() {
        assert_eq!(deobfuscate("var x = 1n + 1n;"), "var x = 2n;");
        assert_eq!(deobfuscate("var x = 5n - 2n;"), "var x = 3n;");
        assert_eq!(
            deobfuscate("var x = 1n - 25n + 47n - 6n - 2n -99n + 120n + 33n;"),
            "var x = 69n;"
        );
    }

    #[test]
    fn test_mult_div_mod_int() {
        assert_eq!(deobfuscate("var x = 3 * 4;"), "var x = 12;");
        assert_eq!(deobfuscate("var x = 10 / 2;"), "var x = 5;");
        assert_eq!(deobfuscate("var x = 10 / 0;"), "var x = Infinity;");
        assert_eq!(deobfuscate("var x = 0 / 0;"), "var x = NaN;");
        assert_eq!(deobfuscate("var x = 10 % 3;"), "var x = 1;");
        assert_eq!(deobfuscate("var x = 10 * 2 / 5 % 2;"), "var x = 0;");
    }

    #[test]
    fn test_wierd_mult_div_mod_int() {
        assert_eq!(deobfuscate("var x = '3' * [4];"), "var x = 12;");
        assert_eq!(deobfuscate("var x = '10' / [2];"), "var x = 5;");
        assert_eq!(deobfuscate("var x = '10' % [3];"), "var x = 1;");
        assert_eq!(deobfuscate("var x = '10' * [2] / 5 % 2;"), "var x = 0;");
    }

    #[test]
    fn test_mult_div_mod_bigint() {
        assert_eq!(deobfuscate("var x = 3n * 4n;"), "var x = 12n;");
        assert_eq!(deobfuscate("var x = 10n / 2n;"), "var x = 5n;");
        assert_eq!(deobfuscate("var x = 10n % 3n;"), "var x = 1n;");
        assert_eq!(deobfuscate("var x = 10n * 2n / 5n % 2n;"), "var x = 0n;");
    }

    #[test]
    fn test_op_priority() {
        assert_eq!(deobfuscate("var x = 1 + 3 * 36;"), "var x = 109;");
        assert_eq!(deobfuscate("var x = 1 + 9 * 6 % 28 - 3 * 7;"), "var x = 6;");
    }

    #[test]
    fn test_pow_int() {
        assert_eq!(deobfuscate("var x = 50 ** 8;"), "var x = 39062500000000;");
    }

    #[test]
    fn test_pow_bigint() {
        let mut excepted_value = String::from("1");
        for _ in 0..1000 {
            excepted_value = excepted_value + "0";
        }

        assert_eq!(
            deobfuscate("var x = 10n ** 1000n;"),
            format!("var x = {}n;", excepted_value)
        );
    }

    #[test]
    fn test_shift_int() {
        assert_eq!(deobfuscate("var x = 1 << 3;"), "var x = 8;");
        assert_eq!(deobfuscate("var x = 16 >> 2;"), "var x = 4;");
        assert_eq!(deobfuscate("let x = -16 >>> 2;"), "let x = 1073741820;"); // test fails
        assert_eq!(deobfuscate("var x = 1 << 3 >> 2;"), "var x = 2;");
        assert_eq!(deobfuscate("var x = 2 >> 31;"), "var x = 0;");
        assert_eq!(deobfuscate("var x = 2 >> 32;"), "var x = 2;");
        assert_eq!(deobfuscate("var x = 2 >> 33;"), "var x = 1;");
        assert_eq!(deobfuscate("let x = -16 >> 2;"), "let x = -4;");
    }

    #[test]
    fn test_shift_bigint() {
        assert_eq!(deobfuscate("var x = 1n << 3n;"), "var x = 8n;");
        assert_eq!(deobfuscate("var x = 16n >> 2n;"), "var x = 4n;");
        assert_eq!(deobfuscate("var x = 1n << 3n >> 2n;"), "var x = 2n;");
        assert_eq!(deobfuscate("var x = 2n >> 31n;"), "var x = 0n;");
        assert_eq!(deobfuscate("let x = -16n >> 2n;"), "let x = -4n;");
    }

    #[test]
    fn test_bitwise_int() {
        assert_eq!(deobfuscate("var x = 0x4 & 0x8;"), "var x = 0;");
        assert_eq!(deobfuscate("var x = 0x4 | 0x8;"), "var x = 12;");
        assert_eq!(deobfuscate("var x = 0x4 ^ 0x8;"), "var x = 12;");
        assert_eq!(deobfuscate("var x = ~0x4;"), "var x = -5;");
        assert_eq!(
            deobfuscate("var x = 0x15487596 ^ 0x5216598 | 0x36598745 & ~0x21215487;"),
            "var x = 377066318;",
        );
    }

    #[test]
    fn test_bitwise_bigint() {
        assert_eq!(deobfuscate("var x = 0x4n & 0x8n;"), "var x = 0n;");
        assert_eq!(deobfuscate("var x = 0x4n | 0x8n;"), "var x = 12n;");
        assert_eq!(deobfuscate("var x = 0x4n ^ 0x8n;"), "var x = 12n;");
        assert_eq!(deobfuscate("var x = ~0x4n;"), "var x = -5n;");
        assert_eq!(
            deobfuscate("var x = 0x15487596n ^ 0x5216598n | 0x36598745n & ~0x21215487n;"),
            "var x = 377066318n;",
        );
    }

    #[test]
    fn test_incr_decr() {
        assert_eq!(deobfuscate("var y = ++5;"), "var y = 6;");
        assert_eq!(deobfuscate("var y = --5;"), "var y = 4;");
        assert_eq!(deobfuscate("var y = 5++;"), "var y = 6;");
        assert_eq!(deobfuscate("var y = 5--;"), "var y = 4;");
        assert_eq!(deobfuscate("var y = ++5 + 1;"), "var y = 7;");
        assert_eq!(
            deobfuscate("for (var i = 0; i < 10; i++) {}"),
            "for (var i = 0; i < 10; i++) {}"
        );
        assert_eq!(
            deobfuscate("for (var i = 0; i < 10; --i) {}"),
            "for (var i = 0; i < 10; --i) {}"
        );
    }

    #[test]
    fn test_builtin_value_of() {
        assert_eq!(deobfuscate("var x = 12.34.valueOf();"), "var x = 12.34;");
        assert_eq!(deobfuscate("var x = 12.34['valueOf']();"), "var x = 12.34;");
    }

    #[test]
    fn test_builtin_to_precision() {
        assert_eq!(
            deobfuscate("var x = 12.3456.toPrecision();"),
            "var x = '12.3456';"
        );
        assert_eq!(
            deobfuscate("var x = 12.3456.toPrecision(1);"),
            "var x = '1e+1';"
        );
        assert_eq!(
            deobfuscate("var x = 12.3456.toPrecision(2);"),
            "var x = '12';"
        );
        assert_eq!(
            deobfuscate("var x = 12.3456.toPrecision(4);"),
            "var x = '12.35';"
        );
        assert_eq!(
            deobfuscate("var x = 12.3456.toPrecision(6);"),
            "var x = '12.3456';"
        );
        assert_eq!(
            deobfuscate("var x = 12.3456.toPrecision(8);"),
            "var x = '12.345600';"
        );
        assert_eq!(deobfuscate("var x = (0).toPrecision(1);"), "var x = '0';");
        assert_eq!(
            deobfuscate("var x = (0).toPrecision(4);"),
            "var x = '0.000';"
        );
        assert_eq!(
            deobfuscate("var x = (-12.3456).toPrecision(4);"),
            "var x = '-12.35';"
        );
        assert_eq!(
            deobfuscate("var x = (-12.3456).toPrecision(1);"),
            "var x = '-1e+1';"
        );
        assert_eq!(
            // output '0.0000000' instead of '0.0000012'
            deobfuscate("var x = (0.000001234).toPrecision(2);"),
            "var x = '0.0000012';"
        );
        assert_eq!(
            // output '0e-7' instead of '1e-7'
            deobfuscate("var x = (0.0000001).toPrecision(1);"),
            "var x = '1e-7';"
        );
        assert_eq!(
            // output '0.000000' instead of '0.000001'
            deobfuscate("var x = (0.000001).toPrecision(1);"),
            "var x = '0.000001';"
        );
        assert_eq!(
            deobfuscate("var x = (123456789).toPrecision(4);"),
            "var x = '1.235e+8';"
        );
        assert_eq!(
            deobfuscate("var x = (9.9999).toPrecision(1);"),
            "var x = '1e+1';"
        );
        assert_eq!(
            deobfuscate("var x = (Infinity).toPrecision(3);"),
            "var x = 'Infinity';"
        );
        assert_eq!(
            deobfuscate("var x = (-Infinity).toPrecision(3);"),
            "var x = '-Infinity';"
        );
        assert_eq!(
            deobfuscate("var x = (1.5).toPrecision(0);"),
            "var x = 1.5.toPrecision(0);"
        );
        assert_eq!(
            deobfuscate("var x = (1.5).toPrecision(101);"),
            "var x = 1.5.toPrecision(101);"
        );
    }

    #[test]
    fn test_builtin_to_fixed() {
        assert_eq!(deobfuscate("var x = 12.3456.toFixed();"), "var x = '12';");
        assert_eq!(deobfuscate("var x = (1.005).toFixed();"), "var x = '1';");
        assert_eq!(
            deobfuscate("var x = 12.3456.toFixed(2);"),
            "var x = '12.35';"
        );
        assert_eq!(
            deobfuscate("var x = 12.3456.toFixed(4);"),
            "var x = '12.3456';"
        );
        assert_eq!(
            deobfuscate("var x = (1.5).toFixed(4);"),
            "var x = '1.5000';"
        );
        assert_eq!(deobfuscate("var x = (0).toFixed(3);"), "var x = '0.000';");
        assert_eq!(deobfuscate("var x = (1.9).toFixed(0);"), "var x = '2';");
        assert_eq!(deobfuscate("var x = (1.1).toFixed(0);"), "var x = '1';");
        assert_eq!(deobfuscate("var x = (-1.5).toFixed(0);"), "var x = '-2';");
        assert_eq!(
            deobfuscate("var x = (-12.3456).toFixed(2);"),
            "var x = '-12.35';"
        );
        assert_eq!(deobfuscate("var x = (0).toFixed(0);"), "var x = '0';");
        assert_eq!(
            deobfuscate("var x = (-0).toFixed(2);"),
            "var x = '0.00';" // -0 has no sign in output per spec step 9
        );
        assert_eq!(
            deobfuscate("var x = (0.000001).toFixed(8);"),
            "var x = '0.00000100';"
        );
        assert_eq!(
            deobfuscate("var x = (1e15).toFixed(0);"),
            "var x = '1000000000000000';"
        );
        assert_eq!(
            deobfuscate("var x = (1e21).toFixed(2);"),
            "var x = '1e+21';"
        );
        assert_eq!(
            deobfuscate("var x = (1.5e21).toFixed(0);"),
            "var x = '1.5e+21';"
        );
        assert_eq!(
            deobfuscate("var x = (Infinity).toFixed(2);"),
            "var x = 'Infinity';"
        );
        assert_eq!(
            deobfuscate("var x = (-Infinity).toFixed(2);"),
            "var x = '-Infinity';"
        );
        assert_eq!(
            deobfuscate("var x = (1.5).toFixed(-1);"),
            "var x = 1.5.toFixed(-1);"
        );
        assert_eq!(
            deobfuscate("var x = (1.5).toFixed(101);"),
            "var x = 1.5.toFixed(101);"
        );
    }

    #[test]
    fn test_builtin_to_exponential() {
        assert_eq!(
            deobfuscate("var x = (12345.6789).toExponential();"),
            "var x = '1.23456789e+4';"
        );
        assert_eq!(
            deobfuscate("var x = (100).toExponential();"),
            "var x = '1e+2';"
        );
        assert_eq!(
            deobfuscate("var x = (1.5).toExponential();"),
            "var x = '1.5e+0';"
        );
        assert_eq!(
            deobfuscate("var x = (12345.6789).toExponential(2);"),
            "var x = '1.23e+4';"
        );
        assert_eq!(
            deobfuscate("var x = (12345.6789).toExponential(6);"),
            "var x = '1.234568e+4';"
        );
        assert_eq!(
            deobfuscate("var x = (12345.6789).toExponential(0);"),
            "var x = '1e+4';"
        );
        assert_eq!(
            deobfuscate("var x = (1.5).toExponential(4);"),
            "var x = '1.5000e+0';"
        );
        assert_eq!(
            deobfuscate("var x = (0).toExponential();"),
            "var x = '0e+0';"
        );
        assert_eq!(
            deobfuscate("var x = (0).toExponential(3);"),
            "var x = '0.000e+0';"
        );
        assert_eq!(
            deobfuscate("var x = (-12345.6789).toExponential(2);"),
            "var x = '-1.23e+4';"
        );
        assert_eq!(
            deobfuscate("var x = (-0.00123).toExponential(2);"),
            "var x = '-1.23e-3';"
        );
        assert_eq!(
            deobfuscate("var x = (0.00123).toExponential(2);"),
            "var x = '1.23e-3';"
        );
        assert_eq!(
            deobfuscate("var x = (0.0000001).toExponential(1);"),
            "var x = '1.0e-7';"
        );
        assert_eq!(
            deobfuscate("var x = (5).toExponential(3);"),
            "var x = '5.000e+0';"
        );
        assert_eq!(
            deobfuscate("var x = (Infinity).toExponential();"),
            "var x = 'Infinity';"
        );
        assert_eq!(
            deobfuscate("var x = (-Infinity).toExponential(2);"),
            "var x = '-Infinity';"
        );
        assert_eq!(
            deobfuscate("var x = (1.5).toExponential(-1);"),
            "var x = 1.5.toExponential(-1);"
        );
        assert_eq!(
            deobfuscate("var x = (1.5).toExponential(101);"),
            "var x = 1.5.toExponential(101);"
        );
    }
}
