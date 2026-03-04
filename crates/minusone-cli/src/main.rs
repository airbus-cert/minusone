extern crate clap;
extern crate minusone;
mod cli;
mod utils;

use clap::Parser;
use cli::{Cli, Language};
use minusone::engine::DeobfuscateEngine;
use minusone::ps::backend::PowershellBackend;
use std::{fs, process};
use utils::run_deobf;

fn main() {
    let cli = Cli::parse();
    if cli.help {
        todo!("Implement clap-help functionality");
    }

    if cli.list {
        println!(
            "Available rules:\n{}",
            DeobfuscateEngine::<PowershellBackend>::language_rules()
                .into_iter()
                .map(|s| format!("- {}", s))
                .collect::<Vec<_>>()
                .join("\n")
        );
        process::exit(0);
    }

    if cli.rules.is_some() && cli.skip_rules.is_some() {
        eprintln!("ERROR: Cannot use --rules and --skip-rules at the same time");
        process::exit(1);
    }

    let source = fs::read_to_string(&cli.path).unwrap_or_else(|e| {
        eprintln!("[x] ERROR: Failed to read file {}: {}", cli.path, e);
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

    let result = match cli.lang {
        Language::Powershell => {
            run_deobf::<PowershellBackend>(&source, debug, rule_set, skip_rule_set)
        }
    };

    if cli.time {
        let elapsed = now.elapsed();
        println!("\n\nElapsed: {:.2?}", elapsed);
    }

    if let Err(e) = result {
        eprintln!("[x] ERROR: {:?}", e);
        process::exit(1);
    }
}
