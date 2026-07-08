#[cfg(test)]
mod test_maths {
    use crate::js::array::ParseArray;
    use crate::js::build_javascript_tree;
    use crate::js::integer::{AddInt, MultDivMod, ParseInt, PosNeg};
    use crate::js::linter::Linter;
    use crate::js::math::MathBuiltins;
    use crate::js::objects::object::ObjectField;
    use crate::js::specials::ParseSpecials;
    use crate::js::string::ParseString;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseString::default(),
            ParseArray::default(),
            ParseSpecials::default(),
            PosNeg::default(),
            AddInt::default(),
            MultDivMod::default(),
            ObjectField::default(),
            MathBuiltins::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    // Absolute value
    #[test]
    fn test_math_abs() {
        assert_eq!(deobfuscate("Math.abs(-5)"), "5");
        assert_eq!(deobfuscate("Math.abs(3)"), "3");
        assert_eq!(deobfuscate("Math.abs(0)"), "0");
        assert_eq!(deobfuscate("Math.abs(-0)"), "0");
        assert_eq!(deobfuscate("Math.abs(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.abs()"), "NaN");
        assert_eq!(deobfuscate("Math.abs(null)"), "0");
    }

    // Angles
    #[test]
    fn test_math_acos() {
        assert_eq!(deobfuscate("Math.acos(1)"), "0");
        assert_eq!(deobfuscate("Math.acos(0)"), "1.5707963267948966");
        assert_eq!(deobfuscate("Math.acos(-1)"), "3.141592653589793");
        assert_eq!(deobfuscate("Math.acos(2)"), "NaN");
        assert_eq!(deobfuscate("Math.acos(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.acos()"), "NaN");
    }

    #[test]
    fn test_math_asin() {
        assert_eq!(deobfuscate("Math.asin(0)"), "0");
        assert_eq!(deobfuscate("Math.asin(1)"), "1.5707963267948966");
        assert_eq!(deobfuscate("Math.asin(-1)"), "-1.5707963267948966");
        assert_eq!(deobfuscate("Math.asin(2)"), "NaN");
        assert_eq!(deobfuscate("Math.asin(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.asin()"), "NaN");
    }

    #[test]
    fn test_math_atan() {
        assert_eq!(deobfuscate("Math.atan(0)"), "0");
        assert_eq!(deobfuscate("Math.atan(1)"), "0.7853981633974483");
        assert_eq!(deobfuscate("Math.atan(-1)"), "-0.7853981633974483");
        assert_eq!(deobfuscate("Math.atan(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.atan()"), "NaN");
    }

    #[test]
    fn test_math_atan2() {
        assert_eq!(deobfuscate("Math.atan2(0, 0)"), "0");
        assert_eq!(deobfuscate("Math.atan2(1, 0)"), "1.5707963267948966");
        assert_eq!(deobfuscate("Math.atan2(0, 1)"), "0");
        assert_eq!(deobfuscate("Math.atan2(-1, 0)"), "-1.5707963267948966");
        assert_eq!(deobfuscate("Math.atan2(0, -1)"), "3.141592653589793");
        assert_eq!(deobfuscate("Math.atan2(NaN, 1)"), "NaN");
        assert_eq!(deobfuscate("Math.atan2(1, NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.atan2()"), "NaN");
    }

    #[test]
    fn test_math_cos() {
        assert_eq!(deobfuscate("Math.cos(0)"), "1");
        assert_eq!(deobfuscate("Math.cos(Math.PI)"), "-1");
        assert_eq!(
            deobfuscate("Math.cos(Math.PI / 2)"),
            "6.123233995736766e-17"
        );
        assert_eq!(
            deobfuscate("Math.cos(-Math.PI / 2)"),
            "6.123233995736766e-17"
        );
        assert_eq!(deobfuscate("Math.cos(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.cos()"), "NaN");
    }

    #[test]
    fn test_math_sin() {
        assert_eq!(deobfuscate("Math.sin(0)"), "0");
        assert_eq!(deobfuscate("Math.sin(Math.PI / 2)"), "1");
        assert_eq!(deobfuscate("Math.sin(-Math.PI / 2)"), "-1");
        assert_eq!(deobfuscate("Math.sin(Math.PI)"), "1.2246467991473532e-16");
        assert_eq!(deobfuscate("Math.sin(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.sin()"), "NaN");
    }

