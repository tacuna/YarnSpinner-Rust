//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner.Compiler/Utility.cs>

use crate::compiler::line_tag_generator::*;
use crate::listeners::{DiagnosticVec, UntaggedLineCollector, UntaggedLineListener};
use crate::prelude::generated::yarnspinnerparser::YarnSpinnerParserTreeWalker;
use crate::prelude::*;
use std::collections::HashSet;
use std::sync::atomic::Ordering;

impl Compiler {
    /// Given Yarn source code, adds line tags to the ends of all lines
    /// that need one and do not already have one.
    ///
    /// This method ensures that it does not generate line
    /// tags that are already present in the file, or present in the
    /// `existing_line_tags` collection.
    ///
    /// Line tags are added to any line of source code that contains
    /// user-visible text: lines, options, and shortcut options.
    ///
    /// ## Parameters
    ///
    /// `contents`: The source code to add line tags
    /// to.
    /// `existing_line_tags`: The collection of line tags
    /// already exist elsewhere in the source code; the newly added
    /// line tags will not be duplicates of any in this
    /// collection.
    ///
    /// ## Return value
    /// Returns the modified source code, with line tags added.
    /// If all nodes already have line tags, returns `None`.
    /// Given Yarn source code, adds line tags to the ends of all lines
    /// that need one and do not already have one.
    ///
    /// This method ensures that it does not generate line
    /// tags that are already present in the file, or present in the
    /// `existing_line_tags` collection.
    ///
    /// Line tags are added to any line of source code that contains
    /// user-visible text: lines, options, and shortcut options.
    ///
    /// ## Parameters
    ///
    /// `contents`: The source code to add line tags
    /// to.
    /// `existing_line_tags`: The collection of line tags
    /// already exist elsewhere in the source code; the newly added
    /// line tags will not be duplicates of any in this
    /// collection.
    ///
    /// ## Return value
    /// Returns Tuple of the modified source code, with line tags
    /// added and the list of new line tags generated.
    pub fn tag_lines(contents: impl Into<String>, existing_line_tags: Vec<LineId>) -> crate::Result<Option<(String, Vec<LineId>)>> {
        let contents = contents.into();
        let chars: Vec<_> = contents.chars().map(|c| c as u32).collect();
        // First, get the parse tree for this source code.
        let file = File {
            file_name: "<input>".to_string(),
            source: contents,
        };
        let (parse_source, diagnostics) = parse_source(&file, &chars);
        let tree = parse_source.tree.clone();
        // Were there any error-level diagnostics?
        if diagnostics.has_errors() {
            // We encountered a parse error. Bail here; we aren't confident in our ability to correctly insert a line tag.
            return Err(CompilerError(diagnostics));
        }

        // Create the line listener, which will produce TextReplacements for each new line tag.
        let untagged_line_listener = Box::new(UntaggedLineListener::new(existing_line_tags, parse_source));
        let rewritten_nodes = untagged_line_listener.rewritten_lines.clone();
        let rewrote_anything = untagged_line_listener.rewrote_anything.clone();

        // Walk the tree with this listener, and generate text replacements containing line tags.
        let untagged_line_listener = YarnSpinnerParserTreeWalker::walk(untagged_line_listener, tree.as_ref()).expect("Tree walk failed");
        // Apply these text replacements to the original source and return it.

        if rewrote_anything.load(Ordering::Relaxed) {
            let result = rewritten_nodes.take();
            let mut string = result.join("\n");
            string.push('\n');
            Ok(Some((string, untagged_line_listener.existing_line_tags.clone())))
        } else {
            Ok(None)
        }
    }

    /// Given Yarn source code, adds line tags to the ends of all lines
    /// that need one and do not already have one, using a custom line tag generator.
    ///
    /// This method ensures that it does not generate line
    /// tags that are already present in the file, or present in the
    /// `excluded_line_ids` collection.
    ///
    /// Line tags are added to any line of source code that contains
    /// user-visible text: lines, options, and shortcut options.
    ///
    /// ## Parameters
    ///
    /// * `contents`: The source code to add line tags to.
    /// * `excluded_line_ids`: Line IDs that should not be used for generation
    ///   (e.g., from other files in the same project).
    /// * `line_tag_generator`: The generator to use for creating new line IDs.
    ///   If `None`, uses [`RandomLineTagGenerator`].
    /// * `abort_behaviour`: Controls how tagging errors are handled.
    ///
    /// ## Return value
    /// Returns a tuple of:
    /// - The modified source code with line tags added
    /// - The list of newly generated line IDs
    /// - Any tagging errors that were encountered
    ///
    /// Returns `None` if all lines already have tags.
    #[allow(clippy::type_complexity)]
    pub fn tag_lines_with_generator(
        contents: impl Into<String>,
        excluded_line_ids: Option<HashSet<String>>,
        line_tag_generator: Option<Box<dyn LineTagGenerator>>,
        abort_behaviour: TagAbortBehaviour,
    ) -> crate::Result<Option<(String, Vec<LineId>, Vec<LineTaggingError>)>> {
        let contents = contents.into();
        let chars: Vec<_> = contents.chars().map(|c| c as u32).collect();
        let file = File {
            file_name: "<input>".to_string(),
            source: contents,
        };
        let (parse_source, diagnostics) = parse_source(&file, &chars);
        let tree = parse_source.tree.clone();

        if diagnostics.has_errors() {
            return Err(CompilerError(diagnostics));
        }

        let excluded = excluded_line_ids.unwrap_or_default();
        let mut generator: Box<dyn LineTagGenerator> = line_tag_generator.unwrap_or_else(|| Box::new(RandomLineTagGenerator::default()));

        // Phase 1: Walk the tree to collect line contexts
        let collector = Box::new(UntaggedLineCollector::new(parse_source, file.file_name.clone()));
        let rewritten_lines = collector.rewritten_lines.clone();
        let rewrote_anything = collector.rewrote_anything.clone();

        let collector = YarnSpinnerParserTreeWalker::walk(collector, tree.as_ref()).expect("Tree walk failed");

        // Phase 2: Run the generator over collected contexts
        let (new_ids, errors) = collector.run_line_tagger(generator.as_mut(), &excluded, abort_behaviour);

        if rewrote_anything.load(Ordering::Relaxed) {
            let result = rewritten_lines.take();
            let mut string = result.join("\n");
            string.push('\n');
            Ok(Some((string, new_ids, errors)))
        } else {
            Ok(None)
        }
    }
}

/// Parses a string of Yarn source code, and produces a [`FileParseResult`]
/// and (if there were any problems) a collection of [`Diagnostic`]s.
fn parse_source<'a, 'b: 'a>(file: &'b File, chars: &'a [u32]) -> (FileParseResult<'a>, Vec<Diagnostic>) {
    let mut diagnostics = Vec::new();

    let result = parse_syntax_tree(file, chars, &mut diagnostics, &[]);

    (result, diagnostics)
}
