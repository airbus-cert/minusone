use crate::error::MinusOneResult;
use crate::ps::Powershell;
use crate::ps::Powershell::{Bytes, Raw, Type};
use crate::ps::Value::{Num, Str};
use crate::ps::tool::StringTool;
use crate::ps::utils::bytes::*;
use crate::ps::utils::string::*;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use log::trace;

fn is_encoding_tag(typename: &str) -> bool {
    matches!(
        typename,
        "text.encoding.ascii"
            | "text.encoding.utf8"
            | "text.encoding.unicode"
            | "text.encoding.bigendianunicode"
            | "text.encoding.utf32"
            | "text.encoding.latin1"
            | "text.encoding.default"
    )
}

fn static_property_tag(name: &str) -> Option<&'static str> {
    match name {
        "ascii" => Some("ascii"),
        "utf8" => Some("utf8"),
        "unicode" => Some("unicode"),
        "bigendianunicode" => Some("bigendianunicode"),
        "utf32" => Some("utf32"),
        "latin1" => Some("latin1"),
        // `Default` is the system ANSI code page on Windows PowerShell but UTF-8 on
        // PowerShell Core, so it's only resolved as a tag here; decode()/encode() below
        // only fold it when the payload is pure ASCII, where every runtime agrees.
        "default" => Some("default"),
        _ => None,
    }
}

fn constructor_tag(typename: &str) -> Option<&'static str> {
    match typename {
        "system.text.asciiencoding" | "text.asciiencoding" => Some("ascii"),
        "system.text.utf8encoding" | "text.utf8encoding" => Some("utf8"),
        "system.text.unicodeencoding" | "text.unicodeencoding" => Some("unicode"),
        "system.text.utf32encoding" | "text.utf32encoding" => Some("utf32"),
        _ => None,
    }
}

/// This rule resolves what concrete encoding a `[System.Text.Encoding]`-typed expression refers
/// to, from any of the ways PowerShell obfuscators reach one:
///
/// - a static property: `[System.Text.Encoding]::UTF8`
/// - `[System.Text.Encoding]::GetEncoding('utf-8')` / `GetEncoding(65001)`
/// - a parameterless constructor: `[System.Text.UTF8Encoding]::new()`, `New-Object System.Text.UTF8Encoding`
///
/// The resolved value is one of the internal `text.encoding.*` type tags, later consumed by
/// [`EncodingGetString`] and [`EncodingGetBytes`].
#[derive(Default)]
pub struct EncodingType;