    #[test]
    fn test_math_tan() {
        assert_eq!(deobfuscate("Math.tan(0)"), "0");
        assert_eq!(deobfuscate("Math.tan(Math.PI / 4)"), "0.9999999999999999");
        assert_eq!(deobfuscate("Math.tan(-Math.PI / 4)"), "-0.9999999999999999");
        assert_eq!(deobfuscate("Math.tan(Math.PI)"), "-1.2246467991473532e-16");
        assert_eq!(deobfuscate("Math.tan(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.tan()"), "NaN");
    }

    // Roots
    #[test]
    fn test_math_cbrt() {
        assert_eq!(deobfuscate("Math.cbrt(27)"), "3");
        assert_eq!(deobfuscate("Math.cbrt(-8)"), "-2");
        assert_eq!(deobfuscate("Math.cbrt(0)"), "0");
        assert_eq!(deobfuscate("Math.cbrt(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.cbrt()"), "NaN");
    }

    #[test]
    fn test_math_sqrt() {
        assert_eq!(deobfuscate("Math.sqrt(16)"), "4");
        assert_eq!(deobfuscate("Math.sqrt(0)"), "0");
        assert_eq!(deobfuscate("Math.sqrt(-1)"), "NaN");
        assert_eq!(deobfuscate("Math.sqrt(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.sqrt()"), "NaN");
    }

    // Rounding
    #[test]
    fn test_math_ceil() {
        assert_eq!(deobfuscate("Math.ceil(3.2)"), "4");
        assert_eq!(deobfuscate("Math.ceil(-3.2)"), "-3");
        assert_eq!(deobfuscate("Math.ceil(0)"), "0");
        assert_eq!(deobfuscate("Math.ceil(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.ceil()"), "NaN");
    }

    #[test]
    fn test_math_f16round() {
        assert_eq!(deobfuscate("Math.f16round(5.5)"), "5.5");
        assert_eq!(deobfuscate("Math.f16round(5.05)"), "5.05078125");
        assert_eq!(deobfuscate("Math.f16round(5)"), "5");
        assert_eq!(deobfuscate("Math.f16round(-5.05)"), "-5.05078125");
        assert_eq!(deobfuscate("Math.f16round(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.f16round()"), "NaN");
    }

    #[test]
    fn test_math_floor() {
        assert_eq!(deobfuscate("Math.floor(3.2)"), "3");
        assert_eq!(deobfuscate("Math.floor(-3.2)"), "-4");
        assert_eq!(deobfuscate("Math.floor(0)"), "0");
        assert_eq!(deobfuscate("Math.floor(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.floor()"), "NaN");
    }

    #[test]
    fn test_math_fround() {
        assert_eq!(deobfuscate("Math.fround(5.5)"), "5.5");
        assert_eq!(deobfuscate("Math.fround(5.05)"), "5.050000190734863");
        assert_eq!(deobfuscate("Math.fround(5)"), "5");
        assert_eq!(deobfuscate("Math.fround(-5.05)"), "-5.050000190734863");
        assert_eq!(deobfuscate("Math.fround(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.fround()"), "NaN");
    }

    #[test]
    fn test_math_round() {
        assert_eq!(deobfuscate("Math.round(3.5)"), "4");
        assert_eq!(deobfuscate("Math.round(3.2)"), "3");
        assert_eq!(deobfuscate("Math.round(-3.5)"), "-3");
        assert_eq!(deobfuscate("Math.round(-3.2)"), "-3");
        assert_eq!(deobfuscate("Math.round(0)"), "0");
        assert_eq!(deobfuscate("Math.round(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.round()"), "NaN");
    }

    #[test]
    fn test_math_trunc() {
        assert_eq!(deobfuscate("Math.trunc(3.5)"), "3");
        assert_eq!(deobfuscate("Math.trunc(3.2)"), "3");
        assert_eq!(deobfuscate("Math.trunc(-3.5)"), "-3");
        assert_eq!(deobfuscate("Math.trunc(-3.2)"), "-3");
        assert_eq!(deobfuscate("Math.trunc(0)"), "0");
        assert_eq!(deobfuscate("Math.trunc(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.trunc()"), "NaN");
    }

