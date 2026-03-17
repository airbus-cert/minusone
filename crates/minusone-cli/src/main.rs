extern crate clap;
extern crate clap_help;
extern crate minusone;

mod cli;
mod utils;

use crate::cli::*;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use base64::{Engine as _, alphabet};
use clap::{CommandFactory, Parser, ValueEnum};
use clap_help::Printer;
use cli::{Cli, INTRO, Language};
use log::{LevelFilter, error};
use minusone::js::backend::JavaScriptBackend;
use minusone::ps::backend::PowershellBackend;
use std::{fs, process};
use termimad::ansi;
use utils::{get_available_rules, run_deobf};

const FLEXIBLE_B64: GeneralPurpose = GeneralPurpose::new(
    &alphabet::STANDARD,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

fn main() {
    let cli = Cli::parse();
    if cli.help {
        let mut printer = Printer::new(Cli::command())
            .with("introduction", INTRO)
            .with("options", clap_help::TEMPLATE_OPTIONS_MERGED_VALUE);
        printer.template_keys_mut().push("languages");
        printer.set_template("languages", LANGUAGES_LIST_TEMPLATE);
        printer.template_keys_mut().push("examples");
        printer.set_template("examples", EXAMPLES_TEMPLATE);
        let skin = printer.skin_mut();
        skin.headers[0].compound_style.set_fg(ansi(39));
        skin.bold.set_fg(ansi(39));
        skin.italic.set_fg(ansi(39));
        for (i, example) in EXAMPLES.iter().enumerate() {
            printer
                .expander_mut()
                .sub("examples")
                .set("example-number", i + 1)
                .set("example-title", example.title)
                .set("example-cmd", example.cmd);
        }
        for language in Language::value_variants() {
            printer
                .expander_mut()
                .sub("languages")
                .set("language", language.to_string());
        }
        printer.print_help();

        return;
    }

    pretty_env_logger::formatted_builder()
        .filter(None, LevelFilter::Off)
        .filter_module("minusone", LevelFilter::from(cli.debug_level))
        .filter_module(APPLICATION_NAME, LevelFilter::Error)
        .init();

    let cli_clone = cli.clone();

    let lang = match cli.lang {
        Some(l) => l,
        None => {
            error!("No language specified. Use --lang to specify the language.");
            error!("Available languages:");
            for l in Language::value_variants() {
                error!("- {}", l.to_string());
            }
            process::exit(1);
        }
    };

    if cli.list {
        let rules = get_available_rules(lang);
        println!("Available rules for {}:", lang);
        for rule in rules {
            println!("- {}", rule);
        }

        return;
    }

    if cli.rules.is_some() && cli.skip_rules.is_some() {
        error!("Cannot use --rules and --skip-rules at the same time");
        process::exit(1);
    }

    if cli.path.is_none() && cli.input.is_none() {
        error!(
            "No file path provided. Use --path to specify the script file or --input to specify a b64 encoded script"
        );
        process::exit(1);
    }

    if cli.path.is_some() && cli.input.is_some() {
        error!("Cannot use --path and --input at the same time");
        process::exit(1);
    }

    let source = if let Some(path) = cli.path {
        fs::read_to_string(&path).unwrap_or_else(|e| {
            error!("Failed to read file {}: {}", path, e);
            process::exit(1);
        })
    } else if let Some(input) = cli.input {
        FLEXIBLE_B64
            .decode(input.as_bytes())
            .map_err(|e| {
                error!("Failed to decode base64 input: {}", e);
                process::exit(1);
            })
            .and_then(|bytes| {
                String::from_utf8(bytes).map_err(|e| {
                    error!("Decoded base64 is not valid UTF-8: {}", e);
                    process::exit(1);
                })
            })
            .unwrap()
    } else {
        unreachable!()
    };

    let rule_set = cli
        .rules
        .map(|vals| vals.into_iter().map(|s| s.to_lowercase()).collect());

    let skip_rule_set = cli
        .skip_rules
        .map(|vals| vals.into_iter().map(|s| s.to_lowercase()).collect());

    let now = std::time::Instant::now();

    let result = match lang {
        Language::Powershell => {
            run_deobf::<PowershellBackend>(&source, cli_clone, rule_set, skip_rule_set)
        }
        Language::Javascript => {
            run_deobf::<JavaScriptBackend>(&source, cli_clone, rule_set, skip_rule_set)
        }
    };

    if cli.time {
        let elapsed = now.elapsed();
        println!("\n\nDeobfuscation time: {:.2?}", elapsed);
    }

    if let Err(e) = result {
        error!("{:?}", e);
        process::exit(1);
    }
}
