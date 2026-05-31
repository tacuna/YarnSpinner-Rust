use antlr4rust::token::Token;

/// Strips an inline `//` comment from a header value string.
///
/// Implements the v3.2.1 grammar behaviour where `HEADER_TEXT` stops at `//`.
/// The old `REST_OF_LINE` rule matched everything including comments; this
/// function trims everything from the first `//` onwards, then right-strips
/// whitespace so `"value // comment"` → `"value"` and `"/"` stays as `"/"`.
pub(crate) fn strip_header_comment(s: &str) -> &str {
    // A single `/` is a valid header value; only `//` starts a comment.
    match s.find("//") {
        Some(idx) => s[..idx].trim_end(),
        None => s,
    }
}

pub(crate) trait TokenExt: Token {
    fn get_line_as_usize(&self) -> usize {
        usize::try_from(self.get_line()).unwrap_or_default()
    }

    fn get_column_as_usize(&self) -> usize {
        usize::try_from(self.get_column()).unwrap_or_default()
    }
}

impl<T: Token + ?Sized> TokenExt for T {}
