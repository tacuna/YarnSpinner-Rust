//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner.Compiler/TypeCheckerListener.cs>

use crate::prelude::generated::yarnspinnerparser::*;
use crate::prelude::generated::yarnspinnerparservisitor::YarnSpinnerParserVisitorCompat;
use crate::prelude::*;
use crate::visitors::constant_value_visitor::ConstantValueVisitor;
use antlr4rust::parser_rule_context::ParserRuleContext;
use antlr4rust::token::Token;
use antlr4rust::tree::{ParseTree, ParseTreeVisitorCompat};
use regex::Regex;
use std::collections::HashMap;
use yarnspinner_core::prelude::*;
use yarnspinner_core::types::*;

/// A visitor that extracts variable declarations from a parse tree.
/// After visiting an entire parse tree for a file, the
/// [`NewDeclarations`] property will contain all explicit
/// variable declarations that were found.
pub(crate) struct DeclarationVisitor<'input> {
    /// Gets the collection of new variable declarations that were
    /// found as a result of using this
    /// [`DeclarationVisitor`] to visit a
    /// [`ParserRuleContext`].
    pub(crate) new_declarations: Vec<Declaration>,

    /// Gets the collection of file-level hashtags that were found as a
    /// result of using this  [`DeclarationVisitor`] to visit a [`ParserRuleContext`].
    pub(crate) file_tags: Vec<String>,

    pub(crate) diagnostics: Vec<Diagnostic>,

    /// The CommonTokenStream derived from the file we're parsing. This
    /// is used to find documentation comments for declarations.
    file: FileParseResult<'input>,

    /// The collection of variable declarations we know about before starting our work.
    /// Uses a HashMap for O(1) lookup by name.
    existing_declarations: HashMap<String, Declaration>,

    /// The name of the node that we're currently visiting.
    current_node_name: Option<String>,

    /// A regular expression used to detect illegal characters in node titles.
    regex: Regex,

    /// Raw source text of the file, used to extract the full title string
    /// (including characters the lexer silently skips on error recovery).
    source: String,

    _dummy: (),
}

impl<'input> DeclarationVisitor<'input> {
    pub(crate) fn new(existing_declarations: Vec<Declaration>, file: FileParseResult<'input>, source: String) -> Self {
        let existing_declarations: HashMap<String, Declaration> = existing_declarations.into_iter().map(|d| (d.name.clone(), d)).collect();
        Self {
            file,
            existing_declarations,
            new_declarations: Default::default(),
            regex: Regex::new(r"[\[<>\]{}|:\s#$.]").unwrap(),
            file_tags: Default::default(),
            diagnostics: Default::default(),
            current_node_name: None,
            source,
            _dummy: Default::default(),
        }
    }

    /// Extracts the raw title text from the source line where the title header appears.
    /// This catches characters like `$`, `.`, and leading digits that the ANTLR lexer
    /// silently skips during error recovery in `HeaderTitleMode`.
    fn get_raw_title_text(&self, title_hdr: &Title_headerContextAll<'input>) -> Option<String> {
        let line = title_hdr.start().get_line_as_usize(); // 1-based
        let source_line = self.source.lines().nth(line.saturating_sub(1))?;
        let colon_pos = source_line.find(':')?;
        let after_colon = source_line[colon_pos + 1..].trim_start();
        let raw = strip_header_comment(after_colon).trim_end();
        if raw.is_empty() { None } else { Some(raw.to_owned()) }
    }

    /// Fast lookup for an explicit (non-implicit) declaration by name.
    fn get_explicit_declaration(&self, name: &str) -> Option<&Declaration> {
        self.existing_declarations
            .get(name)
            .filter(|d| !d.is_implicit)
            .or_else(|| self.new_declarations.iter().find(|d| !d.is_implicit && d.name == name))
    }
}

impl<'input> ParseTreeVisitorCompat<'input> for DeclarationVisitor<'input> {
    type Node = YarnSpinnerParserContextType;
    type Return = ();

    fn temp_result(&mut self) -> &mut Self::Return {
        &mut self._dummy
    }
}

