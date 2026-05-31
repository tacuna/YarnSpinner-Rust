//! Content saliency system for selecting the most appropriate content
//! from a set of candidates (line groups, node groups).
//!
//! This module provides the [`ContentSaliencyStrategy`] trait and several
//! built-in implementations matching the C# YarnSpinner saliency strategies.

use crate::prelude::*;
use core::fmt::Debug;
use core::sync::atomic::{AtomicU64, Ordering};

/// Indicates what type of content a [`ContentSaliencyOption`] represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    /// The content represents a node in a node group.
    Node,
    /// The content represents a line in a line group.
    Line,
}

/// Represents a piece of content that may be selected by a
/// [`ContentSaliencyStrategy`].
#[derive(Debug, Clone)]
pub struct ContentSaliencyOption {
    /// A string that uniquely identifies this content.
    pub content_id: String,
    /// The number of conditions that passed for this piece of content.
    pub passing_condition_count: i32,
    /// The number of conditions that failed for this piece of content.
    pub failing_condition_count: i32,
    /// The complexity score of this option (number of condition variables).
    pub complexity_score: i32,
    /// The type of content that this option represents.
    pub content_type: ContentType,
    /// The destination instruction index (internal to the VM).
    pub(crate) destination: usize,
}

impl ContentSaliencyOption {
    /// Returns the variable key used to track view counts for this content.
    pub fn view_count_key(&self) -> String {
        format!("$Yarn.Internal.Content.ViewCount.{}", self.content_id)
    }
}

/// A strategy for choosing the most appropriate piece of content from a
/// collection of options.
///
/// Implement this trait to provide custom saliency behavior. The built-in
/// strategies are:
/// - [`FirstSaliencyStrategy`] — picks the first passing option
/// - [`BestSaliencyStrategy`] — picks the highest-complexity passing option
/// - [`BestLeastRecentlyViewedSaliencyStrategy`] — picks the least-seen,
///   highest-complexity passing option (default)
/// - [`RandomBestLeastRecentlyViewedSaliencyStrategy`] — same as above but
///   with random tie-breaking
pub trait ContentSaliencyStrategy: Debug + Send + Sync {
    /// Creates a boxed clone of this strategy.
    fn clone_box(&self) -> Box<dyn ContentSaliencyStrategy>;

    /// Chooses the most appropriate item from the provided content options.
    ///
    /// Returns the index into `content` of the selected item, or `None` if
    /// no content should be displayed.
    ///
    /// Implementations should NOT modify any state in this method.
    fn query_best_content(&self, content: &[ContentSaliencyOption], variable_storage: &dyn VariableStorage) -> Option<usize>;

    /// Called after a piece of content has been selected, to allow the
    /// strategy to update any internal state (e.g. view counts).
    fn content_was_selected(&self, content: &ContentSaliencyOption, variable_storage: &mut dyn VariableStorage);
}

impl Clone for Box<dyn ContentSaliencyStrategy> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

// ---------------------------------------------------------------------------
// FirstSaliencyStrategy
// ---------------------------------------------------------------------------

/// A saliency strategy that returns the first non-failing item.
#[derive(Debug, Clone)]
pub struct FirstSaliencyStrategy;

impl ContentSaliencyStrategy for FirstSaliencyStrategy {
    fn clone_box(&self) -> Box<dyn ContentSaliencyStrategy> {
        Box::new(self.clone())
    }

    fn query_best_content(&self, content: &[ContentSaliencyOption], _variable_storage: &dyn VariableStorage) -> Option<usize> {
        content.iter().position(|c| c.failing_condition_count == 0)
    }

    fn content_was_selected(&self, _content: &ContentSaliencyOption, _variable_storage: &mut dyn VariableStorage) {
        // No state to track.
    }
}

// ---------------------------------------------------------------------------
// BestSaliencyStrategy
// ---------------------------------------------------------------------------

/// A saliency strategy that returns the highest-complexity non-failing item.
#[derive(Debug, Clone)]
pub struct BestSaliencyStrategy;

impl ContentSaliencyStrategy for BestSaliencyStrategy {
    fn clone_box(&self) -> Box<dyn ContentSaliencyStrategy> {
        Box::new(self.clone())
    }

    fn query_best_content(&self, content: &[ContentSaliencyOption], _variable_storage: &dyn VariableStorage) -> Option<usize> {
        content
            .iter()
            .enumerate()
            .filter(|(_, c)| c.failing_condition_count == 0)
            .max_by_key(|(_, c)| c.complexity_score)
            .map(|(i, _)| i)
    }

    fn content_was_selected(&self, _content: &ContentSaliencyOption, _variable_storage: &mut dyn VariableStorage) {
        // No state to track.
    }
}

// ---------------------------------------------------------------------------
// BestLeastRecentlyViewedSaliencyStrategy
// ---------------------------------------------------------------------------

