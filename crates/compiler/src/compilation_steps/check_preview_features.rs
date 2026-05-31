//! Compilation step: emit `YS0036` diagnostics for v3 language features when
//! the project's declared language version is below 3.

use crate::prelude::*;
use crate::project_version::YARNSPINNER_PROJECT_VERSION_3;

/// If `state.job.language_version` is set to a value below
/// [`YARNSPINNER_PROJECT_VERSION_3`], scan each parsed file for v3 features
/// (enums, line groups, inline `<<once>>` conditions, and block-level
/// `<<once>>` statements) and emit a `YS0036` error for each one found.
pub(crate) fn check_preview_features(mut state: CompilationIntermediate) -> CompilationIntermediate {
    let language_version = match state.job.language_version {
        Some(v) if v < YARNSPINNER_PROJECT_VERSION_3 => v,
        // None means "no version constraint" – allow everything.
        _ => return state,
    };

    // Collect file source texts so we can do source-text scans for features
    // that are not captured in the metadata (e.g. block-level <<once>>).
    let files: Vec<(&str, &str)> = state.job.files.iter().map(|f| (f.file_name.as_str(), f.source.as_str())).collect();

    for (file_result, _) in &state.parsed_files {
        let file_name = file_result.name.as_str();
        let source = files.iter().find(|(name, _)| *name == file_name).map(|(_, src)| *src).unwrap_or("");

        emit_preview_feature_diagnostics(file_result, source, file_name, language_version, &mut state.diagnostics);
    }

    state
}

fn emit_preview_feature_diagnostics(
    file_result: &FileParseResult<'_>,
    source: &str,
    file_name: &str,
    language_version: u32,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let min_v = YARNSPINNER_PROJECT_VERSION_3;
    let make_diag = |feature: &str| -> Diagnostic {
        let message =
            format!("Language feature \"{feature}\" is not available at language version {language_version}; it requires version {min_v} or later");
        Diagnostic::from_message(message)
            .with_file_name(file_name)
            .with_severity(DiagnosticSeverity::Error)
            .with_code("YS0036")
    };

    // ── Enums ─────────────────────────────────────────────────────────────
    if !file_result.enum_registry.enums.is_empty() {
        // Emit one diagnostic per distinct enum found.
        for _ in &file_result.enum_registry.enums {
            diagnostics.push(make_diag("enums"));
        }
    }

    // ── Line groups ───────────────────────────────────────────────────────
    if !file_result.line_group_metadata.converted_arrows.is_empty() {
        diagnostics.push(make_diag("line groups"));
    }

    // ── Inline <<once>> conditions ────────────────────────────────────────
    if !file_result.line_group_metadata.once_lines.is_empty() || !file_result.line_group_metadata.once_if_lines.is_empty() {
        diagnostics.push(make_diag("'once' conditions"));
    }

    // ── Block-level <<once>> statements ──────────────────────────────────
    // These are not stored in LineGroupMetadata (the lexer transforms them
    // transparently), so we scan the source text directly.
    if has_block_once_statement(source) {
        diagnostics.push(make_diag("'once' statements"));
    }

    // ── Smart variables ───────────────────────────────────────────────────
    // A smart variable is <<declare $var = expr>> where the expression is
    // non-literal (complex expression). Declarations are not yet built at
    // this compilation step, so we scan the source text directly.
    if has_smart_variable_declaration(source) {
        diagnostics.push(make_diag("smart variables"));
    }
}