    // Log
    #[test]
    fn test_math_log() {
        assert_eq!(deobfuscate("Math.log(1)"), "0");
        assert_eq!(deobfuscate("Math.log(Math.E)"), "1");
        assert_eq!(deobfuscate("Math.log(0)"), "-Infinity");
        assert_eq!(deobfuscate("Math.log(-1)"), "NaN");
        assert_eq!(deobfuscate("Math.log(8) / Math.log(2)"), "3");
        assert_eq!(deobfuscate("Math.log(625) / Math.log(5)"), "4");
        assert_eq!(deobfuscate("Math.log(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.log()"), "NaN");
    }

    #[test]
    fn test_math_log1p() {
        assert_eq!(deobfuscate("Math.log1p(0)"), "0");
        assert_eq!(deobfuscate("Math.log1p(Math.E - 1)"), "1");
        assert_eq!(deobfuscate("Math.log1p(-1)"), "-Infinity");
        assert_eq!(deobfuscate("Math.log1p(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.log1p()"), "NaN");
    }

    #[test]
    fn test_math_log2() {
        assert_eq!(deobfuscate("Math.log2(3)"), "1.584962500721156");
        assert_eq!(deobfuscate("Math.log2(2)"), "1");
        assert_eq!(deobfuscate("Math.log2(1)"), "0");
        assert_eq!(deobfuscate("Math.log2(0)"), "-Infinity");
        assert_eq!(deobfuscate("Math.log2(-1)"), "NaN");
        assert_eq!(deobfuscate("Math.log2(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.log2()"), "NaN");
    }

    #[test]
    fn test_math_log10() {
        assert_eq!(deobfuscate("Math.log10(100000)"), "5");
        assert_eq!(deobfuscate("Math.log10(2)"), "0.3010299956639812");
        assert_eq!(deobfuscate("Math.log10(1)"), "0");
        assert_eq!(deobfuscate("Math.log10(0)"), "-Infinity");
        assert_eq!(deobfuscate("Math.log10(-1)"), "NaN");
        assert_eq!(deobfuscate("Math.log10(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.log10()"), "NaN");
    }

    // Exponential
    #[test]
    fn test_math_exp() {
        assert_eq!(deobfuscate("Math.exp(0)"), "1");
        assert_eq!(deobfuscate("Math.exp(1)"), "2.718281828459045");
        assert_eq!(deobfuscate("Math.exp(-1)"), "0.36787944117144233");
        assert_eq!(deobfuscate("Math.exp(-Infinity)"), "0");
        assert_eq!(deobfuscate("Math.exp(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.exp()"), "NaN");
    }

    #[test]
    fn test_math_expm1() {
        assert_eq!(deobfuscate("Math.expm1(0)"), "0");
        assert_eq!(deobfuscate("Math.expm1(1)"), "1.718281828459045");
        assert_eq!(deobfuscate("Math.expm1(-1)"), "-0.6321205588285577");
        assert_eq!(deobfuscate("Math.expm1(-Infinity)"), "-1");
        assert_eq!(deobfuscate("Math.expm1(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.expm1()"), "NaN");
    }

    // Min/max
    #[test]
    fn test_math_min() {
        assert_eq!(deobfuscate("Math.min(1, [2], 3)"), "1");
        assert_eq!(deobfuscate("Math.min(3, [2], 1)"), "1");
        assert_eq!(deobfuscate("Math.min(1, 2, NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.min(1, 2)"), "1");
        assert_eq!(deobfuscate("Math.min(1)"), "1");
        assert_eq!(deobfuscate("Math.min()"), "Infinity");
    }

    #[test]
    fn test_math_max() {
        assert_eq!(deobfuscate("Math.max(1, 2, 3)"), "3");
        assert_eq!(deobfuscate("Math.max(3, 2, 1)"), "3");
        assert_eq!(deobfuscate("Math.max(1, 2, NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.max(1, 2)"), "2");
        assert_eq!(deobfuscate("Math.max(1)"), "1");
        assert_eq!(deobfuscate("Math.max()"), "-Infinity");
    }

