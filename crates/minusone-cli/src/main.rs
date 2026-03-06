extern crate clap;
extern crate clap_help;
extern crate minusone;

mod cli;
mod utils;

use crate::cli::APPLICATION_NAME;
use clap::{CommandFactory, Parser, ValueEnum};
use clap_help::Printer;
use cli::{Cli, INTRO, Language};
use log::{LevelFilter, error};
use minusone::js::backend::JavaScriptBackend;
use minusone::ps::backend::PowershellBackend;
use std::{fs, process};
use utils::{get_available_rules, run_deobf};

fn main() {
    let cli = Cli::parse();
    if cli.help {
        Printer::new(Cli::command())
            .with("introduction", INTRO)
            .print_help();

        return;
    }

    pretty_env_logger::formatted_builder()
        .filter(None, LevelFilter::Off)
        .filter_module("minusone", LevelFilter::from(cli.log_level))
        .filter_module(APPLICATION_NAME, LevelFilter::Error)
        .init();

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

    let path = match cli.path {
        Some(p) => p,
        None => {
            error!("No file path provided. Use --path to specify the script file.");
            process::exit(1);
        }
    };

    let source = fs::read_to_string(&path).unwrap_or_else(|e| {
        error!("Failed to read file {}: {}", path, e);
        process::exit(1);
    });

    let debug = cli.debug;

    let rule_set = cli
        .rules
        .map(|vals| vals.into_iter().map(|s| s.to_lowercase()).collect());
    let skip_rule_set = cli
        .skip_rules
        .map(|vals| vals.into_iter().map(|s| s.to_lowercase()).collect());

    let now = std::time::Instant::now();

    let result = match lang {
        Language::Powershell => {
            run_deobf::<PowershellBackend>(&source, debug, rule_set, skip_rule_set)
        }
        Language::Javascript => {
            run_deobf::<JavaScriptBackend>(&source, debug, rule_set, skip_rule_set)
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
