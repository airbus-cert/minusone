extern crate clap;
extern crate tree_sitter_powershell;
extern crate minusone;

use std::fs;
use clap::{Arg, App};
use minusone::debug::DebugView;
use minusone::ps::{RuleSet, from_powershell_src};
use minusone::ps::linter::Linter;
use minusone::init::Init;
use minusone::ps::strategy::PowershellStrategy;

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
        .get_matches();


    let source = fs::read_to_string(matches.value_of("path").expect("Path arguments is mandatory")).unwrap();
    let mut tree = from_powershell_src(source.as_str()).unwrap();
    tree.apply_mut_with_strategy(&mut RuleSet::init(), PowershellStrategy::default()).unwrap();

    if matches.is_present("debug") {
        let mut debub_view = DebugView::new();
        tree.apply(&mut debub_view).unwrap();
    }
    else {
        let mut ps_litter_view = Linter::new();
        ps_litter_view.print(&tree.root().unwrap()).unwrap();
        println!("{}", ps_litter_view.output);
    }
}