    // Power
    #[test]
    fn test_math_pow() {
        assert_eq!(deobfuscate("Math.pow(2, 3)"), "8");
        assert_eq!(deobfuscate("Math.pow(5, 0)"), "1");
        assert_eq!(deobfuscate("Math.pow(2, -1)"), "0.5");
        assert_eq!(deobfuscate("Math.pow(-2, 3)"), "-8");
        assert_eq!(deobfuscate("Math.pow(-2, 2)"), "4");
        assert_eq!(deobfuscate("Math.pow(-2, 0.5)"), "NaN");
        assert_eq!(deobfuscate("Math.pow(NaN, 2)"), "NaN");
        assert_eq!(deobfuscate("Math.pow(2, NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.pow()"), "NaN");
    }

    // Count leading zeros
    #[test]
    fn test_math_clz32() {
        assert_eq!(deobfuscate("Math.clz32(0)"), "32");
        assert_eq!(deobfuscate("Math.clz32(1)"), "31");
        assert_eq!(deobfuscate("Math.clz32(2)"), "30");
        assert_eq!(deobfuscate("Math.clz32(3)"), "30");
        assert_eq!(deobfuscate("Math.clz32(4)"), "29");
        assert_eq!(deobfuscate("Math.clz32(1024)"), "21");
        assert_eq!(deobfuscate("Math.clz32(4294967295)"), "0");
        assert_eq!(deobfuscate("Math.clz32(-1)"), "0");
        assert_eq!(deobfuscate("Math.clz32(NaN)"), "32");
        assert_eq!(deobfuscate("Math.clz32()"), "32");
        assert_eq!(deobfuscate("Math.clz32(Infinity)"), "32");
        assert_eq!(deobfuscate("Math.clz32(-Infinity)"), "32");
    }

    // Hyperbolic functions
    #[test]
    fn test_math_cosh() {
        assert_eq!(deobfuscate("Math.cosh(0)"), "1");
        assert_eq!(deobfuscate("Math.cosh(1)"), "1.5430806348152437");
        assert_eq!(deobfuscate("Math.cosh(-1)"), "1.5430806348152437");
        assert_eq!(deobfuscate("Math.cosh(2)"), "3.7621956910836314");
        assert_eq!(deobfuscate("Math.cosh(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.cosh()"), "NaN");
    }

    #[test]
    fn test_math_sinh() {
        assert_eq!(deobfuscate("Math.sinh(0)"), "0");
        assert_eq!(deobfuscate("Math.sinh(1)"), "1.1752011936438014");
        assert_eq!(deobfuscate("Math.sinh(-1)"), "-1.1752011936438014");
        assert_eq!(deobfuscate("Math.sinh(2)"), "3.626860407847019");
        assert_eq!(deobfuscate("Math.sinh(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.sinh()"), "NaN");
    }

    #[test]
    fn test_math_tanh() {
        assert_eq!(deobfuscate("Math.tanh(0)"), "0");
        assert_eq!(deobfuscate("Math.tanh(1)"), "0.7615941559557649");
        assert_eq!(deobfuscate("Math.tanh(-1)"), "-0.7615941559557649");
        assert_eq!(deobfuscate("Math.tanh(2)"), "0.9640275800758169");
        assert_eq!(deobfuscate("Math.tanh(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.tanh()"), "NaN");
    }

    #[test]
    fn test_math_acosh() {
        assert_eq!(deobfuscate("Math.acosh(1)"), "0");
        assert_eq!(deobfuscate("Math.acosh(2)"), "1.3169578969248166");
        assert_eq!(deobfuscate("Math.acosh(3)"), "1.762747174039086");
        assert_eq!(deobfuscate("Math.acosh(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.acosh(-1)"), "NaN");
    }

