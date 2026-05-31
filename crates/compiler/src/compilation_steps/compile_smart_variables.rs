use crate::listeners::CompilerListener;
use crate::prelude::generated::yarnspinnerparser::*;
use crate::prelude::*;
use crate::visitors::CodeGenerationVisitor;
use antlr4rust::tree::{ParseTree, ParseTreeVisitorCompat};
use std::collections::{HashMap, HashSet};
use yarnspinner_core::prelude::*;

/// The tag added to synthetic nodes that represent smart variable evaluators.
pub(crate) const SMART_VARIABLE_NODE_TAG: &str = "Yarn.SmartVariable";

pub(crate) fn compile_smart_variables(mut state: CompilationIntermediate) -> CompilationIntermediate {
    let Ok(compilation) = state.result.as_mut().unwrap().as_mut() else {
        return state;
    };
    let Some(program) = compilation.program.as_mut() else {
        return state;
    };

    // Collect smart variable declaration names
    let smart_var_names: HashSet<String> = state
        .known_variable_declarations
        .iter()
        .filter(|d| d.is_inline_expansion)
        .map(|d| d.name.clone())
        .collect();

    if smart_var_names.is_empty() {
        return state;
    }

    // Walk each file's parse tree looking for declare_statements that are smart variables.
    for (file, known_types) in &state.parsed_files {
        // Traverse: dialogue → nodes → body → statements → declare_statement
        let dialogue = file.tree.as_ref();
        for node_ctx in dialogue.node_all() {
            let Some(body) = node_ctx.body() else { continue };
            for stmt in body.statement_all() {
                let Some(decl) = stmt.declare_statement() else {
                    continue;
                };
                let Some(var_ctx) = decl.variable() else {
                    continue;
                };
                let var_name = var_ctx.get_text();
                if !smart_var_names.contains(&var_name) {
                    continue;
                }
                let Some(expr_ctx) = decl.expression() else {
                    continue;
                };

                // Create a CompilerListener to compile the expression
                let mut listener = CompilerListener::new(
                    HashSet::new(),
                    HashSet::new(),
                    known_types.clone(),
                    file.clone(),
                    HashMap::new(),
                    HashSet::new(),
                );

                // Set up a fresh node for the smart variable
                let mut smart_node = Node {
                    name: var_name.clone(),
                    ..Default::default()
                };
                smart_node.tags.push(SMART_VARIABLE_NODE_TAG.to_string());
                smart_node.headers.push(Header {
                    key: "tags".to_string(),
                    value: SMART_VARIABLE_NODE_TAG.to_string(),
                });
                listener.current_node = Some(smart_node);

                // Use CodeGenerationVisitor to compile the expression into bytecode
                let mut code_gen = CodeGenerationVisitor::new(&mut listener, None);
                code_gen.visit(expr_ctx.as_ref());

                // Extract the compiled node and add it to the program
                if let Some(compiled_node) = listener.current_node.take() {
                    program.nodes.insert(var_name, compiled_node);
                }
            }
        }
    }

    state
}
