pub trait Init {
    fn init() -> Self;
}

macro_rules! impl_init {
    ( $($ty:ident),* ) => {
        impl<$($ty),*> Init for ( $( $ty , )* )
            where $( $ty : Default),*
            {
                fn init() -> Self {(
                    $(
                        $ty::default(),
                    )*
                )}
            }
    };
}

mod impl_init {
    #![cfg_attr(rustfmt, rustfmt_skip)]

    use super::*;

    impl_init!(A);
    impl_init!(A, B);
    impl_init!(A, B, C);
    impl_init!(A, B, C, D);
    impl_init!(A, B, C, D, E);
    impl_init!(A, B, C, D, E, F);
    impl_init!(A, B, C, D, E, F, G);
    impl_init!(A, B, C, D, E, F, G, H);
    impl_init!(A, B, C, D, E, F, G, H, I);
    impl_init!(A, B, C, D, E, F, G, H, I, J);
    impl_init!(A, B, C, D, E, F, G, H, I, J, K);
    impl_init!(A, B, C, D, E, F, G, H, I, J, K, L);
    impl_init!(A, B, C, D, E, F, G, H, I, J, K, L, M);
    impl_init!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
    impl_init!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
    impl_init!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);
    impl_init!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q);
    impl_init!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R);
    impl_init!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S);
    impl_init!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T);
    impl_init!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U);
    impl_init!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V);
    impl_init!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W);
    impl_init!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X);
    impl_init!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y);
    impl_init!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z);
    impl_init!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA);
    impl_init!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB);
    impl_init!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC);
    impl_init!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, AB, AC, AD);
}
