#[cfg(test)]
mod tests_ps_cmdlets {
    use crate::ps::build_powershell_tree;
    use crate::ps::cmdlets::{WildcardCmdlet, parse_cmdlet_names, resolve_wildcard_cmdlet};
    use crate::ps::forward::Forward;
    use crate::ps::linter::Linter;
    use crate::ps::strategy::PowershellStrategy;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_powershell_tree(input).unwrap();
        tree.apply_mut_with_strategy(
            &mut (Forward::default(), WildcardCmdlet::default()),
            PowershellStrategy::default(),
        )
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_parse_skips_malformed_lines() {
        let content = "Alias           Add-AppPackage                                     2.0.1.0    Appx\n\
                        hodOverride                  2.1.0.0    International\n\
                        Function        cd..\n";
        let names: Vec<String> = parse_cmdlet_names(content).collect();
        assert_eq!(names, vec!["Add-AppPackage", "cd.."]);
    }

    #[test]
    fn test_resolve_unambiguous_wildcard() {
        assert_eq!(
            resolve_wildcard_cmdlet("G*t-Ch*dItem"),
            Some("Get-ChildItem".to_string())
        );
    }

    #[test]
    fn test_resolve_ambiguous_wildcard_stays_unresolved() {
        assert_eq!(resolve_wildcard_cmdlet("Get-*"), None);
    }

    #[test]
    fn test_resolve_no_wildcard_returns_none() {
        assert_eq!(resolve_wildcard_cmdlet("Get-ChildItem"), None);
    }

    #[test]
    fn test_wildcard_cmdlet_rule_rewrites_output() {
        assert_eq!(
            deobfuscate("g*t-ch*ditem -Path C:\\foo"),
            "Get-ChildItem -Path C:\\foo"
        );
    }

    #[test]
    fn test_backtick_in_command_name() {
        assert_eq!(deobfuscate("Write-`Host \"Hi\""), "Write-Host \"Hi\"");
        assert_eq!(deobfuscate("Wr`ite`-`H`o`st \"Hi\""), "Write-Host \"Hi\"");
    }

    #[test]
    fn test_backtick_wildcard_cmdlet() {
        assert_eq!(deobfuscate("g*t-ch`*ditem"), "Get-ChildItem");
    }

    #[test]
    fn test_backtick_escape_sequence_is_not_a_cmdlet() {
        assert_eq!(deobfuscate("Wri`te-Host"), "wri`te-host");
    }
}
