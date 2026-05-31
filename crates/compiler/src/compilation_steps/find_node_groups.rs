//! Identifies node groups (sets of nodes sharing a title, all with `when:` headers)
//! and assigns unique internal names to each member.

use crate::compiler::run_compilation::NodeGroupMember;
use crate::prelude::generated::yarnspinnerparser::{
    DialogueContextAttrs,
    Header_when_expressionContextAttrs,
    NodeContextAttrs,
    Title_headerContextAttrs,
};
use crate::prelude::*;
use antlr4rust::parser_rule_context::ParserRuleContext;
use antlr4rust::token::Token;
use antlr4rust::tree::ParseTree;
use std::collections::{HashMap, HashSet, VecDeque};

/// `(file_name, title, when_exprs, subtitle?, start_line)` tuples for node-group detection.
type NodeEntry = (String, String, Vec<String>, Option<String>, isize);

pub(crate) fn find_node_groups(mut state: CompilationIntermediate) -> CompilationIntermediate {
    // Collect (file_name, title, when_exprs, subtitle?, start_line) in document order across all files.
    let mut ordered: Vec<NodeEntry> = Vec::new();

    for (file, _) in &state.parsed_files {
        for node in file.tree.node_all() {
            let mut title: Option<String> = None;
            let mut when_exprs: Vec<String> = Vec::new();
            let mut subtitle: Option<String> = None;
            let start_line = node.start().line;

            // title comes from title_header (separate rule in new grammar)
            if let Some(title_hdr) = node.title_header(0) {
                title = title_hdr.ID().map(|t| t.get_text().trim().to_owned());
            }
            // when expressions come from when_header (separate rule)
            for when_hdr in node.when_header_all() {
                if let Some(expr) = when_hdr.header_expression.as_ref() {
                    // Reconstruct with proper spacing: get_text() concatenates tokens without
                    // spaces, so "once if $var" would become "onceif$var".
                    let text = if expr.COMMAND_ONCE().is_some() {
                        if expr.COMMAND_IF().is_some() {
                            let inner = expr.expression().as_deref().map(|x| x.get_text()).unwrap_or_default();
                            format!("once if {inner}")
                        } else {
                            "once".to_owned()
                        }
                    } else {
                        (**expr).get_text()
                    };
                    when_exprs.push(text);
                }
            }
            // subtitle and other headers come from generic header rule
            for header in node.header_all() {
                let key = header.header_key.as_ref().map(|k| k.get_text()).unwrap_or("");
                let value = header.header_value.as_ref().map(|v| strip_header_comment(v.get_text()).trim().to_owned());
                if key == "subtitle" {
                    subtitle = value;
                }
            }

            if let Some(t) = title {
                ordered.push((file.name.clone(), t, when_exprs, subtitle, start_line));
            }
        }
    }

    // Count per title: (total, with_when).
    let mut stats: HashMap<String, (usize, usize)> = HashMap::new();
    for (_, title, when_exprs, _, _) in &ordered {
        let e = stats.entry(title.clone()).or_default();
        e.0 += 1;
        if !when_exprs.is_empty() {
            e.1 += 1;
        }
    }

    // A title forms a node group when ALL its nodes carry a `when:` header.
    // (even single-member groups with one `when: always` node)
    let group_titles: HashSet<String> = stats
        .into_iter()
        .filter(|(_, (total, with_when))| *with_when > 0 && *with_when == *total)
        .map(|(title, _)| title)
        .collect();

    if group_titles.is_empty() {
        return state;
    }

    // Assign unique names in document order and build per-file overrides.
    let mut node_groups: HashMap<String, Vec<NodeGroupMember>> = HashMap::new();
    let mut overrides: HashMap<String, HashMap<String, VecDeque<String>>> = HashMap::new();

    for (file_name, title, when_exprs, subtitle, start_line) in &ordered {
        if !group_titles.contains(title) {
            continue;
        }

        let members = node_groups.entry(title.clone()).or_default();
        let unique_name = get_node_unique_name(file_name, title, subtitle.as_deref(), *start_line);
        // If no when: headers, default to ["always"]
        let when_expressions = if when_exprs.is_empty() {
            vec!["always".to_owned()]
        } else {
            when_exprs.clone()
        };

        members.push(NodeGroupMember {
            when_expressions,
            unique_name: unique_name.clone(),
        });

        overrides
            .entry(file_name.clone())
            .or_default()
            .entry(title.clone())
            .or_default()
            .push_back(unique_name);
    }

    state.node_groups = node_groups;
    state.node_group_name_overrides = overrides;
    state
}

/// Computes the unique internal name for a node group member, matching C# `Utility.GetNodeUniqueName`.
/// If a subtitle header is present, uses `title.subtitle`.
/// Otherwise, uses `title.CRC32(filename + title + startLine)`.
fn get_node_unique_name(source_file_name: &str, title: &str, subtitle: Option<&str>, start_line: isize) -> String {
    if let Some(sub) = subtitle
        && !sub.is_empty()
    {
        return format!("{}.{}", title, sub);
    }
    let description = format!("{}{}{}", source_file_name, title, start_line);
    let checksum = crc32fast::hash(description.as_bytes());
    let bytes = checksum.to_le_bytes();
    format!("{}.{:02x}{:02x}{:02x}{:02x}", title, bytes[0], bytes[1], bytes[2], bytes[3])
}

/// Creates implicit boolean declarations for `$variable` references found in
/// `when:` expressions that are not already in `known_variable_declarations`.
/// This handles the case where a variable is only used in `when:` headers and
/// never explicitly declared with `<<declare>>`.
pub(crate) fn declare_when_expression_variables(mut state: CompilationIntermediate) -> CompilationIntermediate {
    use yarnspinner_core::types::Type;

    if state.node_groups.is_empty() {
        return state;
    }

    let existing_names: HashSet<String> = state.known_variable_declarations.iter().map(|d| d.name.clone()).collect();

    let mut new_decls: Vec<Declaration> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for members in state.node_groups.values() {
        for member in members {
            for expr in &member.when_expressions {
                for var_name in extract_var_names(expr) {
                    if !existing_names.contains(var_name) && seen.insert(var_name.to_owned()) {
                        // Create an implicit boolean declaration (default false).
                        // If the variable is explicitly declared elsewhere with a different
                        // type, get_declarations will add that declaration too and
                        // add_initial_value_registrations will use the last-written value.
                        let decl = Declaration::new(var_name.to_owned(), Type::Boolean)
                            .with_description("Implicitly declared from when: expression")
                            .with_default_value(false)
                            .with_implicit();
                        new_decls.push(decl);
                    }
                }
            }
        }
    }

    state.known_variable_declarations.extend(new_decls);
    state
}

/// Scans a string for `$variable_name` tokens (matching `$[a-zA-Z_][a-zA-Z0-9_]*`).
fn extract_var_names(input: &str) -> impl Iterator<Item = &str> {
    let bytes = input.as_bytes();
    let mut i = 0;
    std::iter::from_fn(move || {
        while i < bytes.len() {
            if bytes[i] == b'$' {
                let start = i;
                i += 1;
                if i < bytes.len() && (bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') {
                    i += 1;
                    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                        i += 1;
                    }
                    return Some(&input[start..i]);
                }
            } else {
                i += 1;
            }
        }
        None
    })
}
