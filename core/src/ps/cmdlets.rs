use crate::ps::Powershell::{self, Raw};
use crate::ps::Value::Str;
use crate::regex::Regex;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use log::{trace, warn};
use std::collections::HashMap;
use std::sync::OnceLock;

// to updated the list, just run `Get-Command` in you powershell and copy the output
// after the ----- part on the second line
const WIN_CMDLETS: &str = include_str!("cmdlets/win.txt");
const UNIX_CMDLETS: &str = include_str!("cmdlets/unix.txt");

// maps lowercase cmdlet name -> original case
static CMDLET_NAMES: OnceLock<HashMap<String, String>> = OnceLock::new();

fn parse_cmdlet_names(content: &str) -> impl Iterator<Item = String> + '_ {
    content.lines().filter_map(|line| {
        let mut columns = line.split_whitespace();
        match columns.next() {
            Some("Alias") | Some("Cmdlet") | Some("Function") => {
                columns.next().map(|name| name.to_string())
            }
            _ => None,
        }
    })
}

fn cmdlet_names() -> &'static HashMap<String, String> {
    CMDLET_NAMES.get_or_init(|| {
        parse_cmdlet_names(WIN_CMDLETS)
            .chain(parse_cmdlet_names(UNIX_CMDLETS))
            .map(|name| (name.to_lowercase(), name))
            .collect()
    })
}

pub fn resolve_wildcard_cmdlet(name: &str) -> Option<String> {
    if !name.contains('*') {
        return None;
    }

    let re = Regex::new(&format!("^{}$", name.to_lowercase().replace('*', ".*"))).ok()?;
    let matches: Vec<&String> = cmdlet_names()
        .iter()
        .filter(|(lower, _)| re.is_match(lower))
        .map(|(_, original)| original)
        .collect();

    match matches.len() {
        0 => None,
        1 => Some(matches[0].clone()),
        x => {
            warn!(
                "Ambiguous wildcard cmdlet match for '{}': {} matches found",
                name, x
            );
            None
        }
    }
}

pub fn resolved_command_name(
    node: &crate::tree::Node<Powershell>,
) -> crate::error::MinusOneResult<String> {
    if let Some(Raw(Str(name))) = node.data() {
        Ok(name.to_lowercase())
    } else {
        Ok(node.text()?.to_lowercase())
    }
}

#[derive(Default)]
pub struct WildcardCmdlet;

impl<'a> RuleMut<'a> for WildcardCmdlet {
    type Language = Powershell;

    fn enter(
        &mut self,
        _node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> crate::error::MinusOneResult<()> {
        Ok(())
    }

    fn leave(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> crate::error::MinusOneResult<()> {
        let view = node.view();
        if view.kind() == "command_name"
            && let Ok(text) = view.text()
            && let Some(resolved) = resolve_wildcard_cmdlet(text)
        {
            trace!("WildcardCmdlet (L): Resolved '{}' to '{}'", text, resolved);
            node.set(Raw(Str(resolved)));
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ps::build_powershell_tree;
    use crate::ps::forward::Forward;
    use crate::ps::linter::Linter;
    use crate::ps::strategy::PowershellStrategy;

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
        let mut tree = build_powershell_tree("g*t-ch*ditem -Path C:\\foo").unwrap();
        tree.apply_mut_with_strategy(
            &mut (Forward::default(), WildcardCmdlet::default()),
            PowershellStrategy::default(),
        )
        .unwrap();

        let mut ps_litter_view = Linter::default();
        tree.apply(&mut ps_litter_view).unwrap();

        assert_eq!(ps_litter_view.output, "Get-ChildItem -Path C:\\foo");
    }
}
