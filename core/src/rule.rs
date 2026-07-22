use crate::error::MinusOneResult;
use crate::tree::{ControlFlow, Node, NodeMut};
use log::warn;

pub trait RuleMut<'a> {
    type Language;
    fn enter(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        flow: ControlFlow,
    ) -> MinusOneResult<()>;
    fn leave(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        flow: ControlFlow,
    ) -> MinusOneResult<()>;
}

/// Rule that will not change the node component
/// Use for displaying or statistic
/// The top down exploring is handling by the enter function
/// te down to top exploring is handling by the leave function
pub trait Rule<'a> {
    type Language;
    fn enter(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<bool>;
    fn leave(&mut self, node: &Node<'a, Self::Language>) -> MinusOneResult<()>;
}

pub struct RuleSet<'a, T> {
    // keeps the original (non-lowercased) rule name alongside each rule,
    // so callers can report which rule fired without re-deriving casing
    rules: Vec<(&'a str, Box<dyn RuleMut<'a, Language = T>>)>,
}

impl<'a, T> RuleSet<'a, T> {
    pub fn new(
        full_ruleset: Vec<(&'a str, Box<dyn RuleMut<'a, Language = T>>)>,
        ctx: RuleSetBuilderType,
    ) -> Self {
        let low_names: Vec<String> = full_ruleset.iter().map(|(n, _)| n.to_lowercase()).collect();

        let low_input: Vec<String> = match &ctx {
            RuleSetBuilderType::WithRules(r) | RuleSetBuilderType::WithoutRules(r) => {
                r.iter().map(|s| s.to_lowercase()).collect()
            }
        };

        for rule in &low_input {
            if !low_names.contains(rule) {
                warn!("Unknown rule: '{}', skipping", rule);
            }
        }

        // delete unknown rules
        let low_input: Vec<String> = low_input
            .into_iter()
            .filter(|s| low_names.contains(s))
            .collect();

        Self {
            rules: full_ruleset
                .into_iter()
                .zip(low_names)
                .filter(|(_, low_name)| match &ctx {
                    RuleSetBuilderType::WithRules(_) => low_input.contains(low_name),
                    RuleSetBuilderType::WithoutRules(_) => !low_input.contains(low_name),
                })
                .map(|((name, rule), _)| (name, rule))
                .collect(),
        }
    }
}

pub enum RuleSetBuilderType<'a> {
    WithRules(Vec<&'a str>),
    WithoutRules(Vec<&'a str>),
}

impl<'a, T> RuleMut<'a> for RuleSet<'a, T> {
    type Language = T;

    fn enter(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        flow: ControlFlow,
    ) -> MinusOneResult<()> {
        self.rules
            .iter_mut()
            .try_for_each(|(_, r)| r.enter(node, flow))
    }

    fn leave(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        flow: ControlFlow,
    ) -> MinusOneResult<()> {
        self.rules
            .iter_mut()
            .try_for_each(|(_, r)| r.leave(node, flow))
    }
}

impl<'a, T: Clone + PartialEq> RuleSet<'a, T> {
    /// Like `leave`, but calls back `on_change` with the name of any rule
    /// that just altered the current node.
    pub fn leave_traced(
        &mut self,
        node: &mut NodeMut<'a, T>,
        flow: ControlFlow,
        mut on_change: impl FnMut(&mut NodeMut<'a, T>, &'a str) -> MinusOneResult<()>,
    ) -> MinusOneResult<()> {
        for (name, rule) in self.rules.iter_mut() {
            let before = node.view().data().cloned();
            rule.leave(node, flow)?;
            let after = node.view().data().cloned();

            let changed = match (&before, &after) {
                (None, Some(_)) => true,
                (Some(a), Some(b)) => a != b,
                _ => false,
            };

            if changed {
                on_change(node, name)?;
            }
        }
        Ok(())
    }
}

macro_rules! impl_data {
    ( $($ty:ident),* ) => {
        impl<'a, Data, $($ty),*> RuleMut<'a> for ( $( $ty , )* )
            where $( $ty : RuleMut<'a, Language=Data>),*
            {
                type Language = Data;

                fn enter(&mut self, node : &mut NodeMut<'a, Self::Language>, flow: ControlFlow) -> MinusOneResult<()>{
                    $(
                        ${ignore($ty)}
                        self.${index()}.enter(node, flow)?;
                    )*
                    Ok(())
                }

                fn leave(&mut self, node : &mut NodeMut<'a, Self::Language>, flow: ControlFlow) -> MinusOneResult<()>{
                    $(
                        ${ignore($ty)}
                        self.${index()}.leave(node, flow)?;
                    )*
                    Ok(())
                }
            }
    };
}

mod impl_data {
    #![cfg_attr(rustfmt, rustfmt_skip)]

    use super::*;

    impl_data!(A);
    impl_data!(A, B);
    impl_data!(A, B, C);
    impl_data!(A, B, C, D);
    impl_data!(A, B, C, D, E);
    impl_data!(A, B, C, D, E, F);
    impl_data!(A, B, C, D, E, F, G);
    impl_data!(A, B, C, D, E, F, G, H);
    impl_data!(A, B, C, D, E, F, G, H, I);
    impl_data!(A, B, C, D, E, F, G, H, I, J);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC, AD);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC, AD, AE);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC, AD, AE, AF);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC, AD, AE, AF, AG);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC, AD, AE, AF, AG, AH);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC, AD, AE, AF, AG, AH, AI);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC, AD, AE, AF, AG, AH, AI, AJ);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC, AD, AE, AF, AG, AH, AI, AJ, AK);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC, AD, AE, AF, AG, AH, AI, AJ, AK, AL);
    impl_data!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC, AD, AE, AF, AG, AH, AI, AJ, AK, AL, AM);
}
