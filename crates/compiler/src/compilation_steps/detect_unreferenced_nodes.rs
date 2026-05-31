use crate::prelude::*;
use regex::Regex;
use std::collections::HashSet;

/// Emits a YS0009 warning for every node that exists in the compiled program
/// but is never referenced by a `<<jump>>`, `<<detour>>`, or shortcut option
/// in any other node.
///
/// Adapted from the C# `UnreferencedNode` diagnostic.  Note that the C#
/// implementation is marked `[Skip]` in tests because "at least one node will
/// almost always be unreferenced, being the entry point".  We follow the same
/// semantics: only warn, never error.
pub(crate) fn detect_unreferenced_nodes(mut state: CompilationIntermediate) -> CompilationIntermediate {
    // Only run after a successful full compilation.
    let Some(Ok(compilation)) = state.result.as_ref() else {
        return state;
    };
    let Some(program) = compilation.program.as_ref() else {
        return state;
    };

    // All known node names in the compiled program.
    let all_nodes: HashSet<&str> = program.nodes.keys().map(String::as_str).collect();

    if all_nodes.is_empty() {
        return state;
    }

    // Regex to match static jump/detour targets.
    // Matches <<jump NodeName>>, <<detour NodeName>>, and option [[NodeName]].
    let ref_re = Regex::new(r"<<\s*(?:jump|detour)\s+([A-Za-z_][A-Za-z0-9_.]*)\s*>>|\[\[([A-Za-z_][A-Za-z0-9_.]*)\]\]").unwrap();

    // Collect all referenced node names by scanning every source file.
    let mut referenced: HashSet<&str> = HashSet::new();
    for file in &state.job.files {
        for cap in ref_re.captures_iter(&file.source) {
            // Group 1 = jump/detour target; group 2 = option target.
            if let Some(m) = cap.get(1).or_else(|| cap.get(2)) {
                let name = m.as_str();
                if all_nodes.contains(name) {
                    referenced.insert(name);
                }
            }
        }
    }

    // If nothing is referenced at all, every node is a candidate entry point
    // (the game launcher will call `set_node` directly).  Only emit warnings
    // when the program has a clear reachability graph (i.e. at least one node
    // IS reached by a jump/detour).
    if referenced.is_empty() {
        return state;
    }

    // Regex to locate `title: NodeName` headers so we can provide a source range.
    let title_re = Regex::new(r"(?m)^(title:[ \t]*)([^\r\n#/]+?)[ \t]*(?://[^\r\n]*)?$").unwrap();

    for node_name in &all_nodes {
        if referenced.contains(node_name) {
            continue;
        }

        // Find the node title in its source file and compute the range.
        let mut diag = DiagnosticDescriptor::UNREFERENCED_NODE.create("<input>", format!("Node '{}' is never referenced", node_name));

        'files: for file in &state.job.files {
            for cap in title_re.captures_iter(&file.source) {
                let value_match = cap.get(2).unwrap();
                if value_match.as_str().trim() == *node_name {
                    let start = value_match.start();
                    let end = value_match.end();
                    let (start_line, start_char) = offset_to_line_col(&file.source, start);
                    let (end_line, end_char) = offset_to_line_col(&file.source, end);
                    diag = DiagnosticDescriptor::UNREFERENCED_NODE.create_with_range(
                        &file.file_name,
                        Position {
                            line: start_line,
                            character: start_char,
                        }..Position {
                            line: end_line,
                            character: end_char,
                        },
                        format!("Node '{}' is never referenced", node_name),
                    );
                    break 'files;
                }
            }
        }

        state.diagnostics.push(diag);
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
