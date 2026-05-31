//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner.Compiler/Visitors/StringTableGeneratorVisitor.cs>
use crate::prelude::generated::yarnspinnerparser::*;
use crate::prelude::generated::yarnspinnerparservisitor::*;
use crate::prelude::*;
use antlr4rust::parser_rule_context::ParserRuleContext;
use antlr4rust::token::Token;
use antlr4rust::tree::{ParseTree, ParseTreeVisitorCompat, Tree};
use std::rc::Rc;

#[derive(Clone)]
/// A Visitor that walks an expression parse tree and generates string
/// table entries, which are provided to a [`StringTableManager`].
/// This string table can then be provided
/// to future compilation passes, or stored for later use. Call the
/// [`visit`] method to begin generating string table entries.
pub(crate) struct StringTableGeneratorVisitor<'input> {
    pub(crate) diagnostics: Vec<Diagnostic>,
    current_node_name: String,
    pub(crate) string_table_manager: StringTableManager,
    file: FileParseResult<'input>,
    _dummy: (),
}

impl<'input> StringTableGeneratorVisitor<'input> {
    pub(crate) fn new(string_table_manager: StringTableManager, file: FileParseResult<'input>) -> Self {
        Self {
            file,
            string_table_manager,
            diagnostics: Default::default(),
            current_node_name: Default::default(),
            _dummy: (),
        }
    }
}

impl<'input> ParseTreeVisitorCompat<'input> for StringTableGeneratorVisitor<'input> {
    type Node = YarnSpinnerParserContextType;

    type Return = ();

    fn temp_result(&mut self) -> &mut Self::Return {
        &mut self._dummy
    }
}

