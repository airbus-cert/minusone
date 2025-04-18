extern crate clap;
extern crate minusone;
extern crate tree_sitter_powershell;

use clap::{App, Arg};
use minusone::engine::DeobfuscateEngine;
use std::{fs, process};

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
    match DeobfuscateEngine::remove_extra(&source) {
        Ok(remove_comment) => {
            let mut engine = DeobfuscateEngine::from_powershell(&remove_comment).unwrap();
            engine.deobfuscate().unwrap();

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
