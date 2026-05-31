use crate::prelude::*;
use yarnspinner_core::prelude::*;
use yarnspinner_core::types::FunctionType;

/// Validates function calls that appear inside `when:` header expressions.
///
/// The main type-checker runs on node *bodies* via the ANTLR parse tree, but
/// `when:` headers are compiled separately by the `WhenExprParser` without
/// going through the type-checker.  This step closes that gap: after
/// `check_types` has populated `known_variable_declarations` with all function
/// signatures, we scan source files for `when:` header lines and validate any
/// function calls they contain.
///
/// Emits:
///  - **YS0013** when a function is called with the wrong number of arguments.
///  - **YS0050** when an argument type doesn't match the expected parameter type.
pub(crate) fn type_check_when_headers(mut state: CompilationIntermediate) -> CompilationIntermediate {
    // Build a lookup map of function declarations.
    let func_decls: Vec<(String, FunctionType)> = state
        .known_variable_declarations
        .iter()
        .filter_map(|d| {
            if let Type::Function(ft) = &d.r#type {
                Some((d.name.clone(), ft.clone()))
            } else {
                None
            }
        })
        .collect();

    if func_decls.is_empty() {
        return state;
    }

    for file_idx in 0..state.job.files.len() {
        let lines: Vec<(usize, String)> = state.job.files[file_idx]
            .source
            .lines()
            .enumerate()
            .map(|(i, l)| (i, l.to_owned()))
            .collect();
        let file_name = state.job.files[file_idx].file_name.clone();

        for (line_idx, line) in &lines {
            let trimmed = line.trim_start();
            if !trimmed.starts_with("when:") {
                continue;
            }
            let after_colon = &trimmed["when:".len()..];
            let expr = after_colon.trim_start();
            if expr.is_empty() {
                continue;
            }

            // The column in the original line where the expression starts.
            let prefix_len = line.len() - trimmed.len() + "when:".len();
            let expr_col_start = prefix_len + (after_colon.len() - after_colon.trim_start().len());

            // Validate all function calls in this expression.
            validate_when_expression(expr, *line_idx, expr_col_start, &func_decls, &file_name, &mut state.diagnostics);
        }
    }

    state
}

// ---------------------------------------------------------------------------
// Simple recursive-descent validator for when-expression function calls
// ---------------------------------------------------------------------------

/// Tokenise and validate a when-expression for function call correctness.
fn validate_when_expression(
    expr: &str,
    line: usize,
    expr_col_start: usize,
    func_decls: &[(String, FunctionType)],
    file_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let tokens = tokenize_when(expr);
    let mut pos = 0;
    validate_expr(&tokens, &mut pos, expr, line, expr_col_start, func_decls, file_name, diagnostics);
}

#[derive(Debug, Clone)]
enum WhenTok<'a> {
    Ident(&'a str, usize), // name, byte offset in expr
    Str(usize),
    Num(usize),
    Bool(usize),
    LParen,
    RParen,
    Comma,
    Op,
    Eof,
}

fn tokenize_when(expr: &str) -> Vec<WhenTok<'_>> {
    let mut tokens = Vec::new();
    let bytes = expr.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Skip whitespace.
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        // String literal.
        if bytes[i] == b'"' {
            let start = i;
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                i += 1;
            }
            if i < bytes.len() {
                i += 1; // closing quote
            }
            tokens.push(WhenTok::Str(start));
            continue;
        }
        // Number literal.
        if bytes[i].is_ascii_digit() || (bytes[i] == b'-' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit()) {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            tokens.push(WhenTok::Num(start));
            continue;
        }
        // Identifier or keyword.
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &expr[start..i];
            match word {
                "true" | "false" => tokens.push(WhenTok::Bool(start)),
                _ => tokens.push(WhenTok::Ident(word, start)),
            }
            continue;
        }
        // Variable ($name).
        if bytes[i] == b'$' {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            tokens.push(WhenTok::Ident(&expr[start..i], start));
            continue;
        }
        match bytes[i] {
            b'(' => {
                tokens.push(WhenTok::LParen);
                i += 1;
            }
            b')' => {
                tokens.push(WhenTok::RParen);
                i += 1;
            }
            b',' => {
                tokens.push(WhenTok::Comma);
                i += 1;
            }
            _ => {
                tokens.push(WhenTok::Op);
                i += 1;
            }
        }
    }
    tokens.push(WhenTok::Eof);
    tokens
}

