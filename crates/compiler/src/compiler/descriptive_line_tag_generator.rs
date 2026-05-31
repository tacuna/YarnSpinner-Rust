//! Descriptive line tag generator.
//!
//! Ported from C# YarnSpinner v3.2.1 `DescriptiveLineTagGenerator`.
//! Generates human-readable line IDs based on node name, line position, and character name.

use super::line_tag_generator::*;
use regex::Regex;
use std::collections::{HashMap, HashSet};

const INDEX_MULTIPLIER: i32 = 100;
const ROUND_FACTOR: i32 = 5;

/// Creates line IDs in a form that attempts to approximate how a person would manually tag lines.
///
/// If you had a yarn node like the following:
/// ```text
/// title: Node
/// ---
/// Alice: This is me saying a line
/// Alice: And another line
/// <<some command>>
/// Bob: And me responding
/// And finally a line that isn't from a character
/// ===
/// ```
/// This would make the following tags:
/// - `#line:Node_0100_Alice`
/// - `#line:Node_0200_Alice`
/// - `#line:Node_0300_Bob`
/// - `#line:Node_0400`
///
/// Where possible the tagger will attempt to add numbers equally in-between existing tags.
#[derive(Debug)]
pub struct DescriptiveLineTagGenerator {
    line_contexts: Option<HashMap<String, Vec<LineTagContext>>>,
    generations: HashMap<String, HashMap<i32, i32>>,
    numbers: HashMap<String, Vec<i32>>,
    exclusions: HashSet<String>,
    is_matching_numbers: Regex,
    is_generation: Regex,
}

impl Default for DescriptiveLineTagGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl DescriptiveLineTagGenerator {
    /// Create a new descriptive line tag generator.
    pub fn new() -> Self {
        Self {
            line_contexts: None,
            generations: HashMap::new(),
            numbers: HashMap::new(),
            exclusions: HashSet::new(),
            is_matching_numbers: Regex::new(r"^[0-9]+$").unwrap(),
            is_generation: Regex::new(r"^g[0-9]+$").unwrap(),
        }
    }

    /// Extract the character name from line text.
    /// In Yarn, character names are indicated by `Name\: text` or `Name: text` in the raw text.
    fn extract_character_name(line_text: &str) -> Option<&str> {
        // Look for the escaped colon pattern first (string table format)
        if let Some(pos) = line_text.find("\\:") {
            let name = line_text[..pos].trim();
            if !name.is_empty() && !name.contains(' ') {
                return Some(name);
            }
        }
        // Also check for the raw `: ` pattern (direct from parse tree)
        // Only if the part before the colon looks like a character name (no spaces, alphanumeric)
        if let Some(pos) = line_text.find(": ") {
            let name = line_text[..pos].trim();
            if !name.is_empty() && !name.contains(' ') && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Some(name);
            }
        }
        None
    }

    fn get_neighbours(line_index: usize, numbers: &[i32]) -> (i32, i32, i32, i32) {
        let mut lc: i32 = -1;
        let mut li: i32 = -1;
        let mut rc: i32 = -1;
        let mut ri: i32 = -1;

        // Walk forwards
        for (i, &num) in numbers.iter().enumerate().skip(line_index + 1) {
            if num == -1 {
                continue;
            }
            ri = i as i32;
            rc = num;
            break;
        }
        // Walk backwards
        if line_index > 0 {
            for i in (0..line_index).rev() {
                if numbers[i] == -1 {
                    continue;
                }
                li = i as i32;
                lc = numbers[i];
                break;
            }
        }
        (lc, li, rc, ri)
    }

    fn get_generation(&mut self, node: &str, number: i32) -> i32 {
        if let Some(node_gens) = self.generations.get_mut(node) {
            if let Some(generation) = node_gens.get_mut(&number) {
                *generation += 1;
                return *generation;
            }
            node_gens.insert(number, 0);
        } else {
            let mut node_data = HashMap::new();
            node_data.insert(number, 0);
            self.generations.insert(node.to_owned(), node_data);
        }
        0
    }
}

impl LineTagGenerator for DescriptiveLineTagGenerator {
    fn prepare_for_lines(&mut self, line_contexts: &HashMap<String, Vec<LineTagContext>>, excluded_ids: &HashSet<String>) {
        self.line_contexts = Some(line_contexts.clone());
        self.numbers = HashMap::new();
        self.exclusions.extend(excluded_ids.iter().cloned());

        for (node_name, lines) in line_contexts {
            let mut elements = vec![-1i32; lines.len()];

            for (i, line) in lines.iter().enumerate() {
                let mut num: i32 = -1;
                let mut generation: i32 = 0;

                if let Some(ref line_id) = line.line_id
                    && !line_id.trim().is_empty()
                {
                    let pieces: Vec<&str> = line_id.split('_').collect();
                    for piece in &pieces {
                        if self.is_matching_numbers.is_match(piece) {
                            if let Ok(value) = piece.parse::<i32>() {
                                num = value;
                            }
                        } else if self.is_generation.is_match(piece)
                            && let Ok(value) = piece[1..].parse::<i32>()
                        {
                            generation = value;
                        }
                    }
                }

                // Record generation data
                if num != -1 {
                    let gen_collection = self.generations.entry(node_name.clone()).or_default();
                    let current = gen_collection.entry(num).or_insert(0);
                    if generation > *current {
                        *current = generation;
                    }
                }

                elements[i] = num;
            }
            self.numbers.insert(node_name.clone(), elements);
        }
    }

