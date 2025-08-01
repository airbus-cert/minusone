use debug::DebugView;
use error::MinusOneResult;
use init::Init;
use ps;
use ps::{build_powershell_tree_for_storage, remove_powershell_extra};
use ps::linter::RemoveUnusedVar;
use tree::{EmptyStorage, HashMapStorage, Storage, Tree};

pub struct Engine<'a, S: Storage> {
    root: Tree<'a, S>,
}

pub type DeobfuscateEngine<'a> = Engine<'a, HashMapStorage<ps::Powershell>>;

impl<'a> DeobfuscateEngine<'a> {
    pub fn remove_extra(src: &'a str) -> MinusOneResult<String> {
        remove_powershell_extra(src)
    }
    pub fn from_powershell(src: &'a str) -> MinusOneResult<Self> {
        Ok(Self {
            root: build_powershell_tree_for_storage(src)?,
        })
    }

    pub fn debug(&self) {
        let mut debub_view = DebugView::new();
        self.root.apply(&mut debub_view).unwrap();
    }

    pub fn deobfuscate(&mut self) -> MinusOneResult<()> {
        self.root.apply_mut_with_strategy(
            &mut ps::RuleSet::init(),
            ps::strategy::PowershellStrategy::default(),
        )?;
        Ok(())
    }

    pub fn lint(&mut self) -> MinusOneResult<String> {
        let mut ps_litter_view = ps::linter::Linter::new();
        self.root.apply(&mut ps_litter_view)?;
        Ok(CleanEngine::from_powershell(&ps_litter_view.output)?.clean()?)
    }

    pub fn lint_format(&mut self, tab_chr: &str) -> MinusOneResult<String> {
        let mut ps_litter_view = ps::linter::Linter::new().set_tab(tab_chr);
        self.root.apply(&mut ps_litter_view)?;

        Ok(CleanEngine::from_powershell(&ps_litter_view.output)?.clean()?)
    }
}

pub type CleanEngine<'a> = Engine<'a, EmptyStorage>;

impl<'a> CleanEngine<'a> {
    pub fn from_powershell(src: &'a str) -> MinusOneResult<Self> {
        Ok(Self {
            root: build_powershell_tree_for_storage(src)?,
        })
    }

    pub fn clean(&mut self) -> MinusOneResult<String> {
        let mut rule = ps::var::UnusedVar::default();
        self.root.apply(
            &mut rule,
        )?;
        let mut clean_view = RemoveUnusedVar::new(rule);
        self.root.apply(&mut clean_view)?;
        Ok(clean_view.clear()?)
    }
}
