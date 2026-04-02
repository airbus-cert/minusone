mod py 'crates/pyminusone'
mod js 'crates/minusonejs'

test:
  cargo test

test-py:
    just py build
    python3 -m venv .venv-pyminusone-test
    source .venv-pyminusone-test/bin/activate
    .venv-pyminusone-test/bin/pip install target/wheels/pyminusone-*.whl
    .venv-pyminusone-test/bin/python crates/pyminusone/test.py