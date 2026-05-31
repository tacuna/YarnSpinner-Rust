//! Contains functions that were originally part of `compiler.rs` according to the original implementation,
//! but were moved to their own file for better organization.

use crate::error_strategy::ErrorStrategy;
use crate::listeners::*;
use crate::output::enum_type::EnumType;
use crate::parser::enum_registry::{EnumCaseValue, EnumDef, EnumRawType, EnumRegistry};
use crate::prelude::generated::yarnspinnerparser::*;
use crate::prelude::generated::{yarnspinnerlexer, yarnspinnerparser};
use crate::prelude::*;
use antlr4rust::Parser;
use antlr4rust::common_token_stream::CommonTokenStream;
use antlr4rust::input_stream::CodePoint32BitCharStream;
use antlr4rust::token::{TOKEN_DEFAULT_CHANNEL, Token};
use std::collections::HashSet;
use std::rc::Rc;
use yarnspinner_core::prelude::*;
use yarnspinner_core::types::FunctionType;

const SHADOW_ID_PREFIX: &str = "shadow:";

pub(crate) fn get_line_id_tag<'a>(hashtag_contexts: &[Rc<HashtagContextAll<'a>>]) -> Option<Rc<HashtagContextAll<'a>>> {
    hashtag_contexts
        .iter()
        .find(|hashtag| {
            let hashtag_text = hashtag.text.as_ref().expect("Hashtag held no text").get_text();
            hashtag_text.starts_with(LINE_ID_PREFIX)
        })
        .cloned()
}

/// Collects all `#line:` and `#shadow:` tags from hashtag contexts.
/// Matches `GetContentIDTags` from C# v3.2.1.
pub(crate) fn get_content_id_tags<'a>(
    hashtag_contexts: &[Rc<HashtagContextAll<'a>>],
) -> (Vec<Rc<HashtagContextAll<'a>>>, Vec<Rc<HashtagContextAll<'a>>>) {
    let mut line_ids = Vec::new();
    let mut shadow_ids = Vec::new();
    for hashtag in hashtag_contexts {
        let tag_text = hashtag.text.as_ref().expect("Hashtag held no text").get_text();
        if tag_text.starts_with(LINE_ID_PREFIX) {
            line_ids.push(hashtag.clone());
        } else if tag_text.starts_with(SHADOW_ID_PREFIX) {
            shadow_ids.push(hashtag.clone());
        }
    }
    (line_ids, shadow_ids)
}

pub(crate) fn parse_syntax_tree<'a, 'b: 'a>(
    file: &'b File,
    file_chars: &'a [u32],
    diagnostics: &mut Vec<Diagnostic>,
    type_declarations: &[EnumType],
) -> FileParseResult<'a> {
    // Using 32 bit codepoints because that's how big a Rust `char` is: 4 bytes.
    let input = CodePoint32BitCharStream::new(file_chars);

    // Pre-scan for enum definitions so the lexer can resolve member references.
    let (mut enum_registry, enum_errors) = EnumRegistry::from_source(&file.source);

    // Pre-populate the registry with any externally-declared enum types so that
    // `EnumName.Member` and `.Member` shorthand inside this file can be resolved.
    for ext in type_declarations {
        if !enum_registry.enums.contains_key(&ext.name) {
            let raw_type = match ext.raw_type {
                Type::Number => EnumRawType::Number,
                Type::String => EnumRawType::String,
                _ => EnumRawType::Number,
            };
            let cases = ext
                .cases
                .iter()
                .map(|case| {
                    let val = match &case.raw_value {
                        YarnValue::Number(n) => EnumCaseValue::Number(*n as f64),
                        YarnValue::String(s) => EnumCaseValue::Str(s.clone()),
                        YarnValue::Boolean(_) => EnumCaseValue::Number(0.0),
                    };
                    (case.name.clone(), val)
                })
                .collect();
            enum_registry.enums.insert(ext.name.clone(), EnumDef { raw_type, cases });
        }
    }

    // Keep a snapshot for callers (the snapshot includes both source-defined
    // and externally-declared enums so that `user_defined_types` can be
    // populated after code generation).
    let registry_snapshot = enum_registry.clone();
    for (msg, (sl, sc, el, ec)) in enum_errors {
        diagnostics.push(DiagnosticDescriptor::ENUM_DECLARATION_ERROR.create_with_range(
            &file.file_name,
            Position { line: sl, character: sc }..Position { line: el, character: ec },
            msg,
        ));
    }

    let mut lexer = YarnSpinnerLexer::new(input, file.file_name.clone());
    lexer.enum_registry = enum_registry;
    lexer.source = file.source.clone();
    let line_group_metadata = lexer.line_group_metadata.clone();

    // turning off the normal error listener and using ours
    let file_name = file.file_name.clone();
    let lexer_error_listener = LexerErrorListener::new(file_name.clone());
    let lexer_error_listener_diagnostics = lexer_error_listener.diagnostics.clone();
    let lexer_diagnostics = lexer.diagnostics.clone();
    lexer.remove_error_listeners();
    lexer.add_error_listener(Box::new(lexer_error_listener));

    let tokens = CommonTokenStream::new(lexer);
    let mut parser = YarnSpinnerParser::with_strategy(tokens, Box::new(ErrorStrategy::new()));
    let parser_error_listener = ParserErrorListener::new(file.clone());
    let parser_error_listener_diagnostics = parser_error_listener.diagnostics.clone();

    parser.remove_error_listeners();
    parser.add_error_listener(Box::new(parser_error_listener));

    // Must be read exactly here, because the error listeners running during the parse borrow the diagnostics mutably,
    // and we want to read them after.
    let tree = parser.dialogue().unwrap();

    let lexer_diagnostics_borrowed = lexer_diagnostics.borrow();
    let lexer_error_listener_diagnostics_borrowed = lexer_error_listener_diagnostics.borrow();
    let parser_error_listener_diagnostics_borrowed = parser_error_listener_diagnostics.borrow();
    let new_diagnostics = lexer_error_listener_diagnostics_borrowed
        .iter()
        .chain(lexer_diagnostics_borrowed.iter())
        .chain(parser_error_listener_diagnostics_borrowed.iter())
        .cloned();
    diagnostics.extend(new_diagnostics);

    FileParseResult::new(file_name, tree, Rc::new(parser), line_group_metadata.borrow().clone(), registry_snapshot)
}

