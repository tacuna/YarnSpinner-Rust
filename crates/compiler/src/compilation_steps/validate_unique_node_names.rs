use crate::prelude::generated::yarnspinnerparser::{DialogueContextAttrs, NodeContextAttrs, Title_headerContextAttrs};
use crate::prelude::*;
use antlr4rust::token::Token;
use antlr4rust::tree::ParseTree;
use std::collections::HashMap;

/// Validates that any `when:` header on a node has a non-empty expression.
pub(crate) fn validate_when_headers(mut state: CompilationIntermediate) -> CompilationIntermediate {
    for (file, _) in &state.parsed_files {
        for node in file.tree.node_all() {
            // In the new grammar, when: is a separate when_header rule, not a generic header
            for when_hdr in node.when_header_all() {
                if when_hdr.header_expression.is_none() {
                    let title = node
                        .title_header(0)
                        .and_then(|h| h.ID().map(|t| t.get_text()))
                        .unwrap_or_else(|| "<unknown>".to_owned());
                    state.diagnostics.push(
                        Diagnostic::from_message(format!("Node '{title}' has a 'when' header, but the header has no expression."))
                            .with_file_name(file.name.clone())
                            .with_parser_context(when_hdr.as_ref(), file.tokens()),
                    );
                }
            }
        }
    }
    state
}

pub(crate) fn validate_unique_node_names(mut state: CompilationIntermediate) -> CompilationIntermediate {
    // Ensure that all nodes names in this compilation are unique. Node
    // name uniqueness is important for several processes, so we do this
    // check here.
    let all_nodes = state
        .parsed_files
        .iter()
        .flat_map(|(file, _)| file.tree.node_all().iter().map(|node| (node.clone(), file)).collect::<Vec<_>>());

    // Pair up every node with its name and whether it has a `when:` header.
    let nodes_with_names = all_nodes.filter_map(|(node, file)| {
        // when_header is a separate rule in the new grammar
        let has_when = !node.when_header_all().is_empty();
        node.title_header(0).and_then(|title_header| {
            let title = title_header.ID()?.get_text();
            Some((title, title_header, file, has_when))
        })
    });

    let nodes_by_name = nodes_with_names.fold(
        HashMap::default(),
        |mut map: HashMap<_, Vec<_>>, (name, header_context, file, has_when)| {
            map.entry(name).or_default().push((header_context, file, has_when));
            map
        },
    );

    // Find groups of nodes with the same name and generate diagnostics
    // for each
    for (name, nodes) in nodes_by_name.into_iter().filter(|(_, nodes)| nodes.len() > 1) {
        // If ALL nodes with this name are node group members (they all have
        // `when:` headers), this is a valid node group — not an error.
        if state.node_groups.contains_key(&name) {
            continue;
        }

        // Check if this is a partial group: some nodes have `when:`, some don't.
        // In that case, nodes missing `when:` get YS0031 (not YS0011).
        let any_has_when = nodes.iter().any(|(_, _, has_when)| *has_when);

        for (header_context, file, has_when) in nodes {
            if any_has_when && !has_when {
                // Partial node group: this node is missing its `when:` clause.
                let message = format!(
                    "All nodes in the group '{name}' must have a 'when' clause \
                     (use 'when: always' if you want this node to not have any conditions)."
                );
                state.diagnostics.push(
                    Diagnostic::from_message(message)
                        .with_file_name(file.name.clone())
                        .with_parser_context(header_context.as_ref(), file.tokens())
                        .with_severity(DiagnosticSeverity::Error)
                        .with_code("YS0031"),
                );
            } else if !any_has_when {
                // Pure duplicate node titles.
                state.diagnostics.push(
                    Diagnostic::from_message(format!("More than one node is named {name}"))
                        .with_file_name(file.name.clone())
                        .with_parser_context(header_context.as_ref(), file.tokens())
                        .with_code("YS0011"),
                );
            }
            // If any_has_when && has_when, this node does have `when:` and is part of a
            // partial group. No diagnostic for it — only nodes lacking `when:` are flagged.
        }
    }
    state
}

/// Validates that within each node group, every member has a unique `subtitle:` header.
pub(crate) fn validate_unique_node_subtitles(mut state: CompilationIntermediate) -> CompilationIntermediate {
    if state.node_groups.is_empty() {
        return state;
    }

    // For each file, collect (group_title, subtitle, header_context) tuples.
    for (file, _) in &state.parsed_files {
        // subtitle → list of header contexts (for error reporting)
        let mut group_subtitles: HashMap<String, HashMap<String, Vec<_>>> = HashMap::new();

        for node in file.tree.node_all() {
            let mut title: Option<String> = None;
            let mut subtitle: Option<(String, _)> = None;

            for header in node.header_all() {
                let key = header.header_key.as_ref().map(|k| k.get_text()).unwrap_or("");
                match key {
                    "title" => {
                        // title is now in title_header (separate rule); skipped here
                    }
                    "subtitle" => {
                        let val = header
                            .header_value
                            .as_ref()
                            .map(|v| strip_header_comment(v.get_text()).trim().to_owned())
                            .unwrap_or_default();
                        subtitle = Some((val, header.clone()));
                    }
                    _ => {}
                }
            }
            // Get title from separate title_header rule
            if let Some(title_hdr) = node.title_header(0) {
                title = title_hdr.ID().map(|t| t.get_text());
            }

            if let (Some(t), Some((sub, ctx))) = (title, subtitle) {
                // Only check nodes that are part of a node group.
                if state.node_groups.contains_key(&t) {
                    group_subtitles.entry(t).or_default().entry(sub).or_default().push(ctx);
                }
            }
        }

        for (group_title, subtitles) in group_subtitles {
            for (subtitle, contexts) in subtitles {
                if contexts.len() > 1 {
                    for ctx in contexts {
                        state.diagnostics.push(
                            Diagnostic::from_message(format!(
                                "Node group '{group_title}' has more than one node with the subtitle '{subtitle}'."
                            ))
                            .with_file_name(file.name.clone())
                            .with_parser_context(ctx.as_ref(), file.tokens())
                            .with_severity(DiagnosticSeverity::Error)
                            .with_code("YS0032"),
                        );
                    }
                }
            }
        }
    }
    state
}