impl<'input> YarnSpinnerParserVisitorCompat<'input> for DeclarationVisitor<'input> {
    fn visit_file_hashtag(&mut self, ctx: &File_hashtagContext<'input>) -> Self::Return {
        let hashtag_text = ctx.text.as_ref().unwrap();
        self.file_tags.push(hashtag_text.get_text().to_owned());
    }

    fn visit_node(&mut self, ctx: &NodeContext<'input>) -> Self::Return {
        for title_hdr in ctx.title_header_all() {
            // Always extract the raw title from the source so we detect chars
            // that the ANTLR lexer silently skips (e.g. `$`, `.`, leading digits).
            let raw_title = self.get_raw_title_text(title_hdr.as_ref());
            if let Some(title_token) = title_hdr.ID() {
                let current_node_name = title_token.get_text();
                self.current_node_name = Some(current_node_name.clone());
                // Use the raw title for validation when available; fall back to token text.
                let name_for_check = raw_title.as_deref().unwrap_or(&current_node_name);
                if self.regex.is_match(name_for_check) || name_for_check.starts_with(|c: char| c.is_ascii_digit()) {
                    let message = format!("The node '{name_for_check}' contains illegal characters.");
                    self.diagnostics.push(
                        Diagnostic::from_message(message)
                            .with_file_name(self.file.name.clone())
                            .with_parser_context(title_hdr.as_ref(), self.file.tokens())
                            .with_code("YS0027"),
                    );
                }
            }
        }
        for header in ctx.header_all() {
            let header_key = header.header_key.as_ref().unwrap();
            let header_key_text = header_key.get_text();
            if header_key_text == "title" {
                let Some(header_value) = header.header_value.as_ref() else {
                    continue;
                };
                let current_node_name = strip_header_comment(header_value.get_text());
                self.current_node_name = Some(current_node_name.to_owned());
                if self.regex.is_match(current_node_name) || current_node_name.starts_with(|c: char| c.is_ascii_digit()) {
                    let message = format!("The node '{current_node_name}' contains illegal characters.");
                    self.diagnostics.push(
                        Diagnostic::from_message(message)
                            .with_file_name(self.file.name.clone())
                            .with_parser_context(header.as_ref(), self.file.tokens())
                            .with_code("YS0027"),
                    );
                }
            } else if header_key_text == "subtitle" {
                let Some(header_value) = header.header_value.as_ref() else {
                    continue;
                };
                let subtitle_name = strip_header_comment(header_value.get_text()).trim();
                if self.regex.is_match(subtitle_name) || subtitle_name.starts_with(|c: char| c.is_ascii_digit()) {
                    let message = format!("The subtitle '{subtitle_name}' contains illegal characters.");
                    self.diagnostics.push(
                        Diagnostic::from_message(message)
                            .with_file_name(self.file.name.clone())
                            .with_parser_context(header.as_ref(), self.file.tokens())
                            .with_code("YS0027"),
                    );
                }
            }
        }
        if let Some(body) = ctx.body() {
            self.visit(body.as_ref());
        }
    }

    fn visit_declare_statement(&mut self, ctx: &Declare_statementContext<'input>) -> Self::Return {
        // Get the name of the variable we're declaring
        let variable_context = ctx.variable().unwrap();
        let variable_name = variable_context.get_text();

        // Does this variable name already exist in our declarations?
        let existing_explicit_declaration = self.get_explicit_declaration(&variable_name).cloned();
        if let Some(existing_explicit_declaration) = existing_explicit_declaration {
            // Then this is an error, because you can't have two explicit declarations for the same variable.
            let line = existing_explicit_declaration
                .source_file_line()
                .map(|l| format!(", line: {l}"))
                .unwrap_or_default();
            let msg = format!(
                "{} has already been declared in {}{line}",
                existing_explicit_declaration.name, existing_explicit_declaration.source_file_name,
            );
            self.diagnostics.push(
                Diagnostic::from_message(msg)
                    .with_file_name(&self.file.name)
                    .with_parser_context(ctx, self.file.tokens())
                    .with_code("YS0039"),
            );
            return;
        }

        // The parser now uses `expression` instead of `value` for the RHS of declare statements.
        // We need to determine if this is a simple literal (stored variable) or a complex
        // expression (smart variable / inline expansion).
        let Some(expression_context) = ctx.expression() else {
            // no expression was provided, declare as undefined and continue
            return;
        };

        // Check if the expression is a simple value (ExpValueContext wrapping a literal).
        // Note: negative number literals like `-1` are parsed as ExpNegativeContext wrapping
        // an ExpValueContext(ValueNumberContext), so we also accept that pattern here.
        // See https://github.com/YarnSpinnerTool/YarnSpinner/issues/421 — negative numbers
        // must NOT be treated as smart variables.
        let is_simple_literal = matches!(
            expression_context.as_ref(),
            ExpressionContextAll::ExpValueContext(exp_value)
                if matches!(
                    exp_value.value().as_deref(),
                    Some(ValueContextAll::ValueNumberContext(_))
                    | Some(ValueContextAll::ValueTrueContext(_))
                    | Some(ValueContextAll::ValueFalseContext(_))
                    | Some(ValueContextAll::ValueStringContext(_))
                )
        ) || matches!(
            expression_context.as_ref(),
            ExpressionContextAll::ExpNegativeContext(neg)
                if neg.expression().as_ref().is_some_and(|inner| matches!(
                    inner.as_ref(),
                    ExpressionContextAll::ExpValueContext(exp_value)
                        if matches!(exp_value.value().as_deref(), Some(ValueContextAll::ValueNumberContext(_)))
                ))
        );

        if is_simple_literal {
            // Simple literal value — this is a stored variable (same as before).
            // Extract the value using ConstantValueVisitor on the inner value context.
            //
            // Two shapes are accepted here:
            //   (a) ExpValueContext wrapping any literal
            //   (b) ExpNegativeContext wrapping ExpValueContext(ValueNumberContext) — a negative literal
            if let ExpressionContextAll::ExpNegativeContext(neg) = expression_context.as_ref()
                && let Some(inner_expr) = neg.expression()
                && let ExpressionContextAll::ExpValueContext(exp_value) = inner_expr.as_ref()
                && let Some(value_context) = exp_value.value()
                && matches!(value_context.as_ref(), ValueContextAll::ValueNumberContext(_))
            {
                // Evaluate the positive part, then negate.
                let mut constant_value_visitor = ConstantValueVisitor::new(self.diagnostics.clone(), self.file.clone());
                let value = constant_value_visitor.visit(value_context.as_ref());
                self.diagnostics.extend_from_slice(&constant_value_visitor.diagnostics);

                let description = get_document_comments(self.file.tokens(), ctx);
                let description_as_option = (!description.is_empty()).then_some(description);
                if let Some(inner_value) = value.as_ref() {
                    // Negate the number.
                    let negated = match inner_value.raw_value {
                        YarnValue::Number(n) => YarnValue::Number(-n),
                        ref other => other.clone(),
                    };
                    let declaration = Declaration::new(variable_name, Type::Number)
                        .with_default_value(negated)
                        .with_description_optional(description_as_option)
                        .with_source_file_name(self.file.name.clone())
                        .with_source_node_name_optional(self.current_node_name.clone())
                        .with_range(variable_context.range());
                    self.new_declarations.push(declaration);
                }
            } else if let ExpressionContextAll::ExpValueContext(exp_value) = expression_context.as_ref()
                && let Some(value_context) = exp_value.value()
            {
                let mut constant_value_visitor = ConstantValueVisitor::new(self.diagnostics.clone(), self.file.clone());
                let value = constant_value_visitor.visit(value_context.as_ref());
                self.diagnostics.extend_from_slice(&constant_value_visitor.diagnostics);

                // Check explicit type annotation
                if let Some(declaration_type) = ctx.type_.as_ref() {
                    let explicit_type = match keyword_to_type(declaration_type.get_text()) {
                        Some(builtin_type) => Some(builtin_type),
                        None => Type::EXPLICITLY_CONSTRUCTABLE
                            .iter()
                            .find(|t| t.to_string() == declaration_type.get_text())
                            .cloned(),
                    };

                    if let Some(explicit_type) = explicit_type
                        && let Some(value) = value.as_ref()
                        && !value.r#type.is_sub_type_of(&explicit_type)
                    {
                        let msg = format!(
                            "{variable_name} is declared to be a {}, but its initial value '{}' is a {}",
                            explicit_type.format(),
                            value_context.get_text(),
                            value.r#type.format()
                        );
                        self.diagnostics.push(
                            Diagnostic::from_message(msg)
                                .with_file_name(&self.file.name)
                                .with_parser_context(ctx, self.file.tokens())
                                .with_code("YS0053"),
                        );
                        return;
                    }
                }

                // Create the declaration
                let description = get_document_comments(self.file.tokens(), ctx);
                let description_as_option = (!description.is_empty()).then_some(description);
                if let Some(value) = value.as_ref() {
                    let declaration = Declaration::new(variable_name, value.r#type.clone())
                        .with_default_value(value.raw_value.clone())
                        .with_description_optional(description_as_option)
                        .with_source_file_name(self.file.name.clone())
                        .with_source_node_name_optional(self.current_node_name.clone())
                        .with_range(variable_context.range());
                    self.new_declarations.push(declaration);
                }
            }
        } else {
            // Complex expression — this is a smart variable (inline expansion).
            // Determine the type from the explicit annotation or infer from the expression.
            let var_type = if let Some(declaration_type) = ctx.type_.as_ref() {
                keyword_to_type(declaration_type.get_text()).unwrap_or(Type::Any)
            } else {
                // Infer type from the top-level expression operator
                infer_type_from_expression(expression_context.as_ref())
            };

            let description = get_document_comments(self.file.tokens(), ctx);
            let description_as_option = (!description.is_empty()).then_some(description);
            let declaration = Declaration::new(variable_name, var_type)
                .with_description_optional(description_as_option)
                .with_source_file_name(self.file.name.clone())
                .with_source_node_name_optional(self.current_node_name.clone())
                .with_range(variable_context.range())
                .with_inline_expansion();
            self.new_declarations.push(declaration);
        }
    }
}

