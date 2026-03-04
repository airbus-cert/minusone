extern crate clap;
extern crate minusone;

use clap::{App, Arg};
use minusone::engine::DeobfuscateEngine;
use std::{fs, process};
use minusone::ps::backend::PowershellBackend;

const APPLICATION_NAME: &str = "minusone-cli";

fn main() {
    let matches = App::new(APPLICATION_NAME)
        .version("0.1.0")
        .author("Airbus CERT <cert@airbus.com>")
        .about("A script deobfuscator")
        .arg(
            Arg::with_name("path")
                .long("path")
                .takes_value(true)
                .help("Path to the script file"),
        )
        .arg(
            Arg::with_name("debug")
                .long("debug")
                .help("Print the tree-sitter tree with inferred value on each node"),
        )
        .arg(
            Arg::with_name("list")
                .long("list")
                .short("l")
                .help("List rules available for a language"),
        )
        .arg(
            Arg::with_name("rules")
                .long("rules")
                .short("r")
                .multiple(true)
                .takes_value(true)
                .help("Custom comma separated list of rules to apply for the deobfuscation"),
        )
        .arg(
            Arg::with_name("skip-rules")
                .long("skip-rules")
                .short("R")
                .multiple(true)
                .takes_value(true)
                .help("Custom comma separated list of rules to skip for the deobfuscation"),
        )
        .arg(Arg::with_name("time").long("time").help("Time computation"))
        .get_matches();

    use std::time::Instant;
    let now = Instant::now();

    if matches.is_present("list") {
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

    if matches.is_present("rules") && matches.is_present("skip-rules") {
        eprintln!("ERROR: Cannot use --rules and --skip-rules at the same time");
        process::exit(1);
    }

    let source = fs::read_to_string(
        matches
            .value_of("path")
            .expect("Path arguments is mandatory"),
    )
    .unwrap();

    // Always remove extra rule (comments) to get an accurate version of the deobfuscated scripts
    match DeobfuscateEngine::<PowershellBackend>::remove_extra(&source) {
        Ok(remove_comment) => {
            let mut engine = DeobfuscateEngine::from_powershell(&remove_comment).unwrap();

            if matches.is_present("rules") {
                // TODO: What if -r and -R specified
                let ruleset: Vec<String> = matches
                    .values_of("rules")
                    .unwrap()
                    .map(str::to_lowercase)
                    .collect();
                engine
                    .deobfuscate_with_custom_ruleset(ruleset.iter().map(AsRef::as_ref).collect())
                    .unwrap();
            }
            if matches.is_present("skip-rules") {
                let skiped_rules: Vec<String> = matches
                    .values_of("skip-rules")
                    .unwrap()
                    .map(str::to_lowercase)
                    .collect();
                engine
                    .deobfuscate_without_custom_ruleset(
                        skiped_rules.iter().map(AsRef::as_ref).collect(),
                    )
                    .unwrap();
            } else {
                engine.deobfuscate().unwrap();
            }

            if matches.is_present("debug") {
                engine.debug();
            } else {
                println!("{}", engine.lint().unwrap());
            }

            if matches.is_present("time") {
                let elapsed = now.elapsed();
                println!("\n\nElapsed: {:.2?}", elapsed);
            }
        }
        Err(e) => {
            eprintln!("[x] ERROR: Cannot clean the source\n--> {:?}", e);
            process::exit(1);
        }
    }
}
