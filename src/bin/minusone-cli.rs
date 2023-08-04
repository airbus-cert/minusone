extern crate clap;
extern crate tree_sitter;
extern crate tree_sitter_powershell;
extern crate minusone;

use clap::{Arg, App};
use tree_sitter::{Parser};
use tree_sitter_powershell::language as powershell_language;
use minusone::tree::{HashMapStorage, Tree};
use minusone::debug::DebugView;
use minusone::ps::{InferredValue, InferredValueRules};
use minusone::ps::integer::AddInt;

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

    let source = "\"4\"+\"5\"+\"\"";

    let tree = parser.parse(source, None).unwrap();

    let mut t = Tree::<HashMapStorage<InferredValue>>::new(source.as_bytes(), tree.root_node());
    t.apply_mut(InferredValueRules::default()).unwrap();

    let debub_view = DebugView::new();
    t.apply(debub_view);
}