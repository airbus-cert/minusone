use crate::error::MinusOneResult;
use crate::ps::Powershell;
use crate::ps::Powershell::{Raw, Stream, Type};
use crate::ps::Value::Str;
use crate::ps::tool::StringTool;
use crate::ps::utils::bytes::*;
use crate::ps::utils::string::decode;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, Node, NodeMut};
use flate2::read::{DeflateDecoder, GzDecoder, ZlibDecoder};
use log::trace;
use std::io::Read;

/// Cap on decompressed output, so a crafted archive can't be used to exhaust memory.
const MAX_DECOMPRESSED_SIZE: u64 = 100 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompressionKind {
    Gzip,
    Deflate,
    ZLib,
}

fn strip_system_prefix(typename: &str) -> &str {
    typename.strip_prefix("system.").unwrap_or(typename)
}

fn is_memory_stream_typename(typename: &str) -> bool {
    strip_system_prefix(typename) == "io.memorystream"
}

fn is_stream_reader_typename(typename: &str) -> bool {
    strip_system_prefix(typename) == "io.streamreader"
}

fn is_compression_mode_typename(typename: &str) -> bool {
    strip_system_prefix(typename) == "io.compression.compressionmode"
}

fn compression_kind_of(typename: &str) -> Option<CompressionKind> {
    match strip_system_prefix(typename) {
        "io.compression.gzipstream" => Some(CompressionKind::Gzip),
        "io.compression.deflatestream" => Some(CompressionKind::Deflate),
        "io.compression.zlibstream" => Some(CompressionKind::ZLib),
        _ => None,
    }
}

fn decompress(kind: CompressionKind, data: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    match kind {
        CompressionKind::Gzip => GzDecoder::new(data)
            .take(MAX_DECOMPRESSED_SIZE)
            .read_to_end(&mut out),
        CompressionKind::Deflate => DeflateDecoder::new(data)
            .take(MAX_DECOMPRESSED_SIZE)
            .read_to_end(&mut out),
        CompressionKind::ZLib => ZlibDecoder::new(data)
            .take(MAX_DECOMPRESSED_SIZE)
            .read_to_end(&mut out),
    }
    .ok()?;
    Some(out)
}

fn comma_wrapped_bytes(node: &Node<'_, Powershell>) -> Option<Vec<u8>> {
    if let Some(data) = node.data()
        && let Some(bytes) = bytes_from_data(data)
    {
        return Some(bytes);
    }

    let inner = node.smallest_child();
    if inner.kind() == "expression_with_unary_operator"
        && let (Some(op), Some(expr)) = (inner.child(0), inner.child(1))
        && op.text().ok()? == ","
    {
        return expr.data().and_then(bytes_from_data);
    }
    None
}

fn new_object_with_args<'a>(
    view: &Node<'a, Powershell>,
) -> MinusOneResult<Option<(String, Node<'a, Powershell>)>> {
    if view.kind() != "command" {
        return Ok(None);
    }
    let (Some(command_name), Some(command_elements)) = (
        view.named_child("command_name"),
        view.named_child("command_elements"),
    ) else {
        return Ok(None);
    };
    if crate::ps::cmdlets::resolved_command_name(&command_name)? != "new-object" {
        return Ok(None);
    }

    let count = command_elements.child_count();
    if count < 2 {
        return Ok(None);
    }
    let Some(args_list) = command_elements.child(count - 1) else {
        return Ok(None);
    };
    let Some(typename_node) = command_elements.child(count - 2) else {
        return Ok(None);
    };
    if args_list.kind() != "argument_list" || typename_node.kind() != "generic_token" {
        return Ok(None);
    }

    Ok(Some((typename_node.text()?.to_lowercase(), args_list)))
}

