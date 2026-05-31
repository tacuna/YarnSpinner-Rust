//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner/YarnSpinner.Markup/MarkupParseResult.cs>
//! which was split into multiple files.

use crate::markup::{MarkupValue, TagType};
use crate::prelude::*;
use bevy_platform::collections::HashMap;

/// Represents a marker (e.g. `[a]`) in line of marked up text.
///
/// You do not create instances of this struct yourself. It is created
/// by objects that can parse markup, such as [`Dialogue`].
///
/// When implementing [`AttributeMarkerProcessor`], you receive this struct
/// in [`replacement_text_for_marker`](AttributeMarkerProcessor::replacement_text_for_marker)
/// to inspect the marker's name, properties, and type.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkupAttributeMarker {
    /// The name of the marker.
    /// For example, the marker `[wave]` has the name `wave`.
    pub name: Option<String>,
    /// The position of the marker in the plain text.
    pub position: usize,
    /// The list of properties associated with this marker.
    pub properties: HashMap<String, MarkupValue>,
    /// The type of marker that this is.
    pub tag_type: TagType,
    /// The position of this marker in the original source text.
    pub source_position: usize,
}
