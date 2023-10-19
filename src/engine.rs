use tree::{Tree, HashMapStorage, Storage};
use ps;
use error::MinusOneResult;
use init::Init;
use debug::DebugView;
use ps::build_powershell_tree;

pub struct Engine<'a, S: Storage> {
    root: Tree<'a, S>
}

impl<'a> Engine<'a, HashMapStorage<ps::Powershell>>  {
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
}