impl<'a> RuleMut<'a> for EncodingType {
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
            && let (Some(type_lit), Some(op), Some(member_name)) =
                (view.child(0), view.child(1), view.child(2))
        {
            match (
                type_lit.data(),
                op.text()?,
                &member_name.text()?.to_string(),
                member_name.data(),
            ) {
                (Some(Type(typename)), "::", m, _)
                | (Some(Type(typename)), "::", _, Some(Raw(Str(m))))
                    if (typename == "system.text.encoding" || typename == "text.encoding")
                        && static_property_tag(&m.clone().normalize()).is_some() =>
                {
                    let tag = static_property_tag(&m.clone().normalize()).unwrap();
                    trace!("EncodingType (L): Setting node with encoding type: {}", tag);
                    node.set(Type(format!("text.encoding.{}", tag)));
                }
                _ => (),
            }
        } else if view.kind() == "invokation_expression"
            && let (Some(type_node), Some(op), Some(member_name), Some(args_list)) =
                (view.child(0), view.child(1), view.child(2), view.child(3))
        {
            match (
                type_node.data(),
                op.text()?,
                &member_name.text()?.to_string(),
                member_name.data(),
            ) {
                (Some(Type(typename)), "::", m, _)
                | (Some(Type(typename)), "::", _, Some(Raw(Str(m))))
                    if (typename == "system.text.encoding" || typename == "text.encoding")
                        && m.clone().normalize() == "getencoding" =>
                {
                    if let Some(argument_expression_list) =
                        args_list.named_child("argument_expression_list")
                        && let Some(arg_1) = argument_expression_list.child(0)
                    {
                        let tag = match arg_1.data() {
                            Some(Raw(Str(s))) => encoding_name_tag(&s.to_lowercase()),
                            Some(Raw(Num(n))) => encoding_codepage_tag(*n),
                            _ => None,
                        };
                        if let Some(tag) = tag {
                            trace!(
                                "EncodingType (L): Setting node with GetEncoding type: {}",
                                tag
                            );
                            node.set(Type(format!("text.encoding.{}", tag)));
                        }
                    }
                }
                (Some(Type(typename)), "::", m, _)
                | (Some(Type(typename)), "::", _, Some(Raw(Str(m))))
                    if m.clone().normalize() == "new"
                        && constructor_tag(typename).is_some()
                        && args_list.named_child("argument_expression_list").is_none() =>
                {
                    let tag = constructor_tag(typename).unwrap();
                    trace!(
                        "EncodingType (L): Setting node with constructor type: {}",
                        tag
                    );
                    node.set(Type(format!("text.encoding.{}", tag)));
                }
                _ => (),
            }
        } else if view.kind() == "command"
            && let (Some(command_name), Some(command_elements)) = (
                view.named_child("command_name"),
                view.named_child("command_elements"),
            )
            && crate::ps::cmdlets::resolved_command_name(&command_name)
                .is_ok_and(|name| name == "new-object")
        {
            let shape_ok = match command_elements.child_count() {
                2 => true,
                4 => command_elements
                    .child(1)
                    .is_some_and(|c| c.kind() == "command_parameter"),
                _ => false,
            };
            if shape_ok
                && let Some(last) = command_elements.child(command_elements.child_count() - 1)
                && last.kind() == "generic_token"
                && let Ok(typename) = last.text()
                && let Some(tag) = constructor_tag(&typename.to_lowercase())
            {
                trace!(
                    "EncodingType (L): Setting node with New-Object constructor type: {}",
                    tag
                );
                node.set(Type(format!("text.encoding.{}", tag)));
            }
        }
        Ok(())
    }
}

/// This rule infers `GetString(byte[])` calls on a resolved [`EncodingType`], decoding the byte
/// array into a string.
///
/// # Example
/// ```
/// use minusone::ps::build_powershell_tree;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::linter::Linter;
/// use minusone::ps::typing::ParseType;
/// use minusone::ps::integer::ParseInt;
/// use minusone::ps::array::{ComputeArrayExpr, ParseArrayLiteral};
/// use minusone::ps::encoding::{EncodingType, EncodingGetString};
///
/// let mut tree = build_powershell_tree("[System.Text.Encoding]::UTF8.GetString(@(102, 111, 111))").unwrap();
/// tree.apply_mut(&mut (
///     Forward::default(),
///     ParseType::default(),
///     EncodingType::default(),
///     ParseInt::default(),
///     ParseArrayLiteral::default(),
///     ComputeArrayExpr::default(),
///     EncodingGetString::default(),
/// )).unwrap();
///
/// let mut ps_litter_view = Linter::default();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "\"foo\"");
/// ```
#[derive(Default)]
pub struct EncodingGetString;

impl<'a> RuleMut<'a> for EncodingGetString {
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
            && let (Some(type_lit), Some(op), Some(member_name)) =
                (view.child(0), view.child(1), view.child(2))
        {
            match (
                type_lit.data(),
                op.text()?,
                &member_name.text()?.to_string(),
                member_name.data(),
            ) {
                (Some(Type(typename)), ".", m, _)
                | (Some(Type(typename)), ".", _, Some(Raw(Str(m))))
                    if is_encoding_tag(typename) && m.clone().normalize() == "getstring" =>
                {
                    let function_typename = format!("{}.getstring", typename);
                    trace!(
                        "EncodingGetString (L): Setting node with getstring type: {:?}",
                        function_typename
                    );
                    node.set(Type(function_typename));
                }
                _ => (),
            }
        } else if view.kind() == "invokation_expression"
            && let (Some(type_node), Some(op), Some(member_name), Some(args_list)) =
                (view.child(0), view.child(1), view.child(2), view.child(3))
        {
            match (
                type_node.data(),
                op.text()?,
                &member_name.text()?.to_string(),
                member_name.data(),
            ) {
                (Some(Type(typename)), ".", m, _)
                | (Some(Type(typename)), ".", _, Some(Raw(Str(m))))
                    if (is_encoding_tag(typename) && m.clone().normalize() == "getstring")
                        || (typename.ends_with(".getstring")
                            && m.clone().normalize() == "invoke") =>
                {
                    let tag = typename
                        .strip_prefix("text.encoding.")
                        .and_then(|t| t.strip_suffix(".getstring").or(Some(t)))
                        .unwrap_or(typename);

                    if let Some(argument_expression_list) =
                        args_list.named_child("argument_expression_list")
                        && let Some(arg_1) = argument_expression_list.child(0)
                        && let Some(data) = arg_1.data()
                        && let Some(bytes) = bytes_from_data(data)
                        && let Some(s) = decode(tag, &bytes)
                    {
                        trace!(
                            "EncodingGetString (L): Setting node with decoded string: {:?}",
                            s
                        );
                        node.set(Raw(Str(s)));
                    }
                }
                _ => (),
            }
        }
        Ok(())
    }
}

