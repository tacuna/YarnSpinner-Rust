//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner/YarnSpinner.Markup/MarkupParseResult.cs>
//! which was split into multiple files.

/// A type of [`MarkupAttributeMarker`].
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum TagType {
    /// An open marker. For example, `[a]`.
    Open,
    /// A closing marker. For example, `[/a]`.
    Close,
    /// A self-closing marker. For example, `[a/]`.
    SelfClosing,
    /// The close-all marker, `[/]`.
    CloseAll,
}