/// A saliency strategy that picks the least-recently-seen, highest-complexity
/// non-failing item. This is the default strategy.
///
/// View counts are stored in the variable storage under keys like
/// `$Yarn.Internal.Content.ViewCount.<content_id>`.
#[derive(Debug, Clone)]
pub struct BestLeastRecentlyViewedSaliencyStrategy;

impl ContentSaliencyStrategy for BestLeastRecentlyViewedSaliencyStrategy {
    fn clone_box(&self) -> Box<dyn ContentSaliencyStrategy> {
        Box::new(self.clone())
    }

    fn query_best_content(&self, content: &[ContentSaliencyOption], variable_storage: &dyn VariableStorage) -> Option<usize> {
        // Linear scan: prefer fewest views, break ties by highest complexity.
        // Document order is preserved for fully-tied elements because
        // `min_by` returns the first minimum it encounters.
        content
            .iter()
            .enumerate()
            .filter(|(_, c)| c.failing_condition_count == 0)
            .map(|(i, c)| (i, get_view_count(c, variable_storage), c.complexity_score))
            .min_by(|a, b| a.1.cmp(&b.1).then_with(|| b.2.cmp(&a.2)))
            .map(|(i, _, _)| i)
    }

    fn content_was_selected(&self, content: &ContentSaliencyOption, variable_storage: &mut dyn VariableStorage) {
        increment_view_count(content, variable_storage);
    }
}

// ---------------------------------------------------------------------------
// RandomBestLeastRecentlyViewedSaliencyStrategy
// ---------------------------------------------------------------------------

/// A saliency strategy that picks a random choice among the best,
/// least-recently-seen, highest-complexity non-failing items.
///
/// Uses a simple internal counter for pseudo-random tie-breaking. For
/// deterministic randomness, set the seed via [`Self::with_seed`].
#[derive(Debug)]
pub struct RandomBestLeastRecentlyViewedSaliencyStrategy {
    /// Simple counter used for pseudo-random selection among tied candidates.
    /// Incremented on each selection to vary the pick.
    counter: AtomicU64,
}

impl Clone for RandomBestLeastRecentlyViewedSaliencyStrategy {
    fn clone(&self) -> Self {
        Self {
            counter: AtomicU64::new(self.counter.load(Ordering::Relaxed)),
        }
    }
}

impl RandomBestLeastRecentlyViewedSaliencyStrategy {
    /// Creates a new strategy with seed 0.
    pub fn new() -> Self {
        Self { counter: AtomicU64::new(0) }
    }

    /// Creates a new strategy with a specific seed for reproducibility.
    pub fn with_seed(seed: u64) -> Self {
        Self {
            counter: AtomicU64::new(seed),
        }
    }
}

impl Default for RandomBestLeastRecentlyViewedSaliencyStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentSaliencyStrategy for RandomBestLeastRecentlyViewedSaliencyStrategy {
    fn clone_box(&self) -> Box<dyn ContentSaliencyStrategy> {
        Box::new(self.clone())
    }

    fn query_best_content(&self, content: &[ContentSaliencyOption], variable_storage: &dyn VariableStorage) -> Option<usize> {
        let mut candidates: Vec<_> = content
            .iter()
            .enumerate()
            .filter(|(_, c)| c.failing_condition_count == 0)
            .map(|(i, c)| {
                let view_count = get_view_count(c, variable_storage);
                (i, view_count, c.complexity_score)
            })
            .collect();

        if candidates.is_empty() {
            return None;
        }

        // Narrow to the group with fewest views, then highest complexity.
        let min_views = candidates.iter().map(|e| e.1).min().unwrap();
        candidates.retain(|e| e.1 == min_views);
        let max_complexity = candidates.iter().map(|e| e.2).max().unwrap();
        candidates.retain(|e| e.2 == max_complexity);

        // Pick pseudo-randomly from the tied group
        let counter = self.counter.fetch_add(1, Ordering::Relaxed);
        let idx = (counter as usize) % candidates.len();
        Some(candidates[idx].0)
    }

    fn content_was_selected(&self, content: &ContentSaliencyOption, variable_storage: &mut dyn VariableStorage) {
        increment_view_count(content, variable_storage);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn get_view_count(option: &ContentSaliencyOption, storage: &dyn VariableStorage) -> i32 {
    let key = option.view_count_key();
    storage
        .get(&key)
        .ok()
        .and_then(|v| {
            let f: core::result::Result<f32, _> = v.try_into();
            f.ok()
        })
        .map(|f| f as i32)
        .unwrap_or(0)
}

fn increment_view_count(option: &ContentSaliencyOption, storage: &mut dyn VariableStorage) {
    let key = option.view_count_key();
    let current = storage
        .get(&key)
        .ok()
        .and_then(|v| {
            let f: core::result::Result<f32, _> = v.try_into();
            f.ok()
        })
        .unwrap_or(0.0);
    storage.set(key, YarnValue::from(current + 1.0)).ok();
}
