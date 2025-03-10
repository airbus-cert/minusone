# PyMinusone

It's a `python` wrapper for `minusone`.

## Build

`pyminusone` use `maturin`. To build `pyminusone` you need `maturin` first :

```
pip install maturin
```

Then call `maturin`:

```
maturin build
```

## Use

```
import pyminusone
pyminusone.deobfuscate_powershell("1+2")
"3"
```

HTML renderer:

```
import pyminusone
pyminusone.deobfuscate_powershell_html("1+2")
'<span class="number">3</span>\n'
```