extern crate clap;
extern crate tree_sitter;
extern crate tree_sitter_powershell;
extern crate minusone;

use clap::{Arg, App, ArgMatches};
use tree_sitter::{Parser, Language};
use tree_sitter_powershell::language as powershell_language;
use minusone::core::rule::{RuleMut, Rule};
use minusone::ps::inferred::InferredType;
use minusone::core::tree::{NodeMut, Visit, Storage, VisitMut, HashMapStorage, Tree};
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

    let mut t = Tree::<HashMapStorage<InferredType>>::new(tree.root_node());
    let mut rules = (ParseInt::default(), Forward::default());
    t.apply_mut(rules);

    let mut debub_view = DebugView::new();
    t.apply(debub_view);
}