    fn generate_line_tag(&mut self, node: &str, line_index: usize) -> Result<String, LineTaggingError> {
        // Extract needed data from line_contexts up front to avoid borrow conflicts
        let (line_text, source_file_name, line_number) = {
            let line_contexts = self.line_contexts.as_ref().ok_or_else(|| {
                LineTaggingError::new(format!(
                    "Asked to generate a line at index {line_index} for {node} node but we haven't been given a context"
                ))
            })?;

            let lines_for_node = line_contexts.get(node).ok_or_else(|| {
                LineTaggingError::new(format!(
                    "Asked to generate a line at index {line_index} for {node} node but we have no node with this name"
                ))
            })?;

            if lines_for_node.is_empty() {
                return Err(LineTaggingError::new(format!(
                    "Asked to generate a line at index {line_index} for {node} node but this list is empty"
                )));
            }

            if line_index >= lines_for_node.len() {
                return Err(LineTaggingError::new(format!(
                    "Asked to generate a line at index {line_index} for {node} node but the index is out of bounds"
                )));
            }

            let context = &lines_for_node[line_index];
            (
                context.line_text.clone(),
                context.source_file_name.clone().unwrap_or_else(|| "<unknown>".to_owned()),
                context.line_number,
            )
        };

        let ranges = self
            .numbers
            .get(node)
            .ok_or_else(|| LineTaggingError::new("Asked to generate a line tag but haven't been given the context"))?;

        let mut line_id_components: Vec<String> = vec![node.to_owned()];

        let neighbours = Self::get_neighbours(line_index, ranges);
        let (left_count, left_index, right_count, right_index) = neighbours;

        let increment;
        let mut final_index;

        if left_count == -1 && right_count == -1 {
            // No tagged lines anywhere - use line index
            final_index = (line_index as i32 + 1) * INDEX_MULTIPLIER;
            increment = INDEX_MULTIPLIER;
        } else if left_count != -1 && right_count != -1 {
            // Between two tagged lines
            if left_count > right_count {
                return Err(LineTaggingError::with_location(
                    format!("The preceding dialogue has a greater tagged value ({left_count}) than the following ({right_count})."),
                    &source_file_name,
                    line_number,
                ));
            }

            let diff = right_count - left_count;
            let insertions = right_index - 1 - left_index;
            increment = if insertions > 0 { diff / (insertions + 1) } else { diff / 2 };
            let relative_index = line_index as i32 - left_index;

            if increment > 0 {
                final_index = left_count + relative_index * increment;
            } else {
                final_index = (left_count + relative_index).min(right_count);
                // Force increment to 1 for rounding check
            }
        } else if left_index == -1 {
            // Before any tagged lines
            increment = right_count / (right_index + 1);
            if increment > 0 {
                final_index = increment * (1 + line_index as i32);
            } else {
                final_index = (line_index as i32).min(right_count);
            }
        } else {
            // After any tagged lines
            let next_multiple = left_count + INDEX_MULTIPLIER - 1 - (left_count + INDEX_MULTIPLIER - 1) % INDEX_MULTIPLIER;
            increment = INDEX_MULTIPLIER;
            final_index = (line_index as i32 - left_index) * INDEX_MULTIPLIER + next_multiple;
        };

        // Round up to nearest ROUND_FACTOR if there's enough space
        if increment > ROUND_FACTOR {
            final_index = final_index + ROUND_FACTOR - 1 - (final_index + ROUND_FACTOR - 1) % ROUND_FACTOR;
        }

        line_id_components.push(format!("{final_index:04}"));

        let generation = self.get_generation(node, final_index);
        if generation != 0 {
            line_id_components.push(format!("g{generation}"));
        }

        // Try to extract character name from line text
        if let Some(character) = Self::extract_character_name(&line_text) {
            line_id_components.push(character.to_owned());
        }

        let id = format!("line:{}", line_id_components.join("_"));

        if self.exclusions.contains(&id) {
            return Err(LineTaggingError::with_location(
                format!("The generated id '{id}' conflicts with an id we were excluded from using."),
                &source_file_name,
                line_number,
            ));
        }

        Ok(id)
    }
}