impl<'input> YarnSpinnerParserVisitorCompat<'input> for StringTableGeneratorVisitor<'input> {
    fn visit_node(&mut self, ctx: &NodeContext<'input>) -> Self::Return {
        let mut tags = Vec::new();
        for title_hdr in ctx.title_header_all() {
            if let Some(title_token) = title_hdr.ID() {
                self.current_node_name = title_token.get_text();
            }
        }
        for header in ctx.header_all() {
            let header_key = header.header_key.as_ref().unwrap().get_text();
            if header_key == "title" {
                // title is now in title_header_all(); kept for backwards-compat
                if let Some(val) = header.header_value.as_ref() {
                    val.get_text().clone_into(&mut self.current_node_name);
                }
            } else if header_key == "tags" {
                let header_value = header.header_value.as_ref().map(|header| header.get_text()).unwrap_or_default();
                // Split the list of tags by spaces, and use that
                tags = header_value.split_whitespace().map(ToOwned::to_owned).collect();
            }
        }
        if !self.current_node_name.is_empty() && tags.contains(&"rawText".to_owned()) {
            // This is a raw text node. Use its entire contents as a
            // string and don't use its contents.
            let line_id = get_line_id_for_node_name(&self.current_node_name);
            self.string_table_manager.insert(
                line_id,
                StringInfo {
                    text: ctx.body().unwrap().get_text(),
                    node_name: self.current_node_name.clone(),
                    line_number: ctx.body().unwrap().start().line as usize,
                    file_name: self.file.name.clone(),
                    ..Default::default()
                },
            );
        } else {
            // This is a regular node
            // String table generator: don't crash if a node has no body
            if let Some(body) = ctx.body() {
                self.visit(body.as_ref());
            }
        }
    }

    fn visit_line_statement(&mut self, ctx: &Line_statementContext<'input>) -> Self::Return {
        let hashtags = ctx.hashtag_all();
        let (line_ids, shadow_ids) = get_content_id_tags(&hashtags);

        // v3.2.1: detect multiple line/shadow IDs on a single line
        if line_ids.len() + shadow_ids.len() > 1 {
            // Both line and shadow tags present → YS0017 for each pair
            if !line_ids.is_empty() && !shadow_ids.is_empty() {
                for a in &line_ids {
                    for b in &shadow_ids {
                        let message = "Lines cannot have both a '#line' tag and a '#shadow' tag.";
                        self.diagnostics.push(
                            Diagnostic::from_message(message.to_string())
                                .with_parser_context(b.as_ref(), self.file.tokens())
                                .with_file_name(&self.file.name)
                                .with_code("YS0017"),
                        );
                        self.diagnostics.push(
                            Diagnostic::from_message(message.to_string())
                                .with_parser_context(a.as_ref(), self.file.tokens())
                                .with_file_name(&self.file.name)
                                .with_code("YS0017"),
                        );
                    }
                }
                return;
            }

            // Multiple line IDs → YS0062 for each
            if line_ids.len() > 1 {
                for id in &line_ids {
                    self.diagnostics.push(
                        Diagnostic::from_message("Multiple line IDs are not allowed on a single line.".to_string())
                            .with_parser_context(id.as_ref(), self.file.tokens())
                            .with_file_name(&self.file.name)
                            .with_code("YS0062"),
                    );
                }
            }

            // Multiple shadow IDs → YS0062 for each
            if shadow_ids.len() > 1 {
                for id in &shadow_ids {
                    self.diagnostics.push(
                        Diagnostic::from_message("Multiple shadow IDs are not allowed on a single line.".to_string())
                            .with_parser_context(id.as_ref(), self.file.tokens())
                            .with_file_name(&self.file.name)
                            .with_code("YS0062"),
                    );
                }
            }

            return;
        }

        let line_id_tag = line_ids.into_iter().next();
        let line_id = line_id_tag.as_ref().and_then(|t| t.text.as_ref());

        if let Some(line_id) = line_id
            && self.string_table_manager.contains_key(&line_id.get_text().into())
        {
            let diagnostic_context = line_id_tag.clone().unwrap();
            let line_id = line_id.get_text();
            self.diagnostics.push(
                Diagnostic::from_message(format!("Duplicate line ID {line_id}"))
                    .with_parser_context(diagnostic_context.as_ref(), self.file.tokens())
                    .with_file_name(&self.file.name)
                    .with_code("YS0018"),
            );
            return;
        };

        // Detect shadow tag
        let shadow_tag = shadow_ids.into_iter().next();
        let shadow_line_id = shadow_tag.as_ref().map(|tag| {
            let tag_text = tag.text.as_ref().expect("Hashtag held no text").get_text();
            format!("line:{}", &tag_text["shadow:".len()..])
        });

        let line_number = ctx.start().get_line_as_usize();
        let hashtag_texts = get_hashtag_texts(&hashtags);

        let composed_string = generate_formatted_text(&ctx.line_formatted_text().unwrap());

        // Check for malformed markup before moving the string.
        let markup_error = check_markup(&composed_string);
        let composed_len = composed_string.len();

        let string_id = self.string_table_manager.insert(
            line_id.map(|t| t.get_text().into()),
            StringInfo {
                text: composed_string,
                node_name: self.current_node_name.clone(),
                line_number,
                file_name: self.file.name.clone(),
                metadata: hashtag_texts,
                shadow_line_id,
                ..Default::default()
            },
        );

        if line_id.is_none() {
            add_hashtag_child(ctx, string_id.0);
        }

        if let Some(msg) = markup_error {
            let line_0 = line_number.saturating_sub(1);
            let range = Position { line: line_0, character: 0 }..Position {
                line: line_0,
                character: composed_len,
            };
            self.diagnostics.push(DiagnosticDescriptor::MARKUP_FAILED_TO_PARSE.create_with_range(
                self.file.name.clone(),
                range,
                format!("Dialogue has malformed or invalid markup. {msg}"),
            ));
        }
    }
}

