extern crate clap;
extern crate tree_sitter;
extern crate tree_sitter_powershell;

use clap::{Arg, App, ArgMatches};
use tree_sitter::{Parser, Language};
use tree_sitter_powershell::language as powershell_language;

const APPLICATION_NAME: &str = "minusone-cli";

fn main() {
    let matches = App::new(APPLICATION_NAME)
        .version("0.1.0")
        .author("Sylvain Peyrefitte <citronneur@gmail.com>")
        .about("A script deobfuscator")
        .arg(Arg::with_name("path")
                 .long("path")
                 .takes_value(true)
                 .help("Path to the script file"));

    let mut parser = Parser::new();
    parser.set_language(powershell_language()).unwrap();

    let tree = parser.parse("foo", None).unwrap();
    println!("{}", tree.root_node().to_sexp());
}