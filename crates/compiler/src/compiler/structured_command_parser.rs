//! Parses structured command strings.
//!
//! Equivalent to `StructuredCommandParser` in C# (`Yarn.Compiler.StructuredCommandParser`).
//!
//! ## Argument boundary semantics
//!
//! In the YarnSpinner grammar, `EXPR_WS -> channel(HIDDEN)` — whitespace in
//! `ExpressionMode` is on the HIDDEN channel and is therefore invisible to the
//! parser.  Argument boundaries are therefore determined by **expression
//! grammar**, not by whitespace:
//!
//! - `$a + $b` is **one** argument (binary operator continues the expression).
//! - `$a $b`   is **two** arguments (`$b` starts a fresh expression).
//!
//! This module uses the ANTLR lexer in `ExpressionMode` to tokenise the source
//! and then applies expression-aware grouping to match the C# grammar behaviour.

use crate::listeners::Diagnostic;
use crate::parser::generated::yarnspinnerlexer::{self, YarnSpinnerLexer as RawLexer};
use antlr4rust::TokenSource;
use antlr4rust::input_stream::CodePoint32BitCharStream;
use antlr4rust::token::{TOKEN_DEFAULT_CHANNEL, Token};

/// The result of parsing a structured command string.
///
/// Equivalent to `StructuredCommandParser.ParseResult` in C#.
#[derive(Debug, Clone)]
pub struct StructuredCommandParseResult {
    /// The command name (first identifier in the command string), if one was found.
    pub command_name: Option<String>,

    /// The number of argument values following the command name.
    ///
    /// Argument boundaries follow the same expression-grammar rules as the C#
    /// ANTLR parser: operator tokens (e.g. `+`, `&&`) **continue** the current
    /// argument rather than starting a new one, so `$a + $b` counts as **one**
    /// argument, not three.
    pub argument_count: usize,

    /// Any diagnostics produced during parsing (e.g. from invalid tokens such
    /// as old-style `{expr}` brace expressions).
    pub diagnostics: Vec<Diagnostic>,
}

/// Parses a structured command string into its components.
///
/// A structured command has the form `command_name [arg1] [arg2] ...` where
/// arguments are separated by whitespace.  Arguments may be:
/// - Plain identifiers (`SomeID`, `EnumA.Member`, `.Member`)
/// - Variables (`$var`)
/// - Numbers (`2.3`)
/// - Quoted strings (`"hello"`)
/// - Booleans (`true`, `false`)
/// - Function calls (`func(2, "arg")`)
/// - Operator expressions (`$a + $b`, `!$flag`) counted as **one** argument each
///
/// Old-style brace expressions (`{$myVar}`) are invalid and produce diagnostics,
/// but parsing continues so that the command name can still be recovered.
///
/// Equivalent to `StructuredCommandParser.ParseStructuredCommand()` in C#.
pub fn parse_structured_command(source: &str) -> StructuredCommandParseResult {
    // Encode the source as UTF-32 for the ANTLR character stream.
    let source_chars: Vec<u32> = source.chars().map(|c| c as u32).collect();
    let input = CodePoint32BitCharStream::new(&source_chars);

    let mut lexer = RawLexer::new(input);
    // Start directly in ExpressionMode, matching what the C# implementation
    // does with `lexer.PushMode(YarnSpinnerLexer.ExpressionMode)`.
    lexer.mode = yarnspinnerlexer::ExpressionMode;

    // Drain all tokens from the lexer, collecting the ones on the DEFAULT
    // channel (EXPR_WS is on HIDDEN and is therefore skipped automatically).
    //
    // Gap detection: when the ANTLR lexer encounters a character it cannot
    // match in ExpressionMode (e.g. `{` or `}` from old-style brace
    // expressions), it silently consumes the character and retries — it does
    // NOT emit a TOKEN_INVALID_TYPE.  We detect such gaps by comparing each
    // token's start index against our expected position; any difference means
    // one or more characters were skipped by error recovery.
    let mut tok_types: Vec<i32> = Vec::new();
    let mut tok_texts: Vec<String> = Vec::new();
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    // `covered` tracks the next source-char index we expect a token to start at.
    let mut covered: isize = 0;

    loop {
        let tok = TokenSource::next_token(&mut lexer);
        let tt = tok.get_token_type();

        // Gap check — runs for every token, including EOF, so that chars
        // skipped just before EOF are also caught.
        let start = tok.get_start();
        if start >= 0 && start > covered {
            for pos in covered..start {
                let bad_char = char::from_u32(source_chars[pos as usize]).unwrap_or('\u{FFFD}');
                diagnostics
                    .push(Diagnostic::from_message(format!("token recognition error at: '{bad_char}'")).with_file_name("<structured_command>"));
            }
        }

        if tt == antlr4rust::token::TOKEN_EOF {
            break;
        }

        // Advance `covered` past this token.
        let stop = tok.get_stop();
        if stop >= 0 {
            covered = stop + 1;
        }

        if tok.get_channel() != TOKEN_DEFAULT_CHANNEL {
            // HIDDEN channel (e.g. EXPR_WS) — skip but position was tracked.
            continue;
        }
        tok_types.push(tt);
        tok_texts.push(tok.get_text().to_string());
    }

    if tok_types.is_empty() {
        return StructuredCommandParseResult {
            command_name: None,
            argument_count: 0,
            diagnostics,
        };
    }

    // The first DEFAULT-channel token is the command name (should be FUNC_ID).
    let command_name = Some(tok_texts[0].clone());
    let mut i = 1usize;
    let mut argument_count = 0usize;

    while i < tok_types.len() {
        argument_count += 1;
        i = consume_expression(&tok_types, i);
    }

    StructuredCommandParseResult {
        command_name,
        argument_count,
        diagnostics,
    }
}

