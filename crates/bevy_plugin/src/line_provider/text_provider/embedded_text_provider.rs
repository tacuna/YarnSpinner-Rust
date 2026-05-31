use crate::UnderlyingTextProvider;
use crate::prelude::*;

use bevy::prelude::*;
use std::any::Any;
use std::collections::HashMap;

/// A [`TextProvider`] that serves all locale strings from memory, with no filesystem I/O.
///
/// Used in non-dev (precompiled) builds where all translations are packed into the binary
/// data blob at build time. This provider is constructed directly from the deserialized
/// locale tables and is ready to use immediately — no asset loading or event polling needed.
#[derive(Debug, Clone)]
pub struct EmbeddedTextProvider {
    base_string_table: HashMap<LineId, StringInfo>,
    /// Maps a BCP-47 language tag to a table of `LineId → translated text`.
    locale_tables: HashMap<Language, HashMap<LineId, String>>,
    current_language: Option<Language>,
}

impl EmbeddedTextProvider {
    /// Constructs a new [`EmbeddedTextProvider`].
    ///
    /// - `base_string_table`: base-language strings sourced from `compilation.string_table`.
    /// - `locale_tables`: all translations, keyed by BCP-47 language tag.
    pub fn new(base_string_table: HashMap<LineId, StringInfo>, locale_tables: HashMap<Language, HashMap<LineId, String>>) -> Self {
        Self {
            base_string_table,
            locale_tables,
            current_language: None,
        }
    }
}

impl UnderlyingTextProvider for EmbeddedTextProvider {
    fn clone_shallow(&self) -> Box<dyn UnderlyingTextProvider> {
        Box::new(self.clone())
    }

    fn accept_line_hints(&mut self, _line_ids: &[LineId]) {
        // All strings are already in memory — no prefetching required.
    }

    fn get_text(&self, id: &LineId) -> Option<String> {
        if let Some(lang) = &self.current_language {
            if let Some(table) = self.locale_tables.get(lang) {
                if let Some(text) = table.get(id) {
                    return Some(text.clone());
                }
                warn!(
                    "EmbeddedTextProvider: no translation for line {:?} in language {:?}, falling back to base language.",
                    id, lang
                );
            }
        }
        self.base_string_table.get(id).map(|info| info.text.clone())
    }

    fn set_language(&mut self, language: Option<Language>) {
        self.current_language = language;
    }

    fn get_language(&self) -> Option<Language> {
        self.current_language.clone()
    }

    fn are_lines_available(&self) -> bool {
        true // Everything is already in memory.
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl TextProvider for EmbeddedTextProvider {
    fn set_base_string_table(&mut self, string_table: HashMap<LineId, StringInfo>) {
        self.base_string_table = string_table;
    }

    fn extend_base_string_table(&mut self, string_table: HashMap<LineId, StringInfo>) {
        self.base_string_table.extend(string_table);
    }

    fn take_fetched_assets(&mut self, _asset: Box<dyn Any>) {
        // No assets to fetch or store.
    }

    fn fetch_assets(&self, _world: &World) -> Option<Box<dyn Any + 'static>> {
        None // No file I/O needed.
    }
}
