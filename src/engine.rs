use tree::{Tree, HashMapStorage, Storage};
use ps;
use error::MinusOneResult;
use init::Init;
use debug::DebugView;
use ps::build_powershell_tree;
use ps::detect::Detection;

pub struct Engine<'a, S: Storage> {
    root: Tree<'a, S>
}

pub type DeobfuscateEngine<'a> = Engine<'a, HashMapStorage<ps::Powershell>>;
pub type DetectionEngine<'a> = Engine<'a, HashMapStorage<ps::detect::PowershellDetect>>;

impl<'a> DeobfuscateEngine<'a>  {
    pub fn from_powershell(src: &'a str) -> MinusOneResult<Self> {
        Ok(Self {
            root: build_powershell_tree(src)?
        })
    }

    pub fn debug(&self) {
        let mut debub_view = DebugView::new();
        self.root.apply(&mut debub_view).unwrap();
    }

    pub fn deobfuscate(mut self) -> MinusOneResult<Self> {
        self.root.apply_mut_with_strategy(&mut ps::RuleSet::init(), ps::strategy::PowershellStrategy::default())?;
        Ok(self)
    }

    pub fn lint(&mut self) -> MinusOneResult<String> {
        let mut ps_litter_view = ps::linter::Linter::new();
        ps_litter_view.print(&self.root.root()?)?;
        Ok(ps_litter_view.output)
    }

    pub fn lint_format(&mut self, tab_chr: &str) -> MinusOneResult<String> {
        let mut ps_litter_view = ps::linter::Linter::new().tab(tab_chr);
        ps_litter_view.print(&self.root.root()?)?;
        Ok(ps_litter_view.output)
    }
}

impl<'a> DetectionEngine<'a>  {
    pub fn from_powershell(src: &'a str) -> MinusOneResult<Self> {
        Ok(Self {
            root: build_powershell_tree(src)?
        })
    }

    pub fn detect(mut self) -> MinusOneResult<Self> {
        let mut detection_rule_set = ps::detect::RuleSet::init();
        self.root.apply_mut(&mut detection_rule_set)?;
        for m in detection_rule_set.1.get_nodes() {
            println!("find static array at {}:{}", m.start_offset, m.end_offset);
        }
        for m in detection_rule_set.2.get_nodes() {
            println!("find static format expression at {}:{}", m.start_offset, m.end_offset);
        }
        Ok(self)
    }

    pub fn debug(&self) {
        let mut debub_view = DebugView::new();
        self.root.apply(&mut debub_view).unwrap();
    }
}