pub(crate) fn get_line_id_for_node_name(name: &str) -> LineId {
    format!("{LINE_ID_PREFIX}{name}").into()
}

/// Gets the text of the documentation comments that either immediately
/// precede `context`, or are on the same line as `context`.
///
/// Documentation comments begin with a triple-slash (`///`), and
/// are used to describe variable declarations. If documentation
/// comments precede a declaration (that is, they're not on the same
/// line as the declaration), then they may span multiple lines, as long
/// as each line begins with a triple-slash.
///
/// If there are both doc comments preceding the declaration and on the same line,
/// only the the latter will be returned.
///
/// ## Implementation notes
///
/// The flag `allowCommentsAfter` was not ported because it was always set to `true` anyway.
pub(crate) fn get_document_comments<'input>(
    tokens: &ActualTokenStream<'input>,
    context: &impl YarnSpinnerParserContext<'input, TF = LocalTokenFactory<'input>, Ctx = YarnSpinnerParserContextType>,
) -> String {
    let subsequent_comments = tokens.get_hidden_tokens_to_right(context.stop().get_token_index(), yarnspinnerlexer::COMMENTS as i32);

    let subsequent_doc_comment = subsequent_comments
        .iter()
        // This comment is on the same line as the end of
        // the declaration
        .filter(|t| t.get_line() == context.stop().get_line())
        // The comment starts with a triple-slash
        .filter(|t| t.get_text().starts_with("///"))
        // Get its text
        .map(|t| t.get_text().replace("///", "").trim().to_owned())
        // Get the first one, or null
        .next();

    if let Some(subsequent_doc_comment) = subsequent_doc_comment {
        return subsequent_doc_comment;
    }

    let preceding_comments = tokens.get_hidden_tokens_to_left(context.start().get_token_index(), yarnspinnerlexer::COMMENTS as i32);

    let preceding_doc_comments: Vec<_> = preceding_comments
        .iter()
        // There are no tokens on the main channel with this
        // one on the same line
        .filter(|t| {
            !tokens
                .get_tokens()
                .iter()
                .filter(|ot| ot.get_line() == t.get_line())
                .filter(|ot| ot.get_token_type() != yarnspinnerlexer::INDENT && ot.get_token_type() != yarnspinnerlexer::DEDENT)
                .any(|ot| ot.get_channel() == TOKEN_DEFAULT_CHANNEL)
        })
        .filter(|t| t.get_text().starts_with("///"))
        // Get its text
        .map(|t| t.get_text().replace("///", "").trim().to_owned())
        .collect();
    preceding_doc_comments.join(" ")
}

/// Not part of original implementation, but needed because we lack some convenience methods
/// that the C# implementation of ANTLR would provide but antlr4rust does not.
pub(crate) fn add_hashtag_child<'input>(parent: &impl YarnSpinnerParserContext<'input>, text: impl Into<String>) {
    let parent = parent.ref_to_rc();
    let string_id_token = create_common_token(yarnspinnerparser::HASHTAG_TEXT, text);
    let invoking_state_according_to_original_implementation = 0;
    // `new_with_text` was hacked into the generated parser. Also, `FooContextExt::new` is usually private...
    let hashtag = HashtagContextExt::new_with_text(Some(parent.clone()), invoking_state_according_to_original_implementation, string_id_token);
    parent.add_child(hashtag);
}

pub(crate) trait ContextRefExt<'input> {
    fn ref_to_rc(self) -> Rc<ActualParserContext<'input>>;
}

