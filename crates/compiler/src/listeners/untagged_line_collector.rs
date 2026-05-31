//! Two-phase line tag collection and generation.
//!
//! Phase 1: Walk the parse tree to collect all lines and their context (per node).
//! Phase 2: Run the line tag generator over the collected contexts to produce tags.

use crate::compiler::line_tag_generator::*;
use crate::parser::generated::yarnspinnerparser::{HeaderContext, Line_statementContext, NodeContext, Title_headerContext, Title_headerContextAttrs};
use crate::prelude::generated::yarnspinnerparser::{Line_statementContextAttrs, YarnSpinnerParserContextType};
use crate::prelude::generated::yarnspinnerparserlistener::YarnSpinnerParserListener;
use crate::prelude::*;
use crate::visitors::get_hashtag_texts;
use antlr4rust::int_stream::IntStream;
use antlr4rust::parser_rule_context::ParserRuleContext;
use antlr4rust::token::Token;
use antlr4rust::token_stream::TokenStream;
use antlr4rust::tree::{ParseTree, ParseTreeListener};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::atomic::AtomicBool;

/// Information collected during the tree walk about a line that may need tagging.
#[derive(Debug, Clone)]
pub(crate) struct CollectedLineInfo {
    /// Zero-based line index in the source.
    pub line_index: usize,
    /// The token index of the insertion point (previous token on default channel before newline).
    pub insertion_token_index: isize,
    /// The existing line ID (if any). Contains the full "line:xxx" or "shadow:xxx" string.
    pub line_id: Option<String>,
    /// The text content of the line.
    pub line_text: String,
}

/// Listener that collects line contexts during tree walk without generating tags.
/// After walking, call [`run_line_tagger`] to apply a [`LineTagGenerator`].
pub(crate) struct UntaggedLineCollector<'input> {
    file: FileParseResult<'input>,
    source_file_name: String,
    pub(crate) rewritten_lines: Rc<RefCell<Vec<String>>>,
    pub(crate) rewrote_anything: Rc<AtomicBool>,
    /// Collected lines per node. Key = node title, Value = list of lines in order.
    pub(crate) line_contexts: HashMap<String, Vec<CollectedLineInfo>>,
    /// The current node's title (set during walk).
    current_node_title: Option<String>,
    /// Lines collected for the current node.
    current_lines: Option<Vec<CollectedLineInfo>>,
}

impl<'input> UntaggedLineCollector<'input> {
    pub fn new(file: FileParseResult<'input>, source_file_name: String) -> Self {
        let original_source = file.tokens().get_all_text().lines().map(|s| s.to_owned()).collect();
        Self {
            file,
            source_file_name,
            rewritten_lines: Rc::new(RefCell::new(original_source)),
            rewrote_anything: Default::default(),
            line_contexts: HashMap::new(),
            current_node_title: None,
            current_lines: None,
        }
    }

    /// Run the line tag generator over the collected line contexts.
    ///
    /// Returns the list of newly generated line IDs and any errors encountered.
    pub fn run_line_tagger(
        &self,
        generator: &mut dyn LineTagGenerator,
        excluded_ids: &HashSet<String>,
        abort_behaviour: TagAbortBehaviour,
    ) -> (Vec<LineId>, Vec<LineTaggingError>) {
        // Build LineTagContext map for the generator
        let tag_contexts: HashMap<String, Vec<LineTagContext>> = self
            .line_contexts
            .iter()
            .map(|(node, lines)| {
                let contexts = lines
                    .iter()
                    .map(|info| LineTagContext {
                        line_number: info.line_index,
                        source_file_name: Some(self.source_file_name.clone()),
                        line_text: info.line_text.clone(),
                        line_id: info.line_id.clone(),
                    })
                    .collect();
                (node.clone(), contexts)
            })
            .collect();

        generator.prepare_for_lines(&tag_contexts, excluded_ids);

        let mut new_ids = Vec::new();
        let mut errors = Vec::new();
        let mut all_known_ids: HashSet<String> = excluded_ids.clone();
        // Add existing IDs from contexts
        for lines in self.line_contexts.values() {
            for line in lines {
                if let Some(ref id) = line.line_id {
                    all_known_ids.insert(id.clone());
                }
            }
        }

        let mut full_rewrites: Vec<(usize, isize, String)> = Vec::new(); // (line_index, token_index, tag)

        let result: Result<(), ()> = (|| {
            for (node, lines) in &self.line_contexts {
                let mut node_rewrites: Vec<(usize, isize, String)> = Vec::new();
                let mut node_ids: HashSet<String> = HashSet::new();

                let node_result: Result<(), ()> = (|| {
                    for (i, line_info) in lines.iter().enumerate() {
                        if line_info.line_id.is_some() {
                            // Already has a tag, skip
                            continue;
                        }

                        match generator.generate_line_tag(node, i) {
                            Ok(new_id) => {
                                // Validate the generated ID
                                if new_id.is_empty() {
                                    let err = LineTaggingError::with_location(
                                        "Line ID generator returned an empty line ID",
                                        &self.source_file_name,
                                        line_info.line_index,
                                    );
                                    errors.push(err);
                                    if abort_behaviour != TagAbortBehaviour::CurrentLine {
                                        return Err(());
                                    }
                                    continue;
                                }

                                if !new_id.starts_with("line:") {
                                    let err = LineTaggingError::with_location(
                                        format!("Line IDs must start with 'line:' - generator returned '{new_id}'"),
                                        &self.source_file_name,
                                        line_info.line_index,
                                    );
                                    errors.push(err);
                                    if abort_behaviour != TagAbortBehaviour::CurrentLine {
                                        return Err(());
                                    }
                                    continue;
                                }

                                if all_known_ids.contains(&new_id) || node_ids.contains(&new_id) {
                                    let err = LineTaggingError::with_location(
                                        format!("Line ID generator returned a duplicate line tag '{new_id}'"),
                                        &self.source_file_name,
                                        line_info.line_index,
                                    );
                                    errors.push(err);
                                    if abort_behaviour != TagAbortBehaviour::CurrentLine {
                                        return Err(());
                                    }
                                    continue;
                                }

                                node_ids.insert(new_id.clone());
                                node_rewrites.push((line_info.line_index, line_info.insertion_token_index, new_id));
                            }
                            Err(err) => {
                                errors.push(err);
                                if abort_behaviour != TagAbortBehaviour::CurrentLine {
                                    return Err(());
                                }
                            }
                        }
                    }
                    Ok(())
                })();

                match node_result {
                    Ok(()) => {
                        // Commit this node's rewrites
                        for id in &node_ids {
                            all_known_ids.insert(id.clone());
                        }
                        full_rewrites.extend(node_rewrites);
                    }
                    Err(()) => {
                        if abort_behaviour == TagAbortBehaviour::EntireTagging {
                            return Err(());
                        }
                        // CurrentNode: discard this node's rewrites, continue
                    }
                }
            }
            Ok(())
        })();

        if result.is_err() {
            // EntireTagging: discard everything
            return (Vec::new(), errors);
        }

        // Apply all rewrites to the source lines
        if !full_rewrites.is_empty() {
            let tokens = self.file.tokens();
            let mut lines = self.rewritten_lines.borrow_mut();

            for (line_index, token_index, tag) in &full_rewrites {
                let previous_token = tokens.get(*token_index);
                let line = lines.get_mut(*line_index).unwrap();

                let insertion_index = line
                    .char_indices()
                    .map(|(byte_pos, _char)| byte_pos)
                    .nth(previous_token.get_column_as_usize())
                    .expect("Internal error: failed to convert char pos to byte pos for insertion index")
                    + previous_token.get_text().len();
                line.insert_str(insertion_index, &format!(" #{tag} "));

                new_ids.push(LineId(tag.clone()));
            }

            self.rewrote_anything.store(true, std::sync::atomic::Ordering::Relaxed);
        }

        (new_ids, errors)
    }
}

