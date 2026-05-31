//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner/YarnSpinner.Markup/IAttributeMarkerProcessor.cs>

pub use self::dialogue_text_processor::*;
pub(crate) use self::no_markup_text_processor::*;
use crate::markup::MarkupAttributeMarker;
use crate::prelude::*;
use core::fmt::Debug;

mod dialogue_text_processor;
mod no_markup_text_processor;

/// Provides a mechanism for producing replacement text for a marker.
///
/// Implement this trait to create custom markup processors that can
/// dynamically replace text in dialogue lines. Register your processor
/// with [`Dialogue::register_marker_processor`].
///
/// For example, you might create a processor that handles `[color=red]text[/color]`
/// markers by stripping the markers and recording the color information.
pub trait AttributeMarkerProcessor: Debug + Send + Sync {
    /// Produces the replacement text that should be inserted into a parse
    /// result for a given attribute.
    ///
    /// If the marker is an `open` marker, the text from the marker's
    /// position to its corresponding closing marker is provided as a string
    /// property called `contents`.
    fn replacement_text_for_marker(&self, marker: &MarkupAttributeMarker) -> String;
    /// Called when the language code changes (e.g. for localization).
    fn set_language_code(&mut self, language_code: Option<Language>);
    /// Creates a boxed clone of this processor.
    fn clone_box(&self) -> Box<dyn AttributeMarkerProcessor>;
}

impl Clone for Box<dyn AttributeMarkerProcessor> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
