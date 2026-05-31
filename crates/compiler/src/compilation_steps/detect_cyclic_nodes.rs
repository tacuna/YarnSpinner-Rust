use crate::prelude::*;
use regex::Regex;
use std::collections::{HashMap, HashSet};

/// Detects cycles in the static node-jump graph and emits a YS0015 warning
/// for each unique cycle found.
///
/// A "static" jump is a `<<jump NodeName>>` or `<<detour NodeName>>` where the
/// target is a literal node name (not an expression).  At the bytecode level
/// this is a `PushString("NodeName")` instruction immediately followed by
/// `RunNode` (jump) or `DetourToNode` (detour).
///
/// Cycles are normalized by rotating the cycle so that the alphabetically
/// smallest node name comes first, ensuring each cycle is reported exactly once
/// regardless of the DFS traversal order.
pub(crate) fn detect_cyclic_nodes(mut state: CompilationIntermediate) -> CompilationIntermediate {
    let Some(Ok(compilation)) = state.result.as_ref() else {
        return state;
    };
    let Some(program) = compilation.program.as_ref() else {
        return state;
    };

    let all_node_names: HashSet<&str> = program.nodes.keys().map(String::as_str).collect();
    if all_node_names.len() < 2 {
        return state;
    }

    // Build adjacency list: node -> set of statically-reachable nodes.
    // Pattern: PushString(name) immediately followed by RunNode or DetourToNode.
    let mut adjacency: HashMap<&str, Vec<&str>> = all_node_names.iter().map(|&n| (n, Vec::new())).collect();

    for (node_name, node) in &program.nodes {
        let targets = adjacency.entry(node_name.as_str()).or_default();
        let instrs = &node.instructions;
        for i in 0..instrs.len().saturating_sub(1) {
            if instrs[i].opcode != OpCode::PushString as i32 {
                continue;
            }
            let next_op = instrs[i + 1].opcode;
            if next_op != OpCode::RunNode as i32 && next_op != OpCode::DetourToNode as i32 {
                continue;
            }
            if let Some(OperandValue::StringValue(target)) = instrs[i].operands.first().and_then(|o| o.value.as_ref())
                && all_node_names.contains(target.as_str())
                && !targets.contains(&target.as_str())
            {
                targets.push(target.as_str());
            }
        }
    }

    // Sort adjacency lists for deterministic traversal.
    for targets in adjacency.values_mut() {
        targets.sort_unstable();
    }

    let mut sorted_names: Vec<&str> = all_node_names.iter().copied().collect();
    sorted_names.sort_unstable();

    let mut visited: HashSet<&str> = HashSet::new();
    let mut reported: HashSet<Vec<String>> = HashSet::new();
    let mut new_diagnostics: Vec<Diagnostic> = Vec::new();

    for &start in &sorted_names {
        if !visited.contains(start) {
            let mut path: Vec<&str> = Vec::new();
            let mut on_path: HashSet<&str> = HashSet::new();
            dfs(
                start,
                &adjacency,
                &mut visited,
                &mut path,
                &mut on_path,
                &mut reported,
                &mut new_diagnostics,
                &state.job.files,
            );
        }
    }

    state.diagnostics.extend(new_diagnostics);
    state
}

#[allow(clippy::too_many_arguments)]
fn dfs<'a>(
    node: &'a str,
    adjacency: &HashMap<&'a str, Vec<&'a str>>,
    visited: &mut HashSet<&'a str>,
    path: &mut Vec<&'a str>,
    on_path: &mut HashSet<&'a str>,
    reported: &mut HashSet<Vec<String>>,
    diagnostics: &mut Vec<Diagnostic>,
    files: &[File],
) {
    path.push(node);
    on_path.insert(node);

    if let Some(targets) = adjacency.get(node) {
        for &target in targets {
            if on_path.contains(target) {
                // Back-edge: extract the cycle from `path` starting at `target`.
                let cycle_start = path.iter().position(|&n| n == target).unwrap();
                let cycle: Vec<&str> = path[cycle_start..].to_vec();

                // Rotate so that the alphabetically smallest node is first.
                let min_idx = cycle.iter().enumerate().min_by_key(|&(_, &n)| n).map(|(i, _)| i).unwrap_or(0);
                let normalized: Vec<String> = cycle[min_idx..].iter().chain(cycle[..min_idx].iter()).map(|&s| s.to_string()).collect();

                if reported.insert(normalized.clone()) {
                    let message = format!("Cyclic dependency detected: {}", normalized.join(" -> "));
                    diagnostics.push(build_cycle_diagnostic(&normalized[0], &message, files));
                }
            } else if !visited.contains(target) {
                dfs(target, adjacency, visited, path, on_path, reported, diagnostics, files);
            }
        }
    }

    path.pop();
    on_path.remove(node);
    visited.insert(node);
}

fn build_cycle_diagnostic(node_name: &str, message: &str, files: &[File]) -> Diagnostic {
    let title_re = Regex::new(r"(?m)^title:[ \t]*([^\r\n#/]+?)[ \t]*(?://[^\r\n]*)?$").unwrap();

    for file in files {
        for cap in title_re.captures_iter(&file.source) {
            let m = cap.get(1).unwrap();
            if m.as_str().trim() == node_name {
                let (sl, sc) = offset_to_line_col(&file.source, m.start());
                let (el, ec) = offset_to_line_col(&file.source, m.end());
                return DiagnosticDescriptor::CYCLIC_DEPENDENCY.create_with_range(
                    &file.file_name,
                    Position { line: sl, character: sc }..Position { line: el, character: ec },
                    message,
                );
            }
        }
    }

    DiagnosticDescriptor::CYCLIC_DEPENDENCY.create("<input>", message)
}

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
