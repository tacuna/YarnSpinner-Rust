//! Example: Node Groups and Custom Saliency Strategies
//!
//! This example demonstrates how to use node groups (multiple nodes with
//! the same title but different `when:` conditions) and how to configure
//! or implement a custom content saliency strategy.
//!
//! For full documentation on node groups, see:
//! https://docs.yarnspinner.dev/

use std::collections::HashMap;

use yarnspinner::compiler::{Compiler, File};
use yarnspinner::core::{LineId, YarnValue};
use yarnspinner::runtime::{
    BestLeastRecentlyViewedSaliencyStrategy,
    BestSaliencyStrategy,
    ContentSaliencyOption,
    ContentSaliencyStrategy,
    Dialogue,
    DialogueEvent,
    FirstSaliencyStrategy,
    MemoryVariableStorage,
    RandomBestLeastRecentlyViewedSaliencyStrategy,
    StringTableTextProvider,
    VariableStorage,
};

fn main() -> anyhow::Result<()> {
    println!("=== Node Groups & Saliency Strategy Example ===\n");

    // --- Part 1: Show built-in strategies ---
    println!("--- Part 1: Built-in Strategies ---\n");

    println!("1. FirstSaliencyStrategy (picks first passing node):");
    run_with_strategy(Box::new(FirstSaliencyStrategy))?;

    println!("\n2. BestSaliencyStrategy (picks highest complexity passing node):");
    run_with_strategy(Box::new(BestSaliencyStrategy))?;

    println!("\n3. BestLeastRecentlyViewedSaliencyStrategy (default — least-seen, highest complexity):");
    run_with_strategy(Box::new(BestLeastRecentlyViewedSaliencyStrategy))?;

    println!("\n4. RandomBestLeastRecentlyViewedSaliencyStrategy (random tie-breaking):");
    run_with_strategy(Box::new(RandomBestLeastRecentlyViewedSaliencyStrategy::with_seed(42)))?;

    // --- Part 2: Custom strategy ---
    println!("\n--- Part 2: Custom Saliency Strategy ---\n");
    println!("PrioritizeNewcomerStrategy (always picks the 'not met' variant first):");
    run_with_strategy(Box::new(PrioritizeNewcomerStrategy))?;

    Ok(())
}

/// Compiles the node_groups.yarn script and runs it with the given strategy,
/// printing each line of dialogue.
fn run_with_strategy(strategy: Box<dyn ContentSaliencyStrategy>) -> anyhow::Result<()> {
    let source = include_str!("../../assets/dialogue/node_groups.yarn");

    let compilation = {
        let mut compiler = Compiler::new();
        compiler.add_file(File {
            file_name: "node_groups.yarn".to_string(),
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

    // Set the saliency strategy
    dialogue.set_content_saliency_strategy(strategy);

    dialogue.add_program(compilation.program.expect("should compile"));
    dialogue.set_node("NodeGroupsStart")?;

    // Run through the dialogue
    let mut line_count = 0;
    loop {
        let events = dialogue.continue_()?;
        let mut complete = false;

        for event in events {
            match event {
                DialogueEvent::Line(line) => {
                    println!("  {}", line.text);
                    line_count += 1;
                    if line_count > 20 {
                        println!("  ... (truncated)");
                        return Ok(());
                    }
                }
                DialogueEvent::Options(options) => {
                    // Auto-select first available option
                    if let Some(opt) = options.iter().find(|o| o.is_available) {
                        dialogue.set_selected_option(opt.id)?;
                    }
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

    Ok(())
}

// --- Custom Saliency Strategy ---

/// A custom strategy that always picks the candidate with the most
/// failing conditions first (i.e. prefers simpler/less-constrained nodes),
/// with a fallback to the first passing option.
///
/// This is useful for scenarios where you want "catch-all" nodes to fire
/// first when they're available.
#[derive(Debug, Clone)]
struct PrioritizeNewcomerStrategy;

impl ContentSaliencyStrategy for PrioritizeNewcomerStrategy {
    fn clone_box(&self) -> Box<dyn ContentSaliencyStrategy> {
        Box::new(self.clone())
    }

    fn query_best_content(&self, content: &[ContentSaliencyOption], variable_storage: &dyn VariableStorage) -> Option<usize> {
        // Filter to only passing options (no failing conditions)
        let eligible: Vec<_> = content.iter().enumerate().filter(|(_, c)| c.failing_condition_count == 0).collect();

        if eligible.is_empty() {
            return None;
        }

        // Among eligible options, prefer the one that's been viewed least,
        // but break ties by picking the LOWEST complexity (simplest condition).
        // This means generic/catch-all nodes fire before specific ones.
        let mut best_idx = eligible[0].0;
        let mut best_views = get_view_count(&eligible[0].1.content_id, variable_storage);
        let mut best_complexity = eligible[0].1.complexity_score;

        for &(idx, option) in &eligible[1..] {
            let views = get_view_count(&option.content_id, variable_storage);
            if views < best_views || (views == best_views && option.complexity_score < best_complexity) {
                best_idx = idx;
                best_views = views;
                best_complexity = option.complexity_score;
            }
        }

        Some(best_idx)
    }

    fn content_was_selected(&self, content: &ContentSaliencyOption, variable_storage: &mut dyn VariableStorage) {
        // Track view counts
        let key = content.view_count_key();
        let current = variable_storage
            .get(&key)
            .ok()
            .and_then(|v| {
                let f: Result<f32, _> = v.try_into();
                f.ok()
            })
            .unwrap_or(0.0);
        variable_storage.set(key, YarnValue::from(current + 1.0)).ok();
    }
}

fn get_view_count(content_id: &str, storage: &dyn VariableStorage) -> i32 {
    let key = format!("$Yarn.Internal.Content.ViewCount.{content_id}");
    storage
        .get(&key)
        .ok()
        .and_then(|v| {
            let f: Result<f32, _> = v.try_into();
            f.ok()
        })
        .map(|f| f as i32)
        .unwrap_or(0)
}
