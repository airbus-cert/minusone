use crate::error::MinusOneResult;
use crate::ps::Powershell;
use crate::ps::Powershell::{Bytes, Raw, Type};
use crate::ps::Value::Num;
use crate::ps::tool::StringTool;
use crate::ps::utils::bytes::*;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use aes::{Aes128, Aes192, Aes256};
use cbc::cipher::block_padding::Pkcs7;
use cbc::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit};
use cbc::{Decryptor, Encryptor};
use log::trace;

fn aes_cbc_decrypt(key: &[u8], iv: &[u8], ciphertext: &[u8]) -> Option<Vec<u8>> {
    let mut buf = ciphertext.to_vec();
    let plaintext: &[u8] = match key.len() {
        16 => Decryptor::<Aes128>::new_from_slices(key, iv)
            .ok()?
            .decrypt_padded::<Pkcs7>(&mut buf)
            .ok()?,
        24 => Decryptor::<Aes192>::new_from_slices(key, iv)
            .ok()?
            .decrypt_padded::<Pkcs7>(&mut buf)
            .ok()?,
        32 => Decryptor::<Aes256>::new_from_slices(key, iv)
            .ok()?
            .decrypt_padded::<Pkcs7>(&mut buf)
            .ok()?,
        _ => return None,
    };
    Some(plaintext.to_vec())
}

fn aes_cbc_encrypt(key: &[u8], iv: &[u8], plaintext: &[u8]) -> Option<Vec<u8>> {
    let mut buf = vec![0u8; plaintext.len() + 16];
    buf[..plaintext.len()].copy_from_slice(plaintext);
    let ciphertext: &[u8] = match key.len() {
        16 => Encryptor::<Aes128>::new_from_slices(key, iv)
            .ok()?
            .encrypt_padded::<Pkcs7>(&mut buf, plaintext.len())
            .ok()?,
        24 => Encryptor::<Aes192>::new_from_slices(key, iv)
            .ok()?
            .encrypt_padded::<Pkcs7>(&mut buf, plaintext.len())
            .ok()?,
        32 => Encryptor::<Aes256>::new_from_slices(key, iv)
            .ok()?
            .encrypt_padded::<Pkcs7>(&mut buf, plaintext.len())
            .ok()?,
        _ => return None,
    };
    Some(ciphertext.to_vec())
}

fn is_aes_constructor_typename(typename: &str) -> bool {
    let t = typename.strip_prefix("system.").unwrap_or(typename);
    matches!(
        t,
        "security.cryptography.aesmanaged"
            | "security.cryptography.aescryptoserviceprovider"
            | "security.cryptography.rijndaelmanaged"
    )
}

fn is_aes_factory_typename(typename: &str) -> bool {
    let t = typename.strip_prefix("system.").unwrap_or(typename);
    t == "security.cryptography.aes"
}

fn parse_aes_transform_tag(typename: &str) -> Option<(bool, Vec<u8>, Vec<u8>)> {
    let (is_decrypt, rest) = if let Some(rest) = typename.strip_prefix("crypto.aes.decryptor.") {
        (true, rest)
    } else if let Some(rest) = typename.strip_prefix("crypto.aes.encryptor.") {
        (false, rest)
    } else {
        return None;
    };
    let mut parts = rest.splitn(2, '.');
    let key = from_hex(parts.next()?)?;
    let iv = from_hex(parts.next()?)?;
    Some((is_decrypt, key, iv))
}

/// Resolves an AES algorithm object down to a `CreateDecryptor`/`CreateEncryptor` transform, from
/// any of the ways PowerShell obfuscators reach one:
///
/// - `New-Object System.Security.Cryptography.AesManaged`
/// - `[System.Security.Cryptography.AesManaged]::new()`
/// - `[System.Security.Cryptography.Aes]::Create()`
///
/// followed by `.CreateDecryptor(key, iv)` / `.CreateEncryptor(key, iv)` with constant key/iv
/// bytes. The resolved value is a `crypto.aes.{decryptor,encryptor}.<keyhex>.<ivhex>` type tag,
/// later consumed by [`AesTransformFinalBlock`].
///
/// # Example
/// ```
/// use minusone::ps::build_powershell_tree;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::linter::Linter;
/// use minusone::ps::typing::ParseType;
/// use minusone::ps::string::ParseString;
/// use minusone::ps::integer::ParseInt;
/// use minusone::ps::encoding::{EncodingType, EncodingGetBytes};
/// use minusone::ps::crypto::{AesType, AesTransformFinalBlock};
///
/// let mut tree = build_powershell_tree(
///     "(New-Object System.Security.Cryptography.AesManaged).CreateDecryptor([System.Text.Encoding]::UTF8.GetBytes('0123456789abcdef'), [System.Text.Encoding]::UTF8.GetBytes('abcdef0123456789')).TransformFinalBlock([System.Text.Encoding]::UTF8.GetBytes('foo'), 0, 3)"
/// ).unwrap();
/// tree.apply_mut(&mut (
///     Forward::default(),
///     ParseType::default(),
///     ParseString::default(),
///     ParseInt::default(),
///     EncodingType::default(),
///     EncodingGetBytes::default(),
///     AesType::default(),
///     AesTransformFinalBlock::default(),
/// )).unwrap();
///
/// let mut ps_litter_view = Linter::default();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// // 'foo' isn't a valid padded AES block, it's just an example
/// assert!(!ps_litter_view.output.is_empty());
/// ```
#[derive(Default)]
pub struct AesType;