/// This rule infers `GetBytes(string)` calls on a resolved [`EncodingType`], encoding the string
/// into a [`Powershell::Bytes`], rendered by the [`crate::ps::linter::Linter`] as a `@(...)`
/// array literal.
///
/// # Example
/// ```
/// use minusone::ps::build_powershell_tree;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::linter::Linter;
/// use minusone::ps::typing::ParseType;
/// use minusone::ps::string::ParseString;
/// use minusone::ps::encoding::{EncodingType, EncodingGetBytes};
///
/// let mut tree = build_powershell_tree("[System.Text.Encoding]::UTF8.GetBytes('foo')").unwrap();
/// tree.apply_mut(&mut (
///     Forward::default(),
///     ParseType::default(),
///     EncodingType::default(),
///     ParseString::default(),
///     EncodingGetBytes::default(),
/// )).unwrap();
///
/// let mut ps_litter_view = Linter::default();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "@(102, 111, 111)");
/// ```
#[derive(Default)]
pub struct EncodingGetBytes;

impl<'a> RuleMut<'a> for EncodingGetBytes {
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
            && let (Some(type_lit), Some(op), Some(member_name)) =
                (view.child(0), view.child(1), view.child(2))
        {
            match (
                type_lit.data(),
                op.text()?,
                &member_name.text()?.to_string(),
                member_name.data(),
            ) {
                (Some(Type(typename)), ".", m, _)
                | (Some(Type(typename)), ".", _, Some(Raw(Str(m))))
                    if is_encoding_tag(typename) && m.clone().normalize() == "getbytes" =>
                {
                    let function_typename = format!("{}.getbytes", typename);
                    trace!(
                        "EncodingGetBytes (L): Setting node with getbytes type: {:?}",
                        function_typename
                    );
                    node.set(Type(function_typename));
                }
                _ => (),
            }
        } else if view.kind() == "invokation_expression"
            && let (Some(type_node), Some(op), Some(member_name), Some(args_list)) =
                (view.child(0), view.child(1), view.child(2), view.child(3))
        {
            match (
                type_node.data(),
                op.text()?,
                &member_name.text()?.to_string(),
                member_name.data(),
            ) {
                (Some(Type(typename)), ".", m, _)
                | (Some(Type(typename)), ".", _, Some(Raw(Str(m))))
                    if (is_encoding_tag(typename) && m.clone().normalize() == "getbytes")
                        || (typename.ends_with(".getbytes")
                            && m.clone().normalize() == "invoke") =>
                {
                    let tag = typename
                        .strip_prefix("text.encoding.")
                        .and_then(|t| t.strip_suffix(".getbytes").or(Some(t)))
                        .unwrap_or(typename);

                    if let Some(argument_expression_list) =
                        args_list.named_child("argument_expression_list")
                        && let Some(arg_1) = argument_expression_list.child(0)
                        && let Some(Raw(Str(s))) = arg_1.data()
                        && let Some(bytes) = encode(tag, s)
                    {
                        trace!(
                            "EncodingGetBytes (L): Setting node with encoded bytes: {:?}",
                            bytes
                        );
                        node.set(Bytes(bytes));
                    }
                }
                _ => (),
            }
        }
        Ok(())
    }
}
