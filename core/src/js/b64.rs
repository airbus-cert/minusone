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
            && let (Some(identifier), Some(arguments)) = (view.child(0), view.child(1)) {
                if identifier.text()? == "atob" {
                    if arguments.child_count() == 3 {
                        if let Some(encoded_string) = arguments.child(1)
                            && let Some(Raw(Str(encoded))) = encoded_string.data() {
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

                                let decoded_string =
                                    String::from_utf8_lossy(&decoded_bytes).to_string();

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
                            && let Some(Raw(Str(decoded))) = decoded_string.data() {
                                let encoded_string = GeneralPurpose::new(
                                    &alphabet::STANDARD,
                                    GeneralPurposeConfig::new(),
                                )
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

#[cfg(test)]
mod tests_js_b64 {
    use crate::js::b64::B64;
    use crate::js::b64::js_bytes_to_string;
    use crate::js::build_javascript_tree;
    use crate::js::linter::Linter;
    use crate::js::string::ParseString;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (ParseString::default(), B64::default()))
            .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_parse_b64() {
        assert_eq!(
            deobfuscate("var x = atob('bWludXNvbmU=');"),
            "var x = 'minusone';",
        );
    }

    #[test]
    fn test_parse_b64_encode() {
        assert_eq!(
            deobfuscate("var x = btoa('minusone');"),
            "var x = 'bWludXNvbmU=';",
        );
    }

    #[test]
    fn test_bytes_to_string() {
        let mut bytes = Vec::new();
        for i in 0..=255 {
            bytes.push(i);
        }
        let decoded_string = js_bytes_to_string(&bytes);
        println!("{}", decoded_string);
        assert_eq!(
            decoded_string,
            "\\x00\\x01\\x02\\x03\\x04\\x05\\x06\\x07\\b\\t\\n\\v\\f\\r\\x0E\\x0F\\x10\\x11\\x12\\x13\\x14\\x15\\x16\\x17\\x18\\x19\\x1A\\x1B\\x1C\\x1D\\x1E\\x1F !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~\\x7F\\x80\\x81\\x82\\x83\\x84\\x85\\x86\\x87\\x88\\x89\\x8A\\x8B\\x8C\\x8D\\x8E\\x8F\\x90\\x91\\x92\\x93\\x94\\x95\\x96\\x97\\x98\\x99\\x9A\\x9B\\x9C\\x9D\\x9E\\x9F ¡¢£¤¥¦§¨©ª«¬­®¯°±²³´µ¶·¸¹º»¼½¾¿ÀÁÂÃÄÅÆÇÈÉÊËÌÍÎÏÐÑÒÓÔÕÖ×ØÙÚÛÜÝÞßàáâãäåæçèéêëìíîïðñòóôõö÷øùúûüýþÿ"
        );
    }
}