fn keyword_to_type(keyword: &str) -> Option<Type> {
    match keyword {
        "string" => Some(Type::String),
        "number" => Some(Type::Number),
        "bool" => Some(Type::Boolean),
        _ => None,
    }
}

/// Infer the result type of an expression from its top-level operator.
/// Used for smart variable declarations that don't have an explicit `as Type` annotation.
fn infer_type_from_expression(expr: &ExpressionContextAll<'_>) -> Type {
    match expr {
        // Comparison and logical operators always return Boolean
        ExpressionContextAll::ExpComparisonContext(_)
        | ExpressionContextAll::ExpEqualityContext(_)
        | ExpressionContextAll::ExpAndOrXorContext(_)
        | ExpressionContextAll::ExpNotContext(_) => Type::Boolean,
        // Arithmetic operators return Number, except `+` which may also concatenate strings.
        ExpressionContextAll::ExpAddSubContext(ctx) => {
            let operands = ctx.expression_all();
            let any_string = operands.iter().any(|e| {
                matches!(
                    e.as_ref(),
                    ExpressionContextAll::ExpValueContext(v)
                        if matches!(v.value().as_deref(), Some(ValueContextAll::ValueStringContext(_)))
                )
            });
            if any_string { Type::String } else { Type::Number }
        }
        ExpressionContextAll::ExpMultDivModContext(_) | ExpressionContextAll::ExpNegativeContext(_) => Type::Number,
        // Parenthesized expression — recurse into inner expression
        ExpressionContextAll::ExpParensContext(ctx) => {
            if let Some(inner) = ctx.expression() {
                infer_type_from_expression(inner.as_ref())
            } else {
                Type::Any
            }
        }
        // Value expression (variable ref, function call) — can't easily infer without
        // looking up the variable/function type, so default to Any.
        // The type checker will resolve this later.
        _ => Type::Any,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_variable_declarations() {
        let file = File {
            file_name: "test.yarn".to_string(),
            source: "title: test
---
<<declare $foo to 1>>
<<declare $bar = \"2\">>
<<declare $baz to true>>
<<declare $quux = \"hello there\" as string>>
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

        assert!(result.warnings.is_empty());
        assert_eq!(result.declarations.len(), 4);
        assert_eq!(
            result.declarations[0],
            Declaration::new("$foo", Type::Number)
                .with_default_value(1.0)
                .with_source_file_name("test.yarn")
                .with_source_node_name("test")
                .with_range(Position { line: 2, character: 10 }..Position { line: 2, character: 14 })
        );

        assert_eq!(
            result.declarations[1],
            Declaration::new("$bar", Type::String)
                .with_default_value("2")
                .with_source_file_name("test.yarn")
                .with_source_node_name("test")
                .with_range(Position { line: 3, character: 10 }..Position { line: 3, character: 14 })
        );

        assert_eq!(
            result.declarations[2],
            Declaration::new("$baz", Type::Boolean)
                .with_default_value(true)
                .with_source_file_name("test.yarn")
                .with_source_node_name("test")
                .with_range(Position { line: 4, character: 10 }..Position { line: 4, character: 14 })
        );

        assert_eq!(
            result.declarations[3],
            Declaration::new("$quux", Type::String)
                .with_default_value("hello there")
                .with_source_file_name("test.yarn")
                .with_source_node_name("test")
                .with_range(Position { line: 5, character: 10 }..Position { line: 5, character: 15 })
        );
    }

    #[test]
    fn catches_type_errors() {
        let file = File {
            file_name: "test.yarn".to_string(),
            source: "title: test
---
<<declare $foo to 1 as string>>
==="
            .to_string(),
        };
        let result = Compiler {
            files: vec![file.clone()],
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
        assert_eq!(
            diagnostics[0],
            Diagnostic::from_message("Type string does not match value 1 (Number)".to_string())
                .with_file_name("test.yarn".to_string())
                .with_context(file.source.clone())
                .with_range(Position { line: 2, character: 0 }..Position { line: 2, character: 31 })
        );
        assert_eq!(
            diagnostics[1],
            Diagnostic::from_message(
                "Can't figure out the type of variable $foo given its context. Specify its type with a <<declare>> statement.".to_string()
            )
            .with_file_name("test.yarn".to_string())
            .with_context(file.source)
            .with_range(Position { line: 2, character: 10 }..Position { line: 2, character: 14 })
        );
    }
}
