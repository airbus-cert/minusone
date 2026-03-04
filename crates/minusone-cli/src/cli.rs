use clap::{Parser, ValueEnum};
use std::fmt::Display;

const APPLICATION_NAME: &str = env!("CARGO_PKG_NAME");

#[derive(Debug, Default, Clone, ValueEnum, Copy)]
pub enum Language {
    #[default]
    Powershell,
}

pub static INTRO: &str = "
minusone (-1) is a deobfuscation tool that uses tree-sitter to parse and transform obfuscated code.
It supports multiple languages and allows users to apply custom rules for deobfuscation.
";

#[derive(Parser, Debug)]
#[clap(
    name = APPLICATION_NAME,
    author,
    version,
    about,
    disable_help_flag = true,
    disable_version_flag = true,
)]
pub struct Cli {
    /// Help information
    #[arg(long, short)]
    pub help: bool,

    /// Version information
    #[arg(short = 'v', short_alias = 'V', long, action = clap::builder::ArgAction::Version)]
    version: (),

    /// Path to the script file
    #[arg(long, short)]
    pub path: Option<String>,

    /// Debug mode: print the tree-sitter tree with inferred value on each node
    #[arg(long, short)]
    pub debug: bool,

    /// Language of the script (default: powershell)
    #[arg(long, short, value_enum, default_value_t = Language::Powershell)]
    pub lang: Language,

    /// List rules available for a language
    #[arg(long, short = 'L')]
    pub list: bool,

    /// Custom comma separated list of rules to apply for the deobfuscation (optional)
    #[arg(long, short, value_delimiter = ',')]
    pub rules: Option<Vec<String>>,

    /// Custom comma separated list of rules to skip for the deobfuscation (optional)
    #[arg(long, short = 'R', value_delimiter = ',')]
    pub skip_rules: Option<Vec<String>>,

    /// Show computation time for the deobfuscation process
    #[arg(long, short)]
    pub time: bool,
}

impl Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Language::Powershell => "powershell".to_string(),
            }
        )
    }
}
