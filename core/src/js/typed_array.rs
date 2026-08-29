use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::*;
use crate::js::Value::*;
use crate::js::node::buffer::{array_node_to_bytes, normalize_encoding};
use crate::js::utils::{get_positional_arguments, method_name};
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use log::trace;

/// Infers `new Uint8Array(...)` calls, reducing them to the same `Buffer`
/// byte-vector representation used for node's `Buffer`.
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::integer::ParseInt;
/// use minusone::js::array::ParseArray;
/// use minusone::js::typed_array::ParseUint8Array;
/// use minusone::js::node::buffer::BufferToString;
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = new Uint8Array([65, 66, 67]).toString();").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(), ParseArray::default(), ParseUint8Array::default(), BufferToString::default()
/// )).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
/// assert_eq!(linter.output, "var x = 'ABC';");
/// ```
#[derive(Default)]
pub struct ParseUint8Array;

impl<'a> RuleMut<'a> for ParseUint8Array {
    type Language = JavaScript;

    fn enter(
        &mut self,
        _node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        Ok(())
    }

    fn leave(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        let view = node.view();
        if view.kind() != "new_expression" {
            return Ok(());
        }

        let Some(constructor) = view.named_child("constructor").or_else(|| view.child(1)) else {
            return Ok(());
        };
        if constructor.text().ok() != Some("Uint8Array") {
            return Ok(());
        }

        let args =
            get_positional_arguments(view.named_child("arguments").or_else(|| view.child(2)));

        let bytes = match args.first() {
            None => vec![],
            Some(arg) => match arg.data() {
                Some(Array(values)) => array_node_to_bytes(values),
                Some(Buffer(bytes)) | Some(Bytes(bytes)) => bytes.clone(),
                Some(value) => match value.as_js_num() {
                    Raw(Num(n)) if n.is_finite() && n >= 0.0 => vec![0u8; n as usize],
                    _ => return Ok(()),
                },
                // argument present but not yet resolved: wait for a later pass
                None => return Ok(()),
            },
        };

        trace!(
            "ParseUint8Array: reducing new Uint8Array(...) to buffer of {} bytes",
            bytes.len()
        );
        node.reduce(Buffer(bytes));
        Ok(())
    }
}

/// Infers `new TextDecoder([label]).decode(...)` calls on deterministic byte inputs.
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::integer::ParseInt;
/// use minusone::js::array::ParseArray;
/// use minusone::js::string::ParseString;
/// use minusone::js::typed_array::{ParseUint8Array, TextDecoderDecode};
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = new TextDecoder().decode(new Uint8Array([65, 66, 67]));").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(), ParseArray::default(), ParseString::default(),
///     ParseUint8Array::default(), TextDecoderDecode::default()
/// )).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
/// assert_eq!(linter.output, "var x = 'ABC';");
/// ```
#[derive(Default)]
pub struct TextDecoderDecode;

impl<'a> RuleMut<'a> for TextDecoderDecode {
    type Language = JavaScript;

    fn enter(
        &mut self,
        _node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        Ok(())
    }

    fn leave(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        let view = node.view();
        if view.kind() != "call_expression" {
            return Ok(());
        }

        let Some(callee) = view.named_child("function").or_else(|| view.child(0)) else {
            return Ok(());
        };
        let Some(method) = method_name(&callee) else {
            return Ok(());
        };
        if method != "decode" {
            return Ok(());
        }

        let Some(object) = callee.child(0).or_else(|| callee.named_child("object")) else {
            return Ok(());
        };
        if object.kind() != "new_expression" {
            return Ok(());
        }
        let Some(constructor) = object
            .named_child("constructor")
            .or_else(|| object.child(1))
        else {
            return Ok(());
        };
        if constructor.text().ok() != Some("TextDecoder") {
            return Ok(());
        }

        let ctor_args =
            get_positional_arguments(object.named_child("arguments").or_else(|| object.child(2)));
        let encoding = match ctor_args.first() {
            None => "utf8".to_string(),
            Some(arg) => match arg.data() {
                None => return Ok(()),
                Some(Undefined) => "utf8".to_string(),
                Some(Raw(Str(label))) => normalize_encoding(label),
                _ => return Ok(()),
            },
        };
        if !matches!(encoding.as_str(), "utf8" | "utf16le" | "ucs2") {
            return Ok(());
        }

        let args = get_positional_arguments(view.named_child("arguments"));
        let bytes = match args.first() {
            None => vec![],
            Some(arg) => match arg.data() {
                Some(Buffer(bytes)) | Some(Bytes(bytes)) => bytes.clone(),
                Some(Array(values)) => array_node_to_bytes(values),
                _ => return Ok(()),
            },
        };

        let decoded = match encoding.as_str() {
            "utf16le" | "ucs2" => String::from_utf16le_lossy(&bytes),
            _ => String::from_utf8_lossy(&bytes).to_string(),
        };

        trace!(
            "TextDecoderDecode: decoding {} bytes ({}) to '{}'",
            bytes.len(),
            encoding,
            decoded
        );
        node.reduce(Raw(Str(decoded)));
        Ok(())
    }
}
