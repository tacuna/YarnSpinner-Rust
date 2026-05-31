use crate::output::DeclarationSource;
use crate::prelude::*;
use regex::Regex;
use std::collections::HashSet;
use yarnspinner_core::types::Type;

/// Checks for variables that are declared in source but never referenced.
/// Emits a YS0010 Warning for each such variable.
///
/// This matches the C# compiler's behavior of scanning `nodeMetadata.VariableReferences`
/// and flagging any explicitly-declared variable whose name never appears outside of
/// its own `<<declare ...>>` statement.
///
/// Note: C# emits `DiagnosticSeverity.Info` for this diagnostic. The Rust compiler
/// maps that to `DiagnosticSeverity::Warning` since `Info` is not a supported variant.
pub(crate) fn check_unused_variables(mut state: CompilationIntermediate) -> CompilationIntermediate {
    // Regex matching any $variable name.
    let var_re = Regex::new(r"\$[A-Za-z_][A-Za-z0-9_.]*").unwrap();
    // Regex matching the target variable in a <<declare $var ...>> statement.
    let declare_re = Regex::new(r"<<\s*declare\s+(\$[A-Za-z_][A-Za-z0-9_.]*)").unwrap();

    let mut referenced: HashSet<String> = HashSet::new();

    // Collect variable references from all source files.
    // A reference is any $var occurrence that is NOT the declaration target
    // of a <<declare ...>> statement.
    for file in &state.job.files {
        // Find byte offsets of declare-target variables in this file.
        let declare_positions: HashSet<usize> = declare_re
            .captures_iter(&file.source)
            .filter_map(|cap| cap.get(1))
            .map(|m| m.start())
            .collect();

        for m in var_re.find_iter(&file.source) {
            if !declare_positions.contains(&m.start()) {
                referenced.insert(m.as_str().to_owned());
            }
        }
    }

    // Also treat every $var in when: header expressions as a reference.
    // These are stored as raw strings and are not covered by the file source scan
    // relative to declare positions (they're in headers, never inside <<declare>>).
    for members in state.node_groups.values() {
        for member in members {
            for expr in &member.when_expressions {
                for m in var_re.find_iter(expr) {
                    referenced.insert(m.as_str().to_owned());
                }
            }
        }
    }

    // Examine every variable declaration that was derived during this compilation.
    // Flag any that is never referenced.
    let derived = state.derived_variable_declarations.clone();
    for decl in &derived {
        // Only consider explicit declarations (not inferred-from-usage ones).
        if decl.is_implicit {
            continue;
        }
        // Smart variables (inline expansions) are their own expressions; skip them.
        if decl.is_inline_expansion {
            continue;
        }
        // Skip internal Yarn-runtime variables.
        if decl.name.starts_with("$Yarn.Internal") {
            continue;
        }
        // Only variable declarations, not function declarations.
        if matches!(decl.r#type, Type::Function(_)) {
            continue;
        }
        // Only declarations that originate from a source file.
        let DeclarationSource::File(ref file_name) = decl.source_file_name else {
            continue;
        };

        if !referenced.contains(&decl.name) {
            let mut diagnostic = Diagnostic::from_message(format!("Variable '{}' is declared but never used", decl.name))
                .with_file_name(file_name.clone())
                .with_severity(DiagnosticSeverity::Warning)
                .with_code("YS0010");

            if let Some(range) = decl.range.clone() {
                diagnostic = diagnostic.with_range(range);
            }

            state.diagnostics.push(diagnostic);
        }
    }

    state
}
