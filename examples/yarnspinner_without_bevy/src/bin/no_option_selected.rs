//! Example: NoOptionSelected — Falling Through Empty Option Groups
//!
//! This example demonstrates `Dialogue::set_no_option_selected()`, which
//! tells the runtime to skip past an option group when no options are
//! available for selection.
//!
//! In a real game, you might call this when all options are filtered out
//! by their conditions, or when the player dismisses the option list.
//!
//! For full documentation, see:
//! https://docs.yarnspinner.dev/

use std::collections::HashMap;

use yarnspinner::compiler::{Compiler, File};
use yarnspinner::core::LineId;
use yarnspinner::runtime::{Dialogue, DialogueEvent, DialogueOption, MemoryVariableStorage, StringTableTextProvider};

fn main() -> anyhow::Result<()> {
    println!("=== NoOptionSelected Example ===\n");

    // --- Scenario 1: Normal selection ---
    println!("--- Scenario 1: Selecting options normally ---\n");
    run_dialogue(SelectionMode::PickFirst)?;

    // --- Scenario 2: Using set_no_option_selected ---
    println!("\n--- Scenario 2: Using set_no_option_selected() to skip options ---\n");
    println!("(Simulates a case where no options are acceptable)\n");
    run_dialogue(SelectionMode::SkipAll)?;

    Ok(())
}

#[derive(Clone, Copy)]
enum SelectionMode {
    /// Always pick the first available option
    PickFirst,
    /// Always call set_no_option_selected to fall through
    SkipAll,
}

fn run_dialogue(mode: SelectionMode) -> anyhow::Result<()> {
    let source = include_str!("../../assets/dialogue/no_option_selected.yarn");

    let compilation = {
        let mut compiler = Compiler::new();
        compiler.add_file(File {
            file_name: "no_option_selected.yarn".to_string(),
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

    dialogue.add_program(compilation.program.expect("should compile"));
    dialogue.set_node("NoOptionStart")?;

    let mut iterations = 0;
    loop {
        let events = dialogue.continue_()?;
        let mut complete = false;

        for event in events {
            match event {
                DialogueEvent::Line(line) => {
                    println!("  {}", line.text);
                }
                DialogueEvent::Options(options) => {
                    handle_options(&mut dialogue, &options, mode)?;
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

        iterations += 1;
        if iterations > 20 {
            println!("  ... (stopping after 20 iterations)");
            break;
        }
    }

    Ok(())
}

fn handle_options(dialogue: &mut Dialogue, options: &[DialogueOption], mode: SelectionMode) -> anyhow::Result<()> {
    let available: Vec<_> = options.iter().filter(|o| o.is_available).collect();

    match mode {
        SelectionMode::PickFirst => {
            if let Some(opt) = available.first() {
                println!("  [Selecting: {}]", opt.line.text);
                dialogue.set_selected_option(opt.id)?;
            } else {
                // No options available — use set_no_option_selected to
                // fall through past the option block.
                println!("  [No options available — falling through]");
                dialogue.set_no_option_selected()?;
            }
        }
        SelectionMode::SkipAll => {
            // Demonstrate calling set_no_option_selected even when options
            // ARE available. The dialogue will skip past the option block
            // and continue with the next content.
            println!("  [Options shown ({} available) — choosing to skip]", available.len());
            dialogue.set_no_option_selected()?;
        }
    }

    Ok(())
}