/// Resolves `MemoryStream` / `GzipStream` / `DeflateStream` / `ZLibStream` / `StreamReader` chain
///
/// ```powershell
/// $s = New-Object IO.MemoryStream(,[Convert]::FromBase64String("H4s..."))
/// IEX ((New-Object IO.StreamReader((New-Object IO.Compression.GzipStream($s,[IO.Compression.CompressionMode]::Decompress)))).ReadToEnd())
/// ```
///
/// - `New-Object IO.MemoryStream(,<bytes>)` is tracked as a [`Powershell::Stream`] wrapping the
///   backing buffer.
/// - `New-Object IO.Compression.{Gzip,Deflate,ZLib}Stream(<stream>, [IO.Compression.CompressionMode]::Decompress)`
///   decompresses eagerly and is tracked as a `Stream` over the decompressed bytes.
/// - `New-Object IO.StreamReader(<stream>)` just carries the wrapped `Stream` forward, later
///   consumed by [`StreamReadToEnd`].
///
/// Also resolves `[System.IO.Compression.CompressionMode]::Decompress`/`::Compress` to `compression.mode.*`
///
/// # Example
/// ```
/// use minusone::ps::build_powershell_tree;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::linter::Linter;
/// use minusone::ps::typing::ParseType;
/// use minusone::ps::string::ParseString;
/// use minusone::ps::integer::ParseInt;
/// use minusone::ps::method::DecodeBase64;
/// use minusone::ps::var::Var;
/// use minusone::ps::compression::{StreamType, StreamReadToEnd};
///
/// let mut tree = build_powershell_tree(
///     r#"$s=New-Object IO.MemoryStream(,[Convert]::FromBase64String("H4sIAAAAAAAC//NIzcnJ11HIzcwrLc7PS1UEAOUrYEIQAAAA"));IEX ((New-Object IO.StreamReader((New-Object IO.Compression.GzipStream($s,[IO.Compression.CompressionMode]::Decompress)))).ReadToEnd())"#
/// ).unwrap();
/// tree.apply_mut(&mut (
///     Forward::default(),
///     ParseType::default(),
///     ParseString::default(),
///     ParseInt::default(),
///     DecodeBase64::default(),
///     Var::default(),
///     StreamType::default(),
///     StreamReadToEnd::default(),
/// )).unwrap();
///
/// let mut ps_litter_view = Linter::default();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert!(ps_litter_view.output.contains("Hello, minusone!"));
/// ```
#[derive(Default)]
pub struct StreamType;

impl<'a> RuleMut<'a> for StreamType {
    type Language = Powershell;

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

        if view.kind() == "member_access"
            && let (Some(type_node), Some(op), Some(member_name)) =
                (view.child(0), view.child(1), view.child(2))
            && op.text()? == "::"
            && let Some(Type(typename)) = type_node.data()
            && is_compression_mode_typename(typename)
        {
            let member = member_name.text()?.to_string().normalize();
            if member == "decompress" || member == "compress" {
                trace!(
                    "StreamType (L): Setting node with compression mode type: {}",
                    member
                );
                node.set(Type(format!("compression.mode.{}", member)));
            }
            return Ok(());
        }

        if let Some((typename, args_list)) = new_object_with_args(&view)? {
            let Some(argument_expression_list) = args_list.named_child("argument_expression_list")
            else {
                return Ok(());
            };

            if is_memory_stream_typename(&typename)
                && argument_expression_list.child_count() == 1
                && let Some(arg) = argument_expression_list.child(0)
                && let Some(bytes) = comma_wrapped_bytes(&arg)
            {
                trace!(
                    "StreamType (L): Setting node with MemoryStream buffer of {} bytes",
                    bytes.len()
                );
                node.set(Stream(bytes));
            } else if let Some(kind) = compression_kind_of(&typename)
                && let (Some(arg_stream), Some(arg_mode)) = (
                    argument_expression_list.child(0),
                    argument_expression_list.child(2),
                )
                && let Some(source) = arg_stream.data().and_then(bytes_from_data)
                && let Some(Type(mode_tag)) = arg_mode.data()
                && mode_tag == "compression.mode.decompress"
                && let Some(result) = decompress(kind, &source)
            {
                trace!(
                    "StreamType (L): Setting node with decompressed buffer of {} bytes",
                    result.len()
                );
                node.set(Stream(result));
            } else if is_stream_reader_typename(&typename)
                && argument_expression_list.child_count() == 1
                && let Some(arg) = argument_expression_list.child(0)
                && let Some(Stream(bytes)) = arg.data()
            {
                trace!("StreamType (L): Setting node with StreamReader over known buffer");
                node.set(Stream(bytes.clone()));
            }
        }
        Ok(())
    }
}

/// This rule infers `ReadToEnd()` calls on a resolved [`StreamType`] stream.
///
/// # Example
/// See [`StreamType`] for a full decompression example ending in a `ReadToEnd()` call.
#[derive(Default)]
pub struct StreamReadToEnd;

impl<'a> RuleMut<'a> for StreamReadToEnd {
    type Language = Powershell;

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

        if view.kind() == "invokation_expression"
            && let (Some(type_node), Some(op), Some(member_name)) =
                (view.child(0), view.child(1), view.child(2))
            && op.text()? == "."
            && member_name.text()?.to_string().normalize() == "readtoend"
            && let Some(Stream(bytes)) = type_node.data()
        {
            let without_bom = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
            if let Some(s) = decode("utf8", without_bom) {
                trace!(
                    "StreamReadToEnd (L): Setting node with decoded string: {:?}",
                    s
                );
                node.set(Raw(Str(s)));
            }
        }
        Ok(())
    }
}