impl<'input> ParseTreeListener<'input, YarnSpinnerParserContextType> for UntaggedLineCollector<'input> {}

impl<'input> YarnSpinnerParserListener<'input> for UntaggedLineCollector<'input> {
    fn enter_node(&mut self, _ctx: &NodeContext<'input>) {
        self.current_node_title = None;
        self.current_lines = Some(Vec::new());
    }

    fn exit_title_header(&mut self, ctx: &Title_headerContext<'input>) {
        if let Some(title_token) = ctx.ID() {
            self.current_node_title = Some(title_token.get_text().trim().to_owned());
        }
    }

    fn exit_header(&mut self, ctx: &HeaderContext<'input>) {
        let key = ctx.header_key.as_ref().unwrap().get_text();
        if key == "title" {
            // title is now handled by exit_title_header; kept for compatibility
            let value = ctx.header_value.as_ref().map(|v| v.get_text().trim().to_owned()).unwrap_or_default();
            self.current_node_title = Some(value);
        }
    }

    fn exit_node(&mut self, _ctx: &NodeContext<'input>) {
        if let (Some(title), Some(lines)) = (self.current_node_title.take(), self.current_lines.take())
            && !lines.is_empty()
        {
            self.line_contexts.insert(title, lines);
        }
    }

    fn exit_line_statement(&mut self, ctx: &Line_statementContext<'input>) {
        let Some(ref mut current_lines) = self.current_lines else {
            return;
        };

        // Get hashtags and check for existing line/shadow ID
        let hashtags = ctx.hashtag_all();
        let texts = get_hashtag_texts(&hashtags);

        let mut line_id: Option<String> = None;
        for tag in &texts {
            if tag.starts_with(LINE_ID_PREFIX) || tag.starts_with("shadow:") {
                line_id = Some(tag.clone());
            }
        }

        // Find the insertion point token
        let index = ctx.NEWLINE().unwrap().symbol.get_token_index();
        let tokens = self.file.tokens();
        let previous_token_index = index_of_previous_token_on_channel(tokens, index);
        let line_index = ctx.start().get_line_as_usize().saturating_sub(1);

        let previous_token_index = previous_token_index.unwrap_or_else(|| {
            bug!("Internal error: failed to find any tokens before the newline in line statement on line {line_index}.");
        });

        // Get the line text (the content of the line_statement minus hashtags/newline)
        let line_text = ctx.line_formatted_text().map(|ft| ft.get_text()).unwrap_or_default();

        current_lines.push(CollectedLineInfo {
            line_index,
            insertion_token_index: previous_token_index,
            line_id,
            line_text,
        });
    }
}

/// Gets the index of the first token to the left of the token at `index` that's on the default token channel.
pub(crate) fn index_of_previous_token_on_channel(token_stream: &ActualTokenStream, index: isize) -> Option<isize> {
    let default_token_channel = 0;
    if index >= token_stream.size() {
        return Some(token_stream.size() - 1);
    }
    (0..index).rev().find(|&i| token_stream.get(i).get_channel() == default_token_channel)
}