impl<'a> RuleMut<'a> for AesType {
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
            && let (Some(type_node), Some(op), Some(member_name), Some(args_list)) =
                (view.child(0), view.child(1), view.child(2), view.child(3))
        {
            let op_text = op.text()?;
            let member = member_name.text()?.to_string().normalize();

            if op_text == "::"
                && let Some(Type(typename)) = type_node.data()
                && args_list.named_child("argument_expression_list").is_none()
            {
                if member == "new" && is_aes_constructor_typename(typename) {
                    trace!("AesType (L): Setting node with AES algorithm type");
                    node.set(Type("crypto.aes".to_string()));
                    return Ok(());
                } else if member == "create" && is_aes_factory_typename(typename) {
                    trace!("AesType (L): Setting node with AES algorithm type");
                    node.set(Type("crypto.aes".to_string()));
                    return Ok(());
                }
            }

            if let Some(Type(typename)) = type_node.data()
                && typename == "crypto.aes"
                && op_text == "."
                && (member == "createdecryptor" || member == "createencryptor")
                && let Some(argument_expression_list) =
                    args_list.named_child("argument_expression_list")
                && let (Some(arg_key), Some(arg_iv)) = (
                    argument_expression_list.child(0),
                    argument_expression_list.child(2),
                )
                && let (Some(key), Some(iv)) = (
                    arg_key.data().and_then(bytes_from_data),
                    arg_iv.data().and_then(bytes_from_data),
                )
            {
                let kind = if member == "createdecryptor" {
                    "decryptor"
                } else {
                    "encryptor"
                };
                let tag = format!("crypto.aes.{}.{}.{}", kind, to_hex(&key), to_hex(&iv));
                trace!("AesType (L): Setting node with AES transform type: {}", tag);
                node.set(Type(tag));
            }
        } else if view.kind() == "command"
            && let (Some(command_name), Some(command_elements)) = (
                view.named_child("command_name"),
                view.named_child("command_elements"),
            )
            && command_name
                .text()
                .is_ok_and(|name| name.to_lowercase() == "new-object")
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
                && is_aes_constructor_typename(&typename.to_lowercase())
            {
                trace!("AesType (L): Setting node with New-Object AES algorithm type");
                node.set(Type("crypto.aes".to_string()));
            }
        }
        Ok(())
    }
}

/// This rule infers `TransformFinalBlock(bytes, offset, count)` calls on a resolved
/// [`AesType`] transform, decrypting/encrypting the byte range with AES-CBC/PKCS7 (the .NET
/// defaults for `AesManaged`/`AesCryptoServiceProvider`/`RijndaelManaged` when `Mode`/`Padding`
/// aren't explicitly overridden).
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
/// use minusone::ps::encoding::{EncodingType, EncodingGetBytes};
/// use minusone::ps::crypto::{AesType, AesTransformFinalBlock};
///
/// let mut tree = build_powershell_tree(
///     r#"(New-Object System.Security.Cryptography.AesManaged).CreateDecryptor([System.text.encoding]::UTF8.GetBytes("0123456789abcdef"), [System.text.encoding]::UTF8.GetBytes("abcdef0123456789")).TransformFinalBlock([Convert]::FromBase64String("H6eyhfvPHCP9VlON0Wfk9cx+9TT3kIu2PZ4DdCcvAaY="), 0, 32)"#
/// ).unwrap();
/// tree.apply_mut(&mut (
///     Forward::default(),
///     ParseType::default(),
///     ParseString::default(),
///     ParseInt::default(),
///     EncodingType::default(),
///     EncodingGetBytes::default(),
///     DecodeBase64::default(),
///     AesType::default(),
///     AesTransformFinalBlock::default(),
/// )).unwrap();
///
/// let mut ps_litter_view = Linter::default();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// // Decodes to "Hello, minusone!" (the PKCS7 padding is stripped automatically).
/// assert_eq!(
///     ps_litter_view.output,
///     "@(72, 101, 108, 108, 111, 44, 32, 109, 105, 110, 117, 115, 111, 110, 101, 33)"
/// );
/// ```
#[derive(Default)]
pub struct AesTransformFinalBlock;