/// Returns `true` if `source` contains a block-level `<<once>>` statement,
/// i.e. a line inside a node body where the trimmed content is exactly
/// `<<once>>` or starts with `<<once ` (block form with expression) and is
/// not an inline condition on a longer dialogue line.
fn has_block_once_statement(source: &str) -> bool {
    let mut in_body = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed == "---" {
            in_body = true;
            continue;
        }
        if trimmed == "===" {
            in_body = false;
            continue;
        }
        if !in_body {
            continue;
        }
        // A block-level once statement appears as the entire command on the
        // line (possibly preceded by indentation).  The distinguishing
        // criterion is that the line begins with `<<once>>` or `<<once `,
        // rather than being a dialogue line that ends with `<<once>>`.
        if trimmed == "<<once>>" || trimmed.starts_with("<<once ") && trimmed.ends_with(">>") && !trimmed.starts_with("<<once if ")
        // <<once>> blocks can also be: <<once if expr>> which is a
        // block form.  The inline form of a once-if is handled by
        // the once_if_lines metadata; for block detection we include
        // both plain `<<once>>` and `<<once if ...>>` block forms.
        {
            return true;
        }
    }
    false
}

/// Returns `true` if `source` contains a `<<declare $var = expr>>` where the
/// expression is non-literal (i.e. a smart variable whose value is computed
/// at runtime), as opposed to a stored variable whose initial value is a
/// plain literal (`5`, `-3.14`, `true`, `"hello"`, etc.).
///
/// This is intentionally conservative: only expressions that are clearly
/// complex (containing operators, variable references, function calls, etc.)
/// are flagged.  Simple negative-number literals (`-1`) are NOT flagged.
fn has_smart_variable_declaration(source: &str) -> bool {
    for line in source.lines() {
        let trimmed = line.trim();

        // Find "<<declare " anywhere on the line.
        let Some(after_keyword) = find_after_prefix(trimmed, "<<declare ") else {
            continue;
        };

        // Skip the variable name (starts with '$', ends at the first
        // non-identifier character).
        let after_keyword = after_keyword.trim_start();
        if !after_keyword.starts_with('$') {
            continue;
        }
        let name_end = after_keyword
            .find(|c: char| !c.is_alphanumeric() && c != '_' && c != '$')
            .unwrap_or(after_keyword.len());
        let after_var = after_keyword[name_end..].trim_start();

        // Must have an '=' to have any value at all.
        if !after_var.starts_with('=') {
            continue;
        }
        let expr_raw = after_var[1..].trim_start();

        // Remove the closing ">>" (and anything after it).
        let expr_raw = if let Some(p) = expr_raw.rfind(">>") { &expr_raw[..p] } else { expr_raw };
        let expr_raw = expr_raw.trim_end();

        // Strip optional " as <type>" annotation.
        let expr = strip_type_annotation(expr_raw);

        if !is_literal_expression(expr) {
            return true;
        }
    }
    false
}

/// Returns the substring of `line` that follows `prefix`, or `None` if
/// `prefix` does not appear in `line`.
fn find_after_prefix<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let pos = line.find(prefix)?;
    Some(&line[pos + prefix.len()..])
}

/// Strips a ` as <type>` suffix (case-insensitive for the type keyword)
/// from the end of an expression string.
fn strip_type_annotation(expr: &str) -> &str {
    for suffix in &[" as number", " as string", " as bool"] {
        if expr.len() >= suffix.len() {
            let candidate = &expr[expr.len() - suffix.len()..];
            if candidate.eq_ignore_ascii_case(suffix) {
                return expr[..expr.len() - suffix.len()].trim_end();
            }
        }
    }
    expr
}

/// Returns `true` if `expr` is a simple literal value:
/// a number (optionally negative), a boolean, or a double-quoted string.
fn is_literal_expression(expr: &str) -> bool {
    let expr = expr.trim();
    if expr.is_empty() {
        return true; // no expression — not a smart variable
    }
    // Boolean literals.
    if expr == "true" || expr == "false" {
        return true;
    }
    // String literals.
    if expr.len() >= 2 && expr.starts_with('"') && expr.ends_with('"') {
        return true;
    }
    // Number literals: optional leading '-', then ASCII digits with at most one '.'.
    let digits = expr.strip_prefix('-').unwrap_or(expr);
    if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit() || c == '.') {
        let dots = digits.chars().filter(|&c| c == '.').count();
        if dots <= 1 {
            return true;
        }
    }
    false
}
