//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner/YarnSpinner.Markup/MarkupParseResult.cs>

pub use self::markup_attribute::*;
pub use self::markup_attribute_marker::*;
pub use self::markup_value::*;
pub use self::tag_type::*;
use crate::prelude::*;
use core::fmt::Debug;

mod markup_attribute;
mod markup_attribute_marker;
mod markup_value;
mod tag_type;

/// The result of parsing a line of marked-up text.
///
/// You do not create instances of this struct yourself. It is created
/// by objects that can parse markup, such as [`Dialogue`].
///
/// ## Implementation Notes
/// - This is called `MarkupParseResult` in the original C# code, but was renamed because [`Result`] already carries meaning in Rust.
/// - The API has been merged with [`Line`], so this is now only an internal type.

#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct ParsedMarkup {
    /// The original text, with all parsed markers removed.
    pub text: String,
    /// The list of [`MarkupAttribute`] in this parse result.
    pub attributes: Vec<MarkupAttribute>,
}

impl ParsedMarkup {
    /// Creates an empty [`ParsedMarkup`] with no text and no attributes.
    pub fn new() -> Self {
        Self::default()
    }
}
