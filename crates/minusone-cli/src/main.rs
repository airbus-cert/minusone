extern crate clap;
extern crate minusone;

use clap::{App, Arg};
use minusone::engine::{DeobfuscateEngine, DeobfuscationBackend};
use minusone::error::MinusOneResult;
use minusone::ps::backend::PowershellBackend;
use std::fmt::Debug;
use std::{fs, process};

const APPLICATION_NAME: &str = "minusone-cli";

fn run_deobf<B: DeobfuscationBackend>(
    source: &str,
    debug: bool,
    rule_set: Option<Vec<String>>,
    skip_rule_set: Option<Vec<String>>,
) -> MinusOneResult<()>
where
    <B as DeobfuscationBackend>::Language: Debug,
{
    let cleaned = DeobfuscateEngine::<B>::remove_extra(source)?;

    /*let mut engine = DeobfuscateEngine::<B>::from_source(&cleaned)?;
    engine.deobfuscate()?;*/

    let mut engine = DeobfuscateEngine::<B>::from_source(&cleaned)?;

    if let Some(rules) = rule_set {
        engine.deobfuscate_with_custom_ruleset(rules.iter().map(AsRef::as_ref).collect())?;
    } else if let Some(skip_rules) = skip_rule_set {
        engine
            .deobfuscate_without_custom_ruleset(skip_rules.iter().map(AsRef::as_ref).collect())?;
    } else {
        engine.deobfuscate()?;
    }

    if debug {
        engine.debug();
    } else {
        println!("{}", engine.lint()?);
    }
    Ok(())
}

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
            Arg::with_name("lang")
                .long("lang")
                .takes_value(true)
                .help("The language of the script (default: powershell)"),
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

    let debug = matches.is_present("debug");

    let rule_set = matches
        .values_of("rules")
        .map(|vals| vals.map(str::to_lowercase).collect());
    let skip_rule_set = matches
        .values_of("skip-rules")
        .map(|vals| vals.map(str::to_lowercase).collect());

    let now = std::time::Instant::now();

    let result = match matches.value_of("lang") {
        Some(lang) => match lang.to_lowercase().as_str() {
            "powershell" => run_deobf::<PowershellBackend>(&source, debug, rule_set, skip_rule_set),
            _ => {
                eprintln!("[x] ERROR: Language {} not implemented", lang);
                process::exit(1);
            }
        },
        None => run_deobf::<PowershellBackend>(&source, debug, rule_set, skip_rule_set),
    };

    if matches.is_present("time") {
        let elapsed = now.elapsed();
        println!("\n\nElapsed: {:.2?}", elapsed);
    }

    if let Err(e) = result {
        eprintln!("[x] ERROR: {:?}", e);
        process::exit(1);
    }
}
