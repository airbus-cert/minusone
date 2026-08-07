#[cfg(test)]
mod test_printer {
    use crate::js::build_javascript_tree;
    use crate::js::printer::JavaScriptPrinter;
    use crate::printer::{Printer, PrinterMode};

    #[test]
    fn test_js_printer_unchanged_returns_original_source() {
        let src = "function a(){return 1+2;}";
        let tree = build_javascript_tree(src).unwrap();
        let mut printer = JavaScriptPrinter::default();

        let output = printer.print(&tree, PrinterMode::Unchanged).unwrap();

        assert_eq!(output, src);
    }

    #[test]
    fn test_js_printer_compact_oneline_and_trim_spaces() {
        let tree = build_javascript_tree("const x = 1\nconst y = x + 2").unwrap();
        let mut printer = JavaScriptPrinter::default();

        let output = printer.print(&tree, PrinterMode::Compact).unwrap();

        assert_eq!(output, "const x=1;const y=x+2;");
    }

    #[test]
    fn test_js_printer_compact_adds_missing_semicolon_before_closing_brace() {
        let tree = build_javascript_tree("function a(){return 1+2}").unwrap();
        let mut printer = JavaScriptPrinter::default();

        let output = printer.print(&tree, PrinterMode::Compact).unwrap();

        assert!(output.contains("return 1+2;"));
    }

    #[test]
    fn test_js_printer_pretty_adds_basic_structure() {
        let tree = build_javascript_tree("function a(){if(true){x=1;}} ").unwrap();
        let mut printer = JavaScriptPrinter::with_indent("  ");

        let output = printer.print(&tree, PrinterMode::Pretty).unwrap();

        assert!(output.contains("function a() {"));
        assert!(output.contains("\n  if(true) {"));
        assert!(output.contains("\n    x=1;"));
    }

    #[test]
    fn test_js_printer_pretty_keeps_newline_statements_without_semicolons() {
        let src = "console.log(a(0))\nconsole.log(a(1))\nconsole.log(a(2))";
        let tree = build_javascript_tree(src).unwrap();
        let mut printer = JavaScriptPrinter::default();

        let output = printer.print(&tree, PrinterMode::Pretty).unwrap();

        assert!(output.contains("console.log(a(0));\nconsole.log(a(1));\nconsole.log(a(2));"));
    }

    #[test]
    fn test_js_printer_pretty_keeps_object_property_in_destructuring_assignment() {
        let src = "const {execSync:F}=require('x')";
        let tree = build_javascript_tree(src).unwrap();
        let mut printer = JavaScriptPrinter::default();

        let output = printer.print(&tree, PrinterMode::Pretty).unwrap();

        assert!(output.contains("execSync: F"));
        assert!(!output.contains("execSync: F;"));
    }

    #[test]
    fn test_js_printer_pretty_keeps_empty_block_inline() {
        let src = "if(a){ } function b(){} try{}catch{}";
        let tree = build_javascript_tree(src).unwrap();
        let mut printer = JavaScriptPrinter::default();

        let output = printer.print(&tree, PrinterMode::Pretty).unwrap();

        assert!(output.contains("if(a) {}"));
        assert!(output.contains("function b() {}"));
        assert!(output.contains("try {}"));
        assert!(output.contains("catch {}"));
    }

    #[test]
    fn test_js_printer_pretty_indents_nested_blocks_with_source_newlines() {
        let src =
            "function f() {\nwhile (true) {\nif (a === 1) {\nlet x = 1;\n}\nbreak;\n}\n}\nf();";
        let tree = build_javascript_tree(src).unwrap();
        let mut printer = JavaScriptPrinter::default();

        let output = printer.print(&tree, PrinterMode::Pretty).unwrap();

        assert_eq!(
            output,
            "function f() {\n    while (true) {\n        if (a === 1) {\n            let x = 1;\n        }\n        break;\n    }\n}\nf();"
        );
    }
}
