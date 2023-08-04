use tree::{NodeMut, Node};
use error::MinusOneResult;


pub trait RuleMut<'a> {
    type Language;
    fn enter(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>;
    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>;
}


pub trait Rule<'a> {
    type Language;
    fn enter(&mut self, node : &Node<'a, Self::Language>) -> MinusOneResult<()>;
    fn leave(&mut self, node : &Node<'a, Self::Language>) -> MinusOneResult<()>;
}


macro_rules! impl_data {
    ( $($ty:ident),* ) => {
        impl<'a, Data, $($ty),*> RuleMut<'a> for ( $( $ty , )* )
            where $( $ty : RuleMut<'a, Language=Data>),*
            {
                type Language = Data;

                fn enter(&mut self, node : &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
                    $(
                        ${ignore(ty)}
                        self.${index()}.enter(node)?;
                    )*
                    Ok(())
                }

                fn leave(&mut self, node : &mut NodeMut<'a, Self::Language>) -> MinusOneResult<()>{
                    $(
                        ${ignore(ty)}
                        self.${index()}.leave(node)?;
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
}