    #[test]
    fn test_math_asinh() {
        assert_eq!(deobfuscate("Math.asinh(0)"), "0");
        assert_eq!(deobfuscate("Math.asinh(1)"), "0.881373587019543");
        assert_eq!(deobfuscate("Math.asinh(-1)"), "-0.881373587019543");
        assert_eq!(deobfuscate("Math.asinh(2)"), "1.4436354751788103");
        assert_eq!(deobfuscate("Math.asinh(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.asinh()"), "NaN");
    }

    #[test]
    fn test_math_atanh() {
        assert_eq!(deobfuscate("Math.atanh(0)"), "0");
        assert_eq!(deobfuscate("Math.atanh(0.5)"), "0.5493061443340548");
        assert_eq!(deobfuscate("Math.atanh(-0.5)"), "-0.5493061443340548");
        assert_eq!(deobfuscate("Math.atanh(1)"), "Infinity");
        assert_eq!(deobfuscate("Math.atanh(-1)"), "-Infinity");
        assert_eq!(deobfuscate("Math.atanh(2)"), "NaN");
        assert_eq!(deobfuscate("Math.atanh(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.atanh()"), "NaN");
    }

    // Hypotenuse
    #[test]
    fn test_math_hypot() {
        assert_eq!(deobfuscate("Math.hypot(3, 4)"), "5");
        assert_eq!(deobfuscate("Math.hypot(5, 12)"), "13");
        assert_eq!(deobfuscate("Math.hypot(3, 4, 5)"), "7.0710678118654755");
        assert_eq!(deobfuscate("Math.hypot(-5)"), "5");
        assert_eq!(deobfuscate("Math.hypot()"), "0");
        assert_eq!(deobfuscate("Math.hypot(NaN)"), "NaN");
    }

    // Multiplication
    #[test]
    fn test_math_imul() {
        assert_eq!(deobfuscate("Math.imul(3, 4)"), "12");
        assert_eq!(deobfuscate("Math.imul(-5, 12)"), "-60");
        assert_eq!(deobfuscate("Math.imul(0xffffffff, 5)"), "-5");
        assert_eq!(deobfuscate("Math.imul(0xfffffffe, 5)"), "-10");
        assert_eq!(deobfuscate("Math.imul(1, 2, 3)"), "2");
        assert_eq!(deobfuscate("Math.imul(1)"), "0");
        assert_eq!(deobfuscate("Math.imul()"), "0");
    }

    // Sign
    #[test]
    fn test_math_sign() {
        assert_eq!(deobfuscate("Math.sign(3)"), "1");
        assert_eq!(deobfuscate("Math.sign(-3)"), "-1");
        assert_eq!(deobfuscate("Math.sign(0)"), "0");
        // -0 in memory, but it will print 0 because in JS the string version of -0 is 0
        assert_eq!(deobfuscate("Math.sign(-0)"), "0");
        assert_eq!(deobfuscate("Math.sign(NaN)"), "NaN");
        assert_eq!(deobfuscate("Math.sign()"), "NaN");
        assert_eq!(deobfuscate("Math.sign(Infinity)"), "1");
        assert_eq!(deobfuscate("Math.sign(-Infinity)"), "-1");
    }

    // Sum
    #[test]
    fn test_math_sum_precise() {
        // -0 in memory, but it will print 0 because in JS the string version of -0 is 0
        assert_eq!(deobfuscate("Math.sumPrecise([])"), "0");
        // same here
        assert_eq!(deobfuscate("Math.sumPrecise([-0, -0])"), "0");
        assert_eq!(deobfuscate("Math.sumPrecise([1, 2, 3])"), "6");
        assert_eq!(deobfuscate("Math.sumPrecise([1e20, 0.1, -1e20])"), "0.1");
        assert_eq!(
            deobfuscate("Math.sumPrecise([0.1, 0.2])"),
            "0.30000000000000004"
        );
        assert_eq!(deobfuscate("Math.sumPrecise([NaN, 1])"), "NaN");
        assert_eq!(deobfuscate("Math.sumPrecise([Infinity, -Infinity])"), "NaN");
        assert_eq!(deobfuscate("Math.sumPrecise([Infinity, 1])"), "Infinity");
        assert_eq!(deobfuscate("Math.sumPrecise([-Infinity])"), "-Infinity");
    }
}
