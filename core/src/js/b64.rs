use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::*;
use crate::js::Value::*;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use base64::{Engine, alphabet};
use log::{trace, warn};

/// Base64 encoding and decoding
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::string::ParseString;
/// use minusone::js::b64::B64;
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = atob('bWludXNvbmU=');").unwrap();
/// tree.apply_mut(&mut (ParseString::default(), B64::default())).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "var x = 'minusone';");
/// ```
#[derive(Default)]
pub struct B64;

impl<'a> RuleMut<'a> for B64 {
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
        if view.kind() == "call_expression"
            && let (Some(identifier), Some(arguments)) = (view.child(0), view.child(1))
        {
            if identifier.text()? == "atob" {
                if arguments.child_count() == 3 {
                    if let Some(encoded_string) = arguments.child(1)
                        && let Some(Raw(Str(encoded))) = encoded_string.data()
                    {
                        let config = GeneralPurposeConfig::new()
                            .with_decode_padding_mode(DecodePaddingMode::Indifferent)
                            .with_decode_allow_trailing_bits(true);

                        let engine = GeneralPurpose::new(&alphabet::STANDARD, config);

                        let decoded_bytes = match engine.decode(encoded) {
                            Ok(bytes) => bytes,
                            Err(e) => {
                                warn!(
                                    "ParseB64: failed to decode base64 string '{}': {}",
                                    encoded, e
                                );
                                return Ok(());
                            }
                        };

                        let decoded_string = String::from_utf8_lossy(&decoded_bytes).to_string();

                        trace!(
                            "ParseB64: decoded base64 string '{}' to '{}'",
                            encoded, decoded_string
                        );
                        node.reduce(Raw(Str(decoded_string)));
                    }
                } else {
                    warn!(
                        "ParseB64: atob called with unexpected number of arguments: {}",
                        arguments.child_count() - 2
                    );
                }
            } else if identifier.text()? == "btoa" {
                if arguments.child_count() == 3 {
                    if let Some(decoded_string) = arguments.child(1)
                        && let Some(Raw(Str(decoded))) = decoded_string.data()
                    {
                        let encoded_string =
                            GeneralPurpose::new(&alphabet::STANDARD, GeneralPurposeConfig::new())
                                .encode(decoded.as_bytes());

                        trace!(
                            "ParseB64: encoded string '{}' to base64 '{}'",
                            decoded, encoded_string
                        );
                        node.reduce(Raw(Str(encoded_string)));
                    }
                } else {
                    warn!(
                        "ParseB64: btoa called with unexpected number of arguments: {}",
                        arguments.child_count() - 2
                    );
                }
            }
        }

        Ok(())
    }
}

pub fn js_bytes_to_string(bytes: &[u8]) -> String {
    let mut string = String::new();
    for &byte in bytes {
        if byte <= 0x07 {
            string.push_str(&format!("\\x{:02X}", byte));
        } else if byte == 0x08 {
            string.push_str("\\b");
        } else if byte == 0x09 {
            string.push_str("\\t");
        } else if byte == 0x0a {
            string.push_str("\\n");
        } else if byte == 0x0b {
            string.push_str("\\v");
        } else if byte == 0x0c {
            string.push_str("\\f");
        } else if byte == 0x0d {
            string.push_str("\\r");
        } else if byte <= 0x1f {
            string.push_str(&format!("\\x{:02X}", byte));
        } else if byte < 0x7f {
            string.push(byte as char);
        } else if byte <= 0x9f {
            string.push_str(&format!("\\x{:02X}", byte));
        } else {
            string.push(byte as char);
        }
    }
    string
}
