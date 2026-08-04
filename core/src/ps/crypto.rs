use crate::error::MinusOneResult;
use crate::ps::Powershell;
use crate::ps::Powershell::{Bytes, Crypto, Raw, Type};
use crate::ps::Value::Num;
use crate::ps::tool::StringTool;
use crate::ps::utils::bytes::*;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use aes::{Aes128, Aes192, Aes256};
use cbc::cipher::block_padding::{NoPadding, Padding, Pkcs7};
use cbc::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyInit, KeyIvInit};
use log::trace;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AesMode {
    Cbc,
    Ecb,
    Cfb8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AesPadding {
    Pkcs7,
    None,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AesState {
    pub key: Option<Vec<u8>>,
    pub iv: Option<Vec<u8>>,
    pub mode: Option<AesMode>,
    pub padding: Option<AesPadding>,
    pub is_decrypt: Option<bool>,
}

fn aes_cbc_decrypt_padded<P: Padding>(key: &[u8], iv: &[u8], ciphertext: &[u8]) -> Option<Vec<u8>> {
    let mut buf = ciphertext.to_vec();
    let plaintext: &[u8] = match key.len() {
        16 => cbc::Decryptor::<Aes128>::new_from_slices(key, iv)
            .ok()?
            .decrypt_padded::<P>(&mut buf)
            .ok()?,
        24 => cbc::Decryptor::<Aes192>::new_from_slices(key, iv)
            .ok()?
            .decrypt_padded::<P>(&mut buf)
            .ok()?,
        32 => cbc::Decryptor::<Aes256>::new_from_slices(key, iv)
            .ok()?
            .decrypt_padded::<P>(&mut buf)
            .ok()?,
        _ => return None,
    };
    Some(plaintext.to_vec())
}

fn aes_cbc_encrypt_padded<P: Padding>(key: &[u8], iv: &[u8], plaintext: &[u8]) -> Option<Vec<u8>> {
    let mut buf = vec![0u8; plaintext.len() + 16];
    buf[..plaintext.len()].copy_from_slice(plaintext);
    let ciphertext: &[u8] = match key.len() {
        16 => cbc::Encryptor::<Aes128>::new_from_slices(key, iv)
            .ok()?
            .encrypt_padded::<P>(&mut buf, plaintext.len())
            .ok()?,
        24 => cbc::Encryptor::<Aes192>::new_from_slices(key, iv)
            .ok()?
            .encrypt_padded::<P>(&mut buf, plaintext.len())
            .ok()?,
        32 => cbc::Encryptor::<Aes256>::new_from_slices(key, iv)
            .ok()?
            .encrypt_padded::<P>(&mut buf, plaintext.len())
            .ok()?,
        _ => return None,
    };
    Some(ciphertext.to_vec())
}

fn aes_ecb_decrypt_padded<P: Padding>(key: &[u8], ciphertext: &[u8]) -> Option<Vec<u8>> {
    let mut buf = ciphertext.to_vec();
    let plaintext: &[u8] = match key.len() {
        16 => ecb::Decryptor::<Aes128>::new_from_slice(key)
            .ok()?
            .decrypt_padded::<P>(&mut buf)
            .ok()?,
        24 => ecb::Decryptor::<Aes192>::new_from_slice(key)
            .ok()?
            .decrypt_padded::<P>(&mut buf)
            .ok()?,
        32 => ecb::Decryptor::<Aes256>::new_from_slice(key)
            .ok()?
            .decrypt_padded::<P>(&mut buf)
            .ok()?,
        _ => return None,
    };
    Some(plaintext.to_vec())
}

fn aes_ecb_encrypt_padded<P: Padding>(key: &[u8], plaintext: &[u8]) -> Option<Vec<u8>> {
    let mut buf = vec![0u8; plaintext.len() + 16];
    buf[..plaintext.len()].copy_from_slice(plaintext);
    let ciphertext: &[u8] = match key.len() {
        16 => ecb::Encryptor::<Aes128>::new_from_slice(key)
            .ok()?
            .encrypt_padded::<P>(&mut buf, plaintext.len())
            .ok()?,
        24 => ecb::Encryptor::<Aes192>::new_from_slice(key)
            .ok()?
            .encrypt_padded::<P>(&mut buf, plaintext.len())
            .ok()?,
        32 => ecb::Encryptor::<Aes256>::new_from_slice(key)
            .ok()?
            .encrypt_padded::<P>(&mut buf, plaintext.len())
            .ok()?,
        _ => return None,
    };
    Some(ciphertext.to_vec())
}

fn aes_cfb8_decrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Option<Vec<u8>> {
    let mut buf = data.to_vec();
    match key.len() {
        16 => cfb8::Decryptor::<Aes128>::new_from_slices(key, iv)
            .ok()?
            .decrypt(&mut buf),
        24 => cfb8::Decryptor::<Aes192>::new_from_slices(key, iv)
            .ok()?
            .decrypt(&mut buf),
        32 => cfb8::Decryptor::<Aes256>::new_from_slices(key, iv)
            .ok()?
            .decrypt(&mut buf),
        _ => return None,
    };
    Some(buf)
}

fn aes_cfb8_encrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Option<Vec<u8>> {
    let mut buf = data.to_vec();
    match key.len() {
        16 => cfb8::Encryptor::<Aes128>::new_from_slices(key, iv)
            .ok()?
            .encrypt(&mut buf),
        24 => cfb8::Encryptor::<Aes192>::new_from_slices(key, iv)
            .ok()?
            .encrypt(&mut buf),
        32 => cfb8::Encryptor::<Aes256>::new_from_slices(key, iv)
            .ok()?
            .encrypt(&mut buf),
        _ => return None,
    };
    Some(buf)
}

fn aes_transform(state: &AesState, is_decrypt: bool, data: &[u8]) -> Option<Vec<u8>> {
    let key = state.key.as_ref()?;
    let mode = state.mode.unwrap_or(AesMode::Cbc);
    let padding = state.padding.unwrap_or(AesPadding::Pkcs7);

    match (mode, is_decrypt) {
        (AesMode::Cbc, true) => {
            let iv = state.iv.as_ref()?;
            match padding {
                AesPadding::Pkcs7 => aes_cbc_decrypt_padded::<Pkcs7>(key, iv, data),
                AesPadding::None => aes_cbc_decrypt_padded::<NoPadding>(key, iv, data),
            }
        }
        (AesMode::Cbc, false) => {
            let iv = state.iv.as_ref()?;
            match padding {
                AesPadding::Pkcs7 => aes_cbc_encrypt_padded::<Pkcs7>(key, iv, data),
                AesPadding::None => aes_cbc_encrypt_padded::<NoPadding>(key, iv, data),
            }
        }
        (AesMode::Ecb, true) => match padding {
            AesPadding::Pkcs7 => aes_ecb_decrypt_padded::<Pkcs7>(key, data),
            AesPadding::None => aes_ecb_decrypt_padded::<NoPadding>(key, data),
        },
        (AesMode::Ecb, false) => match padding {
            AesPadding::Pkcs7 => aes_ecb_encrypt_padded::<Pkcs7>(key, data),
            AesPadding::None => aes_ecb_encrypt_padded::<NoPadding>(key, data),
        },
        (AesMode::Cfb8, true) => aes_cfb8_decrypt(key, state.iv.as_ref()?, data),
        (AesMode::Cfb8, false) => aes_cfb8_encrypt(key, state.iv.as_ref()?, data),
    }
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

fn is_cipher_mode_typename(typename: &str) -> bool {
    let t = typename.strip_prefix("system.").unwrap_or(typename);
    t == "security.cryptography.ciphermode"
}

fn is_padding_mode_typename(typename: &str) -> bool {
    let t = typename.strip_prefix("system.").unwrap_or(typename);
    t == "security.cryptography.paddingmode"
}

fn parse_aes_mode(name: &str) -> Option<AesMode> {
    match name {
        "cbc" => Some(AesMode::Cbc),
        "ecb" => Some(AesMode::Ecb),
        // .net's CFB looks to be CFB-8
        "cfb" => Some(AesMode::Cfb8),
        _ => None,
    }
}

fn parse_aes_padding(name: &str) -> Option<AesPadding> {
    match name {
        "pkcs7" => Some(AesPadding::Pkcs7),
        "none" => Some(AesPadding::None),
        _ => None,
    }
}

pub fn assign_aes_property(state: &mut AesState, member: &str, value: &Powershell) -> bool {
    match member {
        "key" => {
            if let Some(bytes) = bytes_from_data(value) {
                state.key = Some(bytes);
                return true;
            }
        }
        "iv" => {
            if let Some(bytes) = bytes_from_data(value) {
                state.iv = Some(bytes);
                return true;
            }
        }
        "mode" => {
            if let Type(typename) = value
                && let Some(tag) = typename.strip_prefix("crypto.mode.")
                && let Some(mode) = parse_aes_mode(tag)
            {
                state.mode = Some(mode);
                return true;
            }
        }
        "padding" => {
            if let Type(typename) = value
                && let Some(tag) = typename.strip_prefix("crypto.padding.")
                && let Some(padding) = parse_aes_padding(tag)
            {
                state.padding = Some(padding);
                return true;
            }
        }
        _ => (),
    }
    false
}

/// Resolves an AES algorithm object down to a `CreateDecryptor`/`CreateEncryptor` transform, from
/// any of the ways PowerShell obfuscators reach one:
///
/// - `New-Object System.Security.Cryptography.AesManaged`
/// - `[System.Security.Cryptography.AesManaged]::new()`
/// - `[System.Security.Cryptography.Aes]::Create()`
///
/// The resulting object is tracked as a [`Powershell::Crypto`] value. `.Key`/`.IV`/`.Mode`/
/// `.Padding` property assignments on a variable holding it are folded in by [`crate::ps::var::Var`]
/// (via [`assign_aes_property`]), and `.CreateDecryptor(...)`/`.CreateEncryptor(...)` (with or
/// without explicit key/iv arguments) resolves to a transform, later consumed by
/// [`AesTransformFinalBlock`].
///
/// Also resolves `[System.Security.Cryptography.CipherMode]`/`[...PaddingMode]` static members to
/// `crypto.mode.*`/`crypto.padding.*` type tags, so they can be assigned via `.Mode`/`.Padding`.
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

        if view.kind() == "member_access"
            && let (Some(type_node), Some(op), Some(member_name)) =
                (view.child(0), view.child(1), view.child(2))
            && op.text()? == "::"
            && let Some(Type(typename)) = type_node.data()
        {
            let member = member_name.text()?.to_string().normalize();
            if is_cipher_mode_typename(typename) && parse_aes_mode(&member).is_some() {
                trace!(
                    "AesType (L): Setting node with cipher mode type: {}",
                    member
                );
                node.set(Type(format!("crypto.mode.{}", member)));
            } else if is_padding_mode_typename(typename) && parse_aes_padding(&member).is_some() {
                trace!(
                    "AesType (L): Setting node with padding mode type: {}",
                    member
                );
                node.set(Type(format!("crypto.padding.{}", member)));
            }
        } else if view.kind() == "invokation_expression"
            && let (Some(type_node), Some(op), Some(member_name), Some(args_list)) =
                (view.child(0), view.child(1), view.child(2), view.child(3))
        {
            let op_text = op.text()?;
            let member = member_name.text()?.to_string().normalize();

            if op_text == "::"
                && let Some(Type(typename)) = type_node.data()
                && args_list.named_child("argument_expression_list").is_none()
                && ((member == "new" && is_aes_constructor_typename(typename))
                    || (member == "create" && is_aes_factory_typename(typename)))
            {
                trace!("AesType (L): Setting node with AES algorithm object");
                node.set(Crypto(AesState::default()));
                return Ok(());
            }

            if op_text == "."
                && let Some(Crypto(state)) = type_node.data()
                && (member == "createdecryptor" || member == "createencryptor")
            {
                let mut new_state = state.clone();
                new_state.is_decrypt = Some(member == "createdecryptor");

                if let Some(argument_expression_list) =
                    args_list.named_child("argument_expression_list")
                {
                    if let (Some(arg_key), Some(arg_iv)) = (
                        argument_expression_list.child(0),
                        argument_expression_list.child(2),
                    ) && let (Some(key), Some(iv)) = (
                        arg_key.data().and_then(bytes_from_data),
                        arg_iv.data().and_then(bytes_from_data),
                    ) {
                        new_state.key = Some(key);
                        new_state.iv = Some(iv);
                        trace!(
                            "AesType (L): Setting node with AES transform object (explicit key/iv)"
                        );
                        node.set(Crypto(new_state));
                    }
                } else {
                    // no argument: relies on .Key/.IV already assigned on the algorithm object
                    trace!("AesType (L): Setting node with AES transform object (from state)");
                    node.set(Crypto(new_state));
                }
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
                && is_aes_constructor_typename(&typename.to_lowercase())
            {
                trace!("AesType (L): Setting node with New-Object AES algorithm object");
                node.set(Crypto(AesState::default()));
            }
        }
        Ok(())
    }
}

/// This rule infers `TransformFinalBlock(bytes, offset, count)` calls on a resolved
/// [`AesType`] transform, decrypting/encrypting the byte range according to the transform's
/// tracked [`AesState`] (CBC/PKCS7 unless `Mode`/`Padding` were explicitly overridden, matching
/// the .NET defaults for `AesManaged`/`AesCryptoServiceProvider`/`RijndaelManaged`).
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
            && let Some(Crypto(state)) = type_node.data()
            && let Some(is_decrypt) = state.is_decrypt
            && state.key.is_some()
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
            && let Some(result) = aes_transform(state, is_decrypt, slice)
        {
            trace!(
                "AesTransformFinalBlock (L): Setting node with transformed bytes: {:?}",
                result
            );
            node.set(Bytes(result));
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::ps::Powershell::Bytes;
    use crate::ps::build_powershell_tree;
    use crate::ps::crypto::{AesState, AesTransformFinalBlock, AesType};
    use crate::ps::encoding::{EncodingGetBytes, EncodingType};
    use crate::ps::forward::Forward;
    use crate::ps::integer::ParseInt;
    use crate::ps::method::DecodeBase64;
    use crate::ps::string::ParseString;
    use crate::ps::typing::ParseType;
    use crate::ps::var::Var;
    use base64::{Engine as _, engine::general_purpose};

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

        let plaintext = super::aes_transform(
            &AesState {
                key: Some(b"0123456789abcdef".to_vec()),
                iv: Some(b"abcdef0123456789".to_vec()),
                mode: None,
                padding: None,
                is_decrypt: Some(true),
            },
            true,
            &ciphertext,
        )
        .unwrap();
        assert_eq!(plaintext, b"secretmsg");
    }

    #[test]
    fn test_aes_decrypt_via_property_assignment_cfb() {
        let key = b"0123456789abcdef";
        let iv = b"abcdef0123456789";
        let plaintext = b"synthetic-msg!!!"; // 16 bytes, block-aligned so CFB8 needs no padding talk
        let ciphertext = super::aes_cfb8_encrypt(key, iv, plaintext).unwrap();
        let ciphertext_b64 = general_purpose::STANDARD.encode(&ciphertext);

        let source = format!(
            r#"
$aes = New-Object System.Security.Cryptography.AesCryptoServiceProvider
$aes.Mode = [System.Security.Cryptography.CipherMode]::CFB
$aes.Padding = [System.Security.Cryptography.PaddingMode]::None
$aes.Key = [System.text.encoding]::UTF8.GetBytes("0123456789abcdef")
$aes.IV = [System.text.encoding]::UTF8.GetBytes("abcdef0123456789")
$decryptor = $aes.CreateDecryptor()
$decryptor.TransformFinalBlock([Convert]::FromBase64String("{}"), 0, {})
"#,
            ciphertext_b64,
            ciphertext.len()
        );

        let mut tree = build_powershell_tree(&source).unwrap();
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
            Var::default(),
        ))
        .unwrap();

        let last_statement = tree
            .root()
            .unwrap()
            .child(0)
            .unwrap()
            .child(6)
            .unwrap()
            .data()
            .expect("Inferred type")
            .clone();

        assert_eq!(last_statement, Bytes(plaintext.to_vec()));
    }
}
