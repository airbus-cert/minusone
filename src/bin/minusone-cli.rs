extern crate clap;
extern crate tree_sitter;
extern crate tree_sitter_powershell;
extern crate minusone;

use std::fs;
use clap::{Arg, App};
use minusone::debug::DebugView;
use minusone::ps::{RuleSet, from_powershell_src};
use minusone::ps::litter::Litter;

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
    tree.apply_mut(&mut RuleSet::default()).unwrap();

    let mut debub_view = DebugView::new();
    tree.apply(&mut debub_view).unwrap();

    let mut ps_litter_view = Litter::new();
    ps_litter_view.print(&tree.root().unwrap()).unwrap();

    println!("\nPowershell litter");
    println!("{}", ps_litter_view.output);
}