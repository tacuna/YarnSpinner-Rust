use crate::prelude::generated::yarnspinnerlexer;
use crate::prelude::generated::yarnspinnerparser::YarnSpinnerParserContextType;
use crate::prelude::generated::yarnspinnerparserlistener::YarnSpinnerParserListener;
use crate::prelude::*;
use antlr4rust::char_stream::InputData;
use antlr4rust::error_listener::ErrorListener;
use antlr4rust::errors::ANTLRError;
use antlr4rust::recognizer::Recognizer;
use antlr4rust::token::Token;
use antlr4rust::token_factory::TokenFactory;
use antlr4rust::tree::ParseTreeListener;
pub use diagnostic::*;
use std::cell::RefCell;
use std::rc::Rc;
use yarnspinner_core::prelude::*;

mod diagnostic;
mod diagnostic_descriptor;
pub use diagnostic_descriptor::DiagnosticDescriptor;

/// Classifies a parser error message string into the appropriate diagnostic
/// code, mirroring the `GetDiagnosticForParserError` logic in the C# compiler.
///
/// Used for non-EOF, non-BODY_END, non-COMMAND_TEXT_NEWLINE errors.
fn classify_parser_error_code(msg: &str) -> &'static str {
    let lower = msg.to_lowercase();
    // "missing '==='" / "missing '---'" → missing node-body delimiter.
    if lower.contains("missing") && (lower.contains("===") || lower.contains("'==='") || lower.contains("'---'") || lower.contains("delimiter")) {
        "YS0004"
    // "missing 'COMMAND_END'" / "missing '>>'" → unclosed command.
    } else if lower.contains("missing") && (lower.contains("command_end") || lower.contains(">>") || lower.contains("'>'")) {
        "YS0006"
    } else {
        "YS0005"
    }
}
pub(crate) struct LexerErrorListener {
    pub(crate) diagnostics: RefCell<Vec<Diagnostic>>,
    file_name: String,
}

impl LexerErrorListener {
    pub(crate) fn new(file_name: String) -> Self {
        Self {
            file_name,
            diagnostics: Default::default(),
        }
    }
}

impl<'input, T: Recognizer<'input>> ErrorListener<'input, T> for LexerErrorListener {
    fn syntax_error(
        &self,
        _recognizer: &T,
        _offending_symbol: Option<&<T::TF as TokenFactory<'input>>::Inner>,
        line: isize,
        column: isize,
        msg: &str,
        _error: Option<&ANTLRError>,
    ) {
        let line = (line - 1) as usize;
        let column = column as usize;
        let range = Position { line, character: column }..Position { line, character: column + 1 };
        self.diagnostics.borrow_mut().push(
            Diagnostic::from_message(msg)
                .with_range(range)
                .with_file_name(&self.file_name)
                .with_code("YS0005"),
        );
    }
}

pub(crate) struct ParserErrorListener {
    pub(crate) diagnostics: Rc<RefCell<Vec<Diagnostic>>>,
    /// Not in original implementation, but needed because
    /// [`Token::get_source`] is not implemented by antlr4rust.
    /// So we take the entire file instead of just the file name
    /// and extract the lines ourselves.
    file: File,
}

impl ParserErrorListener {
    pub fn new(file: File) -> Self {
        Self {
            diagnostics: Default::default(),
            file,
        }
    }
}

impl<'input, T: Recognizer<'input>> ErrorListener<'input, T> for ParserErrorListener {
    fn syntax_error(
        &self,
        _recognizer: &T,
        offending_symbol: Option<&<T::TF as TokenFactory<'input>>::Inner>,
        line: isize,
        column: isize,
        msg: &str,
        _error: Option<&ANTLRError>,
    ) {
        let line = line as usize;
        let range = Position {
            line: line.saturating_sub(1),
            character: (column + 1) as usize,
        }..Position {
            line: line.saturating_sub(1),
            character: (column + 1) as usize,
        };

        // Determine the diagnostic code based on the offending token type first
        // (mirrors C# ErrorListener), then fall back to message-text classification.
        let code_from_token = offending_symbol.map(|sym| {
            let tt = sym.get_token_type();
            if tt == yarnspinnerlexer::BODY_END {
                // BODY_END (===) encountered while the parser is inside an if/once
                // block → the scope was never closed with <<endif>>/<<endonce>>.
                "YS0007"
            } else if tt == yarnspinnerlexer::COMMAND_TEXT_NEWLINE {
                // A newline inside a command block → the >> was missing.
                "YS0006"
            } else if tt == antlr4rust::token::TOKEN_EOF {
                // EOF: distinguish between unclosed command (COMMAND_END expected)
                // and a missing node-body delimiter (BODY_END / === expected).
                let lower = msg.to_lowercase();
                if lower.contains("command_end") || lower.contains(">>") {
                    "YS0006"
                } else if lower.contains("===") || lower.contains("'==='") {
                    "YS0004"
                } else {
                    classify_parser_error_code(msg)
                }
            } else {
                classify_parser_error_code(msg)
            }
        });
        let code = code_from_token.unwrap_or_else(|| classify_parser_error_code(msg));

        let mut diagnostic = Diagnostic::from_message(msg)
            .with_file_name(&self.file.file_name)
            .with_range(range)
            .with_code(code);
        if let Some(offending_symbol) = offending_symbol {
            let mut string = String::new();

            // the line with the error on it
            let input = &self.file.source;
            let mut lines = input.lines();
            let error_line = lines.nth(line.saturating_sub(1)).unwrap_or(input);
            string.push_str(error_line);
            string.push('\n');

            // adding indicator symbols pointing out where the error is
            // on the line
            let start = offending_symbol.get_start();
            let stop = offending_symbol.get_stop();
            if start >= 0 && stop >= 0 {
                // the end point of the error in "line space"
                let end = (stop - start) + column + 1;
                for i in 0..end {
                    // move over until we are at the point we need to be
                    if i >= column && i < end {
                        string.push('^');
                    } else {
                        string.push(' ');
                    }
                }
            }

            let line = offending_symbol.get_line_as_usize().saturating_sub(1);
            let column = offending_symbol.get_column_as_usize();
            let length = offending_symbol.get_text().len();
            diagnostic = diagnostic.with_context(string).with_start_line(line).with_range(
                Position { line, character: column }..Position {
                    line,
                    character: column + length,
                },
            );
        }
        self.diagnostics.borrow_mut().push(diagnostic);

        // If the offending token is `null`, emit a dedicated YS0046 diagnostic.
        // In v3 grammar, `null` is not a valid expression token, so it causes a parse
        // error before reaching the type checker. We still need the user-facing message.
        if let Some(sym) = offending_symbol
            && sym.get_token_type() == yarnspinnerlexer::KEYWORD_NULL
        {
            let line = sym.get_line_as_usize().saturating_sub(1);
            let column = sym.get_column_as_usize();
            let null_diag = Diagnostic::from_message("Null is not a permitted type in Yarn Spinner 2.0 and later")
                .with_file_name(&self.file.file_name)
                .with_range(Position { line, character: column }..Position { line, character: column + 4 })
                .with_code("YS0046");
            self.diagnostics.borrow_mut().push(null_diag);
        }
    }
}

impl ParseTreeListener<'_, YarnSpinnerParserContextType> for ParserErrorListener {}
impl YarnSpinnerParserListener<'_> for ParserErrorListener {}