impl<'a> RuleMut<'a> for AesTransformFinalBlock {
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
            && let (Some(type_node), Some(op), Some(member_name), Some(args_list)) =
                (view.child(0), view.child(1), view.child(2), view.child(3))
            && op.text()? == "."
            && member_name.text()?.to_string().normalize() == "transformfinalblock"
            && let Some(Type(typename)) = type_node.data()
            && let Some((is_decrypt, key, iv)) = parse_aes_transform_tag(typename)
            && let Some(argument_expression_list) =
                args_list.named_child("argument_expression_list")
            && let (Some(arg_data), Some(arg_offset), Some(arg_count)) = (
                argument_expression_list.child(0),
                argument_expression_list.child(2),
                argument_expression_list.child(4),
            )
            && let Some(data) = arg_data.data().and_then(bytes_from_data)
            && let (Some(Raw(Num(offset))), Some(Raw(Num(count)))) =
                (arg_offset.data(), arg_count.data())
            && *offset >= 0
            && *count >= 0
            && let Some(slice) = data.get(*offset as usize..(*offset + *count) as usize)
        {
            let result = if is_decrypt {
                aes_cbc_decrypt(&key, &iv, slice)
            } else {
                aes_cbc_encrypt(&key, &iv, slice)
            };
            if let Some(bytes) = result {
                trace!(
                    "AesTransformFinalBlock (L): Setting node with transformed bytes: {:?}",
                    bytes
                );
                node.set(Bytes(bytes));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::ps::Powershell::Bytes;
    use crate::ps::build_powershell_tree;
    use crate::ps::crypto::{AesTransformFinalBlock, AesType};
    use crate::ps::encoding::{EncodingGetBytes, EncodingType};
    use crate::ps::forward::Forward;
    use crate::ps::integer::ParseInt;
    use crate::ps::method::DecodeBase64;
    use crate::ps::string::ParseString;
    use crate::ps::typing::ParseType;

    fn root_data(
        tree: &crate::tree::Tree<crate::tree::HashMapStorage<crate::ps::Powershell>>,
    ) -> crate::ps::Powershell {
        tree.root()
            .unwrap()
            .child(0)
            .unwrap()
            .child(0)
            .unwrap()
            .data()
            .expect("Inferred type")
            .clone()
    }

    #[test]
    fn test_aes_decrypt_new_object() {
        let mut tree = build_powershell_tree(
            r#"(New-Object System.Security.Cryptography.AesManaged).CreateDecryptor([System.text.encoding]::UTF8.GetBytes("0123456789abcdef"), [System.text.encoding]::UTF8.GetBytes("abcdef0123456789")).TransformFinalBlock([Convert]::FromBase64String("H6eyhfvPHCP9VlON0Wfk9cx+9TT3kIu2PZ4DdCcvAaY="), 0, 32)"#,
        )
        .unwrap();
        tree.apply_mut(&mut (
            Forward::default(),
            ParseType::default(),
            ParseString::default(),
            ParseInt::default(),
            EncodingType::default(),
            EncodingGetBytes::default(),
            DecodeBase64::default(),
            AesType::default(),
            AesTransformFinalBlock::default(),
        ))
        .unwrap();

        // ASCII codes of "Hello, minusone!"
        assert_eq!(
            root_data(&tree),
            Bytes(vec![
                72, 101, 108, 108, 111, 44, 32, 109, 105, 110, 117, 115, 111, 110, 101, 33
            ])
        );
    }

    #[test]
    fn test_aes_decrypt_new_syntax() {
        let mut tree = build_powershell_tree(
            r#"[System.Security.Cryptography.AesManaged]::new().CreateDecryptor([System.text.encoding]::UTF8.GetBytes("0123456789abcdef"), [System.text.encoding]::UTF8.GetBytes("abcdef0123456789")).TransformFinalBlock([Convert]::FromBase64String("H6eyhfvPHCP9VlON0Wfk9cx+9TT3kIu2PZ4DdCcvAaY="), 0, 32)"#,
        )
        .unwrap();
        tree.apply_mut(&mut (
            Forward::default(),
            ParseType::default(),
            ParseString::default(),
            ParseInt::default(),
            EncodingType::default(),
            EncodingGetBytes::default(),
            DecodeBase64::default(),
            AesType::default(),
            AesTransformFinalBlock::default(),
        ))
        .unwrap();

        assert_eq!(
            root_data(&tree),
            Bytes(vec![
                72, 101, 108, 108, 111, 44, 32, 109, 105, 110, 117, 115, 111, 110, 101, 33
            ])
        );
    }

    #[test]
    fn test_aes_encrypt_round_trip_via_create() {
        let mut tree = build_powershell_tree(
            r#"[System.Security.Cryptography.Aes]::Create().CreateEncryptor([System.text.encoding]::UTF8.GetBytes("0123456789abcdef"), [System.text.encoding]::UTF8.GetBytes("abcdef0123456789")).TransformFinalBlock([System.text.encoding]::UTF8.GetBytes("secretmsg"), 0, 9)"#,
        )
        .unwrap();
        tree.apply_mut(&mut (
            Forward::default(),
            ParseType::default(),
            ParseString::default(),
            ParseInt::default(),
            EncodingType::default(),
            EncodingGetBytes::default(),
            AesType::default(),
            AesTransformFinalBlock::default(),
        ))
        .unwrap();

        let Bytes(ciphertext) = root_data(&tree) else {
            panic!("Expected Bytes")
        };

        let plaintext =
            super::aes_cbc_decrypt(b"0123456789abcdef", b"abcdef0123456789", &ciphertext).unwrap();
        assert_eq!(plaintext, b"secretmsg");
    }
}