/// Takes a string like
/// `Hi there { some_expression }, how are you { another_expression } doing?`
/// and turns it into
/// `Hi there {0}, how are you {1}? doing`
fn generate_formatted_text(ctx: &Line_formatted_textContext) -> String {
    let mut expression_count = 0;
    let mut composed_string = String::new();
    // First, visit all of the nodes, which are either terminal
    // text nodes or expressions. if they're expressions, we
    // evaluate them, and inject a positional reference into the
    // final string.
    for child in ctx.get_children() {
        if child.get_child_count() == 0 {
            composed_string.push_str(&child.get_text());
        } else {
            // Expressions in the final string are denoted as the
            // index of the expression, surrounded by braces { }.
            // However, we don't need to write the braces here
            // ourselves, because the text itself that the parser
            // captured already has them. So, we just need to write
            // the expression count.
            composed_string.push_str(&expression_count.to_string());
            expression_count += 1;
        }
    }
    composed_string.trim().to_owned()
}

pub(crate) fn get_hashtag_texts(hashtags: &[Rc<HashtagContext>]) -> Vec<String> {
    hashtags
        .iter()
        .map(|t| t.text.as_ref().expect("No text in hashtag").get_text().trim().to_owned())
        .collect()
}

/// Returns an error description if the markup in `text` is malformed
/// (e.g. an opened tag is never closed), or `None` if markup is valid.
///
/// Handles: `[[double-bracket]]` links (skipped), `\[escaped brackets\]`,
/// `[self-closing/]` tags, `[/closing]` tags, and `[opening]` tags.
fn check_markup(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut depth: i32 = 0;
    let mut i = 0;
    while i < len {
        if i + 1 < len && bytes[i] == b'[' && bytes[i + 1] == b'[' {
            // Double-bracket link: skip until ]]
            i += 2;
            while i + 1 < len && !(bytes[i] == b']' && bytes[i + 1] == b']') {
                i += 1;
            }
            i += 2; // skip ]]
            continue;
        }
        if bytes[i] == b'\\' && i + 1 < len && bytes[i + 1] == b'[' {
            // Escaped opening bracket
            i += 2;
            continue;
        }
        if bytes[i] == b'\\' && i + 1 < len && bytes[i + 1] == b']' {
            // Escaped closing bracket
            i += 2;
            continue;
        }
        if bytes[i] == b'[' {
            // Find the matching ]
            let start = i;
            i += 1;
            while i < len && bytes[i] != b']' {
                i += 1;
            }
            if i >= len {
                return Some(format!("Unmatched '[' at position {start}"));
            }
            // Check if self-closing: last char before ] is /
            let last_before_close = if i > start + 1 { bytes[i - 1] } else { 0 };
            // Check if closing tag: first char after [ is /
            let first_after_open = if i > start + 1 { bytes[start + 1] } else { 0 };
            if last_before_close == b'/' {
                // self-closing [foo/] — no depth change
            } else if first_after_open == b'/' {
                // closing tag [/foo]
                depth -= 1;
                if depth < 0 {
                    return Some(format!("Unexpected closing tag at position {start}"));
                }
            } else {
                // opening tag [foo]
                depth += 1;
            }
            i += 1; // skip ]
            continue;
        }
        i += 1;
    }
    if depth > 0 {
        Some(format!("{depth} unclosed markup tag(s)"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use antlr4rust::InputStream;
    use antlr4rust::common_token_stream::CommonTokenStream;
    use yarnspinner_core::prelude::Position;

    #[test]
    fn ignores_lines_without_expression() {
        let input = "title: Title
---
A line
===
";
        let result = process_input(input);
        let expected = "A line";
        assert_eq!(result, expected);
    }

    #[test]
    fn formats_lines_with_expression() {
        let input = "title: Title
---
A line with a {$cool} expression
===
";
        let result = process_input(input);
        let expected = "A line with a {0} expression";
        assert_eq!(result, expected);
    }

    #[test]
    fn formats_lines_with_multiple_expressions() {
        let input = "title: Title
---
A line with {$many} many {(1 -(1 * 2))}{$cool} expressions
===
";
        let result = process_input(input);
        let expected = "A line with {0} many {1}{2} expressions";
        assert_eq!(result, expected);
    }

    fn process_input(input: &str) -> String {
        let lexer = YarnSpinnerLexer::new(InputStream::new(input), "input.yarn".to_owned());
        let mut parser = YarnSpinnerParser::new(CommonTokenStream::new(lexer));
        let line_formatted_text = parser
            .dialogue()
            .unwrap()
            .node(0)
            .unwrap()
            .body()
            .unwrap()
            .statement(0)
            .unwrap()
            .line_statement()
            .unwrap()
            .line_formatted_text()
            .unwrap();
        generate_formatted_text(&line_formatted_text)
    }

    #[test]
    fn populates_string_table() {
        let file = File {
            file_name: "test.yarn".to_string(),
            source: "title: test
---
foo
bar
a {1 + 3} cool expression
==="
            .to_string(),
        };
        let result = Compiler {
            files: vec![file],
            library: Default::default(),
            compilation_type: CompilationType::FullCompilation,
            variable_declarations: vec![],
            diagnostic_severities: Default::default(),
            language_version: None,
            type_declarations: vec![],
        }
        .compile()
        .unwrap();

        let string_table = result.string_table;
        assert_eq!(string_table.len(), 3);
        assert_eq!(
            string_table[&"line:1337986088".into()],
            StringInfo {
                text: "foo".to_string(),
                node_name: "test".to_string(),
                line_number: 3,
                file_name: "test.yarn".to_string(),
                is_implicit_tag: true,
                metadata: vec![],
                shadow_line_id: None,
            }
        );
        assert_eq!(
            string_table[&"line:952581310".into()],
            StringInfo {
                text: "bar".to_string(),
                node_name: "test".to_string(),
                line_number: 4,
                file_name: "test.yarn".to_string(),
                is_implicit_tag: true,
                metadata: vec![],
                shadow_line_id: None,
            }
        );
        assert_eq!(
            string_table[&"line:2714660100".into()],
            StringInfo {
                text: "a {0} cool expression".to_string(),
                node_name: "test".to_string(),
                line_number: 5,
                file_name: "test.yarn".to_string(),
                is_implicit_tag: true,
                metadata: vec![],
                shadow_line_id: None,
            }
        );
    }

    #[test]
    fn catches_expression_errors() {
        let file = File {
            file_name: "test.yarn".to_string(),
            source: "title: test
---
foo
bar
a {very} cool expression
==="
            .to_string(),
        };
        let result = Compiler {
            files: vec![file],
            library: Default::default(),
            compilation_type: CompilationType::FullCompilation,
            variable_declarations: vec![],
            diagnostic_severities: Default::default(),
            language_version: None,
            type_declarations: vec![],
        }
        .compile();

        let diagnostics = result.unwrap_err().0;
        assert_eq!(2, diagnostics.len());

        let range = Position { line: 4, character: 7 }..Position { line: 4, character: 8 };
        let context = "a {very} cool expression\n       ^".to_owned();
        let first_expected = Diagnostic::from_message("Unexpected \"}\" while reading a function call".to_string())
            .with_file_name("test.yarn".to_string())
            .with_range(range.clone())
            .with_context(context.clone())
            .with_start_line(4)
            .with_severity(DiagnosticSeverity::Error);

        let second_expected = Diagnostic::from_message("mismatched input '}' expecting '('".to_string())
            .with_file_name("test.yarn".to_string())
            .with_range(range)
            .with_context(context)
            .with_start_line(4)
            .with_severity(DiagnosticSeverity::Error);
        if diagnostics[0] == first_expected {
            assert_eq!(diagnostics[1], second_expected);
        } else {
            assert_eq!(diagnostics[0], second_expected);
            assert_eq!(diagnostics[1], first_expected);
        }
    }
}
