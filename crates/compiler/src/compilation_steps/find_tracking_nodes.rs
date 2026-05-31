use crate::prelude::*;
use crate::visitors::NodeTrackingVisitor;
use antlr4rust::tree::ParseTreeVisitorCompat;
use std::collections::HashSet;

pub(crate) fn find_tracking_nodes(mut state: CompilationIntermediate) -> CompilationIntermediate {
    // determining the nodes we need to track visits on
    // this needs to be done before we finish up with declarations
    // so that any tracking variables are included in the compiled declarations
    let mut tracking_nodes = HashSet::new();
    let mut ignore_nodes = HashSet::new();
    for (file, _) in &state.parsed_files {
        let mut visitor = NodeTrackingVisitor::new();
        visitor.visit(file.tree.as_ref());
        tracking_nodes.extend(visitor.tracking_nodes);
        ignore_nodes.extend(visitor.ignoring_nodes);
    }
    let mut result: HashSet<String> = tracking_nodes.difference(&ignore_nodes).cloned().collect();

    // v3.1.0: The Rust visitor reads raw `title:` headers (= source/group
    // title), but the compiler listener checks against unique names.
    // Expand a tracked group title to all of its member unique names so
    // that each member gets visit-tracking code.  Also do the reverse:
    // if a unique member name is explicitly tracked (e.g. via
    // visited("Title.Subtitle")), ensure the hub is tracked too.
    for (group_title, members) in &state.node_groups {
        if result.contains(group_title) {
            for member in members {
                result.insert(member.unique_name.clone());
            }
        }
        for member in members {
            if result.contains(&member.unique_name) {
                result.insert(group_title.clone());
                break;
            }
        }
    }

    state.tracking_nodes = result;
    state
}
