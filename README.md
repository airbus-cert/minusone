# minusone

$$\textit{obfuscation}^{-1}$$

Reverse operation of script obfuscation

🌐 An online version is available: https://minusone.skyblue.team/ 🌐


## Description

MinusOne is a deobfuscation engine focused on scripting languages. MinusOne is based on [tree-sitter](https://tree-sitter.github.io/tree-sitter/) for parsing, and will apply a set of rules to infer node values and simplify expressions.

MinusOne supports the following languages:
* Powershell

By taking the following example from [`Invoke-Obfuscation`](https://github.com/gh0x0st/Invoke-PSObfuscation/blob/main/layer-0-obfuscation.md#final-payload):

```
${Pop-pKkAp}=1;${Clear-OK3Emf}=4;${Push-Jh8ps}=9;${Format-qqM9C}=16;${Redo-kSQuo}=86;${Format-LyC}=51;${Pop-ASPJ}=74;${Join-pIuV}=112;${Hide-Rhpet}=100;${Copy-TWaj}=71;${Set-yYE}=85;${Exit-shq}=116;${Skip-5qa}=83;${Push-bAik}=57;${Split-f7hDr6}=122;${Open-YGi}=65;${Open-LPQk}=61;${Select-YUyq}=84;${Move-sS6mJ}=87;${Search-wa0}=108;${Join-YJq}=117;${Hide-iQ5}=88;${Select-iV0F7}=78;${Select-cI9j}=80;${Open-Hec}=98;${Reset-4QePz}=109;${Format-4e7UHy}=103;${Lock-UyaF}=97;${Select-ZGdxB}=77;${Move-FtkTLt}=104;${Push-VUUQsE}=73;${Add-LHgggw}=99;${Reset-sc3}=81;${Format-AlmdYS}=50;${Resize-mYqZ}=121;${Reset-hp9}=66;${Reset-qC3Yd}=48;${Find-6QywvV}=120;${Select-v7sja}=110;${Step-7WvUL}=82;$DJ2=[System.Text.Encoding];$1Ro=[System.Convert];${Step-xE2}=-join'8FTU'[-${Pop-pKkAp}..-${Clear-OK3Emf}];${Unlock-Zdbkvh}=-join'gnirtSteG'[-${Pop-pKkAp}..-${Push-Jh8ps}];${Close-yjy}=-join'gnirtS46esaBmorF'[-${Pop-pKkAp}..-${Format-qqM9C}];. ($DJ2::${Step-xE2}.${Unlock-Zdbkvh}($1Ro::${Close-yjy}(([char]${Redo-kSQuo}+[char]${Format-LyC}+[char]${Pop-ASPJ}+[char]${Join-pIuV}+[char]${Hide-Rhpet}+[char]${Copy-TWaj}+[char]${Set-yYE}+[char]${Exit-shq}+[char]${Skip-5qa}+[char]${Copy-TWaj}+[char]${Push-bAik}+[char]${Split-f7hDr6}+[char]${Hide-Rhpet}+[char]${Open-YGi}+[char]${Open-LPQk}+[char]${Open-LPQk})))) ($DJ2::${Step-xE2}.${Unlock-Zdbkvh}($1Ro::${Close-yjy}(([char]${Select-YUyq}+[char]${Move-sS6mJ}+[char]${Search-wa0}+[char]${Join-YJq}+[char]${Hide-Rhpet}+[char]${Hide-iQ5}+[char]${Select-iV0F7}+[char]${Select-cI9j}+[char]${Open-Hec}+[char]${Reset-4QePz}+[char]${Set-yYE}+[char]${Format-4e7UHy}+[char]${Lock-UyaF}+[char]${Hide-iQ5}+[char]${Select-ZGdxB}+[char]${Format-4e7UHy}+[char]${Hide-Rhpet}+[char]${Copy-TWaj}+[char]${Move-FtkTLt}+[char]${Search-wa0}+[char]${Push-VUUQsE}+[char]${Copy-TWaj}+[char]${Pop-ASPJ}+[char]${Search-wa0}+[char]${Add-LHgggw}+[char]${Format-LyC}+[char]${Reset-sc3}+[char]${Format-4e7UHy}+[char]${Add-LHgggw}+[char]${Format-AlmdYS}+[char]${Select-iV0F7}+[char]${Resize-mYqZ}+[char]${Lock-UyaF}+[char]${Hide-iQ5}+[char]${Reset-hp9}+[char]${Reset-qC3Yd}+[char]${Push-VUUQsE}+[char]${Copy-TWaj}+[char]${Find-6QywvV}+[char]${Join-pIuV}+[char]${Open-Hec}+[char]${Select-v7sja}+[char]${Step-7WvUL}+[char]${Search-wa0}+[char]${Add-LHgggw}+[char]${Format-4e7UHy}+[char]${Open-LPQk}+[char]${Open-LPQk}))))
```

It will produce the following output :

```powershell
Write-Host "MinusOne is the best script linter"
```

## Usage

MinusOne is written in Rust and can be built, deployed or executed through the Cargo package manager:

```
cargo run --features="minusone-cli" -- --path test.ps1
```

Python bindings are also available, allowing MinusOne to be easily integrated into Jupyter notebooks for example.

## What is a Rule?

A rule will produce a result when visiting a particular node, depending on its children or parent. A rule will be called when entering and leaving a node.

Creating a rule for Powershell is as easy as implementing the `RuleMut` trait :

```rust
#[derive(Default)]
pub struct MyRule;

impl<'a> RuleMut<'a> for MyRule {
    type Language = Powershell;

    fn enter(&mut self, node: &mut NodeMut<'a, Self::Language>, flow: BranchFlow) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>, flow: BranchFlow) -> MinusOneResult<()>{
        Ok(())
    }
}
```

The `enter()` method is called before visiting the node, and the `leave()` method will be called when leaving the node, so after visiting the node and all its children.

### Example: A rule that adds two integers

In this example we will see how to infer value from :

```powershell
$a = 40 + 2
```

To :

```powershell
$a = 42
```

The first rule we need is a rule to parse integers :

```rust
#[derive(Default)]
pub struct ParseInt;

impl<'a> RuleMut<'a> for ParseInt {
    type Language = Powershell;

    fn enter(&mut self, node: &mut NodeMut<'a, Self::Language>, flow: BranchFlow) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>, flow: BranchFlow) -> MinusOneResult<()>{
        let view = node.view();
        let token = view.text()?;
        match view.kind() {
            "decimal_integer_literal" => {
                if let Ok(number) = token.parse::<i32>() {
                    node.set(Raw(Num(number)));
                }
            },
            _ => ()
        }

        Ok(())
    }
}
```

The rule will be processed when leaving a node of type `decimal_integer_literal` in the `tree-sitter-powershell` grammar, 
then it will try to parse the token by using the [`std::str::parse`](https://doc.rust-lang.org/std/primitive.str.html#method.parse) method (`token.parse::<i32>()`).

A more complete implementation of this rule can be found [here](src/ps/integer.rs).

Now we will create a new rule that will infer the value of two nodes involved in a `+` operation. This rule will be focused on the `additive_expression` node type.

It will check if the node has three children:

* The first one must inferred by the previous rule as an integer
* The second one must be the token `+`
* The third one must inferred by the previous rule as an integer

```rust
#[derive(Default)]
pub struct AddInt;

impl<'a> RuleMut<'a> for AddInt {
    type Language = Powershell;

    fn enter(&mut self, _node: &mut NodeMut<'a, Self::Language>, flow: BranchFlow) -> MinusOneResult<()>{
        Ok(())
    }

    fn leave(&mut self, node: &mut NodeMut<'a, Self::Language>, flow: BranchFlow) -> MinusOneResult<()>{
        let node_view = node.view();
        if node_view.kind() == "additive_expression" {
            if let (Some(left_op), Some(operator), Some(right_op)) = (node_view.child(0), node_view.child(1), node_view.child(2)) {
                match (left_op.data(), operator.text()?, right_op.data()) {
                    (Some(Raw(Num(number_left))), "+", Some(Raw(Num(number_right)))) => node.set(Raw(Num(number_left + number_right))),
                    _ => {}
                }
            }
        }
        Ok(())
    }
}
```

Then we can apply these rule to the Powershell tree generated by `tree-sitter-powershell`:

```rust

let mut tree = build_powershell_tree("40 + 2").unwrap();

tree.apply_mut(&mut (
    ParseInt::default(), 
    Forward::default(), 
    AddInt::default()
)).unwrap();

```

The `Forward` rule is a particular rule that will forward a node's inferred type in case a node is not used in a semantic way, which is mainly due to how the Powershell grammar was generated.

Then, you can print the Powershell result by using the object `Linter`:

```rust
let mut ps_linter_view = Linter::new();
ps_linter_view.print(&tree.root().unwrap()).unwrap();

// => 42
```

## Rules for Powershell

When using the `Engine` object, you will automatically use predefined rules designed for Powershell. These can be found in [src/ps/mod.rs](src/ps/mod.rs) :

```rust
pub type RuleSet = (
    Forward,            // Special rule that will forward inferred value in case the node is transparent
    ParseInt,           // Parse integer
    AddInt,             // +, - operations on integer
    MultInt,            // *, / operations on integer
    ParseString,        // Parse string token, including multiline strings
    ConcatString,       // String concatenation operation
    Cast,               // cast operation, like [char]0x65
    ParseArrayLiteral,  // It will parse array declared using separate value (integer or string) by a comma
    ParseRange,         // It will parse .. operator and generate an array
    AccessString,       // The access operator [] apply to a string : "foo"[0] => "f"
    JoinComparison,     // It will infer join string operation using the -join operator : @('a', 'b', 'c') -join '' => "abc"
    JoinStringMethod,   // It will infer join string operation using the [string]::join method : [string]::join('', @('a', 'b', 'c'))
    JoinOperator,       // It will infer join string operation using the -join unary operator -join @('a', 'b', 'c')
    PSItemInferrator,   // PsItem is used to inferred commandlet pattern like % { [char] $_ }
    ForEach,            // It will used PSItem rules to inferred foreach-object command
    StringReplaceMethod,// It will infer replace method apply to a string : "foo".replace("oo", "aa") => "faa"
    ComputeArrayExpr,   // It will infer array that start with @
    NewObjectArray      // It will infer arrays created via New-Object
    StringReplaceOp,    // It will infer replace method apply to a string by using the -replace operator
    StaticVar,          // It will infer value of known variable : $pshome, $shellid
    CastNull,           // It will infer value of +$() or -$() which will produce 0
    ParseHash,          // Parse hashtable
    FormatString,       // It will infer string when format operator is used ; "{1}-{0}" -f "Debug", "Write"
    ParseBool,          // It will infer boolean operator
    Comparison,         // It will infer comparison when it's possible
    Not,                // It will infer the ! operator
    ParseType,          // Parse type
    DecodeBase64,       // Decode calls to FromBase64
    FromUTF,            // Decode calls to FromUTF{8,16}.GetText
    Length,             // Decode attribute length of string and array
    BoolAlgebra,        // Add support to boolean algebra (or and)
    Var,                // Variable replacement in case of predictable flow
);
```

## Roadmap

* More accurate parsing of Powershell HashTables
* Basic support of Javascript