impl<'input, T> ContextRefExt<'input> for &T
where
    T: YarnSpinnerParserContext<'input> + ?Sized,
{
    fn ref_to_rc(self) -> Rc<ActualParserContext<'input>> {
        self.get_children()
            .next()
            .map(|child| child.get_parent().unwrap())
            .or_else(|| {
                let interval = self.get_source_interval();
                self.get_parent()
                    .unwrap()
                    .get_children()
                    .find(|child| child.get_source_interval() == interval)
            })
            .unwrap()
    }
}

/// Returns a collection of [`Declaration`] structs that
/// describe the functions present in `library`.
///
/// ## Implementation note
///
/// In contrast to the original implementation, we don't return any diagnostics
/// because Rust's type system already guarantees at compile-time that all registered
/// functions are valid and compatible with Yarn.
pub(crate) fn get_declarations_from_library(library: &Library) -> Vec<Declaration> {
    let operators: HashSet<_> = Type::EXPLICITLY_CONSTRUCTABLE
        .iter()
        .flat_map(|r#type| {
            r#type
                .methods()
                .names()
                .map(|name| r#type.get_canonical_name_for_method(name))
                .collect::<Vec<_>>()
        })
        .collect();
    library
        .iter()
        // Operators are type checked by visitors instead
        .filter(|(name, _function)| !operators.contains(*name))
        .map(|(name, function)| {
            let mut function_type = FunctionType::default();
            let parameters = function
                .parameter_types()
                .into_iter()
                .map(|t| Type::try_from(t).unwrap())
                .map(Some)
                .collect();
            function_type.parameters = parameters;
            let return_type = Type::try_from(function.return_type()).unwrap();
            function_type.set_return_type(return_type);
            if let Some(var_type_id) = function.variadic_parameter_type_id() {
                function_type.variadic_parameter_type = Type::try_from(var_type_id).ok().map(Box::new);
            }
            Declaration::new(name, function_type).with_source_file_name(DeclarationSource::External)
        })
        .collect()
}

/// Generates a Yarn source file that declares a set of variables.
///
/// This is intended to be called by tools that let the user manage variable
/// declarations. The returned string can be saved as a `.yarn` file and then
/// added to a Yarn Project to declare the variables it contains.
///
/// # Arguments
///
/// - `declarations` — the variable declarations to include.  Declarations
///   whose type is [`Type::Function`] are silently skipped.
/// - `title` — the node title to use (e.g. `"Program"`).
/// - `tags` — optional node tags to include in the `tags:` header.
/// - `headers` — optional additional key/value headers to include after
///   `tags:` and before `---`.
pub fn generate_yarn_file_with_declarations<'a>(
    declarations: impl IntoIterator<Item = &'a Declaration>,
    title: &str,
    tags: &[&str],
    headers: &[(&str, &str)],
) -> String {
    let mut out = String::new();

    out.push_str(&format!("title: {title}\n"));

    if !tags.is_empty() {
        out.push_str(&format!("tags: {}\n", tags.join(" ")));
    }

    for (key, value) in headers {
        out.push_str(&format!("{key}: {value}\n"));
    }

    out.push_str("---\n");

    let mut count = 0usize;
    for decl in declarations {
        // Function declarations cannot be expressed in Yarn source.
        if matches!(decl.r#type, Type::Function(_)) {
            continue;
        }

        if let Some(ref desc) = decl.description
            && !desc.is_empty()
        {
            if count > 0 {
                // Blank line above the comment for readability.
                out.push('\n');
            }
            out.push_str(&format!("/// {desc}\n"));
        }

        out.push_str(&format!("<<declare {} = ", decl.name));

        match &decl.default_value {
            Some(YarnValue::Number(n)) => {
                // Match C# behavior: omit the decimal point for whole numbers.
                if n.fract() == 0.0 && n.is_finite() {
                    out.push_str(&format!("{}", *n as i64));
                } else {
                    out.push_str(&format!("{n}"));
                }
            }
            Some(YarnValue::String(s)) => {
                out.push_str(&format!("\"{s}\""));
            }
            Some(YarnValue::Boolean(b)) => {
                out.push_str(if *b { "true" } else { "false" });
            }
            None => {
                // No default value — emit nothing (should not normally occur
                // for well-formed variable declarations).
            }
        }

        out.push_str(">>\n");
        count += 1;
    }

    out.push_str("===\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warns_about_mixed_indentation() {
        let mut diagnostics = Vec::new();
        let mixed_indentation_input = File {
            file_name: "test.yarn".to_owned(),
            source: "title: Start
---
-> Option 1
\t   Nice.
==="
            .to_owned(),
        };
        let chars: Vec<_> = mixed_indentation_input.source.chars().map(|c| c as u32).collect();
        let _parsed_file = parse_syntax_tree(&mixed_indentation_input, &chars, &mut diagnostics, &[]);
        assert_eq!(1, diagnostics.len());
        assert_eq!(
            Diagnostic::from_message("Indentation contains tabs and spaces")
                .with_context("\t   ")
                .with_start_line(3)
                .with_file_name("test.yarn")
                .with_range(Position { line: 3, character: 0 }..Position { line: 3, character: 5 })
                .with_severity(DiagnosticSeverity::Warning),
            diagnostics[0]
        );
    }
}