/// Walk the token stream and validate function calls. Returns the "type hint"
/// for the subexpression (used to validate argument types).
#[allow(clippy::too_many_arguments)]
fn validate_expr<'a>(
    tokens: &[WhenTok<'a>],
    pos: &mut usize,
    expr: &str,
    line: usize,
    expr_col_start: usize,
    func_decls: &[(String, FunctionType)],
    file_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> ExprKind {
    // Binary operator expressions: expr op expr* — we handle by just
    // consuming operands and operators recursively at the primary level.
    let mut kind = validate_primary(tokens, pos, expr, line, expr_col_start, func_decls, file_name, diagnostics);
    loop {
        match tokens.get(*pos) {
            Some(WhenTok::Op) | Some(WhenTok::Ident(_, _)) if matches!(tokens.get(*pos), Some(WhenTok::Ident(k, _)) if *k == "and" || *k == "or" || *k == "not") =>
            {
                *pos += 1;
                validate_primary(tokens, pos, expr, line, expr_col_start, func_decls, file_name, diagnostics);
                kind = ExprKind::Unknown;
            }
            Some(WhenTok::Op) => {
                *pos += 1;
                validate_primary(tokens, pos, expr, line, expr_col_start, func_decls, file_name, diagnostics);
                kind = ExprKind::Unknown;
            }
            _ => break,
        }
    }
    kind
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExprKind {
    Number,
    String,
    Bool,
    Unknown,
}

#[allow(clippy::too_many_arguments)]
fn validate_primary<'a>(
    tokens: &[WhenTok<'a>],
    pos: &mut usize,
    expr: &str,
    line: usize,
    expr_col_start: usize,
    func_decls: &[(String, FunctionType)],
    file_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> ExprKind {
    match tokens.get(*pos).cloned() {
        Some(WhenTok::LParen) => {
            *pos += 1;
            let kind = validate_expr(tokens, pos, expr, line, expr_col_start, func_decls, file_name, diagnostics);
            if matches!(tokens.get(*pos), Some(WhenTok::RParen)) {
                *pos += 1;
            }
            kind
        }
        Some(WhenTok::Num(_)) => {
            *pos += 1;
            ExprKind::Number
        }
        Some(WhenTok::Str(_)) => {
            *pos += 1;
            ExprKind::String
        }
        Some(WhenTok::Bool(_)) => {
            *pos += 1;
            ExprKind::Bool
        }
        Some(WhenTok::Ident(name, name_offset)) if name != "and" && name != "or" && name != "not" => {
            let name_owned = name.to_owned();
            *pos += 1;
            // Is it a function call?
            if matches!(tokens.get(*pos), Some(WhenTok::LParen)) {
                *pos += 1; // consume '('
                // Collect argument types.
                let mut arg_types: Vec<ExprKind> = Vec::new();
                while !matches!(tokens.get(*pos), Some(WhenTok::RParen) | Some(WhenTok::Eof)) {
                    let arg_kind = validate_expr(tokens, pos, expr, line, expr_col_start, func_decls, file_name, diagnostics);
                    arg_types.push(arg_kind);
                    if matches!(tokens.get(*pos), Some(WhenTok::Comma)) {
                        *pos += 1;
                    }
                }
                if matches!(tokens.get(*pos), Some(WhenTok::RParen)) {
                    *pos += 1; // consume ')'
                }

                // Validate against known function declarations.
                if let Some((_, ft)) = func_decls.iter().find(|(n, _)| n == &name_owned) {
                    let expected_count = ft.parameters.len();
                    let is_variadic = ft.variadic_parameter_type.is_some();
                    let supplied_count = arg_types.len();

                    // Check argument count.
                    let count_ok = if is_variadic {
                        supplied_count >= expected_count
                    } else {
                        supplied_count == expected_count
                    };

                    if !count_ok {
                        let params = if expected_count == 1 { "parameter" } else { "parameters" };
                        let func_col = expr_col_start + name_offset;
                        let func_end_col = func_col + name_owned.len();
                        diagnostics.push(
                            Diagnostic::from_message(format!(
                                "Function \"{}\" expects {} {}, but received {}",
                                name_owned, expected_count, params, supplied_count
                            ))
                            .with_file_name(file_name)
                            .with_range(
                                Position { line, character: func_col }..Position {
                                    line,
                                    character: func_end_col,
                                },
                            )
                            .with_code("YS0013"),
                        );
                        return ExprKind::Unknown;
                    }

                    // Check argument types (fixed parameters only).
                    for (i, (arg_kind, expected_opt)) in arg_types.iter().zip(ft.parameters.iter()).enumerate() {
                        let expected_type = match expected_opt.as_ref() {
                            Some(t) => t,
                            None => continue, // unbound parameter type, skip
                        };
                        let mismatch = match (arg_kind, expected_type) {
                            (ExprKind::Number, Type::Number) => false,
                            (ExprKind::String, Type::String) => false,
                            (ExprKind::Bool, Type::Boolean) => false,
                            (ExprKind::Unknown, _) => false, // can't tell
                            (_, Type::Any) => false,
                            _ => true,
                        };
                        if mismatch {
                            // Find the token that produced this argument.
                            // We walk back through the token stream to find the
                            // Nth primary token's offset.
                            let (arg_text, arg_offset) = find_nth_arg_token(tokens, expr, i);
                            let arg_col = expr_col_start + arg_offset;
                            let arg_end_col = arg_col + arg_text.len();
                            let arg_type_name = match arg_kind {
                                ExprKind::Number => "Number",
                                ExprKind::String => "String",
                                ExprKind::Bool => "Bool",
                                ExprKind::Unknown => "Unknown",
                            };
                            let expected_type_name = match expected_type {
                                Type::Number => "Number",
                                Type::String => "String",
                                Type::Boolean => "Bool",
                                _ => "Unknown",
                            };
                            diagnostics.push(
                                Diagnostic::from_message(format!("{} ({}) is not convertible to {}", arg_text, arg_type_name, expected_type_name))
                                    .with_file_name(file_name)
                                    .with_range(
                                        Position { line, character: arg_col }..Position {
                                            line,
                                            character: arg_end_col,
                                        },
                                    )
                                    .with_code("YS0050"),
                            );
                        }
                    }
                }

                ExprKind::Unknown
            } else {
                ExprKind::Unknown
            }
        }
        Some(WhenTok::Ident(_, _)) => {
            // "and"/"or"/"not" keywords — not a value primary.
            ExprKind::Unknown
        }
        _ => ExprKind::Unknown,
    }
}

/// Attempt to recover the source text and offset for the `n`-th argument in
/// the first function call found in `tokens`.
///
/// This is a best-effort helper: if we can't find the exact token, we return
/// an empty slice at offset 0 (the diagnostic will still be emitted, just
/// without an accurate range).
fn find_nth_arg_token<'a>(tokens: &[WhenTok<'a>], expr: &'a str, n: usize) -> (&'a str, usize) {
    // Find the first '(' then count arguments.
    let mut i = 0;
    while i < tokens.len() {
        if let WhenTok::LParen = &tokens[i] {
            i += 1; // skip '('
            let mut arg_idx = 0;
            let mut depth = 1usize;
            while i < tokens.len() && depth > 0 {
                match &tokens[i] {
                    WhenTok::LParen => depth += 1,
                    WhenTok::RParen => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    WhenTok::Comma if depth == 1 => {
                        arg_idx += 1;
                    }
                    _ if depth == 1 && arg_idx == n => {
                        // This is the first token of the n-th argument.
                        return match &tokens[i] {
                            WhenTok::Ident(s, off) => (s, *off),
                            WhenTok::Num(off) => {
                                let start = *off;
                                let mut end = start;
                                let bytes = expr.as_bytes();
                                while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b'.') {
                                    end += 1;
                                }
                                (&expr[start..end], start)
                            }
                            WhenTok::Bool(off) => {
                                let start = *off;
                                let word = if expr[start..].starts_with("true") { "true" } else { "false" };
                                (word, start)
                            }
                            WhenTok::Str(off) => {
                                // Include quotes.
                                let start = *off;
                                let mut end = start + 1;
                                let bytes = expr.as_bytes();
                                while end < bytes.len() && bytes[end] != b'"' {
                                    end += 1;
                                }
                                if end < bytes.len() {
                                    end += 1;
                                }
                                (&expr[start..end], start)
                            }
                            _ => ("", 0),
                        };
                    }
                    _ => {}
                }
                i += 1;
            }
            break;
        }
        i += 1;
    }
    ("", 0)
}
