use std::marker::PhantomData;

use debug::DebugView;
use error::MinusOneResult;
use init::Init;
use tree::{EmptyStorage, HashMapStorage, Storage, Tree};

use ps::{
    self, build_powershell_tree_for_storage, linter::RemoveUnusedVars, remove_powershell_extra,
};

pub struct Engine<'a, L, S: Storage + Default> {
    root: Tree<'a, S>,
    _phantom: PhantomData<L>,
}

pub type PowershellEngine<'a, S> = Engine<'a, ps::Powershell, S>;
impl<'a, S: Storage + Default> PowershellEngine<'a, S> {
    pub fn new(src: &'a str) -> MinusOneResult<Self> {
        Ok(Self {
            root: build_powershell_tree_for_storage(src)?,
            _phantom: PhantomData,
        })
    }
}

pub struct DeobfuscatePowershellEngine<'a>(
    pub PowershellEngine<'a, HashMapStorage<ps::Powershell>>,
);

impl<'a> DeobfuscatePowershellEngine<'a> {
    pub fn debug(&self) {
        let mut debub_view = DebugView::new();
        self.0.root.apply(&mut debub_view).unwrap();
    }

    pub fn deobfuscate(&mut self) -> MinusOneResult<()> {
        self.0
            .root
            .apply_mut_with_strategy(&mut ps::RuleSet::init(), ps::strategy::PowershellStrategy)
    }

    pub fn lint(&mut self) -> MinusOneResult<String> {
        let mut ps_litter_view = ps::linter::Linter::new();
        self.0.root.apply(&mut ps_litter_view)?;
        CleanPowershellEngine(PowershellEngine::new(&ps_litter_view.output)?).clean_output()
    }

    pub fn lint_format(&mut self, tab_chr: &str) -> MinusOneResult<String> {
        let mut ps_litter_view = ps::linter::Linter::new().set_tab(tab_chr);
        self.0.root.apply(&mut ps_litter_view)?;
        CleanPowershellEngine(PowershellEngine::new(&ps_litter_view.output)?).clean_output()
    }
}

pub struct CleanPowershellEngine<'a>(PowershellEngine<'a, EmptyStorage>);

impl<'a> CleanPowershellEngine<'a> {
    pub fn clean_source(src: &'a str) -> MinusOneResult<String> {
        remove_powershell_extra(src)
    }

    pub fn clean_output(&mut self) -> MinusOneResult<String> {
        let mut rule = ps::var::UnusedVars::default();
        self.0.root.apply(&mut rule)?;
        let mut clean_view = RemoveUnusedVars::new(rule);
        self.0.root.apply(&mut clean_view)?;
        clean_view.clear()
    }
}
