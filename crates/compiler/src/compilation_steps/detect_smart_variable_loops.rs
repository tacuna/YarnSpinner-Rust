use crate::prelude::generated::yarnspinnerparser::*;
use crate::prelude::*;
use antlr4rust::tree::ParseTree;
use std::collections::{HashMap, HashSet};

/// Recursively walks an expression parse tree and collects all variable references.
fn collect_variables(expr: &ExpressionContextAll<'_>, vars: &mut HashSet<String>) {
    match expr {
        ExpressionContextAll::ExpValueContext(ctx) => {
            if let Some(val) = ctx.value() {
                match val.as_ref() {
                    ValueContextAll::ValueVarContext(var_val) => {
                        if let Some(var_ctx) = var_val.variable() {
                            vars.insert(var_ctx.get_text());
                        }
                    }
                    ValueContextAll::ValueFuncContext(func_val) => {
                        if let Some(func) = func_val.function_call() {
                            for arg in func.expression_all() {
                                collect_variables(arg.as_ref(), vars);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        ExpressionContextAll::ExpParensContext(ctx) => {
            if let Some(inner) = ctx.expression() {
                collect_variables(inner.as_ref(), vars);
            }
        }
        ExpressionContextAll::ExpNegativeContext(ctx) => {
            if let Some(inner) = ctx.expression() {
                collect_variables(inner.as_ref(), vars);
            }
        }
        ExpressionContextAll::ExpNotContext(ctx) => {
            if let Some(inner) = ctx.expression() {
                collect_variables(inner.as_ref(), vars);
            }
        }
        ExpressionContextAll::ExpMultDivModContext(ctx) => {
            for sub in ctx.expression_all() {
                collect_variables(sub.as_ref(), vars);
            }
        }
        ExpressionContextAll::ExpAddSubContext(ctx) => {
            for sub in ctx.expression_all() {
                collect_variables(sub.as_ref(), vars);
            }
        }
        ExpressionContextAll::ExpComparisonContext(ctx) => {
            for sub in ctx.expression_all() {
                collect_variables(sub.as_ref(), vars);
            }
        }
        ExpressionContextAll::ExpEqualityContext(ctx) => {
            for sub in ctx.expression_all() {
                collect_variables(sub.as_ref(), vars);
            }
        }
        ExpressionContextAll::ExpAndOrXorContext(ctx) => {
            for sub in ctx.expression_all() {
                collect_variables(sub.as_ref(), vars);
            }
        }
        ExpressionContextAll::Error(_) => {}
    }
}

/// Detects circular references among smart variables.
/// Must run after get_declarations (so we know which variables are smart)
/// and after parse_files (so we have parse trees).
pub(crate) fn detect_smart_variable_loops(mut state: CompilationIntermediate) -> CompilationIntermediate {
    // Collect smart variable names
    let smart_var_names: HashSet<String> = state
        .known_variable_declarations
        .iter()
        .filter(|d| d.is_inline_expansion)
        .map(|d| d.name.clone())
        .collect();

    if smart_var_names.is_empty() {
        return state;
    }

    // Build a map: smart variable name → set of variable references in its expression,
    // along with file name for error reporting.
    let mut dependency_map: HashMap<String, (HashSet<String>, String)> = HashMap::new();

    for (file, _known_types) in &state.parsed_files {
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

                // Extract variable references by walking the expression parse tree
                let mut refs = HashSet::new();
                collect_variables(expr_ctx.as_ref(), &mut refs);
                refs.remove(&var_name); // exclude self-references within the expression
                dependency_map.insert(var_name, (refs, file.name.clone()));
            }
        }
    }

    // For each smart variable, do DFS to detect cycles
    for start_var in &smart_var_names {
        let Some((_, file_name)) = dependency_map.get(start_var) else {
            continue;
        };
        let file_name = file_name.clone();

        // DFS: check if start_var can reach itself through smart variable dependencies
        let mut visited: HashSet<&str> = HashSet::new();
        visited.insert(start_var);
        let mut stack: Vec<&str> = Vec::new();

        // Initialize stack with the smart variable dependencies of start_var
        if let Some((deps, _)) = dependency_map.get(start_var) {
            for dep in deps {
                if smart_var_names.contains(dep) {
                    stack.push(dep);
                }
            }
        }

        while let Some(current) = stack.pop() {
            if current == start_var {
                // Cycle detected!
                state.diagnostics.push(
                    Diagnostic::from_message(format!(
                        "Smart variables cannot contain reference loops \
                         (the smart variable {} references itself through a chain of other smart variables)",
                        start_var
                    ))
                    .with_file_name(&file_name)
                    .with_code("YS0045"),
                );
                // Mark as early break to prevent further compilation
                state.early_break = true;
                break;
            }

            if !visited.insert(current) {
                // Already visited this node (but it's not the start, so no cycle through start)
                continue;
            }

            // Push dependencies of current
            if let Some((deps, _)) = dependency_map.get(current) {
                for dep in deps {
                    if smart_var_names.contains(dep) {
                        stack.push(dep);
                    }
                }
            }
        }
    }

    // Compute transitive closure of dependencies for each smart variable.
    // We stop recursing into non-smart vars since they have no further var deps.
    let mut transitive_deps: HashMap<String, Vec<String>> = HashMap::new();

    for start_var in &smart_var_names {
        if !dependency_map.contains_key(start_var) {
            continue;
        }
        let mut all_deps: HashSet<String> = HashSet::new();
        let mut work_stack: Vec<String> = Vec::new();
        let mut visited: HashSet<String> = HashSet::new();
        visited.insert(start_var.clone());

        if let Some((direct_deps, _)) = dependency_map.get(start_var) {
            for dep in direct_deps {
                work_stack.push(dep.clone());
            }
        }

        while let Some(dep) = work_stack.pop() {
            if !visited.insert(dep.clone()) {
                continue;
            }
            all_deps.insert(dep.clone());
            // If this dependency is itself a smart variable, recurse into its deps
            if smart_var_names.contains(&dep)
                && let Some((dep_deps, _)) = dependency_map.get(&dep)
            {
                for sub_dep in dep_deps {
                    if !visited.contains(sub_dep) {
                        work_stack.push(sub_dep.clone());
                    }
                }
            }
        }

        transitive_deps.insert(start_var.clone(), all_deps.into_iter().collect());
    }

    // Build inverse map: var_name → list of smart vars whose transitive deps include it
    let mut dependents_map: HashMap<String, Vec<String>> = HashMap::new();
    for (smart_var, deps) in &transitive_deps {
        for dep in deps {
            dependents_map.entry(dep.clone()).or_default().push(smart_var.clone());
        }
    }

    // Write dependencies and dependents into all known declarations
    for decl in &mut state.known_variable_declarations {
        if let Some(deps) = transitive_deps.get(&decl.name) {
            decl.dependencies.clone_from(deps);
        }
        if let Some(dependents) = dependents_map.get(&decl.name) {
            decl.dependents.clone_from(dependents);
        }
    }
    // Also mirror into derived_variable_declarations (which becomes Compilation.declarations)
    for decl in &mut state.derived_variable_declarations {
        if let Some(deps) = transitive_deps.get(&decl.name) {
            decl.dependencies.clone_from(deps);
        }
        if let Some(dependents) = dependents_map.get(&decl.name) {
            decl.dependents.clone_from(dependents);
        }
    }

    state
}
