use alloc::string::String;

#[derive(Debug, Default, Clone, Copy)]
/// A Windows Codepage replacement, which only supports ascii.
pub struct Codepage;

impl Codepage {
    pub(crate) fn decode(&self, v: &[u8]) -> String {
        let mut result = String::with_capacity(v.len() * 2);

        for c in v {
            if *c < 128 {
                result.push(char::from(*c));
            } else {
                result.push(char::REPLACEMENT_CHARACTER)
            }
        }

        result
    }

    pub(crate) fn encode_char_checked(&self, c: char) -> Option<u8> {
        if c.is_ascii() {
            Some(c as u8)
        } else {
            None
        }
    }
}
