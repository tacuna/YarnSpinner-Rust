//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner.Tests/TestPlan.cs>

use std::fmt::Debug;
use std::io::Read;
use std::str::FromStr;

pub(crate) struct Reader<'a> {
    content: &'a [u8],
}

impl<'a> Reader<'a> {
    pub(crate) fn new(content: &'a str) -> Self {
        Self { content: content.as_bytes() }
    }

    pub(crate) fn read_next<T>(&mut self) -> T
    where
        T: FromStr,
        <T as FromStr>::Err: Debug,
    {
        let string = self.read_next_raw();
        string.parse().unwrap()
    }

    pub(crate) fn read_to_end(&mut self) -> String {
        let mut result = String::new();
        self.content.read_to_string(&mut result).unwrap();
        result
    }

    /// Parse the next T from this string, ignoring leading whitespace
    fn read_next_raw(&mut self) -> String {
        let mut string = String::new();
        loop {
            let Some(character) = self.read_char() else {
                break;
            };
            if character.is_whitespace() {
                // eat leading whitespace
                continue;
            }
            string.push(character);

            if let Some(next) = self.peek_char()
                && (next.is_alphanumeric() || next == '_')
            {
                continue;
            }
            break;
        }
        string
    }

    fn read_char(&mut self) -> Option<char> {
        let mut buf = [0u8; 1];
        let read = self.content.read(&mut buf).unwrap();
        (read != 0).then_some(buf[0] as char)
    }

    fn peek_char(&mut self) -> Option<char> {
        let buf = self.content.first()?;
        Some(*buf as char)
    }
}
