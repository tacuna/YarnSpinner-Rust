use crate::prelude::*;

/// Validates all shadow lines registered in the string table.
///
/// A shadow line uses `#shadow:<id>` instead of `#line:<id>`. It must:
/// 1. Reference a source line that actually exists in the string table.
/// 2. Have identical text to that source line (no silent divergence).
/// 3. Not be a source line that itself contains inline expressions
///    (`{$var}`), because expressions are substituted at runtime per-line.
///
/// # Divergence from C#
/// The C# compiler nulls out `text` on valid shadow entries after this step
/// (`shadowLineTableEntry.text = null`). We skip that mutation because our
/// [`StringInfo::text`] field is a plain `String`, not `Option<String>`.
/// Shadow lines retain their validated-identical text copy instead.
/// Callers that need to know a line is a shadow should check
/// `StringInfo::shadow_line_id.is_some()`.
pub(crate) fn validate_shadow_lines(mut state: CompilationIntermediate) -> CompilationIntermediate {
    // Collect shadow line entries for validation
    let shadow_entries: Vec<_> = state
        .string_table
        .iter()
        .filter(|(_, info)| info.shadow_line_id.is_some())
        .map(|(id, info)| {
            (
                id.clone(),
                info.shadow_line_id.clone().unwrap(),
                info.text.clone(),
                info.file_name.clone(),
            )
        })
        .collect();

    for (_shadow_id, source_id, shadow_text, file_name) in shadow_entries {
        let source_id: LineId = source_id.into();

        // Check that the source line exists
        let Some(source_info) = state.string_table.get(&source_id) else {
            state.diagnostics.push(
                Diagnostic::from_message(format!("\"{source_id}\" is not a known line ID."))
                    .with_file_name(&file_name)
                    .with_code("YS0042"),
            );
            continue;
        };

        let source_text = &source_info.text;

        // Check that the source line has no expressions.
        // Expressions in compiled formatted text appear as `{0}`, `{1}`, etc.
        // (see `generate_formatted_text`). A bare `{` followed by a digit is
        // sufficient — literal `{` in Yarn source is escaped and never reaches
        // this form.
        let has_expressions = source_text
            .chars()
            .zip(source_text.chars().skip(1))
            .any(|(a, b)| a == '{' && b.is_ascii_digit());
        if has_expressions {
            state.diagnostics.push(
                Diagnostic::from_message("Shadow lines must not have expressions".to_string())
                    .with_file_name(&file_name)
                    .with_code("YS0043"),
            );
        }

        // Check that the shadow text matches the source text
        if *source_text != shadow_text {
            state.diagnostics.push(
                Diagnostic::from_message("Shadow lines must have the same text as their source lines".to_string())
                    .with_file_name(&file_name)
                    .with_code("YS0044"),
            );
        }
    }

    state
}
