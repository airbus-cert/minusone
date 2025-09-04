extern crate clap;
extern crate minusone;

use clap::{App, Arg};
use std::fs;

use minusone::engine::{CleanPowershellEngine, DeobfuscatePowershellEngine, PowershellEngine};
use minusone::error::exit_with_error;

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
        .arg(Arg::with_name("time").long("time").help("Time computation"))
        .get_matches();

    use std::time::Instant;
    let now = Instant::now();

    let source = fs::read_to_string(
        matches
            .value_of("path")
            .expect("Path arguments is mandatory"),
    )
    .unwrap();

    // Always remove extra rule (comments) to get an accurate version of the deobfuscated scripts
    match CleanPowershellEngine::clean_source(&source) {
        Ok(clean_source) => match PowershellEngine::new(&clean_source) {
            Ok(engine) => {
                let mut ps_engine = DeobfuscatePowershellEngine(engine);
                ps_engine.deobfuscate().unwrap();

                if matches.is_present("debug") {
                    ps_engine.debug();
                } else {
                    println!("{}", ps_engine.lint().unwrap());
                }

                if matches.is_present("time") {
                    let elapsed = now.elapsed();
                    println!("\n\nElapsed: {:.2?}", elapsed);
                }
            }
            Err(e) => exit_with_error("Cannot parse the clean source", e),
        },
        Err(e) => exit_with_error("Cannot clean the source", e),
    }
}