// ---------------------------------------------------------------------------
// Expression-aware token grouping
// ---------------------------------------------------------------------------
//
// These helpers mirror the C# grammar rules:
//
//   structured_command_value : expression | FUNC_ID ;
//   expression : ... (full YarnSpinner expression grammar)
//
// After consuming one "value unit" at paren_depth 0, if the next token is a
// binary operator the expression continues; otherwise the argument ends.

/// Returns `true` for tokens that continue an expression as a binary infix
/// operator.  Whitespace is never seen here (HIDDEN channel, filtered out).
///
/// **Note**: `DOT` is intentionally absent.  Enum-member access (`EnumA.Member`)
/// is a `value`-level construct (`enum_member_ref : FUNC_ID DOT FUNC_ID`), not
/// an infix operator in the `expression` rule.  It is handled inside
/// `consume_value` so that `EnumA.Member .Member` is counted as two separate
/// arguments rather than one chained expression.
fn is_binary_operator(tt: i32) -> bool {
    use yarnspinnerlexer::*;
    matches!(
        tt,
        OPERATOR_MATHS_ADDITION
            | OPERATOR_MATHS_SUBTRACTION
            | OPERATOR_MATHS_MULTIPLICATION
            | OPERATOR_MATHS_DIVISION
            | OPERATOR_MATHS_MODULUS
            | OPERATOR_LOGICAL_AND
            | OPERATOR_LOGICAL_OR
            | OPERATOR_LOGICAL_XOR
            | OPERATOR_LOGICAL_EQUALS
            | OPERATOR_LOGICAL_NOT_EQUALS
            | OPERATOR_LOGICAL_LESS
            | OPERATOR_LOGICAL_GREATER
            | OPERATOR_LOGICAL_LESS_THAN_EQUALS
            | OPERATOR_LOGICAL_GREATER_THAN_EQUALS
    )
}

/// Consume one complete expression (including operator chains) starting at
/// index `i`.  Returns the index of the first token that was NOT consumed.
fn consume_expression(types: &[i32], mut i: usize) -> usize {
    i = consume_value(types, i);
    while i < types.len() && is_binary_operator(types[i]) {
        i += 1; // consume the operator
        i = consume_value(types, i); // consume the right-hand operand
    }
    i
}

/// Consume one primary "value" starting at index `i`.
fn consume_value(types: &[i32], mut i: usize) -> usize {
    use yarnspinnerlexer::*;
    if i >= types.len() {
        return i;
    }
    match types[i] {
        FUNC_ID => {
            i += 1;
            // Look ahead for:
            //   FUNC_ID ( ... )       → function call
            //   FUNC_ID . FUNC_ID     → enum-member reference (one value)
            if i < types.len() {
                if types[i] == LPAREN {
                    i = consume_paren_group(types, i);
                } else if types[i] == DOT {
                    // Enum-member access: consume `DOT FUNC_ID` as part of
                    // this value so that `EnumA.Member` is one argument and
                    // the following `.Member` (DOT FUNC_ID) is a separate one.
                    i += 1; // consume DOT
                    if i < types.len() && types[i] == FUNC_ID {
                        i += 1; // consume Member name
                    }
                }
            }
        }
        DOT => {
            // Shorthand enum-member reference: `.Member`
            i += 1; // DOT
            if i < types.len() && types[i] == FUNC_ID {
                i += 1; // Member name
            }
        }
        LPAREN => {
            i = consume_paren_group(types, i);
        }
        OPERATOR_LOGICAL_NOT | OPERATOR_MATHS_SUBTRACTION => {
            // Prefix / unary operators.
            i += 1;
            i = consume_value(types, i);
        }
        _ => {
            // VAR_ID, NUMBER, STRING, KEYWORD_TRUE/FALSE, TOKEN_INVALID_TYPE …
            i += 1;
        }
    }
    i
}

/// Consume a parenthesised group `( ... )`, handling nesting.
/// `types[i]` must be `LPAREN` on entry.
fn consume_paren_group(types: &[i32], mut i: usize) -> usize {
    use yarnspinnerlexer::*;
    debug_assert!(i < types.len() && types[i] == LPAREN);
    let mut depth = 0i32;
    while i < types.len() {
        match types[i] {
            LPAREN => {
                depth += 1;
                i += 1;
            }
            RPAREN => {
                depth -= 1;
                i += 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {
                i += 1;
            }
        }
    }
    i
}
