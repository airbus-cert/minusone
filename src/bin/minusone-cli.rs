extern crate clap;
extern crate tree_sitter_powershell;
extern crate minusone;

use std::fs;
use clap::{Arg, App};
use minusone::engine::{DeobfuscateEngine, DetectionEngine};

const APPLICATION_NAME: &str = "minusone-cli";


fn main() {
    let matches = App::new(APPLICATION_NAME)
        .version("0.1.0")
        .author("Airbus CERT <cert@airbus.com>")
        .about("A script deobfuscator")
        .arg(Arg::with_name("path")
                 .long("path")
                 .takes_value(true)
                 .help("Path to the script file"))
        .arg(Arg::with_name("debug")
                 .long("debug")
                 .help("Print the tree-sitter tree with inferred value on each node"))
        .arg(Arg::with_name("detect")
                 .long("detect")
                 .help("Detection mode of obfuscated pattern"))
        .arg(Arg::with_name("strategy")
                 .long("strategy")
                 .help("Use branch strategy : May cause stack overflow"))
        .get_matches();


    let source = fs::read_to_string(matches.value_of("path").expect("Path arguments is mandatory")).unwrap();

    if matches.is_present("detect") {
        let mut engine = DetectionEngine::from_powershell(&source).unwrap();
        let detected_nodes = engine.detect().unwrap();

        if matches.is_present("debug") {
            engine.debug();
        }
        else {
            println!("{}", serde_json::to_string(&detected_nodes).unwrap());
        }
    }
    else {
        let mut engine = DeobfuscateEngine::from_powershell(&source).unwrap();
        if matches.is_present("strategy") {
            engine.deobfuscate_with_strategy().unwrap();
        }
        else {
            engine.deobfuscate().unwrap();
        }

        if matches.is_present("debug") {
            engine.debug();
        }
        else {
            println!("{}", engine.lint().unwrap());
        }
    }
}