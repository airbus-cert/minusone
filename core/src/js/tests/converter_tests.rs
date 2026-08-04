#[cfg(test)]
mod tests_converter {
    use crate::js::converter::format_f64_to_string;

    #[test]
    fn test_format_f64_to_string() {
        // Big numbers
        assert_eq!(
            "100000000000000000000",
            format_f64_to_string(100000000000000000000.0)
        );
        assert_eq!("1e+21", format_f64_to_string(1000000000000000000000.0));
        assert_eq!(
            "1e+21",
            format_f64_to_string(1000000000000000000000.99999999)
        );
        assert_eq!(
            "1.234567898123456e+32",
            format_f64_to_string(123456789812345600900000000000001.99999999)
        );

        // Small numbers
        assert_eq!("0", format_f64_to_string(0.0));
        assert_eq!("0.000001", format_f64_to_string(0.000001));
        assert_eq!("1e-7", format_f64_to_string(0.0000001));
        assert_eq!("9.9999999e-7", format_f64_to_string(0.00000099999999));
        assert_eq!(
            "1.234567898123456e-32",
            format_f64_to_string(0.00000000000000000000000000000001234567898123456)
        );
    }
}
