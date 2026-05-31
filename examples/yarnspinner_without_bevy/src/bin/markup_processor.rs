//! Example: Custom Markup Processors
//!
//! This example demonstrates how to implement and register custom
//! `AttributeMarkerProcessor` implementations that transform text
//! inside markup tags at parse time.
//!
//! The built-in processors handle `select`, `plural`, `ordinal`, and
//! `nomarkup` markers. You can register your own for game-specific
//! markup like `[uppercase]`, `[reverse]`, or any custom tag.
//!
//! For full documentation on Yarn Spinner markup, see:
//! https://docs.yarnspinner.dev/

use std::collections::HashMap;

use yarnspinner::compiler::{Compiler, File};
use yarnspinner::core::LineId;
use yarnspinner::runtime::{
    AttributeMarkerProcessor,
    Dialogue,
    DialogueEvent,
    Language,
    MarkupAttributeMarker,
    MarkupValue,
    MemoryVariableStorage,
    REPLACEMENT_MARKER_CONTENTS,
    StringTableTextProvider,
    TagType,
};

fn main() -> anyhow::Result<()> {
    println!("=== Custom Markup Processor Example ===\n");

    let source = include_str!("../../assets/dialogue/markup_processor.yarn");

    let compilation = {
        let mut compiler = Compiler::new();
        compiler.add_file(File {
            file_name: "markup_processor.yarn".to_string(),
            source: source.to_string(),
        });
        compiler.compile()?
    };

    let mut base_string_table = HashMap::<LineId, String>::default();
    for (k, v) in &compilation.string_table {
        base_string_table.insert(k.clone(), v.text.clone());
    }

    let mut text_provider = StringTableTextProvider::new();
    text_provider.extend_base_language(base_string_table);

    let variable_storage = MemoryVariableStorage::new();
    let mut dialogue = Dialogue::new(Box::new(variable_storage), Box::new(text_provider));

    // Register custom markup processors.
    // Each processor handles a specific marker name.
    dialogue.register_marker_processor("uppercase", Box::new(UppercaseProcessor));
    dialogue.register_marker_processor("reverse", Box::new(ReverseProcessor));

    dialogue.add_program(compilation.program.expect("should compile"));
    dialogue.set_node("MarkupStart")?;

    println!("Lines with custom markup processing applied:\n");

    loop {
        let events = dialogue.continue_()?;
        let mut complete = false;

        for event in events {
            match event {
                DialogueEvent::Line(line) => {
                    // The text has already been processed by our custom processors.
                    // Non-replacement markers (like [wave], [shake], [color]) are
                    // still available as attributes on the line for the game to handle.
                    print!("  {}", line.text);

                    // Show any remaining attributes the game could use for effects
                    if !line.attributes.is_empty() {
                        let attr_names: Vec<_> = line.attributes.iter().map(|a| a.name.as_str()).collect();
                        print!("  (attributes: {:?})", attr_names);
                    }
                    println!();
                }
                DialogueEvent::DialogueComplete => {
                    complete = true;
                }
                _ => {}
            }
        }

        if complete {
            break;
        }
    }

    println!("\nNote: [wave], [shake], and [color] are NOT replacement markers,");
    println!("so they pass through as attributes for the game engine to handle.");
    println!("[uppercase] and [reverse] ARE replacement markers that transform text.");

    Ok(())
}

// --- Custom Markup Processors ---

/// A processor that converts enclosed text to UPPERCASE.
///
/// Usage in Yarn: `[uppercase]some text[/uppercase]`
/// Result: "SOME TEXT"
#[derive(Debug, Clone)]
struct UppercaseProcessor;

impl AttributeMarkerProcessor for UppercaseProcessor {
    fn replacement_text_for_marker(&self, marker: &MarkupAttributeMarker) -> String {
        // For open markers, the enclosed text is available in the "contents" property.
        // For self-closing markers (e.g. [uppercase/]), there's no content.
        match marker.tag_type {
            TagType::Open | TagType::SelfClosing => {
                let contents = marker
                    .properties
                    .get(REPLACEMENT_MARKER_CONTENTS)
                    .and_then(|v| match v {
                        MarkupValue::String(s) => Some(s.as_str()),
                        _ => None,
                    })
                    .unwrap_or("");
                contents.to_uppercase()
            }
            _ => String::new(),
        }
    }

    fn set_language_code(&mut self, _language_code: Option<Language>) {
        // No language-specific behavior needed
    }

    fn clone_box(&self) -> Box<dyn AttributeMarkerProcessor> {
        Box::new(self.clone())
    }
}

/// A processor that reverses the enclosed text.
///
/// Usage in Yarn: `[reverse]hello[/reverse]`
/// Result: "olleh"
#[derive(Debug, Clone)]
struct ReverseProcessor;

impl AttributeMarkerProcessor for ReverseProcessor {
    fn replacement_text_for_marker(&self, marker: &MarkupAttributeMarker) -> String {
        match marker.tag_type {
            TagType::Open | TagType::SelfClosing => {
                let contents = marker
                    .properties
                    .get(REPLACEMENT_MARKER_CONTENTS)
                    .and_then(|v| match v {
                        MarkupValue::String(s) => Some(s.as_str()),
                        _ => None,
                    })
                    .unwrap_or("");
                contents.chars().rev().collect()
            }
            _ => String::new(),
        }
    }

    fn set_language_code(&mut self, _language_code: Option<Language>) {
        // No language-specific behavior needed
    }

    fn clone_box(&self) -> Box<dyn AttributeMarkerProcessor> {
        Box::new(self.clone())
    }
}
