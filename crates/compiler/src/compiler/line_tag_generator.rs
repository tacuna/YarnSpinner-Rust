//! Line tag generator trait and implementations.
//!
//! Ported from C# YarnSpinner v3.2.1 `ILineTagGenerator` and `RandomLineTagGenerator`.

use rand::rngs::{SmallRng, SysRng};
use rand::{RngExt as _, SeedableRng};
use std::collections::{HashMap, HashSet};
use std::fmt;
use yarnspinner_internal_shared::prelude::LINE_ID_PREFIX;

/// Controls how line tagging should handle encountering issues while tagging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TagAbortBehaviour {
    /// Abort the entire tagging operation. All previously generated tags for this run are discarded.
    EntireTagging,
    /// Abort tagging the current node. Previously generated tags for other nodes are kept.
    #[default]
    CurrentNode,
    /// Skip the current line. All other lines (including others in the same node) continue to be tagged.
    CurrentLine,
}

/// An error that occurred during line tag generation.
#[derive(Debug, Clone)]
pub struct LineTaggingError {
    /// A description of the error.
    pub message: String,
    /// The source file name where the error occurred, if available.
    pub source_file_name: Option<String>,
    /// The line number where the error occurred, if available.
    pub line_number: Option<usize>,
}

impl fmt::Display for LineTaggingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(ref file) = self.source_file_name {
            write!(f, " (in {file}")?;
            if let Some(line) = self.line_number {
                write!(f, ", line {line}")?;
            }
            write!(f, ")")?;
        }
        Ok(())
    }
}

impl std::error::Error for LineTaggingError {}

impl LineTaggingError {
    /// Create a new error with just a message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            source_file_name: None,
            line_number: None,
        }
    }

    /// Create a new error with message, source file, and line number.
    pub fn with_location(message: impl Into<String>, source_file_name: impl Into<String>, line_number: usize) -> Self {
        Self {
            message: message.into(),
            source_file_name: Some(source_file_name.into()),
            line_number: Some(line_number),
        }
    }
}

/// Context information about a line that needs tagging.
#[derive(Debug, Clone)]
pub struct LineTagContext {
    /// The zero-based line number in the source file.
    pub line_number: usize,
    /// The source file name, if available.
    pub source_file_name: Option<String>,
    /// The text content of the line (without markup).
    pub line_text: String,
    /// The existing line ID, if one is already present.
    pub line_id: Option<String>,
}

/// A trait for generating line tags for untagged lines in Yarn source.
///
/// Implementations of this trait can customize how line IDs are generated.
/// The default implementation is [`RandomLineTagGenerator`], which generates
/// random hex-based line IDs.
pub trait LineTagGenerator {
    /// Called before any line tags are generated. Provides the full context of all
    /// lines that will be processed, organized by node name.
    ///
    /// `line_contexts` maps node names to the list of lines in that node.
    /// `excluded_ids` contains line IDs that must not be generated (e.g., from other files).
    fn prepare_for_lines(&mut self, line_contexts: &HashMap<String, Vec<LineTagContext>>, excluded_ids: &HashSet<String>);

    /// Generate a new line tag for the line at `line_index` in the given `node`.
    ///
    /// The returned string must start with "line:".
    ///
    /// # Errors
    /// Returns a [`LineTaggingError`] if a tag cannot be generated.
    fn generate_line_tag(&mut self, node: &str, line_index: usize) -> Result<String, LineTaggingError>;
}

/// A line tag generator that produces line IDs containing a random hexadecimal string.
///
/// This is the default generator used when no custom generator is provided.
/// Generated tags have the format `line:XXXXXXX` where X is a hex digit.
#[derive(Debug, Default)]
pub struct RandomLineTagGenerator {
    all_keys: HashSet<String>,
}

impl LineTagGenerator for RandomLineTagGenerator {
    fn prepare_for_lines(&mut self, line_contexts: &HashMap<String, Vec<LineTagContext>>, excluded_ids: &HashSet<String>) {
        self.all_keys = excluded_ids.clone();
        // Also add any existing line IDs from the contexts
        for lines in line_contexts.values() {
            for line in lines {
                if let Some(ref id) = line.line_id {
                    self.all_keys.insert(id.clone());
                }
            }
        }
    }

    fn generate_line_tag(&mut self, _node: &str, _line_index: usize) -> Result<String, LineTaggingError> {
        let mut rng = SmallRng::try_from_rng(&mut SysRng).unwrap();
        let max_iterations = 100_000;

        for _ in 0..max_iterations {
            let value: usize = rng.random_range(0..0x1000000);
            let tag = format!("{LINE_ID_PREFIX}{value:07x}");
            if !self.all_keys.contains(&tag) {
                self.all_keys.insert(tag.clone());
                return Ok(tag);
            }
        }

        Err(LineTaggingError::new(
            "Failed to find a unique line ID within the maximum number of attempts",
        ))
    }
}
