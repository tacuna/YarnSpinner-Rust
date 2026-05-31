//! Example demonstrating the customizable `LineTagGenerator` trait.
//!
//! This shows how to use the built-in generators (`RandomLineTagGenerator` and
//! `DescriptiveLineTagGenerator`) as well as how to implement a custom one.

use std::collections::{HashMap, HashSet};
use yarnspinner::compiler::{
    Compiler,
    DescriptiveLineTagGenerator,
    LineTagContext,
    LineTagGenerator,
    LineTaggingError,
    RandomLineTagGenerator,
    TagAbortBehaviour,
};

/// A sample Yarn script with untagged lines.
const SAMPLE_YARN: &str = "\
title: Meeting
---
Alice: Hey, how are you doing today?
Bob: I'm doing great, thanks for asking!
Alice: That's wonderful to hear.
<<if $weather == \"sunny\">>
    Bob: Want to go for a walk?
<<endif>>
Alice: Sure, let's do it!
===
";

fn main() {
    println!("=== LineTagGenerator Example ===\n");
    println!("--- Original source ---");
    println!("{SAMPLE_YARN}");

    // 1. Using the default RandomLineTagGenerator (random hex IDs)
    println!("--- Tagged with RandomLineTagGenerator ---");
    let result = Compiler::tag_lines_with_generator(
        SAMPLE_YARN,
        None,
        Some(Box::new(RandomLineTagGenerator::default())),
        TagAbortBehaviour::CurrentNode,
    )
    .expect("Compilation failed");

    if let Some((tagged_source, new_ids, errors)) = result {
        println!("{tagged_source}");
        println!("New IDs generated: {}", new_ids.len());
        for id in &new_ids {
            println!("  {}", id.0);
        }
        if !errors.is_empty() {
            println!("Errors: {errors:?}");
        }
    }

    // 2. Using DescriptiveLineTagGenerator (human-readable IDs)
    println!("\n--- Tagged with DescriptiveLineTagGenerator ---");
    let result = Compiler::tag_lines_with_generator(
        SAMPLE_YARN,
        None,
        Some(Box::new(DescriptiveLineTagGenerator::new())),
        TagAbortBehaviour::CurrentNode,
    )
    .expect("Compilation failed");

    if let Some((tagged_source, new_ids, errors)) = result {
        println!("{tagged_source}");
        println!("New IDs generated: {}", new_ids.len());
        for id in &new_ids {
            println!("  {}", id.0);
        }
        if !errors.is_empty() {
            println!("Errors: {errors:?}");
        }
    }

    // 3. Using a custom LineTagGenerator
    println!("\n--- Tagged with custom PrefixLineTagGenerator ---");
    let result = Compiler::tag_lines_with_generator(
        SAMPLE_YARN,
        None,
        Some(Box::new(PrefixLineTagGenerator::new("myproject"))),
        TagAbortBehaviour::CurrentNode,
    )
    .expect("Compilation failed");

    if let Some((tagged_source, new_ids, _)) = result {
        println!("{tagged_source}");
        println!("New IDs generated: {}", new_ids.len());
        for id in &new_ids {
            println!("  {}", id.0);
        }
    }
}

/// A custom line tag generator that creates sequential IDs with a project prefix.
/// Generates tags like `line:myproject_Meeting_001`, `line:myproject_Meeting_002`, etc.
#[derive(Debug)]
struct PrefixLineTagGenerator {
    prefix: String,
    counter: HashMap<String, usize>,
    existing_ids: HashSet<String>,
}

impl PrefixLineTagGenerator {
    fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            counter: HashMap::new(),
            existing_ids: HashSet::new(),
        }
    }
}

impl LineTagGenerator for PrefixLineTagGenerator {
    fn prepare_for_lines(&mut self, line_contexts: &HashMap<String, Vec<LineTagContext>>, excluded_ids: &HashSet<String>) {
        self.existing_ids = excluded_ids.clone();
        // Collect existing IDs from contexts
        for lines in line_contexts.values() {
            for line in lines {
                if let Some(ref id) = line.line_id {
                    self.existing_ids.insert(id.clone());
                }
            }
        }
        // Initialize counters per node
        for node in line_contexts.keys() {
            self.counter.insert(node.clone(), 0);
        }
    }

    fn generate_line_tag(&mut self, node: &str, _line_index: usize) -> Result<String, LineTaggingError> {
        let count = self.counter.entry(node.to_owned()).or_insert(0);
        *count += 1;

        let tag = format!("line:{}_{node}_{count:03}", self.prefix);

        if self.existing_ids.contains(&tag) {
            return Err(LineTaggingError::new(format!("Generated duplicate tag: {tag}")));
        }

        self.existing_ids.insert(tag.clone());
        Ok(tag)
    }
}
