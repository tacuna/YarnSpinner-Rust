//! Replaces the `AddErrorsForEmptyNodes` logic previously in <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner.Compiler/Compiler.cs>

use std::collections::HashSet;

use antlr4rust::tree::ParseTree;

use crate::parser::generated::yarnspinnerparser::{BodyContextAttrs, DialogueContextAttrs, NodeContextAttrs, Title_headerContextAttrs};
use crate::prelude::{CompilationIntermediate, *};

pub(crate) fn add_error_for_empty_nodes(mut state: CompilationIntermediate) -> CompilationIntermediate {
    let mut empty_nodes: HashSet<String> = HashSet::new();

    let empties = state
        .parsed_files
        .iter()
        .flat_map(|(file, _)| file.tree.node_all().iter().map(|node| (node.clone(), file)).collect::<Vec<_>>())
        .filter(|(node, _)| {
            if let Some(body) = node.body() {
                body.statement_all().is_empty()
            } else {
                false
            }
        });

    for (node, file) in empties {
        let (title, title_context) = node
            .title_header(0)
            .and_then(|h| h.ID().map(|t| (t.get_text(), Some(h.clone()))))
            .unwrap_or_default();

        let mut diag = Diagnostic::from_message(format!("Node \"{title}\" is empty and will not be included in the compiled output.",))
            .with_file_name(file.name.clone())
            .with_severity(DiagnosticSeverity::Warning)
            .with_code("YS0033");

        if let Some(context) = title_context {
            diag = diag.with_parser_context(context.as_ref(), file.tokens());
        }

        state.diagnostics.push(diag);

        empty_nodes.insert(title);
    }

    state.skip_nodes = empty_nodes;
    state
}
