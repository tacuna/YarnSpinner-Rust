use crate::prelude::*;
use regex::Regex;

/// Validates that all static `<<jump X>>` and `<<detour X>>` commands refer to
/// nodes that exist in the compiled program. Emits a YS0012 warning for each
/// reference to an undefined node. This matches the behavior of C#'s
/// `UndefinedNodeVisitor` diagnostic (YS0012-UndefinedNode).
///
/// Note: Only static (identifier-based) jump/detour targets are checked.
/// Expression-based targets such as `<<jump {$some_var}>>` are skipped.
pub(crate) fn validate_undefined_node_references(mut state: CompilationIntermediate) -> CompilationIntermediate {
    // Only check if we have a successfully compiled program.
    let Some(Ok(compilation)) = state.result.as_ref() else {
        return state;
    };
    let Some(program) = compilation.program.as_ref() else {
        return state;
    };

    // Collect all known node names from the compiled program. After
    // `compile_node_groups` runs, this includes hub nodes too.
    let known_nodes: std::collections::HashSet<&str> = program.nodes.keys().map(String::as_str).collect();

    // Match <<jump NodeName>> or <<detour NodeName>> where the target is a
    // bare identifier (not an expression like `{$var}`). The node name may
    // contain letters, digits, underscores, and dots.
    let re = Regex::new(r"<<\s*(?:jump|detour)\s+([A-Za-z_][A-Za-z0-9_.]*)\s*>>").unwrap();

    for file in &state.job.files {
        for cap in re.captures_iter(&file.source) {
            let node_name = &cap[1];
            if !known_nodes.contains(node_name) {
                let match_start = cap.get(0).unwrap().start();
                let (line, character) = offset_to_line_col(&file.source, match_start);
                let match_end = cap.get(0).unwrap().end();
                let (end_line, end_character) = offset_to_line_col(&file.source, match_end);

                state.diagnostics.push(
                    Diagnostic::from_message(format!("Jump to undefined node: '{node_name}'"))
                        .with_file_name(file.file_name.clone())
                        .with_range(
                            Position { line, character }..Position {
                                line: end_line,
                                character: end_character,
                            },
                        )
                        .with_severity(DiagnosticSeverity::Warning)
                        .with_code("YS0012"),
                );
            }
        }
    }

    state
}

/// Converts a byte offset in `source` to a zero-based (line, character) pair.
fn offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 0usize;
    let mut col = 0usize;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}
