# JavaScript Rules

- [Parsers](#parsers)
    - [ParseArray](#parsearray)
    - [ParseBool](#parsebool)
    - [ParseInt](#parseint)
    - [ParseSpecials](#parsespecials)
    - [ParseString](#parsestring)
- [Deobfuscators](#deobfuscators)
    - [AddBool](#addbool)
    - [AddSubInt](#addsubint)
    - [AddSubSpecials](#addsubspecials)
    - [ArrayPlusMinus](#arrayplusminus)
    - [AtTrick](#attrick)
    - [B64](#b64)
    - [BitwiseInt](#bitwiseint)
    - [BoolAlgebra](#boolalgebra)
    - [BoolPlusMinus](#boolplusminus)
    - [CharAt](#charat)
    - [CmpOrd](#cmpord)
    - [CombineArrays](#combinearrays)
    - [Concat](#concat)
    - [ConstructorAccessTrick](#constructoraccesstrick)
    - [ConstructorTrick](#constructortrick)
    - [FnCall](#fncall)
    - [Forward](#forward)
    - [GetArrayElement](#getarrayelement)
    - [LooseEq](#looseeq)
    - [MultInt](#multint)
    - [NegInt](#negint)
    - [NotBool](#notbool)
    - [PowInt](#powint)
    - [ShiftInt](#shiftint)
    - [StrictEq](#stricteq)
    - [StringPlusMinus](#stringplusminus)
    - [ToString](#tostring)
    - [Var](#var)

## Parsers

### ParseArray

Parses array literal expressions and represents them as known array values for further inference.

```js
[]                 // -> [] (empty array)
['a', 'b']         // -> ['a', 'b']
['a', [null, 1]]   // -> ['a', [null, 1]] (handle recursive arrays and every types)
```

---

### ParseBool

Parses boolean literal tokens `true` and `false` into their typed boolean values.

```js
true    // -> true  (boolean)
false   // -> false (boolean)
```

---

### ParseInt

Parses integer literals in decimal, hexadecimal, octal, or binary notation into their numeric values.

```js
42       // -> 42   (decimal)
0xFF     // -> 255  (hex)
0o17     // -> 15   (octal)
0b1010   // -> 10   (binary)
```

It also handles numeric separators and floating points

```js
1_000_000_000_000       // -> 1000000000000
1_050.95                // -> 1050.95
0b1010_0001_1000_0101   // -> 41349
0o2_2_5_6               // -> 1198
0xA0_B0_C0              // -> 10531008
```

And special numbers starting with a 0

```js
017   // -> 15 (this is because when you start with a 0 and then directly the numbers, it's an octal number)
019   // -> 19 (this is because 9 is not a valid octal number so it falls back to decimal)
```

---

### ParseSpecials

Parses special value tokens such as `undefined`, `NaN`, and constructs like the `at` trick sentinel.

```js
undefined           // -> undefined
NaN                 // -> NaN
null                // -> null
''['constructor']   // -> Array constructor
```

---

### ParseString

Parses string literals enclosed in single or double quotes into their string values.

```js
'hello'                                                     // -> 'hello'
"world"                                                     // -> 'world'
'it\'s'                                                     // -> "it's"
'\u0030 \u{00030} \u{000030} \u{0000000000000030} \u{30}'   // -> '0 0 0 0 0'
'\x41'                                                      // -> 'A'
```

## Deobfuscators

### AddBool

Infers the result of `+` and `-` arithmetic operations applied to boolean values. JavaScript coerces `true` to `1` and
`false` to `0`.

```js
true + true        // -> 2
true + false       // -> 1
false - true       // -> -1
true + undefined   // -> NaN
false - NaN        // -> NaN
true + []          // -> 'true'
```

---

### AddSubInt

Infers `+` and `-` arithmetic operations on two integer values.

```js
10 + 5   // -> 15
10 - 3   // -> 7
```

---

### AddSubSpecials

Infers addition and subtraction involving `undefined` or `NaN`. Any arithmetic with `NaN` yields `NaN`, and `undefined`
coerces to `NaN`.

```js
[1, '2'] + undefined   // -> '1,2undefined'
['cheese'] + NaN       // -> 'cheeseNaN'
undefined + 1          // -> NaN
NaN + false            // -> NaN
'cheese' + NaN         // -> 'cheeseNaN'
```

---

### ArrayPlusMinus

Infers unary `+` and `-` operations applied to arrays. Arrays are coerced to strings, then to numbers.

```js
+[]           // -> 0 ([] -> "" -> 0)
- [['455']]   // -455
+ ['a']       // NaN
```

---

### AtTrick

Infers the `at` method access on arrays and strings. `[]['at']` resolves to the native `at` function reference.

```js
[]['at'] + 'hello'   // -> 'function at() { [native code] }hello'
[]['at'] + NaN   // -> 'function at() { [native code] }NaN'
([]['at'] + '')[7]   // -> 'n'  (the 7+1th char of 'function at() { [native code] }')
```

---

### B64

Infers and reduces `atob()` and `btoa()` calls to their resulting string literals.

```js
atob('bWludXNvbmU=')   // -> minusone
btoa('minusone')       // -> 'bWludXNvbmU='
```

---

### BitwiseInt

Infers the results of bitwise operations `&`, `|`, `^`, and `~` on integer values.

```js
5 & 3    // -> 1
5 | 3    // -> 7
5 ^ 3    // -> 6
~5       // -> -6
```

---

### BoolAlgebra

Infers boolean algebra operations using `&&` and `||` on every possible values.

```js
true && false   // -> false
true || false   // -> true
'' && 555       // ''
'' || 555       // 555
```

---

### BoolPlusMinus

Infers `+` and `-` unary operations specifically on boolean operands.

```js
+true    // -> 1
+ false   // -> 0
- true    // -> -1
-false   // -> -0
```

---

### CharAt

Infers array index calls on string literals and reduces them to single-character string literals

```js
'minusone'[1]    // -> 'i'
'minusone'[10]   // -> undefined
```

---

### CmpOrd

Infers comparison operators `<`, `>`, `<=`, and `>=` and resolves them to boolean literals.

```js
3 < 5       // -> true
true > 0    // -> true
4 <= 4      // -> true
2 >= 3      // -> false
NaN > 0     // -> false
11 > "10"   // -> true
```

---

### CombineArrays

Infers the `+` operation on arrays, which coerces both elements to strings and concatenates them. Also support `-`
operator

```js
[] + []                           // -> ''
[0, 1, 7] + [3, [7, '2', [88]]]   // -> '0,1,73,7,2,88'
['a'] + 1                         // -> 'a1'
['a'] - 1                         // -> 'NaN'
```

---

### Concat

Infers string concatenation using the `+` operator when at least one operand is a known string literal.

```js
'hello' + ' world'          // -> 'hello world'
'foo' + 'bar'               // -> 'foobar'
'num: ' + 42                // -> 'num: 42'
'Undefined: ' + undefined   // -> 'Undefined: undefined'
```

---

### ConstructorAccessTrick

Infers accessing the `constructor.name`

```js
true['constructor']['name']   // -> 'Boolean'
```

---

### ConstructorTrick

Infers accessing the `constructor` property on a value and resolves it to the corresponding constructor function
reference.

```js
''['constructor']          // -> ƒ String() { [native code] }
1['constructor']           // -> ƒ Number() { [native code] }
true['constructor']        // -> ƒ Boolean() { [native code] }
true['constructor'] + ''   // 'function Boolean() { [native code] }'
```

---

### FnCall

Resolves predictable function calls to their return values when the function and arguments are fully known at analysis
time.

```js
// This function will me marked as dead code because it's never called after (dead function -> delete the function)
function a() {
    return "a";
}

let b = a();     // -> let b = "a"
```

---

### Forward

Forward inferred type in the most simple cases and skip useless parenthesis.

```js
const x = 42;
x              // -> 42  (value forwarded from assignment)

((((0))))      // -> 0
```

---

### GetArrayElement

Retrieves the element at a specific index of a known array literal.

```js
[0, 1, 2][1]               // -> 1
([1, [2, '3'], 4][1])[0]   // -> 2
```

---

### LooseEq

Infers loose equality `==` and `!=` comparisons, accounting for JavaScript's type coercion rules.

```js
0 == false          // -> true
1 == true           // -> true
'' == false         // -> true
null == undefined   // -> true
1 != '1'            // -> false
```

---

### MultInt

Infers `*`, `/`, and `%` operations on integer values.

```js
6 * 7     // -> 42
10 / 2    // -> 5
10 % 3    // -> 1
7 / 2     // -> 3.5
```

---

### NegInt

Infers the unary `-` operator applied to integer values.

```js
-5    // -> -5
- 0    // -> -0
- (-3) // -> 3
- (42) // -> -42
```

---

### NotBool

Infers the unary `!` operator applied to boolean, arrays and numbers.

```js
!true    // -> false
!false   // -> true
!!true   // -> true
!0       // -> true
![]      // -> false
!![]     // -> true
```

---

### PowInt

Infers `**` (exponentiation) operations on integer values.

```js
2 ** 10    // -> 1024
3 ** 3     // -> 27
5 ** 0     // -> 1
2 ** -1    // -> 0.5
2 ** 0.5   // -> 1
```

---

### ShiftInt

Infers bitwise shift operations `<<`, `>>`, and `>>>` on integer values.

```js
1 << 3      // -> 8
16 >> 2     // -> 4
-16 >>> 2   // -> 1073741820  (unsigned right shift)
```

---

### StrictEq

Infers strict equality `===` and `!==` comparisons, which do not perform type coercion.

```js
1 === 1       // -> true
1 === '1'     // -> false
null === null // -> true
1 !== '1'     // -> true
NaN === NaN   // -> false
```

---

### StringPlusMinus

Infers unary `+` and `-` operations applied to string literals, coercing them to numbers.

```js
+'42'     // -> 42
+''       // -> 0
+'abc'    // -> NaN
-'5'      // -> -5
```

---

### ToString

Infers `toString` calls on known values and reduces them to string literals. Also handles `toString(radix)` for base
conversion.

```js
31['toString']()       // -> '31'
31['toString']('32')   // -> 'v'
```

---

### Var

Tracks variable assignments and propagates their known constant values to all usage sites throughout the code.

```js
const x = 10;
const y = x + 5;   // x -> 10, so y -> 15

let a = 'foo';
let b = a + 'bar'; // a -> 'foo', so b -> 'foobar'
```

It also handles these aspects of the var types

|           | *global stored* | *fn scopped* | *bloc scopped* | *mutable* | *redeclarable* |
|-----------|:---------------:|:------------:|:--------------:|:---------:|:--------------:|
| **var**   |       yes       |     yes      |       no       |    yes    |      yes       |
| **let**   |       no        |     yes      |      yes       |    yes    |       no       |
| **const** |       no        |     yes      |      yes       |    no     |       no       |