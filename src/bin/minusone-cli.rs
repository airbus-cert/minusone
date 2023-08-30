extern crate clap;
extern crate tree_sitter;
extern crate tree_sitter_powershell;
extern crate minusone;

use std::fs;
use clap::{Arg, App};
use minusone::debug::DebugView;
use minusone::ps::{InferredValueRules, from_powershell_src};

const APPLICATION_NAME: &str = "minusone-cli";


fn main() {
    let matches = App::new(APPLICATION_NAME)
        .version("0.1.0")
        .author("Sylvain Peyrefitte <citronneur@gmail.com>")
        .about("A script deobfuscator")
        .arg(Arg::with_name("path")
                 .long("path")
                 .takes_value(true)
                 .help("Path to the script file"))
        .get_matches();


    let source = fs::read_to_string(matches.value_of("path").expect("Path arguments is mandatory")).unwrap();
    let mut tree = from_powershell_src(source.as_str()).unwrap();
    tree.apply_mut(InferredValueRules::default()).unwrap();

    let debub_view = DebugView::new();
    tree.apply(debub_view).unwrap();
}