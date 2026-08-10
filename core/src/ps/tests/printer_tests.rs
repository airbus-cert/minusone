#[cfg(test)]
mod test_printer {
    use crate::printer::{Printer, PrinterMode};
    use crate::ps::build_powershell_tree;
    use crate::ps::linter::Linter;

    #[test]
    fn test_ps_printer_compact_joins_statements_with_semicolons() {
        let tree = build_powershell_tree("Write-Host \"a\"\nWrite-Host \"b\"").unwrap();
        let mut printer = Linter::default();

        let output = printer.print(&tree, PrinterMode::Compact).unwrap();

        assert_eq!(output, "Write-Host \"a\"; Write-Host \"b\"");
    }

    #[test]
    fn test_ps_printer_compact_does_not_break_if_elseif_else_chain() {
        let src = "if ($a -eq 1) { Write-Host \"one\" } elseif ($a -eq 2) { Write-Host \"two\" } else { Write-Host \"other\" }";
        let tree = build_powershell_tree(src).unwrap();
        let mut printer = Linter::default();

        let output = printer.print(&tree, PrinterMode::Compact).unwrap();

        assert!(!output.contains("; elseif"));
        assert!(!output.contains("; else"));
    }

    #[test]
    fn test_ps_printer_compact_does_not_break_try_catch_finally_chain() {
        let src =
            "try { Write-Host \"a\" } catch { Write-Host \"b\" } finally { Write-Host \"c\" }";
        let tree = build_powershell_tree(src).unwrap();
        let mut printer = Linter::default();

        let output = printer.print(&tree, PrinterMode::Compact).unwrap();

        assert!(!output.contains("; catch"));
        assert!(!output.contains("; finally"));
    }

    #[test]
    fn test_ps_printer_compact_keeps_param_list_comma_separated() {
        let src = "function f($a, $b) {\n    Write-Host $a\n}";
        let tree = build_powershell_tree(src).unwrap();
        let mut printer = Linter::default();

        let output = printer.print(&tree, PrinterMode::Compact).unwrap();

        assert!(
            output.contains("($a, $b)"),
            "params should stay comma-separated, not `;`-separated: {output}"
        );
    }

    #[test]
    fn test_ps_printer_pretty_indents_nested_blocks() {
        let src = "function f() {\nwhile ($true) {\nWrite-Host \"a\"\nbreak\n}\n}";
        let tree = build_powershell_tree(src).unwrap();
        let mut printer = Linter::default().set_tab("    ");

        let output = printer.print(&tree, PrinterMode::Pretty).unwrap();

        assert!(output.contains("\n    while"));
        assert!(output.contains("\n        Write-Host"));
    }
}
