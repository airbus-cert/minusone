use clap::{Parser, ValueEnum};
use std::fmt::Display;

pub const APPLICATION_NAME: &str = env!("CARGO_PKG_NAME");

#[derive(Debug, Clone, ValueEnum, Copy)]
pub enum Language {
    Powershell,
    Javascript,
}

pub static INTRO: &str = "
minusone (-1) is a deobfuscation tool that uses tree-sitter to parse and transform obfuscated code.
It supports multiple languages and allows users to apply custom rules for deobfuscation.
";

#[derive(Parser, Debug, Clone)]
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

    /// Script in Base64
    #[arg(long, short, value_name = "BASE64")]
    pub input: Option<String>,

    /// Debug mode: print the tree-sitter tree with inferred value on each node
    #[arg(long, short)]
    pub debug: bool,

    /// Debug indent size for the debug mode
    #[arg(long, default_value_t = 2, value_name = "INT")]
    pub debug_indent: u32,

    /// Disable text in debug nodes
    #[arg(long)]
    pub debug_no_text: bool,

    /// Disable child count in debug nodes
    #[arg(long)]
    pub debug_no_count: bool,

    /// Disable colors in debug nodes
    #[arg(long)]
    pub debug_no_colors: bool,

    /// Language of the script
    #[arg(long, short, value_enum)]
    pub lang: Option<Language>,

    /// List rules available for a language
    #[arg(long, short = 'L', alias = "ls")]
    pub list: bool,

    /// Custom comma separated list of rules to apply for the deobfuscation
    #[arg(long, short, value_delimiter = ',')]
    pub rules: Option<Vec<String>>,

    /// Custom comma separated list of rules to skip for the deobfuscation
    #[arg(long, short = 'R', value_delimiter = ',', value_name = "RULES")]
    pub skip_rules: Option<Vec<String>>,

    /// Show computation time for the deobfuscation process
    #[arg(long, short)]
    pub time: bool,

    /// Log level for the deobfuscation process
    #[arg(long, value_enum, default_value_t = LogLevel::Info, alias = "log")]
    pub log_level: LogLevel,
}

impl Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Language::Powershell => "powershell".to_string(),
                Language::Javascript => "javascript".to_string(),
            }
        )
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl From<LogLevel> for log::LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Off => log::LevelFilter::Off,
            LogLevel::Error => log::LevelFilter::Error,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Debug => log::LevelFilter::Debug,
            LogLevel::Trace => log::LevelFilter::Trace,
        }
    }
}

pub struct Example {
    pub title: &'static str,
    pub cmd: &'static str,
}

pub static EXAMPLES_TEMPLATE: &str = "
**Examples:**

${examples
*${example-number})* ${example-title}: `${example-cmd}`
}
";

pub static EXAMPLES: &[Example] = &[
    Example {
        title: "List available rules for a language",
        cmd: "minusone -l powershell -L",
    },
    Example {
        title: "Deobfuscate with all rules",
        cmd: "minusone -l powershell --path obf_scr.ps1",
    },
    Example {
        title: "Deobfuscate with a custom ruleset",
        cmd: "minusone -l powershell --path obf_scr.ps1 --rules rule1,rule2,rule3",
    },
    Example {
        title: "Deobfuscate skipping some rules",
        cmd: "minusone -l powershell --path obf_scr.ps1 --skip-rules rule1,rule2,rule3",
    },
    Example {
        title: "Deobfuscate with the maximum debug information",
        cmd: "minusone -l powershell --path obf_scr.ps1 --debug --log-level trace",
    },
    Example {
        title: "Deobfuscate from b64 input",
        cmd: "minusone -l javascript --input Y29uc29sZS5sb2coMDE3KQ",
    },
];

pub static LANGUAGES_LIST_TEMPLATE: &str = "
**Available languages:**

${languages
* ${language}
}
";
