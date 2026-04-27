mod py 'crates/pyminusone'
mod js 'crates/minusonejs'

test:
    cargo test
    just py test
