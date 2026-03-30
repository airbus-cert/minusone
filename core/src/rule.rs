use crate::error::MinusOneResult;
use crate::tree::{ControlFlow, Node, NodeMut};
use dyn_clone::DynClone;
use log::warn;
use std::any::Any;

pub struct RuleReference<'ctx, 'a, T> {
    pub name: &'ctx str,
    pub rule: &'ctx (dyn RuleMut<'a, Language = T> + 'static),
}

pub struct RuleExecutionContext<'ctx, 'a, T> {
    pub other_rules: &'ctx [RuleReference<'ctx, 'a, T>],
    pub recursion_depth: usize,
}

pub trait RuleMut<'a>: DynClone {
    type Language;

    fn active_rule_names(&self) -> Vec<String> {
        vec![]
    }

    fn enter(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        flow: ControlFlow,
    ) -> MinusOneResult<()>;

    fn enter_with_context(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        flow: ControlFlow,
        _context: &RuleExecutionContext<'_, 'a, Self::Language>,
    ) -> MinusOneResult<()> {
        self.enter(node, flow)
    }

    fn leave(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        flow: ControlFlow,
    ) -> MinusOneResult<()>;

    fn leave_with_context(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        flow: ControlFlow,
        _context: &RuleExecutionContext<'_, 'a, Self::Language>,
    ) -> MinusOneResult<()> {
        self.leave(node, flow)
    }

    fn snapshot_state(&self) -> Option<Box<dyn Any>> {
        None
    }

    fn restore_state(&mut self, _snapshot: &dyn Any) {}
}

dyn_clone::clone_trait_object!(<'a, T> RuleMut<'a, Language = T>);

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
    rule_names: Vec<String>,
    rules: Vec<Box<dyn RuleMut<'a, Language = T>>>,
}

impl<'a, T> Clone for RuleSet<'a, T> {
    fn clone(&self) -> Self {
        RuleSet {
            rule_names: self.rule_names.clone(),
            rules: self
                .rules
                .iter()
                .map(|r| dyn_clone::clone_box(&**r))
                .collect(),
        }
    }
}

