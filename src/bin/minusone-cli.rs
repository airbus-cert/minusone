extern crate clap;
extern crate tree_sitter;
extern crate tree_sitter_powershell;
extern crate minusone;

use clap::{Arg, App, ArgMatches};
use tree_sitter::{Parser, Language};
use tree_sitter_powershell::language as powershell_language;
use minusone::core::rule::{RuleMut, RuleEngineMut, RuleEngine};
use minusone::ps::inferred::InferredType;
use minusone::core::tree::{ComponentHashMap, ComponentDb};
use minusone::ps::charconcat::CharConcatRule;
use minusone::core::debug::DebugView;
use minusone::ps::integer::ParseInt;
use minusone::ps::forward::Forward;

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

    let tree = parser.parse("4+5", None).unwrap();

    let mut db = ComponentHashMap::<InferredType>::new();
    db.init_from(tree.root_node());

    let mut rules = (ParseInt::default(), Forward::default());
    tree.root_node().apply_mut(&mut rules, &mut db);

    tree.root_node().apply(&mut DebugView::new(), &mut db);
}