impl<'a, T> RuleSet<'a, T> {
    pub fn new(
        full_ruleset: Vec<(&'a str, Box<dyn RuleMut<'a, Language = T>>)>,
        ctx: RuleSetBuilderType,
    ) -> Self {
        let (full_names, full_rules): (Vec<String>, Vec<Box<dyn RuleMut<'a, Language = T>>>) =
            full_ruleset
                .into_iter()
                .map(|(n, r)| (n.to_lowercase(), r))
                .unzip();

        let low_input: Vec<String> = match &ctx {
            RuleSetBuilderType::WithRules(r) | RuleSetBuilderType::WithoutRules(r) => {
                r.iter().map(|s| s.to_lowercase()).collect()
            }
        };

        for rule in &low_input {
            if !full_names.contains(rule) {
                warn!("Unknown rule: '{}', skipping", rule);
            }
        }

        // delete unknown rules
        let low_input: Vec<String> = low_input
            .into_iter()
            .filter(|s| full_names.contains(s))
            .collect();

        let selected: Vec<(String, Box<dyn RuleMut<'a, Language = T>>)> = full_names
            .into_iter()
            .zip(full_rules)
            .filter(|(name, _)| match &ctx {
                RuleSetBuilderType::WithRules(_) => low_input.contains(name),
                RuleSetBuilderType::WithoutRules(_) => !low_input.contains(name),
            })
            .collect();

        let (rule_names, rules): (Vec<String>, Vec<Box<dyn RuleMut<'a, Language = T>>>) =
            selected.into_iter().unzip();

        Self { rule_names, rules }
    }

    pub fn from_parts(
        rule_names: Vec<String>,
        rules: Vec<Box<dyn RuleMut<'a, Language = T>>>,
    ) -> Self {
        Self { rule_names, rules }
    }

    pub fn for_each_rule_mut(
        &mut self,
        mut handler: impl FnMut(&str, &mut dyn RuleMut<'a, Language = T>),
    ) {
        self.rule_names
            .iter()
            .zip(self.rules.iter_mut())
            .for_each(|(name, rule)| handler(name.as_str(), rule.as_mut()));
    }
}

pub enum RuleSetBuilderType<'a> {
    WithRules(Vec<&'a str>),
    WithoutRules(Vec<&'a str>),
}

impl<'a, T> RuleMut<'a> for RuleSet<'a, T> {
    type Language = T;

    fn active_rule_names(&self) -> Vec<String> {
        self.rule_names.clone()
    }

    fn enter(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        flow: ControlFlow,
    ) -> MinusOneResult<()> {
        self.rules.iter_mut().try_for_each(|r| r.enter(node, flow))
    }

    fn enter_with_context(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        flow: ControlFlow,
        context: &RuleExecutionContext<'_, 'a, Self::Language>,
    ) -> MinusOneResult<()> {
        let rule_count = self.rules.len();

        for idx in 0..rule_count {
            let (left_rules, right_rules) = self.rules.split_at_mut(idx);
            let (current_rule, right_rules) =
                right_rules.split_first_mut().expect("valid rule index");

            let (left_names, right_names) = self.rule_names.split_at(idx);
            let (_current_name, right_names) = right_names.split_first().expect("valid rule index");

            let other_rules: Vec<RuleReference<'_, 'a, T>> = left_names
                .iter()
                .zip(left_rules.iter())
                .map(|(name, rule)| RuleReference {
                    name,
                    rule: &**rule,
                })
                .chain(
                    right_names
                        .iter()
                        .zip(right_rules.iter())
                        .map(|(name, rule)| RuleReference {
                            name,
                            rule: &**rule,
                        }),
                )
                .collect();

            let nested_context = RuleExecutionContext {
                other_rules: &other_rules,
                recursion_depth: context.recursion_depth,
            };

            current_rule.enter_with_context(node, flow, &nested_context)?;
        }

        Ok(())
    }

    fn leave(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        flow: ControlFlow,
    ) -> MinusOneResult<()> {
        self.rules.iter_mut().try_for_each(|r| r.leave(node, flow))
    }

    fn leave_with_context(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        flow: ControlFlow,
        context: &RuleExecutionContext<'_, 'a, Self::Language>,
    ) -> MinusOneResult<()> {
        let rule_count = self.rules.len();

        for idx in 0..rule_count {
            let (left_rules, right_rules) = self.rules.split_at_mut(idx);
            let (current_rule, right_rules) =
                right_rules.split_first_mut().expect("valid rule index");

            let (left_names, right_names) = self.rule_names.split_at(idx);
            let (_current_name, right_names) = right_names.split_first().expect("valid rule index");

            let other_rules: Vec<RuleReference<'_, 'a, T>> = left_names
                .iter()
                .zip(left_rules.iter())
                .map(|(name, rule)| RuleReference {
                    name,
                    rule: &**rule,
                })
                .chain(
                    right_names
                        .iter()
                        .zip(right_rules.iter())
                        .map(|(name, rule)| RuleReference {
                            name,
                            rule: &**rule,
                        }),
                )
                .collect();

            let nested_context = RuleExecutionContext {
                other_rules: &other_rules,
                recursion_depth: context.recursion_depth,
            };

            current_rule.leave_with_context(node, flow, &nested_context)?;
        }

        Ok(())
    }
}

macro_rules! impl_data {
    ( $($ty:ident),* ) => {
        impl<'a, Data, $($ty),*> RuleMut<'a> for ( $( $ty , )* )
            where $( $ty : RuleMut<'a, Language=Data> + Clone),*
            {
                type Language = Data;

                fn active_rule_names(&self) -> Vec<String> {
                    let mut names = Vec::new();
                    $(
                        ${ignore($ty)}
                        names.extend(self.${index()}.active_rule_names());
                    )*
                    names
                }

                fn enter(&mut self, node : &mut NodeMut<'a, Self::Language>, flow: ControlFlow) -> MinusOneResult<()>{
                    $(
                        ${ignore($ty)}
                        self.${index()}.enter(node, flow)?;
                    )*
                    Ok(())
                }

                fn enter_with_context(&mut self, node : &mut NodeMut<'a, Self::Language>, flow: ControlFlow, context: &RuleExecutionContext<'_, 'a, Self::Language>) -> MinusOneResult<()>{
                    $(
                        ${ignore($ty)}
                        self.${index()}.enter_with_context(node, flow, context)?;
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

                fn leave_with_context(&mut self, node : &mut NodeMut<'a, Self::Language>, flow: ControlFlow, context: &RuleExecutionContext<'_, 'a, Self::Language>) -> MinusOneResult<()>{
                    $(
                        ${ignore($ty)}
                        self.${index()}.leave_with_context(node, flow, context)?;
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
